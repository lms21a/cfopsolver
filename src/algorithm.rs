use std::{fmt, str::FromStr};

use rcuber::moves::Move as EngineMove;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

/// A validated cube move. Display uses standard Singmaster notation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Move(pub(crate) EngineMove);

impl Move {
    pub fn inverse(self) -> Self {
        let notation = self.to_string();
        let inverse = if notation.ends_with('2') {
            notation
        } else if let Some(base) = notation.strip_suffix('\'') {
            base.to_owned()
        } else {
            format!("{notation}'")
        };
        inverse
            .parse()
            .expect("an engine move always has an inverse")
    }

    pub(crate) fn engine(self) -> EngineMove {
        self.0
    }

    pub(crate) fn from_engine(value: EngineMove) -> Option<Self> {
        (value != EngineMove::N).then_some(Self(value))
    }

    fn base_and_turns(self) -> (String, u8) {
        let notation = self.to_string();
        if let Some(base) = notation.strip_suffix('2') {
            (base.to_owned(), 2)
        } else if let Some(base) = notation.strip_suffix('\'') {
            (base.to_owned(), 3)
        } else {
            (notation, 1)
        }
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
                        let suffix = match turns {
                            2 => "2",
                            3 => "'",
                            _ => "",
                        };
                        result.push(format!("{last_base}{suffix}").parse().expect("valid merge"));
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
}
