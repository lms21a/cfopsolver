//! Traceable and batch-friendly Rubik's Cube solving.

pub mod algorithm;
pub mod cube;
pub mod error;
pub mod generator;
pub mod solver;

#[cfg(feature = "python")]
mod python;

pub use algorithm::{Algorithm, Move};
pub use cube::{Cube, SOLVED_FACELETS};
pub use error::{Error, Result};
pub use generator::{
    generate_dataset, generate_to_jsonl, write_jsonl, Difficulty, GenerateConfig, GeneratedSample,
};
pub use solver::{
    AlternativeSolution, CrossMode, Efficiency, ExplorationEvent, F2lPair, F2lStrategy,
    SolveConfig, SolveMethod, SolveResult, SolveStats, Solver, Stage, StageResult, TraceEvent,
};
