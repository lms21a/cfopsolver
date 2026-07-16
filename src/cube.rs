use std::{fmt, str::FromStr};

use std::collections::{HashSet, VecDeque};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    algorithm::{Algorithm, Move},
    engine::{
        cubie::{Corner, CubieCube, Edge, SOLVED_CUBIE_CUBE},
        facelet::{
            Color, FaceCube, CENTER_FACELET, CORNER_COLOR, CORNER_FACELET, EDGE_COLOR, EDGE_FACELET,
        },
        moves::Move as EngineMove,
    },
    error::{Error, Result},
};

/// Canonical state order: U, R, F, D, L, B; nine row-major stickers per face.
pub const SOLVED_FACELETS: &str = "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB";

#[derive(Clone, Copy, PartialEq)]
pub struct Cube {
    pub(crate) inner: CubieCube,
}

impl Default for Cube {
    fn default() -> Self {
        Self::solved()
    }
}

impl Cube {
    pub const fn solved() -> Self {
        Self {
            inner: SOLVED_CUBIE_CUBE,
        }
    }

    pub fn from_facelets(value: &str) -> Result<Self> {
        validate_facelets(value)?;
        let face =
            FaceCube::try_from(value).map_err(|error| Error::InvalidState(error.to_string()))?;
        let inner = cubies_from_facelets(&face)?;
        Ok(Self { inner })
    }

    pub fn from_scramble(value: &str) -> Result<Self> {
        let mut cube = Self::solved();
        cube.apply_algorithm(&value.parse()?);
        Ok(cube)
    }

    /// The one canonical serialized representation used by every API and dataset.
    pub fn facelets(&self) -> String {
        facelets_from_cubies(&self.inner).to_string()
    }

    pub fn serialize(&self) -> String {
        self.facelets()
    }

    pub fn is_solved(&self) -> bool {
        let facelets = self.facelets();
        facelets
            .as_bytes()
            .chunks_exact(9)
            .all(|face| face.iter().all(|sticker| sticker == &face[4]))
    }

    pub fn is_canonically_solved(&self) -> bool {
        self.inner == SOLVED_CUBIE_CUBE
    }

    pub fn has_canonical_orientation(&self) -> bool {
        self.inner.center == SOLVED_CUBIE_CUBE.center
    }

    /// Return a physically equivalent orientation with canonical URFDLB centres,
    /// plus the rotations that transform this cube into it.
    pub fn normalized_orientation(&self) -> (Self, Algorithm) {
        if self.has_canonical_orientation() {
            return (*self, Algorithm::default());
        }
        let rotations: [Move; 3] =
            ["x", "y", "z"].map(|item| item.parse().expect("static rotation"));
        let mut seen = HashSet::new();
        let mut pending = VecDeque::new();
        seen.insert(self.inner.center);
        pending.push_back((*self, Vec::<Move>::new()));
        while let Some((cube, path)) = pending.pop_front() {
            for rotation in rotations {
                let next = cube.after_move(rotation);
                if seen.insert(next.inner.center) {
                    let mut next_path = path.clone();
                    next_path.push(rotation);
                    if next.has_canonical_orientation() {
                        return (next, Algorithm::new(next_path).simplified());
                    }
                    pending.push_back((next, next_path));
                }
            }
        }
        unreachable!("validated centre frames always have a canonical rotation")
    }

    pub fn apply_move(&mut self, cube_move: Move) -> &mut Self {
        self.inner = self.inner.apply_move(cube_move.engine());
        self
    }

    pub fn apply_algorithm(&mut self, algorithm: &Algorithm) -> &mut Self {
        for &cube_move in algorithm.moves() {
            self.inner = self.inner.apply_move(cube_move.engine());
        }
        self
    }

    pub fn apply(&mut self, algorithm: &str) -> Result<&mut Self> {
        let parsed = Algorithm::from_str(algorithm)?;
        Ok(self.apply_algorithm(&parsed))
    }

    pub fn after_move(&self, cube_move: Move) -> Self {
        let mut result = *self;
        result.apply_move(cube_move);
        result
    }

    pub fn after_algorithm(&self, algorithm: &Algorithm) -> Self {
        let mut result = *self;
        result.apply_algorithm(algorithm);
        result
    }

