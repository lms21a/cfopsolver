//! Fault-tolerant, batch-oriented verification for training and evaluation.
//!
//! Invalid cube states, move strings, annotations, and reference solutions are
//! represented in each sample's result instead of aborting the surrounding
//! batch. Phase invariants are evaluated only at declared boundaries: many
//! valid CFOP algorithms temporarily disturb an invariant between boundaries.

use std::collections::HashSet;

use rayon::{prelude::*, ThreadPool, ThreadPoolBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    features::{
        f2l_pairs, phase_progress, solved_f2l_pair_mask, CubeFeatures, FeatureSelection,
        PhaseProgress,
    },
    generator::STATE_FORMAT,
    Algorithm, Cube, Error, F2lPair, Result, Stage,
};

/// Serialized schema returned by [`TrainingVerifier::verify_batch`].
pub const VERIFICATION_SCHEMA: &str = "cfop-verification/v1";

/// One independently verifiable training sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub state: String,
    #[serde(rename = "moves", alias = "solution", alias = "candidate")]
    pub moves: String,
    #[serde(default)]
    pub known_solutions: Vec<String>,
    #[serde(default)]
    pub phase_boundaries: Vec<PhaseBoundary>,
}

/// A declared phase boundary after a cumulative number of candidate moves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseBoundary {
    pub stage: Stage,
    /// Cumulative exclusive move offset. Zero is valid for an already-solved phase.
    pub after_move: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub f2l_pairs: Vec<F2lPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationBatchResult {
    pub schema: String,
    pub state_format: String,
    pub results: Vec<VerificationResult>,
}

