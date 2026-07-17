//! Stable, numeric cube features derived from the authoritative facelet state.
//!
//! Feature extraction never rotates or otherwise normalizes the cube. This is
//! important for training examples whose move labels are expressed in the
//! state's current spatial frame.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    cube::{facelets_from_cubies, Cube},
    engine::{
        cubie::{Corner, Edge, SOLVED_CUBIE_CUBE},
        facelet::Color,
    },
    error::{Error, Result},
    solver::F2lPair,
};

/// Schema for the optional, derived feature payload.
pub const CUBE_FEATURE_SCHEMA: &str = "cube-features/v1";
/// Flat cubie ID layout: centers, corner permutation, corner orientation,
/// edge permutation, then edge orientation.
pub const CUBIE_FEATURE_FORMAT: &str = "cubie-factorized-urfdlb-v1";
/// Numeric facelets use the same URFDLB positions as the canonical state.
pub const FACELET_ID_FORMAT: &str = "facelet-ids-urfdlb-v1";
pub const CUBIE_FEATURE_WIDTH: usize = 46;
pub const FACELET_ID_WIDTH: usize = 54;
pub const CUBIE_ONE_HOT_WIDTH: usize = 292;
pub const FACELET_ONE_HOT_WIDTH: usize = 324;

/// Cardinality of each categorical value in [`CubieFeatures::flat_ids`].
pub const CUBIE_FEATURE_CARDINALITIES: [u8; CUBIE_FEATURE_WIDTH] = [
    6, 6, 6, 6, 6, 6, // centers
    8, 8, 8, 8, 8, 8, 8, 8, // corner permutation
    3, 3, 3, 3, 3, 3, 3, 3, // corner orientation
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, // edge permutation
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // edge orientation
];

/// Optional numeric representations to attach to an otherwise canonical state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureSelection {
    #[default]
    None,
    Cubie,
    FaceletIds,
    Both,
}

impl FeatureSelection {
    pub const fn includes_cubie(self) -> bool {
        matches!(self, Self::Cubie | Self::Both)
    }

    pub const fn includes_facelet_ids(self) -> bool {
        matches!(self, Self::FaceletIds | Self::Both)
    }
}

impl fmt::Display for FeatureSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::None => "none",
            Self::Cubie => "cubie",
            Self::FaceletIds => "facelet_ids",
            Self::Both => "both",
        })
    }
}

impl FromStr for FeatureSelection {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().replace('-', "_").as_str() {
            "none" | "off" => Ok(Self::None),
            "cubie" | "cubies" => Ok(Self::Cubie),
            "facelet" | "facelets" | "facelet_id" | "facelet_ids" => Ok(Self::FaceletIds),
            "both" | "all" => Ok(Self::Both),
            _ => Err(Error::Solver(format!(
                "unknown feature selection {value:?}"
            ))),
        }
    }
}

/// Lossless categorical cubie state in a fixed spatial frame.
///
/// Positions and IDs use these orders:
///
/// - centers: `U R F D L B`
/// - corners: `URF UFL ULB UBR DFR DLF DBL DRB`
/// - edges: `UR UF UL UB DR DF DL DB FR FL BL BR`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CubieFeatures {
    pub centers: [u8; 6],
    pub corner_permutation: [u8; 8],
    pub corner_orientation: [u8; 8],
    pub edge_permutation: [u8; 12],
    pub edge_orientation: [u8; 12],
}

impl CubieFeatures {
    pub fn from_cube(cube: &Cube) -> Self {
        Self {
            centers: cube.inner.center.map(color_id),
            corner_permutation: cube.inner.cp.map(corner_id),
            corner_orientation: cube.inner.co,
            edge_permutation: cube.inner.ep.map(edge_id),
            edge_orientation: cube.inner.eo,
        }
    }

    /// Return the stable 46-value layout used by the solver's compact keys.
    pub fn flat_ids(&self) -> [u8; CUBIE_FEATURE_WIDTH] {
        let mut values = [0; CUBIE_FEATURE_WIDTH];
        values[0..6].copy_from_slice(&self.centers);
        values[6..14].copy_from_slice(&self.corner_permutation);
        values[14..22].copy_from_slice(&self.corner_orientation);
        values[22..34].copy_from_slice(&self.edge_permutation);
        values[34..46].copy_from_slice(&self.edge_orientation);
        values
    }

