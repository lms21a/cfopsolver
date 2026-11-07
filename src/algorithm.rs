use std::{fmt, str::FromStr};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    engine::moves::Move as EngineMove,
    error::{Error, Result},
};

/// A validated cube move. Display uses standard Singmaster notation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Move(pub(crate) EngineMove);

impl Move {
    pub fn inverse(self) -> Self {
        let (layer, turns) = self.base_and_turns();
        Self::from_layer_and_turns(layer, 4 - turns)
    }

    pub(crate) fn engine(self) -> EngineMove {
        self.0
    }

    pub(crate) fn from_engine(value: EngineMove) -> Option<Self> {
        (value != EngineMove::N).then_some(Self(value))
    }

    fn base_and_turns(self) -> (usize, u8) {
        let index = self.0 as usize;
        debug_assert!(index < 54, "validated moves never contain N");
        (index / 3, (index % 3 + 1) as u8)
    }

    fn from_layer_and_turns(layer: usize, turns: u8) -> Self {
        const MOVES: [EngineMove; 54] = [
            EngineMove::U,
            EngineMove::U2,
            EngineMove::U3,
            EngineMove::R,
            EngineMove::R2,
            EngineMove::R3,
            EngineMove::F,
            EngineMove::F2,
            EngineMove::F3,
            EngineMove::D,
            EngineMove::D2,
            EngineMove::D3,
            EngineMove::L,
            EngineMove::L2,
            EngineMove::L3,
            EngineMove::B,
            EngineMove::B2,
            EngineMove::B3,
            EngineMove::M,
            EngineMove::M2,
            EngineMove::M3,
            EngineMove::E,
            EngineMove::E2,
            EngineMove::E3,
            EngineMove::S,
            EngineMove::S2,
            EngineMove::S3,
            EngineMove::Uw,
            EngineMove::Uw2,
            EngineMove::Uw3,
            EngineMove::Rw,
            EngineMove::Rw2,
            EngineMove::Rw3,
            EngineMove::Fw,
            EngineMove::Fw2,
            EngineMove::Fw3,
            EngineMove::Dw,
            EngineMove::Dw2,
            EngineMove::Dw3,
            EngineMove::Lw,
            EngineMove::Lw2,
            EngineMove::Lw3,
            EngineMove::Bw,
            EngineMove::Bw2,
            EngineMove::Bw3,
            EngineMove::x,
            EngineMove::x2,
            EngineMove::x3,
            EngineMove::y,
            EngineMove::y2,
            EngineMove::y3,
            EngineMove::z,
            EngineMove::z2,
            EngineMove::z3,
        ];
        debug_assert!((1..=3).contains(&turns));
        Self(MOVES[layer * 3 + turns as usize - 1])
    }
}

impl FromStr for Move {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let value = value.trim().replace('’', "'");
        let parsed = EngineMove::from_str(&value)
            .map_err(|_| Error::InvalidAlgorithm(format!("unknown move {value:?}")))?;
        if parsed == EngineMove::N {
            return Err(Error::InvalidAlgorithm(
                "N is not an executable move".into(),
            ));
        }
        Ok(Self(parsed))
    }
}

impl fmt::Display for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Move")
            .field(&self.to_string())
            .finish()
    }
}

impl Serialize for Move {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Move {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// An owned, validated move sequence.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Algorithm {
    moves: Vec<Move>,
}

impl Algorithm {
    pub fn new(moves: Vec<Move>) -> Self {
        Self { moves }
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub fn len(&self) -> usize {
        self.moves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    pub fn inverse(&self) -> Self {
        Self::new(self.moves.iter().rev().map(|item| item.inverse()).collect())
    }

    /// Combine adjacent turns of the same layer, removing identities.
    pub fn simplified(&self) -> Self {
        let mut result: Vec<Move> = Vec::with_capacity(self.moves.len());
        for &next in &self.moves {
            if let Some(&last) = result.last() {
                let (last_base, last_turns) = last.base_and_turns();
                let (next_base, next_turns) = next.base_and_turns();
                if last_base == next_base {
                    result.pop();
                    let turns = (last_turns + next_turns) % 4;
                    if turns != 0 {
                        result.push(Move::from_layer_and_turns(last_base, turns));
                    }
                    continue;
                }
            }
            result.push(next);
        }
        Self::new(result)
    }

    pub(crate) fn from_engines(values: Vec<EngineMove>) -> Self {
        Self::new(values.into_iter().filter_map(Move::from_engine).collect())
    }
}

impl FromStr for Algorithm {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        if value.contains(['[', ']', ',', ':']) {
            return Err(Error::InvalidAlgorithm(
                "commutator/conjugate notation is not supported; expand it into moves".into(),
            ));
        }
        let mut nesting = 0usize;
        for character in value.chars() {
            match character {
                '(' => nesting += 1,
                ')' if nesting == 0 => {
                    return Err(Error::InvalidAlgorithm(
                        "unmatched closing parenthesis".into(),
                    ))
                }
                ')' => nesting -= 1,
                _ => {}
            }
        }
        if nesting != 0 {
            return Err(Error::InvalidAlgorithm("unclosed parenthesis".into()));
        }
        let cleaned = value.replace(['(', ')'], " ").replace('’', "'");
        let moves = cleaned
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<Vec<Move>>>()?;
        Ok(Self::new(moves))
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, item) in self.moves.iter().enumerate() {
            if index > 0 {
                formatter.write_str(" ")?;
            }
            item.fmt(formatter)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Algorithm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Algorithm")
            .field(&self.to_string())
            .finish()
    }
}

impl Serialize for Algorithm {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Algorithm {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inverts_and_simplifies() {
        let algorithm: Algorithm = "(R U R') U'".parse().unwrap();
        assert_eq!(algorithm.to_string(), "R U R' U'");
        assert_eq!(algorithm.inverse().to_string(), "U R U' R'");
        assert_eq!(
            "R R R"
                .parse::<Algorithm>()
                .unwrap()
                .simplified()
                .to_string(),
            "R'"
        );
        assert!("R R'".parse::<Algorithm>().unwrap().simplified().is_empty());
    }

    #[test]
    fn rejects_malformed_grouping_without_losing_valid_group_support() {
        for invalid in ["(R U", "R U)", ")(", "((R)"] {
            assert!(invalid.parse::<Algorithm>().is_err(), "{invalid}");
        }
        assert_eq!(
            "(R (U R'))".parse::<Algorithm>().unwrap().to_string(),
            "R U R'"
        );
        assert!("(R U)2".parse::<Algorithm>().is_err());
    }
}
