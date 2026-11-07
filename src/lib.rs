//! Traceable and batch-friendly Rubik's Cube solving.
#![doc = include_str!("../README.md")]

mod engine;

pub mod algorithm;
pub mod cube;
pub mod error;
pub mod features;
pub mod generator;
pub mod solver;
pub mod verifier;

#[cfg(feature = "python")]
mod python;

pub use algorithm::{Algorithm, Move};
pub use cube::{Cube, SOLVED_FACELETS};
pub use error::{Error, Result};
pub use features::{
    CfopMilestone, CfopProgress, CubeFeatures, CubieFeatures, FeatureSelection, PhaseProgress,
    CUBE_FEATURE_SCHEMA, CUBIE_FEATURE_CARDINALITIES, CUBIE_FEATURE_FORMAT, CUBIE_FEATURE_WIDTH,
    CUBIE_ONE_HOT_WIDTH, FACELET_ID_FORMAT, FACELET_ID_WIDTH, FACELET_ONE_HOT_WIDTH,
};
pub use generator::{
    generate_dataset, generate_to_jsonl, write_jsonl, Difficulty, GenerateConfig, GeneratedSample,
};
pub use solver::{
    AlternativeSolution, CrossMode, DecisionDomain, DecisionKind, DecisionOutcome, DecisionReason,
    DecisionRecord, DecisionScore, DecisionTrace, Efficiency, ExplorationEvent, F2lPair,
    F2lStrategy, SolveConfig, SolveMethod, SolveResult, SolveStats, Solver, Stage, StageResult,
    TraceEvent, DECISION_TRACE_SCHEMA,
};
pub use verifier::{
    verify_training_batch, verify_training_item, KnownSolutionComparison, KnownSolutionIssue,
    KnownSolutionIssueKind, PairDelta, ParseIssue, ParseTarget, ParseValidity, PhaseBoundary,
    PhaseInvariantCheck, PhaseInvariantKind, PhaseInvariantStatus, PhaseInvariantViolation,
    TrainingVerifier, VerificationBatchResult, VerificationConfig, VerificationInput,
    VerificationResult, VERIFICATION_SCHEMA,
};