    /// Factorized one-hot representation of the 46 categorical values.
    ///
    /// Public fields and Serde make it possible to construct feature values
    /// that did not originate from a validated cube, so category ranges are
    /// checked before indexing the fixed output buffer.
    pub fn one_hot(&self) -> Result<[u8; CUBIE_ONE_HOT_WIDTH]> {
        let ids = self.flat_ids();
        let mut result = [0; CUBIE_ONE_HOT_WIDTH];
        let mut offset = 0;
        for (index, (id, cardinality)) in
            ids.into_iter().zip(CUBIE_FEATURE_CARDINALITIES).enumerate()
        {
            if id >= cardinality {
                return Err(Error::InvalidState(format!(
                    "cubie feature {index} has category {id}, expected 0..{}",
                    cardinality - 1
                )));
            }
            result[offset + id as usize] = 1;
            offset += cardinality as usize;
        }
        Ok(result)
    }
}

/// Highest sequential human-style milestone satisfied in the exact frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CfopMilestone {
    #[default]
    None,
    Cross,
    #[serde(rename = "xcross")]
    XCross,
    #[serde(rename = "xxcross")]
    XXCross,
    #[serde(rename = "xxxcross")]
    XXXCross,
    F2l,
    Oll,
    Pll,
}

/// State-derived CFOP progress. Counts are useful even before a complete phase
/// invariant has been reached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfopProgress {
    /// Whether CFOP diagnostics are measured in the canonical URFDLB frame.
    pub canonical_orientation: bool,
    pub cross_edges_solved: u8,
    pub cross_solved: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub solved_pairs: Vec<F2lPair>,
    pub f2l_solved: bool,
    pub last_layer_oriented_cubies: u8,
    pub oll_solved: bool,
    pub last_layer_permuted_cubies: u8,
    pub pll_solved: bool,
    pub milestone: CfopMilestone,
}

/// Alias retained for APIs that refer to generic phase progress.
pub type PhaseProgress = CfopProgress;

/// Optional feature payload. The facelet string remains the authoritative state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CubeFeatures {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cubie_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cubie: Option<CubieFeatures>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facelet_id_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facelet_ids: Option<Vec<u8>>,
    pub cfop_progress: CfopProgress,
}

impl CubeFeatures {
    pub fn from_cube(cube: &Cube, selection: FeatureSelection) -> Option<Self> {
        (selection != FeatureSelection::None).then(|| Self {
            schema: CUBE_FEATURE_SCHEMA.into(),
            cubie_format: selection
                .includes_cubie()
                .then(|| CUBIE_FEATURE_FORMAT.into()),
            cubie: selection
                .includes_cubie()
                .then(|| CubieFeatures::from_cube(cube)),
            facelet_id_format: selection
                .includes_facelet_ids()
                .then(|| FACELET_ID_FORMAT.into()),
            facelet_ids: selection
                .includes_facelet_ids()
                .then(|| facelet_ids(cube).to_vec()),
            cfop_progress: phase_progress(cube),
        })
    }
}

impl Cube {
    pub fn cubie_features(&self) -> CubieFeatures {
        CubieFeatures::from_cube(self)
    }

    pub fn cubie_feature_ids(&self) -> [u8; CUBIE_FEATURE_WIDTH] {
        self.cubie_features().flat_ids()
    }

    pub fn cubie_one_hot(&self) -> [u8; CUBIE_ONE_HOT_WIDTH] {
        self.cubie_features()
            .one_hot()
            .expect("features extracted from a validated cube are in range")
    }

    pub fn facelet_ids(&self) -> [u8; FACELET_ID_WIDTH] {
        facelet_ids(self)
    }

    pub fn facelet_one_hot(&self) -> [u8; FACELET_ONE_HOT_WIDTH] {
        let mut result = [0; FACELET_ONE_HOT_WIDTH];
        for (position, id) in self.facelet_ids().into_iter().enumerate() {
            result[position * 6 + id as usize] = 1;
        }
        result
    }

    pub fn cfop_progress(&self) -> CfopProgress {
        phase_progress(self)
    }

    pub fn features(&self, selection: FeatureSelection) -> Option<CubeFeatures> {
        CubeFeatures::from_cube(self, selection)
    }
}

/// Numeric facelets in exact canonical-state position order, without changing
/// the cube's center frame.
pub fn facelet_ids(cube: &Cube) -> [u8; FACELET_ID_WIDTH] {
    facelets_from_cubies(&cube.inner).f.map(color_id)
}