    pub fn after(&self, algorithm: &str) -> Result<Self> {
        let parsed = Algorithm::from_str(algorithm)?;
        Ok(self.after_algorithm(&parsed))
    }

    pub fn verifies(&self, solution: &str) -> Result<bool> {
        Ok(self.after(solution)?.is_solved())
    }
}

impl fmt::Display for Cube {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.facelets())
    }
}

impl fmt::Debug for Cube {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Cube")
            .field(&self.facelets())
            .finish()
    }
}

impl FromStr for Cube {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_facelets(value)
    }
}

impl Serialize for Cube {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.facelets())
    }
}

impl<'de> Deserialize<'de> for Cube {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_facelets(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn validate_facelets(value: &str) -> Result<()> {
    if value.len() != 54 || !value.is_ascii() {
        return Err(Error::InvalidState(format!(
            "expected exactly 54 ASCII facelets, got {} bytes",
            value.len()
        )));
    }
    let mut counts = [0usize; 6];
    for byte in value.bytes() {
        let index = match byte {
            b'U' => 0,
            b'R' => 1,
            b'F' => 2,
            b'D' => 3,
            b'L' => 4,
            b'B' => 5,
            other => {
                return Err(Error::InvalidState(format!(
                    "unexpected facelet character {:?}",
                    other as char
                )))
            }
        };
        counts[index] += 1;
    }
    if counts != [9; 6] {
        return Err(Error::InvalidState(format!(
            "each of URFDLB must occur nine times; counts were {counts:?}"
        )));
    }
    Ok(())
}

fn cubies_from_facelets(face: &FaceCube) -> Result<CubieCube> {
    let mut inner = CubieCube::default();
    for (position, facelet) in CENTER_FACELET.iter().enumerate() {
        inner.center[position] = face.f[*facelet as usize];
    }
    if !valid_center_frame(inner.center) {
        return Err(Error::InvalidState(
            "centres are not one of the 24 physical cube orientations".into(),
        ));
    }

    let mut seen_corners = [false; 8];
    for (position, corner_facelets) in CORNER_FACELET.iter().enumerate() {
        let orientation = (0..3)
            .find(|&index| matches!(face.f[corner_facelets[index] as usize], Color::U | Color::D))
            .ok_or_else(|| Error::InvalidState(format!("corner {position} has no U/D sticker")))?;
        let color1 = face.f[corner_facelets[(orientation + 1) % 3] as usize];
        let color2 = face.f[corner_facelets[(orientation + 2) % 3] as usize];
        let piece = (0..8)
            .find(|&candidate| {
                CORNER_COLOR[candidate][1] == color1 && CORNER_COLOR[candidate][2] == color2
            })
            .ok_or_else(|| Error::InvalidState(format!("invalid colors on corner {position}")))?;
        if seen_corners[piece] {
            return Err(Error::InvalidState(format!(
                "corner piece {piece} occurs more than once"
            )));
        }
        seen_corners[piece] = true;
        inner.cp[position] = Corner::try_from(piece as u8)
            .map_err(|error| Error::InvalidState(error.to_string()))?;
        inner.co[position] = orientation as u8;
    }

    let mut seen_edges = [false; 12];
    for (position, edge_facelets) in EDGE_FACELET.iter().enumerate() {
        let colors = [
            face.f[edge_facelets[0] as usize],
            face.f[edge_facelets[1] as usize],
        ];
        let (piece, orientation) = (0..12)
            .find_map(|candidate| {
                if colors == EDGE_COLOR[candidate] {
                    Some((candidate, 0))
                } else if colors == [EDGE_COLOR[candidate][1], EDGE_COLOR[candidate][0]] {
                    Some((candidate, 1))
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::InvalidState(format!("invalid colors on edge {position}")))?;
        if seen_edges[piece] {
            return Err(Error::InvalidState(format!(
                "edge piece {piece} occurs more than once"
            )));
        }
        seen_edges[piece] = true;
        inner.ep[position] =
            Edge::try_from(piece as u8).map_err(|error| Error::InvalidState(error.to_string()))?;
        inner.eo[position] = orientation;
    }

    if inner.co.iter().map(|&value| value as usize).sum::<usize>() % 3 != 0 {
        return Err(Error::InvalidState(
            "corner orientation sum is impossible".into(),
        ));
    }
    if inner.eo.iter().map(|&value| value as usize).sum::<usize>() % 2 != 0 {
        return Err(Error::InvalidState(
            "edge orientation sum is impossible".into(),
        ));
    }
    let corner_perm: Vec<usize> = inner.cp.iter().map(|value| *value as usize).collect();
    let edge_perm: Vec<usize> = inner.ep.iter().map(|value| *value as usize).collect();
    let center_perm: Vec<usize> = inner.center.iter().map(|value| *value as usize).collect();
    if permutation_parity(&corner_perm)
        ^ permutation_parity(&edge_perm)
        ^ permutation_parity(&center_perm)
        != 0
    {
        return Err(Error::InvalidState(
            "corner/edge/centre permutation parity is impossible".into(),
        ));
    }
    Ok(inner)
}

fn facelets_from_cubies(inner: &CubieCube) -> FaceCube {
    let mut face = FaceCube::default();
    for (position, facelet) in CENTER_FACELET.iter().enumerate() {
        face.f[*facelet as usize] = inner.center[position];
    }
    for (position, corner_facelets) in CORNER_FACELET.iter().enumerate() {
        let piece = inner.cp[position] as usize;
        for (sticker, facelet) in corner_facelets.iter().enumerate() {
            face.f[*facelet as usize] =
                CORNER_COLOR[piece][(sticker + (3 - inner.co[position] as usize)) % 3];
        }
    }
    for (position, edge_facelets) in EDGE_FACELET.iter().enumerate() {
        let piece = inner.ep[position] as usize;
        for (sticker, facelet) in edge_facelets.iter().enumerate() {
            face.f[*facelet as usize] =
                EDGE_COLOR[piece][(sticker + inner.eo[position] as usize) % 2];
        }
    }
    face
}

fn valid_center_frame(center: [Color; 6]) -> bool {
    let mut seen = HashSet::new();
    let mut pending = VecDeque::new();
    let solved = CubieCube::default();
    seen.insert(solved.center);
    pending.push_back(solved);
    while let Some(cube) = pending.pop_front() {
        for rotation in [EngineMove::x, EngineMove::y, EngineMove::z] {
            let next = cube.apply_move(rotation);
            if seen.insert(next.center) {
                pending.push_back(next);
            }
        }
    }
    seen.contains(&center)
}

fn permutation_parity(values: &[usize]) -> u8 {
    let inversions = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            values[index + 1..]
                .iter()
                .filter(|other| value > *other)
                .count()
        })
        .sum::<usize>();
    (inversions % 2) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_verify_solution() {
        let cube = Cube::from_scramble("R U R' U'").unwrap();
        assert_eq!(Cube::from_facelets(&cube.facelets()).unwrap(), cube);
        assert!(cube.verifies("U R U' R'").unwrap());
    }

    #[test]
    fn rejects_bad_counts_and_impossible_state() {
        assert!(Cube::from_facelets(&"U".repeat(54)).is_err());
        let mut impossible = SOLVED_FACELETS.as_bytes().to_vec();
        impossible.swap(0, 9);
        let impossible = String::from_utf8(impossible).unwrap();
        assert!(Cube::from_facelets(&impossible).is_err());
    }

    #[test]
    fn four_turns_are_identity() {
        for face in [
            "U", "R", "F", "D", "L", "B", "M", "E", "S", "Uw", "Rw", "Fw", "Dw", "Lw", "Bw", "x",
            "y", "z",
        ] {
            let cube = Cube::from_scramble(&format!("{face} {face} {face} {face}")).unwrap();
            assert!(cube.is_canonically_solved(), "{face}");
        }
    }

    #[test]
    fn rotated_and_slice_states_round_trip_exactly() {
        for algorithm in ["x", "y R U", "M", "Rw U Fw'"] {
            let cube = Cube::from_scramble(algorithm).unwrap();
            let restored = Cube::from_facelets(&cube.facelets()).unwrap();
            assert_eq!(restored, cube, "{algorithm}");
            assert_eq!(restored.facelets(), cube.facelets());
        }
        assert!(Cube::from_scramble("x").unwrap().is_solved());
    }

    #[test]
    fn r_turn_matches_facelet_golden() {
        assert_eq!(
            Cube::from_scramble("R").unwrap().facelets(),
            "UUFUUFUUFRRRRRRRRRFFDFFDFFDDDBDDBDDBLLLLLLLLLUBBUBBUBB"
        );
    }
}