/// Complete diagnostics for one candidate. Optional values are absent only
/// when invalid input makes them impossible to compute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub parse: ParseValidity,
    pub final_state: Option<String>,
    pub solved: bool,
    pub move_count: Option<usize>,
    pub progress: Option<PhaseProgress>,
    pub pair_delta: Option<PairDelta>,
    pub phase_invariants: PhaseInvariantCheck,
    pub known_solutions: KnownSolutionComparison,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<CubeFeatures>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseValidity {
    pub valid: bool,
    pub input_valid: bool,
    pub state_valid: bool,
    pub moves_valid: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ParseIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseIssue {
    pub target: ParseTarget,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseTarget {
    Input,
    State,
    Moves,
}

/// F2L slot identities before and after the candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairDelta {
    pub initial_solved: Vec<F2lPair>,
    pub final_solved: Vec<F2lPair>,
    pub preserved: Vec<F2lPair>,
    pub broken: Vec<F2lPair>,
    pub gained: Vec<F2lPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInvariantCheck {
    pub status: PhaseInvariantStatus,
    pub checkpoints_checked: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_violation: Option<PhaseInvariantViolation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PhaseInvariantCheck {
    fn not_provided() -> Self {
        Self {
            status: PhaseInvariantStatus::NotProvided,
            checkpoints_checked: 0,
            first_violation: None,
            error: None,
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: PhaseInvariantStatus::Invalid,
            checkpoints_checked: 0,
            first_violation: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseInvariantStatus {
    NotProvided,
    Passed,
    Failed,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInvariantViolation {
    /// `None` denotes the implicit final-state check after the last boundary.
    pub checkpoint_index: Option<usize>,
    pub after_move: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_stage: Option<Stage>,
    pub kind: PhaseInvariantKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_pairs: Vec<F2lPair>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actual_pairs: Vec<F2lPair>,
    pub state: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseInvariantKind {
    NonCanonicalOrientation,
    CrossNotSolved,
    RequiredPairNotSolved,
    CommittedPairBroken,
    F2lNotSolved,
    OllNotSolved,
    CubeNotSolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownSolutionComparison {
    pub provided: usize,
    pub parsed: usize,
    pub verified: usize,
    pub best_move_count: Option<usize>,
    /// Solving candidate length minus the shortest verified known length.
    /// Negative is better; unsolved candidates always return `None`.
    pub excess_moves: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<KnownSolutionIssue>,
}

impl KnownSolutionComparison {
    fn unavailable(provided: usize) -> Self {
        Self {
            provided,
            parsed: 0,
            verified: 0,
            best_move_count: None,
            excess_moves: None,
            issues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownSolutionIssue {
    pub index: usize,
    pub kind: KnownSolutionIssueKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownSolutionIssueKind {
    ParseError,
    DoesNotSolve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VerificationConfig {
    /// Zero uses the machine's available parallelism.
    pub threads: usize,
    /// Optional final-state feature payload for direct model ingestion.
    pub features: FeatureSelection,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            threads: 0,
            features: FeatureSelection::None,
        }
    }
}

/// A reusable verifier whose Rayon pool is constructed only once.
pub struct TrainingVerifier {
    config: VerificationConfig,
    pool: ThreadPool,
}

impl TrainingVerifier {
    pub fn new(config: VerificationConfig) -> Result<Self> {
        let mut builder = ThreadPoolBuilder::new();
        if config.threads != 0 {
            builder = builder.num_threads(config.threads);
        }
        let pool = builder.build().map_err(|error| {
            Error::Solver(format!("could not build verifier thread pool: {error}"))
        })?;
        Ok(Self { config, pool })
    }

    pub fn config(&self) -> &VerificationConfig {
        &self.config
    }

    pub fn verify(&self, input: &VerificationInput) -> VerificationResult {
        verify_at(0, input, &self.config)
    }

    /// Verify one JSON-like value without allowing structural errors to escape
    /// into a surrounding batch.
    pub fn verify_json(&self, input: &Value) -> VerificationResult {
        verify_json_at(0, input, &self.config)
    }

    /// Verify every input independently while retaining exact input order.
    pub fn verify_batch(&self, inputs: &[VerificationInput]) -> VerificationBatchResult {
        let results = self.pool.install(|| {
            inputs
                .par_iter()
                .enumerate()
                .map(|(index, input)| verify_at(index, input, &self.config))
                .collect()
        });
        batch_result(results)
    }

    /// Verify heterogeneous JSON-like values independently. Unknown fields,
    /// missing fields, bad enum values, and wrong types become per-item parse
    /// diagnostics while valid neighbors continue normally.
    pub fn verify_json_batch(&self, inputs: &[Value]) -> VerificationBatchResult {
        let results = self.pool.install(|| {
            inputs
                .par_iter()
                .enumerate()
                .map(|(index, input)| verify_json_at(index, input, &self.config))
                .collect()
        });
        batch_result(results)
    }
}

fn batch_result(results: Vec<VerificationResult>) -> VerificationBatchResult {
    VerificationBatchResult {
        schema: VERIFICATION_SCHEMA.into(),
        state_format: STATE_FORMAT.into(),
        results,
    }
}

/// Single-sample convenience API that does not construct a thread pool.
pub fn verify_training_item(input: &VerificationInput) -> VerificationResult {
    verify_at(0, input, &VerificationConfig::default())
}

/// One-shot convenience API. Reuse [`TrainingVerifier`] for repeated batches.
pub fn verify_training_batch(
    config: VerificationConfig,
    inputs: &[VerificationInput],
) -> Result<VerificationBatchResult> {
    Ok(TrainingVerifier::new(config)?.verify_batch(inputs))
}

fn verify_json_at(index: usize, input: &Value, config: &VerificationConfig) -> VerificationResult {
    match VerificationInput::deserialize(input) {
        Ok(input) => verify_at(index, &input, config),
        Err(error) => structural_error_at(index, input, config, error.to_string()),
    }
}

fn structural_error_at(
    index: usize,
    input: &Value,
    config: &VerificationConfig,
    message: String,
) -> VerificationResult {
    let id = input.get("id").and_then(Value::as_str).map(str::to_owned);
    let state = input.get("state").and_then(Value::as_str);
    let moves = ["moves", "solution", "candidate"]
        .iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str));
    if let (Some(state), Some(moves)) = (state, moves) {
        let known_solutions = input
            .get("known_solutions")
            .and_then(Value::as_array)
            .and_then(|values| {
                values
                    .iter()
                    .map(Value::as_str)
                    .map(|value| value.map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or_default();
        let partial = VerificationInput {
            id,
            state: state.to_owned(),
            moves: moves.to_owned(),
            known_solutions,
            phase_boundaries: Vec::new(),
        };
        let mut result = verify_at(index, &partial, config);
        result.parse.valid = false;
        result.parse.input_valid = false;
        result.parse.errors.insert(
            0,
            ParseIssue {
                target: ParseTarget::Input,
                message: message.clone(),
            },
        );
        result.phase_invariants =
            PhaseInvariantCheck::invalid(format!("input structure is invalid: {message}"));
        return result;
    }
    let parsed_state = input
        .get("state")
        .and_then(Value::as_str)
        .map(Cube::from_facelets);
    let parsed_moves = ["moves", "solution", "candidate"]
        .iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str))
        .map(str::parse::<Algorithm>);
    let state_valid = parsed_state
        .as_ref()
        .is_some_and(std::result::Result::is_ok);
    let moves_valid = parsed_moves
        .as_ref()
        .is_some_and(std::result::Result::is_ok);
    let move_count = parsed_moves
        .as_ref()
        .and_then(|parsed| parsed.as_ref().ok())
        .map(Algorithm::len);
    let provided = input
        .get("known_solutions")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    VerificationResult {
        index,
        id,
        parse: ParseValidity {
            valid: false,
            input_valid: false,
            state_valid,
            moves_valid,
            errors: vec![ParseIssue {
                target: ParseTarget::Input,
                message: message.clone(),
            }],
        },
        final_state: None,
        solved: false,
        move_count,
        progress: None,
        pair_delta: None,
        phase_invariants: PhaseInvariantCheck::invalid(format!(
            "input structure is invalid: {message}"
        )),
        known_solutions: KnownSolutionComparison::unavailable(provided),
        features: None,
    }
}

fn verify_at(
    index: usize,
    input: &VerificationInput,
    config: &VerificationConfig,
) -> VerificationResult {
    let parsed_state = Cube::from_facelets(&input.state);
    let parsed_moves = input.moves.parse::<Algorithm>();
    let mut errors = Vec::new();
    if let Err(error) = &parsed_state {
        errors.push(ParseIssue {
            target: ParseTarget::State,
            message: error.to_string(),
        });
    }
    if let Err(error) = &parsed_moves {
        errors.push(ParseIssue {
            target: ParseTarget::Moves,
            message: error.to_string(),
        });
    }
    let state_valid = parsed_state.is_ok();
    let moves_valid = parsed_moves.is_ok();
    let parse = ParseValidity {
        valid: state_valid && moves_valid,
        input_valid: true,
        state_valid,
        moves_valid,
        errors,
    };
    let move_count = parsed_moves.as_ref().ok().map(Algorithm::len);
    let known_solutions = parsed_state
        .as_ref()
        .ok()
        .map(|cube| compare_known_solutions(cube, &input.known_solutions, move_count, false))
        .unwrap_or_else(|| KnownSolutionComparison::unavailable(input.known_solutions.len()));

    let (initial, moves) = match (parsed_state, parsed_moves) {
        (Ok(initial), Ok(moves)) => (initial, moves),
        _ => {
            let phase_invariants = if input.phase_boundaries.is_empty() {
                PhaseInvariantCheck::not_provided()
            } else {
                PhaseInvariantCheck::invalid(
                    "state and moves must parse before phases can be checked",
                )
            };
            return VerificationResult {
                index,
                id: input.id.clone(),
                parse,
                final_state: None,
                solved: false,
                move_count,
                progress: None,
                pair_delta: None,
                phase_invariants,
                known_solutions,
                features: None,
            };
        }
    };

    let (final_cube, phase_invariants) =
        replay_with_boundaries(initial, &moves, &input.phase_boundaries);
    let progress = phase_progress(&final_cube);
    let normalized_initial = initial.normalized_orientation().0;
    let normalized_final = final_cube.normalized_orientation().0;
    let initial_mask = solved_f2l_pair_mask(&normalized_initial);
    let final_mask = solved_f2l_pair_mask(&normalized_final);
    let pair_delta = PairDelta {
        initial_solved: f2l_pairs(initial_mask),
        final_solved: f2l_pairs(final_mask),
        preserved: f2l_pairs(initial_mask & final_mask),
        broken: f2l_pairs(initial_mask & !final_mask),
        gained: f2l_pairs(final_mask & !initial_mask),
    };
    let features = final_cube.features(config.features);
    let final_state = final_cube.facelets();
    let solved = serialized_state_is_solved(&final_state);
    let known_solutions =
        compare_known_solutions(&initial, &input.known_solutions, move_count, solved);
    VerificationResult {
        index,
        id: input.id.clone(),
        parse,
        final_state: Some(final_state),
        solved,
        move_count,
        progress: Some(progress),
        pair_delta: Some(pair_delta),
        phase_invariants,
        known_solutions,
        features,
    }
}

fn serialized_state_is_solved(state: &str) -> bool {
    state
        .as_bytes()
        .chunks_exact(9)
        .all(|face| face.iter().all(|sticker| sticker == &face[4]))
}

fn compare_known_solutions(
    initial: &Cube,
    known: &[String],
    candidate_move_count: Option<usize>,
    candidate_solved: bool,
) -> KnownSolutionComparison {
    let mut parsed = 0usize;
    let mut verified = 0usize;
    let mut best_move_count = None;
    let mut issues = Vec::new();
    for (index, notation) in known.iter().enumerate() {
        let algorithm = match notation.parse::<Algorithm>() {
            Ok(algorithm) => algorithm,
            Err(error) => {
                issues.push(KnownSolutionIssue {
                    index,
                    kind: KnownSolutionIssueKind::ParseError,
                    message: error.to_string(),
                });
                continue;
            }
        };
        parsed += 1;
        let end = initial.after_algorithm(&algorithm);
        if !end.is_solved() {
            issues.push(KnownSolutionIssue {
                index,
                kind: KnownSolutionIssueKind::DoesNotSolve,
                message: "reference moves did not solve the supplied state".into(),
            });
            continue;
        }
        verified += 1;
        best_move_count =
            Some(best_move_count.map_or(algorithm.len(), |best: usize| best.min(algorithm.len())));
    }
    let excess_moves = if candidate_solved {
        candidate_move_count
            .zip(best_move_count)
            .map(|(candidate, best)| {
                i64::try_from(candidate).unwrap_or(i64::MAX)
                    - i64::try_from(best).unwrap_or(i64::MAX)
            })
    } else {
        None
    };
    KnownSolutionComparison {
        provided: known.len(),
        parsed,
        verified,
        best_move_count,
        excess_moves,
        issues,
    }
}

fn replay_with_boundaries(
    initial: Cube,
    moves: &Algorithm,
    boundaries: &[PhaseBoundary],
) -> (Cube, PhaseInvariantCheck) {
    if boundaries.is_empty() {
        return (
            initial.after_algorithm(moves),
            PhaseInvariantCheck::not_provided(),
        );
    }
    if let Err(message) = validate_boundaries(boundaries, moves.len()) {
        return (
            initial.after_algorithm(moves),
            PhaseInvariantCheck::invalid(message),
        );
    }

    let mut cube = initial;
    let mut cursor = 0usize;
    let mut committed = CommittedInvariants::default();
    let mut first_violation = None;
    let mut checkpoints_checked = 0usize;
    for (checkpoint_index, boundary) in boundaries.iter().enumerate() {
        apply_range(&mut cube, moves, cursor, boundary.after_move);
        cursor = boundary.after_move;
        if first_violation.is_some() {
            continue;
        }
        checkpoints_checked += 1;
        let context = ViolationContext {
            checkpoint_index: Some(checkpoint_index),
            after_move: cursor,
            claimed_stage: Some(boundary.stage),
        };
        if let Some(violation) = check_committed(&cube, committed, context) {
            first_violation = Some(violation);
            continue;
        }
        match check_claim(&cube, boundary, context) {
            Ok(newly_committed) => committed.merge(newly_committed),
            Err(violation) => first_violation = Some(*violation),
        }
    }
    apply_range(&mut cube, moves, cursor, moves.len());
    if first_violation.is_none() {
        let context = ViolationContext {
            checkpoint_index: None,
            after_move: moves.len(),
            claimed_stage: None,
        };
        first_violation = check_committed(&cube, committed, context);
    }
    let status = if first_violation.is_some() {
        PhaseInvariantStatus::Failed
    } else {
        PhaseInvariantStatus::Passed
    };
    (
        cube,
        PhaseInvariantCheck {
            status,
            checkpoints_checked,
            first_violation,
            error: None,
        },
    )
}

fn apply_range(cube: &mut Cube, moves: &Algorithm, start: usize, end: usize) {
    for &cube_move in &moves.moves()[start..end] {
        cube.apply_move(cube_move);
    }
}

fn validate_boundaries(
    boundaries: &[PhaseBoundary],
    move_count: usize,
) -> std::result::Result<(), String> {
    let mut previous_move = 0usize;
    let mut previous_rank = 0u8;
    for (index, boundary) in boundaries.iter().enumerate() {
        if boundary.after_move > move_count {
            return Err(format!(
                "phase boundary {index} ends after move {}, beyond candidate length {move_count}",
                boundary.after_move
            ));
        }
        if index > 0 && boundary.after_move < previous_move {
            return Err(format!(
                "phase boundary {index} moves backwards from {previous_move} to {}",
                boundary.after_move
            ));
        }
        let rank = stage_rank(boundary.stage);
        if index > 0 && rank < previous_rank {
            return Err(format!(
                "phase boundary {index} regresses from rank {previous_rank} to {rank}"
            ));
        }
        if boundary.stage == Stage::F2lPair && boundary.f2l_pairs.is_empty() {
            return Err(format!(
                "phase boundary {index} must identify at least one F2L pair"
            ));
        }
        let unique = boundary.f2l_pairs.iter().copied().collect::<HashSet<_>>();
        if unique.len() != boundary.f2l_pairs.len() {
            return Err(format!(
                "phase boundary {index} contains duplicate F2L pairs"
            ));
        }
        previous_move = boundary.after_move;
        previous_rank = rank;
    }
    Ok(())
}

const fn stage_rank(stage: Stage) -> u8 {
    match stage {
        Stage::Setup => 0,
        Stage::Cross | Stage::XCross | Stage::XXCross | Stage::XXXCross => 1,
        Stage::F2l | Stage::F2lPair => 2,
        Stage::Oll => 3,
        Stage::Pll | Stage::LastLayer | Stage::Fast => 4,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CommittedInvariants {
    canonical_orientation: bool,
    cross: bool,
    pairs: u8,
    f2l: bool,
    oll: bool,
    canonical_solved: bool,
    physical_solved: bool,
}

impl CommittedInvariants {
    fn merge(&mut self, other: Self) {
        self.canonical_orientation |= other.canonical_orientation;
        self.cross |= other.cross;
        self.pairs |= other.pairs;
        self.f2l |= other.f2l;
        self.oll |= other.oll;
        self.canonical_solved |= other.canonical_solved;
        self.physical_solved |= other.physical_solved;
    }
}

#[derive(Clone, Copy)]
struct ViolationContext {
    checkpoint_index: Option<usize>,
    after_move: usize,
    claimed_stage: Option<Stage>,
}

fn check_committed(
    cube: &Cube,
    committed: CommittedInvariants,
    context: ViolationContext,
) -> Option<PhaseInvariantViolation> {
    let progress = phase_progress(cube);
    let actual_mask = solved_f2l_pair_mask(cube);
    if committed.canonical_orientation && !cube.has_canonical_orientation() {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::NonCanonicalOrientation,
            Vec::new(),
            actual_mask,
            "the canonical centre frame established by setup was lost",
        ));
    }
    if committed.cross && !progress.cross_solved {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::CrossNotSolved,
            Vec::new(),
            actual_mask,
            "the committed bottom cross was broken",
        ));
    }
    if committed.pairs & actual_mask != committed.pairs {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::CommittedPairBroken,
            f2l_pairs(committed.pairs),
            actual_mask,
            "one or more committed F2L pairs were broken",
        ));
    }
    if committed.f2l && !progress.f2l_solved {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::F2lNotSolved,
            f2l_pairs(0b1111),
            actual_mask,
            "the completed first two layers were broken",
        ));
    }
    if committed.oll && !progress.oll_solved {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::OllNotSolved,
            f2l_pairs(0b1111),
            actual_mask,
            "last-layer orientation was lost",
        ));
    }
    if committed.canonical_solved && !progress.pll_solved {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::CubeNotSolved,
            f2l_pairs(0b1111),
            actual_mask,
            "moves after the solved boundary left the cube unsolved",
        ));
    }
    if committed.physical_solved && !cube.is_solved() {
        return Some(violation(
            cube,
            context,
            PhaseInvariantKind::CubeNotSolved,
            Vec::new(),
            actual_mask,
            "moves after the physical solved boundary left the cube unsolved",
        ));
    }
    None
}

fn check_claim(
    cube: &Cube,
    boundary: &PhaseBoundary,
    context: ViolationContext,
) -> std::result::Result<CommittedInvariants, Box<PhaseInvariantViolation>> {
    let progress = phase_progress(cube);
    let actual_mask = solved_f2l_pair_mask(cube);
    let declared_mask = pair_mask(&boundary.f2l_pairs);
    let cross_required = matches!(
        boundary.stage,
        Stage::Cross
            | Stage::XCross
            | Stage::XXCross
            | Stage::XXXCross
            | Stage::F2lPair
            | Stage::F2l
            | Stage::Oll
            | Stage::LastLayer
            | Stage::Pll
    );
    if cross_required && !progress.cross_solved {
        return Err(Box::new(violation(
            cube,
            context,
            PhaseInvariantKind::CrossNotSolved,
            boundary.f2l_pairs.clone(),
            actual_mask,
            "the declared phase did not preserve a solved bottom cross",
        )));
    }
    let mut committed = CommittedInvariants::default();
    match boundary.stage {
        Stage::Setup => {
            if !cube.has_canonical_orientation() {
                return Err(Box::new(violation(
                    cube,
                    context,
                    PhaseInvariantKind::NonCanonicalOrientation,
                    Vec::new(),
                    actual_mask,
                    "setup did not establish the canonical URFDLB centre frame",
                )));
            }
            committed.canonical_orientation = true;
        }
        Stage::Cross => {
            if declared_mask & actual_mask != declared_mask {
                return Err(Box::new(required_pair_violation(
                    cube,
                    context,
                    declared_mask,
                    actual_mask,
                )));
            }
            committed.cross = true;
            committed.pairs = declared_mask;
        }
        Stage::XCross | Stage::XXCross | Stage::XXXCross => {
            let required_count = match boundary.stage {
                Stage::XCross => 1,
                Stage::XXCross => 2,
                Stage::XXXCross => 3,
                _ => unreachable!(),
            };
            let committed_pairs = if boundary.f2l_pairs.is_empty() {
                actual_mask
            } else {
                declared_mask
            };
            if actual_mask.count_ones() < required_count
                || committed_pairs.count_ones() < required_count
                || committed_pairs & actual_mask != committed_pairs
            {
                return Err(Box::new(required_pair_violation(
                    cube,
                    context,
                    committed_pairs,
                    actual_mask,
                )));
            }
            committed.cross = true;
            committed.pairs = committed_pairs;
        }
        Stage::F2lPair => {
            if declared_mask & actual_mask != declared_mask {
                return Err(Box::new(required_pair_violation(
                    cube,
                    context,
                    declared_mask,
                    actual_mask,
                )));
            }
            committed.cross = true;
            committed.pairs = declared_mask;
        }
        Stage::F2l => {
            if !progress.f2l_solved {
                return Err(Box::new(violation(
                    cube,
                    context,
                    PhaseInvariantKind::F2lNotSolved,
                    f2l_pairs(0b1111),
                    actual_mask,
                    "the declared F2L boundary did not complete all four slots",
                )));
            }
            committed.cross = true;
            committed.pairs = 0b1111;
            committed.f2l = true;
        }
        Stage::Oll => {
            if !progress.oll_solved {
                return Err(Box::new(violation(
                    cube,
                    context,
                    PhaseInvariantKind::OllNotSolved,
                    f2l_pairs(0b1111),
                    actual_mask,
                    "the declared OLL boundary did not orient the last layer with F2L preserved",
                )));
            }
            committed.cross = true;
            committed.pairs = 0b1111;
            committed.f2l = true;
            committed.oll = true;
        }
        Stage::Pll | Stage::LastLayer => {
            if !progress.pll_solved {
                return Err(Box::new(violation(
                    cube,
                    context,
                    PhaseInvariantKind::CubeNotSolved,
                    f2l_pairs(0b1111),
                    actual_mask,
                    "the declared final boundary did not solve the cube",
                )));
            }
            committed.cross = true;
            committed.pairs = 0b1111;
            committed.f2l = true;
            committed.oll = true;
            committed.canonical_solved = true;
        }
        Stage::Fast => {
            if !cube.is_solved() {
                return Err(Box::new(violation(
                    cube,
                    context,
                    PhaseInvariantKind::CubeNotSolved,
                    Vec::new(),
                    actual_mask,
                    "the declared fast boundary did not physically solve the cube",
                )));
            }
            committed.physical_solved = true;
        }
    }
    Ok(committed)
}

fn required_pair_violation(
    cube: &Cube,
    context: ViolationContext,
    expected_mask: u8,
    actual_mask: u8,
) -> PhaseInvariantViolation {
    violation(
        cube,
        context,
        PhaseInvariantKind::RequiredPairNotSolved,
        f2l_pairs(expected_mask),
        actual_mask,
        "the declared phase did not solve all required F2L pairs",
    )
}

fn violation(
    cube: &Cube,
    context: ViolationContext,
    kind: PhaseInvariantKind,
    expected_pairs: Vec<F2lPair>,
    actual_mask: u8,
    message: impl Into<String>,
) -> PhaseInvariantViolation {
    PhaseInvariantViolation {
        checkpoint_index: context.checkpoint_index,
        after_move: context.after_move,
        claimed_stage: context.claimed_stage,
        kind,
        expected_pairs,
        actual_pairs: f2l_pairs(actual_mask),
        state: cube.facelets(),
        message: message.into(),
    }
}

fn pair_mask(pairs: &[F2lPair]) -> u8 {
    pairs.iter().fold(0u8, |mask, pair| {
        mask | match pair {
            F2lPair::Fr => 1,
            F2lPair::Fl => 2,
            F2lPair::Bl => 4,
            F2lPair::Br => 8,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(state: String, moves: &str) -> VerificationInput {
        VerificationInput {
            id: None,
            state,
            moves: moves.into(),
            known_solutions: Vec::new(),
            phase_boundaries: Vec::new(),
        }
    }

    #[test]
    fn invalid_samples_are_non_throwing_and_ordered() {
        let solved = Cube::solved().facelets();
        let records = vec![
            input(solved.clone(), ""),
            input(solved, "R definitely-not-a-move"),
            input("U".repeat(54), "R"),
        ];
        let batch = TrainingVerifier::new(VerificationConfig {
            threads: 2,
            ..VerificationConfig::default()
        })
        .unwrap()
        .verify_batch(&records);
        assert_eq!(batch.results.len(), 3);
        assert_eq!(batch.results[0].index, 0);
        assert_eq!(batch.results[1].index, 1);
        assert_eq!(batch.results[2].index, 2);
        assert!(batch.results[0].parse.valid);
        assert_eq!(batch.results[0].move_count, Some(0));
        assert!(batch.results[0].solved);
        assert!(!batch.results[1].parse.moves_valid);
        assert!(batch.results[1].final_state.is_none());
        assert!(!batch.results[2].parse.state_valid);
        assert_eq!(batch.results[2].move_count, Some(1));
    }

    #[test]
    fn compares_only_valid_verified_known_solutions() {
        let cube = Cube::from_scramble("R U F").unwrap();
        let mut record = input(cube.facelets(), "U U' F' U' R'");
        record.known_solutions = vec!["F' U' R'".into(), "bad".into(), "R".into()];
        let result = verify_training_item(&record);
        assert!(result.solved);
        assert_eq!(result.move_count, Some(5));
        assert_eq!(result.known_solutions.provided, 3);
        assert_eq!(result.known_solutions.parsed, 2);
        assert_eq!(result.known_solutions.verified, 1);
        assert_eq!(result.known_solutions.best_move_count, Some(3));
        assert_eq!(result.known_solutions.excess_moves, Some(2));
        assert_eq!(result.known_solutions.issues.len(), 2);
        assert!(result.progress.unwrap().pll_solved);
    }

    #[test]
    fn signed_excess_rewards_candidates_better_than_the_known_baseline() {
        let cube = Cube::from_scramble("R").unwrap();
        let mut record = input(cube.facelets(), "R'");
        record.known_solutions = vec!["U U' R'".into()];
        let result = verify_training_item(&record);
        assert!(result.solved);
        assert_eq!(result.known_solutions.best_move_count, Some(3));
        assert_eq!(result.known_solutions.excess_moves, Some(-2));
    }

    #[test]
    fn unsolved_candidate_never_receives_a_favorable_excess_score() {
        let cube = Cube::from_scramble("R U F").unwrap();
        let mut record = input(cube.facelets(), "");
        record.known_solutions = vec!["F' U' R'".into()];
        let result = verify_training_item(&record);
        assert!(!result.solved);
        assert_eq!(result.known_solutions.best_move_count, Some(3));
        assert_eq!(result.known_solutions.excess_moves, None);
    }

    #[test]
    fn rotated_solved_state_preserves_frame_and_reports_physical_solution() {
        let cube = Cube::from_scramble("x").unwrap();
        let exact_state = cube.facelets();
        let mut record = input(exact_state.clone(), "");
        record.known_solutions = vec![String::new()];
        record.phase_boundaries = vec![PhaseBoundary {
            stage: Stage::Fast,
            after_move: 0,
            f2l_pairs: Vec::new(),
        }];
        let result = verify_training_item(&record);
        assert!(result.solved);
        assert_eq!(result.final_state.as_deref(), Some(exact_state.as_str()));
        assert_eq!(result.known_solutions.verified, 1);
        assert_eq!(result.phase_invariants.status, PhaseInvariantStatus::Passed);
        let delta = result.pair_delta.unwrap();
        assert_eq!(delta.preserved.len(), 4);
        assert!(delta.broken.is_empty());
        let progress = result.progress.unwrap();
        assert!(!progress.canonical_orientation);
        assert!(!progress.pll_solved);
    }

    #[test]
    fn malformed_json_item_does_not_abort_valid_neighbors() {
        let cube = Cube::from_scramble("R U F").unwrap();
        let values = vec![
            serde_json::json!({
                "id": "before",
                "state": cube.facelets(),
                "moves": "F' U' R'"
            }),
            serde_json::json!({
                "id": "bad-annotation",
                "state": cube.facelets(),
                "moves": "F' U' R'",
                "phase_boundries": [{"stage": "wat", "after_move": 3}]
            }),
            serde_json::json!({
                "id": "after",
                "state": cube.facelets(),
                "moves": "F' U' R'"
            }),
        ];
        let batch = TrainingVerifier::new(VerificationConfig::default())
            .unwrap()
            .verify_json_batch(&values);
        assert!(batch.results[0].solved);
        assert_eq!(batch.results[0].id.as_deref(), Some("before"));
        assert!(!batch.results[1].parse.input_valid);
        assert!(batch.results[1].parse.state_valid);
        assert!(batch.results[1].parse.moves_valid);
        assert_eq!(batch.results[1].parse.errors[0].target, ParseTarget::Input);
        assert!(batch.results[1].solved);
        assert_eq!(
            batch.results[1].final_state,
            Some(Cube::solved().facelets())
        );
        assert!(batch.results[1].progress.is_some());
        assert!(batch.results[1].pair_delta.is_some());
        assert_eq!(
            batch.results[1].phase_invariants.status,
            PhaseInvariantStatus::Invalid
        );
        assert!(batch.results[2].solved);
        assert_eq!(batch.results[2].id.as_deref(), Some("after"));
    }

    #[test]
    fn reports_preserved_and_broken_pair_identities() {
        let result = verify_training_item(&input(Cube::solved().facelets(), "R"));
        let delta = result.pair_delta.unwrap();
        assert_eq!(
            delta.initial_solved,
            vec![F2lPair::Fr, F2lPair::Fl, F2lPair::Bl, F2lPair::Br]
        );
        assert_eq!(delta.final_solved, vec![F2lPair::Fl, F2lPair::Bl]);
        assert_eq!(delta.preserved, vec![F2lPair::Fl, F2lPair::Bl]);
        assert_eq!(delta.broken, vec![F2lPair::Fr, F2lPair::Br]);
        assert!(delta.gained.is_empty());
    }

    #[test]
    fn phase_checks_happen_at_boundaries_not_every_move() {
        let mut passing = input(Cube::solved().facelets(), "R R'");
        passing.phase_boundaries = vec![
            PhaseBoundary {
                stage: Stage::Pll,
                after_move: 0,
                f2l_pairs: Vec::new(),
            },
            PhaseBoundary {
                stage: Stage::Pll,
                after_move: 2,
                f2l_pairs: Vec::new(),
            },
        ];
        let passed = verify_training_item(&passing);
        assert_eq!(passed.phase_invariants.status, PhaseInvariantStatus::Passed);
        assert!(passed.phase_invariants.first_violation.is_none());

        let mut failing = passing;
        failing.phase_boundaries[1].after_move = 1;
        let failed = verify_training_item(&failing);
        assert_eq!(failed.phase_invariants.status, PhaseInvariantStatus::Failed);
        let violation = failed.phase_invariants.first_violation.unwrap();
        assert_eq!(violation.checkpoint_index, Some(1));
        assert_eq!(violation.after_move, 1);
        assert_eq!(violation.kind, PhaseInvariantKind::CrossNotSolved);
    }

    #[test]
    fn catches_a_committed_pair_broken_after_its_boundary() {
        let mut record = input(Cube::solved().facelets(), "R U R'");
        record.phase_boundaries.push(PhaseBoundary {
            stage: Stage::F2lPair,
            after_move: 0,
            f2l_pairs: vec![F2lPair::Fr],
        });
        let result = verify_training_item(&record);
        assert_eq!(result.phase_invariants.status, PhaseInvariantStatus::Failed);
        let violation = result.phase_invariants.first_violation.unwrap();
        assert_eq!(violation.checkpoint_index, None);
        assert_eq!(violation.kind, PhaseInvariantKind::CommittedPairBroken);
        assert_eq!(violation.expected_pairs, vec![F2lPair::Fr]);
    }

    #[test]
    fn rejects_malformed_boundaries_but_still_computes_the_final_state() {
        let mut record = input(Cube::solved().facelets(), "R U");
        record.phase_boundaries = vec![
            PhaseBoundary {
                stage: Stage::Oll,
                after_move: 2,
                f2l_pairs: Vec::new(),
            },
            PhaseBoundary {
                stage: Stage::Cross,
                after_move: 1,
                f2l_pairs: Vec::new(),
            },
        ];
        let result = verify_training_item(&record);
        assert_eq!(
            result.phase_invariants.status,
            PhaseInvariantStatus::Invalid
        );
        assert!(result.phase_invariants.error.is_some());
        assert!(result.final_state.is_some());
        assert_eq!(result.move_count, Some(2));
    }

    #[test]
    fn optional_structured_features_are_emitted_only_when_requested() {
        let record = input(Cube::solved().facelets(), "");
        let absent = verify_training_item(&record);
        assert!(absent.features.is_none());
        let verifier = TrainingVerifier::new(VerificationConfig {
            features: FeatureSelection::Cubie,
            ..VerificationConfig::default()
        })
        .unwrap();
        let present = verifier.verify(&record);
        assert!(present.features.is_some());
    }

    #[test]
    fn batching_is_deterministic_across_thread_counts() {
        let records = (0..64)
            .map(|index| {
                let mut record =
                    input(Cube::from_scramble("R U F").unwrap().facelets(), "F' U' R'");
                record.id = Some(format!("sample-{index}"));
                record
            })
            .collect::<Vec<_>>();
        let one = TrainingVerifier::new(VerificationConfig {
            threads: 1,
            ..VerificationConfig::default()
        })
        .unwrap()
        .verify_batch(&records);
        let four = TrainingVerifier::new(VerificationConfig {
            threads: 4,
            ..VerificationConfig::default()
        })
        .unwrap()
        .verify_batch(&records);
        assert_eq!(
            serde_json::to_value(one).unwrap(),
            serde_json::to_value(four).unwrap()
        );
    }
}