/// Compute exact-frame CFOP progress. Callers that want orientation-invariant
/// diagnostics must explicitly normalize a copy before calling this function.
pub fn phase_progress(cube: &Cube) -> CfopProgress {
    let solved = SOLVED_CUBIE_CUBE;
    let cross_edges_solved = (4..8)
        .filter(|&index| cube.inner.ep[index] == solved.ep[index] && cube.inner.eo[index] == 0)
        .count() as u8;
    let cross_solved = cube.has_canonical_orientation() && cross_edges_solved == 4;
    let pair_mask = solved_f2l_pair_mask(cube);
    let solved_pairs = f2l_pairs(pair_mask);
    let f2l_solved = cross_solved && pair_mask == 0b1111;

    let last_layer_oriented_cubies = (0..4)
        .map(|index| {
            u8::from(corner_id(cube.inner.cp[index]) < 4 && cube.inner.co[index] == 0)
                + u8::from(edge_id(cube.inner.ep[index]) < 4 && cube.inner.eo[index] == 0)
        })
        .sum();
    let oll_solved = f2l_solved && last_layer_oriented_cubies == 8;

    let last_layer_permuted_cubies = (0..4)
        .map(|index| {
            u8::from(cube.inner.cp[index] == solved.cp[index])
                + u8::from(cube.inner.ep[index] == solved.ep[index])
        })
        .sum();
    let pll_solved = cube.is_canonically_solved();

    let milestone = if pll_solved {
        CfopMilestone::Pll
    } else if oll_solved {
        CfopMilestone::Oll
    } else if f2l_solved {
        CfopMilestone::F2l
    } else if cross_solved {
        match solved_pairs.len() {
            0 => CfopMilestone::Cross,
            1 => CfopMilestone::XCross,
            2 => CfopMilestone::XXCross,
            _ => CfopMilestone::XXXCross,
        }
    } else {
        CfopMilestone::None
    };

    CfopProgress {
        canonical_orientation: cube.has_canonical_orientation(),
        cross_edges_solved,
        cross_solved,
        solved_pairs,
        f2l_solved,
        last_layer_oriented_cubies,
        oll_solved,
        last_layer_permuted_cubies,
        pll_solved,
        milestone,
    }
}

pub(crate) fn solved_f2l_pair_mask(cube: &Cube) -> u8 {
    let solved = SOLVED_CUBIE_CUBE;
    let mut mask = 0;
    for (bit, corner, edge) in [(0, 4, 8), (1, 5, 9), (2, 6, 10), (3, 7, 11)] {
        if cube.inner.cp[corner] == solved.cp[corner]
            && cube.inner.co[corner] == 0
            && cube.inner.ep[edge] == solved.ep[edge]
            && cube.inner.eo[edge] == 0
        {
            mask |= 1 << bit;
        }
    }
    mask
}

pub(crate) fn f2l_pairs(mask: u8) -> Vec<F2lPair> {
    [F2lPair::Fr, F2lPair::Fl, F2lPair::Bl, F2lPair::Br]
        .into_iter()
        .enumerate()
        .filter_map(|(index, pair)| (mask & (1 << index) != 0).then_some(pair))
        .collect()
}

const fn color_id(color: Color) -> u8 {
    match color {
        Color::U => 0,
        Color::R => 1,
        Color::F => 2,
        Color::D => 3,
        Color::L => 4,
        Color::B => 5,
    }
}

const fn corner_id(corner: Corner) -> u8 {
    match corner {
        Corner::URF => 0,
        Corner::UFL => 1,
        Corner::ULB => 2,
        Corner::UBR => 3,
        Corner::DFR => 4,
        Corner::DLF => 5,
        Corner::DBL => 6,
        Corner::DRB => 7,
    }
}

const fn edge_id(edge: Edge) -> u8 {
    match edge {
        Edge::UR => 0,
        Edge::UF => 1,
        Edge::UL => 2,
        Edge::UB => 3,
        Edge::DR => 4,
        Edge::DF => 5,
        Edge::DL => 6,
        Edge::DB => 7,
        Edge::FR => 8,
        Edge::FL => 9,
        Edge::BL => 10,
        Edge::BR => 11,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_from_facelet_ids(ids: [u8; FACELET_ID_WIDTH]) -> String {
        ids.into_iter()
            .map(|id| match id {
                0 => 'U',
                1 => 'R',
                2 => 'F',
                3 => 'D',
                4 => 'L',
                5 => 'B',
                _ => panic!("invalid test ID"),
            })
            .collect()
    }

    #[test]
    fn solved_features_have_the_documented_ids_and_layout() {
        let cube = Cube::solved();
        let cubie = cube.cubie_features();
        assert_eq!(cubie.centers, [0, 1, 2, 3, 4, 5]);
        assert_eq!(cubie.corner_permutation, [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(cubie.corner_orientation, [0; 8]);
        assert_eq!(
            cubie.edge_permutation,
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
        );
        assert_eq!(cubie.edge_orientation, [0; 12]);
        assert_eq!(cubie.flat_ids().len(), CUBIE_FEATURE_WIDTH);
        assert!(cubie
            .flat_ids()
            .iter()
            .zip(CUBIE_FEATURE_CARDINALITIES)
            .all(|(&id, cardinality)| id < cardinality));
        assert_eq!(
            cubie
                .one_hot()
                .unwrap()
                .iter()
                .map(|&value| value as usize)
                .sum::<usize>(),
            CUBIE_FEATURE_WIDTH
        );
    }

    #[test]
    fn r_turn_is_a_stable_feature_golden() {
        let cube = Cube::from_scramble("R").unwrap();
        let cubie = cube.cubie_features();
        assert_eq!(cubie.centers, [0, 1, 2, 3, 4, 5]);
        assert_eq!(cubie.corner_permutation, [4, 1, 2, 0, 7, 5, 6, 3]);
        assert_eq!(cubie.corner_orientation, [2, 0, 0, 1, 1, 0, 0, 2]);
        assert_eq!(
            cubie.edge_permutation,
            [8, 1, 2, 3, 11, 5, 6, 7, 4, 9, 10, 0]
        );
        assert_eq!(cubie.edge_orientation, [0; 12]);
    }

    #[test]
    fn facelet_ids_preserve_the_exact_rotated_frame() {
        for algorithm in ["R U F'", "x", "M", "Rw U"] {
            let cube = Cube::from_scramble(algorithm).unwrap();
            assert_eq!(state_from_facelet_ids(cube.facelet_ids()), cube.facelets());
        }

        let rotated = Cube::from_scramble("x").unwrap();
        assert_eq!(rotated.cubie_features().centers, [2, 1, 3, 5, 4, 0]);
        assert!(rotated.is_solved());
        assert!(!rotated.cfop_progress().canonical_orientation);
        assert!(!rotated.cfop_progress().pll_solved);
    }

    #[test]
    fn feature_selection_omits_unrequested_representations() {
        let cube = Cube::from_scramble("R U").unwrap();
        assert!(cube.features(FeatureSelection::None).is_none());

        let cubie = cube.features(FeatureSelection::Cubie).unwrap();
        assert_eq!(cubie.cubie_format.as_deref(), Some(CUBIE_FEATURE_FORMAT));
        assert!(cubie.cubie.is_some());
        assert!(cubie.facelet_id_format.is_none());
        assert!(cubie.facelet_ids.is_none());

        let facelets = cube.features(FeatureSelection::FaceletIds).unwrap();
        assert!(facelets.cubie_format.is_none());
        assert!(facelets.cubie.is_none());
        assert_eq!(
            facelets.facelet_id_format.as_deref(),
            Some(FACELET_ID_FORMAT)
        );
        assert!(facelets.facelet_ids.is_some());

        let both = cube.features(FeatureSelection::Both).unwrap();
        assert!(both.cubie.is_some());
        assert!(both.facelet_ids.is_some());
        assert_eq!(both.schema, CUBE_FEATURE_SCHEMA);
    }

    #[test]
    fn malformed_public_categories_return_an_error_instead_of_panicking() {
        let mut cubie = Cube::solved().cubie_features();
        cubie.centers[0] = 6;
        assert!(matches!(cubie.one_hot(), Err(Error::InvalidState(_))));
    }

    #[test]
    fn progress_is_sequential_and_pair_order_is_stable() {
        let solved = Cube::solved().cfop_progress();
        assert!(solved.canonical_orientation);
        assert_eq!(solved.cross_edges_solved, 4);
        assert!(solved.cross_solved);
        assert_eq!(
            solved.solved_pairs,
            [F2lPair::Fr, F2lPair::Fl, F2lPair::Bl, F2lPair::Br]
        );
        assert!(solved.f2l_solved);
        assert_eq!(solved.last_layer_oriented_cubies, 8);
        assert!(solved.oll_solved);
        assert_eq!(solved.last_layer_permuted_cubies, 8);
        assert!(solved.pll_solved);
        assert_eq!(solved.milestone, CfopMilestone::Pll);

        let auf = Cube::from_scramble("U").unwrap().cfop_progress();
        assert!(auf.f2l_solved);
        assert!(auf.oll_solved);
        assert!(!auf.pll_solved);
        assert_eq!(auf.milestone, CfopMilestone::Oll);
        assert_eq!(solved_f2l_pair_mask(&Cube::solved()), 0b1111);
    }

    #[test]
    fn one_hot_shapes_are_fixed_and_have_one_value_per_position() {
        let cube = Cube::from_scramble("R U2 F' L D").unwrap();
        let cubie = cube.cubie_one_hot();
        let facelets = cube.facelet_one_hot();
        assert_eq!(cubie.len(), CUBIE_ONE_HOT_WIDTH);
        assert_eq!(facelets.len(), FACELET_ONE_HOT_WIDTH);
        assert_eq!(cubie.iter().map(|&value| value as usize).sum::<usize>(), 46);
        assert_eq!(
            facelets.iter().map(|&value| value as usize).sum::<usize>(),
            54
        );
    }
}
