use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    panic::{catch_unwind, AssertUnwindSafe},
    str::FromStr,
    time::Instant,
};

use serde::{Deserialize, Serialize};

use crate::{
    engine::{
        cfop::{CrossSolver, F2LSolver, OLLSolver, PLLSolver},
        cubie::SOLVED_CUBIE_CUBE,
        min2phase::solver::Solver as TwoPhaseSolver,
        moves::Move as EngineMove,
    },
    Algorithm, Cube, Error, Move, Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveMethod {
    /// Human-style Cross, F2L, OLL, PLL stages.
    Cfop,
    /// High-throughput two-phase search, typically with shorter solutions.
    Fast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossMode {
    Cross,
    #[serde(rename = "xcross")]
    XCross,
    #[serde(rename = "xxcross")]
    XXCross,
    #[serde(rename = "xxxcross")]
    XXXCross,
}

impl CrossMode {
    pub const fn included_pairs(self) -> usize {
        match self {
            Self::Cross => 0,
            Self::XCross => 1,
            Self::XXCross => 2,
            Self::XXXCross => 3,
        }
    }

    const fn stage(self) -> Stage {
        match self {
            Self::Cross => Stage::Cross,
            Self::XCross => Stage::XCross,
            Self::XXCross => Stage::XXCross,
            Self::XXXCross => Stage::XXXCross,
        }
    }
}

impl Default for CrossMode {
    fn default() -> Self {
        Self::Cross
    }
}

impl fmt::Display for CrossMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cross => "cross",
            Self::XCross => "xcross",
            Self::XXCross => "xxcross",
            Self::XXXCross => "xxxcross",
        })
    }
}

impl FromStr for CrossMode {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "cross" | "0" => Ok(Self::Cross),
            "x" | "xcross" | "1" => Ok(Self::XCross),
            "xx" | "xxcross" | "2" => Ok(Self::XXCross),
            "xxx" | "xxxcross" | "3" => Ok(Self::XXXCross),
            _ => Err(Error::InvalidConfig(format!(
                "unknown cross mode {value:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum F2lStrategy {
    Standard,
    Lookahead,
}

impl Default for F2lStrategy {
    fn default() -> Self {
        Self::Lookahead
    }
}

impl fmt::Display for F2lStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Standard => "standard",
            Self::Lookahead => "lookahead",
        })
    }
}

impl FromStr for F2lStrategy {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "standard" | "legacy" => Ok(Self::Standard),
            "lookahead" | "efficient" | "advanced" => Ok(Self::Lookahead),
            _ => Err(Error::InvalidConfig(format!(
                "unknown F2L strategy {value:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Efficiency {
    Fast,
    Balanced,
    Thorough,
    Max,
}

impl Efficiency {
    fn exact_budgets(self, configured_limit: u64) -> Vec<u64> {
        let requested = match self {
            Self::Fast => &[][..],
            Self::Balanced => &[100_000][..],
            Self::Thorough => &[100_000, 500_000][..],
            Self::Max => &[100_000, 500_000, u64::MAX][..],
        };
        let mut budgets = Vec::with_capacity(requested.len());
        for &budget in requested {
            let budget = budget.min(configured_limit);
            if budget > 0 && budgets.last().copied() != Some(budget) {
                budgets.push(budget);
            }
        }
        budgets
    }
}

impl Default for Efficiency {
    fn default() -> Self {
        Self::Balanced
    }
}

impl fmt::Display for Efficiency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Thorough => "thorough",
            Self::Max => "max",
        })
    }
}

impl FromStr for Efficiency {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "fast" | "speed" | "low" => Ok(Self::Fast),
            "balanced" | "default" | "medium" => Ok(Self::Balanced),
            "thorough" | "high" | "quality" => Ok(Self::Thorough),
            "max" | "maximum" | "exhaustive" => Ok(Self::Max),
            _ => Err(Error::InvalidConfig(format!(
                "unknown efficiency profile {value:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum F2lPair {
    Fr,
    Fl,
    Bl,
    Br,
}

impl fmt::Display for F2lPair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Fr => "FR",
            Self::Fl => "FL",
            Self::Bl => "BL",
            Self::Br => "BR",
        })
    }
}

impl Default for SolveMethod {
    fn default() -> Self {
        Self::Cfop
    }
}

impl fmt::Display for SolveMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cfop => "cfop",
            Self::Fast => "fast",
        })
    }
}

impl FromStr for SolveMethod {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "cfop" => Ok(Self::Cfop),
            "fast" | "two_phase" | "two-phase" | "kociemba" => Ok(Self::Fast),
            _ => Err(Error::InvalidConfig(format!(
                "unknown solve method {value:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Setup,
    Cross,
    #[serde(rename = "xcross")]
    XCross,
    #[serde(rename = "xxcross")]
    XXCross,
    #[serde(rename = "xxxcross")]
    XXXCross,
    /// Retained for deserializing v1 aggregate traces.
    F2l,
    F2lPair,
    Oll,
    Pll,
    /// Combined OLL+PLL evaluation used by real decision records.
    LastLayer,
    Fast,
}

impl fmt::Display for Stage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Setup => "setup",
            Self::Cross => "cross",
            Self::XCross => "xcross",
            Self::XXCross => "xxcross",
            Self::XXXCross => "xxxcross",
            Self::F2l => "f2l",
            Self::F2lPair => "f2l_pair",
            Self::Oll => "oll",
            Self::Pll => "pll",
            Self::LastLayer => "last_layer",
            Self::Fast => "fast",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SolveConfig {
    pub method: SolveMethod,
    pub cross_mode: CrossMode,
    pub f2l_strategy: F2lStrategy,
    pub efficiency: Efficiency,
    pub trace: bool,
    pub exploration: bool,
    pub max_exploration_states: usize,
    /// Emit bounded, structured records from the searches that made the
    /// committed decisions. Disabled by default and independent of the legacy
    /// phase trace and one-ply exploration snapshot.
    pub decisions: bool,
    /// Hard cap on structured decision records. Search continues normally
    /// after the recorder reaches this limit.
    pub max_decision_records: usize,
    /// Number of distinct additional solutions to request.
    pub alternatives: usize,
    pub max_depth: usize,
    pub probe_limit: u64,
}

impl SolveConfig {
    /// Validate search limits before allocating search state or producing output.
    pub fn validate(&self) -> Result<()> {
        if !(1..=30).contains(&self.max_depth) {
            return Err(Error::InvalidConfig(
                "max_depth must be between 1 and 30".into(),
            ));
        }
        if self.probe_limit == 0 {
            return Err(Error::InvalidConfig(
                "probe_limit must be at least one".into(),
            ));
        }
        Ok(())
    }
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self {
            method: SolveMethod::Cfop,
            cross_mode: CrossMode::Cross,
            f2l_strategy: F2lStrategy::Lookahead,
            efficiency: Efficiency::Balanced,
            trace: false,
            exploration: false,
            max_exploration_states: 72,
            decisions: false,
            max_decision_records: 512,
            alternatives: 0,
            max_depth: 30,
            probe_limit: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage: Stage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub f2l_pairs: Vec<F2lPair>,
    pub start_state: String,
    pub end_state: String,
    #[serde(rename = "moves", alias = "algorithm")]
    pub algorithm: Algorithm,
    pub move_count: usize,
    pub elapsed_micros: u64,
    /// True when an upstream stage's mutable state diverged from its returned
    /// moves and the transition was independently reconstructed and verified.
    #[serde(default)]
    pub repaired: bool,
    /// True when exact-target search shortened the selected human-style branch.
    #[serde(default)]
    pub optimized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceKind {
    PhaseStart,
    Decision,
    MovesApplied,
    PhaseComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub index: usize,
    pub stage: Stage,
    pub kind: TraceKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub f2l_pairs: Vec<F2lPair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(
        rename = "moves",
        alias = "algorithm",
        skip_serializing_if = "Option::is_none"
    )]
    pub algorithm: Option<Algorithm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplorationKind {
    Candidate,
    Selected,
    Backtrack,
}

/// A bounded, reconstructible view of alternative one-ply decisions at a stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationEvent {
    pub index: usize,
    pub parent_id: Option<usize>,
    pub stage: Stage,
    pub kind: ExplorationKind,
    pub depth: usize,
    pub via_move: Move,
    pub state: String,
    pub score: u16,
    pub accepted: bool,
    pub reason: String,
}

pub const DECISION_TRACE_SCHEMA: &str = "cfop-decisions/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionDomain {
    Phase,
    F2lVariant,
    F2lBeam,
    ExactTarget,
    LastLayer,
    TwoPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    SearchStart,
    Candidate,
    Evaluation,
    CacheHit,
    Prune,
    Backtrack,
    Select,
    SearchEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOutcome {
    Considered,
    Accepted,
    Rejected,
    Pruned,
    Selected,
    Failed,
    Cached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    SetupNormalization,
    PhaseCommitted,
    VariantStarted,
    VariantCompleted,
    VariantFailed,
    PairProgress,
    NoPairProgress,
    PairRegression,
    DuplicateState,
    BeamWidth,
    TerminalScored,
    LowerTotalMoves,
    HigherTotalMoves,
    CacheHit,
    ExactFound,
    ExactUnavailable,
    ExactShorter,
    ExactNotShorter,
    LastLayerSolved,
    TwoPhaseSolution,
}

/// Deterministic, domain-specific search scores. Wall-clock values are
/// intentionally excluded so seeded datasets remain reproducible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionScore {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_moves: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heuristic: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_moves: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_layer_moves: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solved_pairs: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beam_width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auf: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Stable per-solve identifier.
    pub index: usize,
    /// Earlier decision that explains this record's provenance. This is not
    /// necessarily a state-transition edge; use `start_state` to replay moves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<usize>,
    pub stage: Stage,
    pub domain: DecisionDomain,
    pub kind: DecisionKind,
    pub depth: usize,
    pub outcome: DecisionOutcome,
    pub reason: DecisionReason,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub f2l_pairs: Vec<F2lPair>,
    #[serde(
        rename = "moves",
        alias = "algorithm",
        skip_serializing_if = "Option::is_none"
    )]
    pub algorithm: Option<Algorithm>,
    /// Exact replay origin when `moves` represents a state transition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_state: Option<String>,
    /// Evaluated or resulting state. With `start_state` and `moves`, this is
    /// guaranteed to be the replayed transition endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<DecisionScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTrace {
    pub schema: String,
    pub truncated: bool,
    pub dropped_records: usize,
    pub records: Vec<DecisionRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolveStats {
    pub method: SolveMethod,
    pub elapsed_micros: u64,
    pub move_count: usize,
    pub exploration_states: usize,
    #[serde(default)]
    pub f2l_candidates_evaluated: usize,
    #[serde(default)]
    pub exact_target_searches: usize,
    #[serde(default)]
    pub last_layer_evaluations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeSolution {
    pub source: String,
    pub algorithm: Algorithm,
    pub move_count: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResult {
    pub input_state: String,
    pub method: SolveMethod,
    #[serde(default)]
    pub cross_mode: CrossMode,
    #[serde(default)]
    pub f2l_strategy: F2lStrategy,
    #[serde(default)]
    pub efficiency: Efficiency,
    pub solution: Algorithm,
    pub verified: bool,
    pub stages: Vec<StageResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace: Vec<TraceEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exploration: Vec<ExplorationEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisions: Option<DecisionTrace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<AlternativeSolution>,
    pub stats: SolveStats,
}

struct DecisionRecorder {
    enabled: bool,
    max_records: usize,
    attempted_records: usize,
    records: Vec<DecisionRecord>,
}

impl DecisionRecorder {
    fn new(config: &SolveConfig) -> Self {
        Self {
            enabled: config.decisions,
            max_records: config.max_decision_records,
            attempted_records: 0,
            records: Vec::with_capacity(config.max_decision_records.min(if config.decisions {
                512
            } else {
                0
            })),
        }
    }

    /// Builds records only when instrumentation is enabled. This keeps state
    /// serialization and message allocation entirely off the default path;
    /// enabled streams may build omitted records so late priority decisions
    /// can replace earlier ones without affecting search behavior.
    fn record(&mut self, build: impl FnOnce(usize) -> DecisionRecord) -> Option<usize> {
        if !self.enabled {
            return None;
        }
        let index = self.attempted_records;
        self.attempted_records = self.attempted_records.saturating_add(1);
        if self.max_records == 0 {
            return None;
        }
        let record = build(index);
        // Keep early candidate/cache chatter from consuming the entire trace.
        // Priority buckets retain the latest selections and phase commits, so
        // tiny caps still contain the decisions that actually finished the
        // solve. Logical IDs remain stable even if an earlier priority record
        // is evicted; dangling provenance parents are removed in `finish`.
        let quarter_rounded_up = self.max_records.saturating_add(3) / 4;
        let phase_reserve = quarter_rounded_up.min(16);
        let selection_reserve = quarter_rounded_up
            .min(48)
            .min(self.max_records.saturating_sub(phase_reserve));
        let selection_limit = self.max_records.saturating_sub(phase_reserve);
        let normal_limit = selection_limit.saturating_sub(selection_reserve);
        let is_phase = record.domain == DecisionDomain::Phase;
        let is_selection =
            !is_phase && matches!(record.kind, DecisionKind::Select | DecisionKind::SearchEnd);
        let retained = if is_phase {
            let phase_count = self
                .records
                .iter()
                .filter(|existing| existing.domain == DecisionDomain::Phase)
                .count();
            if phase_count < phase_reserve {
                self.records.push(record);
                true
            } else if let Some(position) = self
                .records
                .iter()
                .position(|existing| existing.domain == DecisionDomain::Phase)
            {
                self.records[position] = record;
                true
            } else {
                false
            }
        } else if is_selection {
            let selection_count = self
                .records
                .iter()
                .filter(|existing| {
                    existing.domain != DecisionDomain::Phase
                        && matches!(
                            existing.kind,
                            DecisionKind::Select | DecisionKind::SearchEnd
                        )
                })
                .count();
            if selection_count < selection_reserve {
                self.records.push(record);
                true
            } else if let Some(position) = self.records.iter().position(|existing| {
                existing.domain != DecisionDomain::Phase
                    && matches!(
                        existing.kind,
                        DecisionKind::Select | DecisionKind::SearchEnd
                    )
            }) {
                self.records[position] = record;
                true
            } else {
                false
            }
        } else {
            let normal_count = self
                .records
                .iter()
                .filter(|existing| {
                    existing.domain != DecisionDomain::Phase
                        && !matches!(
                            existing.kind,
                            DecisionKind::Select | DecisionKind::SearchEnd
                        )
                })
                .count();
            if normal_count < normal_limit {
                self.records.push(record);
                true
            } else {
                false
            }
        };
        retained.then_some(index)
    }

    fn finish(mut self) -> Option<DecisionTrace> {
        self.enabled.then(|| {
            self.records.sort_unstable_by_key(|record| record.index);
            let retained_ids = self
                .records
                .iter()
                .map(|record| record.index)
                .collect::<HashSet<_>>();
            for record in &mut self.records {
                if record
                    .parent_id
                    .is_some_and(|parent| !retained_ids.contains(&parent))
                {
                    record.parent_id = None;
                }
            }
            let dropped_records = self.attempted_records.saturating_sub(self.records.len());
            DecisionTrace {
                schema: DECISION_TRACE_SCHEMA.into(),
                truncated: dropped_records > 0,
                dropped_records,
                records: self.records,
            }
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Solver {
    config: SolveConfig,
}

impl Solver {
    pub fn new(config: SolveConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SolveConfig {
        &self.config
    }

    pub fn solve(&self, cube: &Cube) -> Result<SolveResult> {
        self.config.validate()?;
        let started = Instant::now();
        let input_state = cube.facelets();
        let mut decisions = DecisionRecorder::new(&self.config);
        if cube.is_solved() {
            let solution = Algorithm::default();
            let mut alternatives = Vec::new();
            fill_identity_alternatives(
                cube,
                &solution,
                &mut alternatives,
                self.config.alternatives,
            );
            return Ok(SolveResult {
                input_state,
                method: self.config.method,
                cross_mode: self.config.cross_mode,
                f2l_strategy: self.config.f2l_strategy,
                efficiency: self.config.efficiency,
                solution,
                verified: true,
                stages: Vec::new(),
                trace: Vec::new(),
                exploration: Vec::new(),
                decisions: decisions.finish(),
                alternatives,
                stats: SolveStats {
                    method: self.config.method,
                    elapsed_micros: micros(started.elapsed().as_micros()),
                    move_count: 0,
                    exploration_states: 0,
                    f2l_candidates_evaluated: 0,
                    exact_target_searches: 0,
                    last_layer_evaluations: 0,
                },
            });
        }
        let (working_cube, setup) = cube.normalized_orientation();
        if !setup.is_empty() {
            decisions.record(|index| DecisionRecord {
                index,
                parent_id: None,
                stage: Stage::Setup,
                domain: DecisionDomain::Phase,
                kind: DecisionKind::Select,
                depth: 0,
                outcome: DecisionOutcome::Selected,
                reason: DecisionReason::SetupNormalization,
                message: "normalized the cube into the canonical URFDLB solving frame".into(),
                f2l_pairs: Vec::new(),
                algorithm: Some(setup.clone()),
                start_state: Some(cube.facelets()),
                state: Some(working_cube.facelets()),
                score: Some(DecisionScore {
                    path_moves: Some(setup.len()),
                    ..DecisionScore::default()
                }),
            });
        }
        let mut result = match self.config.method {
            SolveMethod::Cfop => self.solve_cfop(&working_cube, &mut decisions)?,
            SolveMethod::Fast => self.solve_fast(&working_cube, &mut decisions)?,
        };

        if self.config.alternatives > 0 {
            result.alternatives = fast_alternatives(
                &working_cube,
                self.config.max_depth,
                self.config.probe_limit,
                self.config.alternatives,
                Some(&result.solution),
            )?
            .into_iter()
            .map(|algorithm| AlternativeSolution {
                move_count: algorithm.len(),
                verified: working_cube.after_algorithm(&algorithm).is_solved(),
                source: "continued_search".into(),
                algorithm,
            })
            .collect();
        }

        if !setup.is_empty() {
            let setup_end = working_cube.facelets();
            result.stages.insert(
                0,
                StageResult {
                    stage: Stage::Setup,
                    f2l_pairs: Vec::new(),
                    start_state: input_state.clone(),
                    end_state: setup_end.clone(),
                    move_count: setup.len(),
                    algorithm: setup.clone(),
                    elapsed_micros: 0,
                    repaired: false,
                    optimized: false,
                },
            );
            if self.config.trace {
                result.trace.insert(
                    0,
                    TraceEvent {
                        index: 0,
                        stage: Stage::Setup,
                        kind: TraceKind::Decision,
                        message: "rotated the cube into the canonical URFDLB solving frame".into(),
                        f2l_pairs: Vec::new(),
                        state: Some(setup_end),
                        algorithm: Some(setup.clone()),
                    },
                );
                for (index, event) in result.trace.iter_mut().enumerate() {
                    event.index = index;
                }
            }
            let mut prefixed = setup.moves().to_vec();
            prefixed.extend(result.solution.moves().iter().copied());
            result.solution = Algorithm::new(prefixed).simplified();
            for alternative in &mut result.alternatives {
                let mut prefixed = setup.moves().to_vec();
                prefixed.extend(alternative.algorithm.moves().iter().copied());
                alternative.algorithm = Algorithm::new(prefixed).simplified();
                alternative.move_count = alternative.algorithm.len();
                alternative.verified = cube.after_algorithm(&alternative.algorithm).is_solved();
            }
        }

        fill_identity_alternatives(
            cube,
            &result.solution,
            &mut result.alternatives,
            self.config.alternatives,
        );

        let verified = cube.after_algorithm(&result.solution).is_solved();
        if !verified {
            return Err(Error::Solver(
                "engine returned a solution that did not solve the cube".into(),
            ));
        }
        for alternative in &result.alternatives {
            if !cube.after_algorithm(&alternative.algorithm).is_solved() {
                return Err(Error::Solver(
                    "engine returned an invalid alternative".into(),
                ));
            }
        }
        result.input_state = input_state;
        result.verified = true;
        result.stats.elapsed_micros = micros(started.elapsed().as_micros());
        result.stats.move_count = result.solution.len();
        result.stats.exploration_states = result.exploration.len();
        result.decisions = decisions.finish();
        Ok(result)
    }

    pub fn solve_scramble(&self, scramble: &str) -> Result<SolveResult> {
        self.solve(&Cube::from_scramble(scramble)?)
    }

    fn solve_cfop(&self, input: &Cube, decisions: &mut DecisionRecorder) -> Result<SolveResult> {
        let mut cube = *input;
        let mut stages = Vec::with_capacity(8);
        let mut trace = Vec::new();
        let mut exploration = Vec::new();
        let mut complete = Vec::new();
        let mut search_metrics = SearchMetrics::new(decisions);

        let cross_started = Instant::now();
        let (plain_cross_end, plain_cross, cross_repaired) = run_cfop_stage(Stage::Cross, cube)?;
        let required_pairs = self.config.cross_mode.included_pairs();
        let pair_plan = plan_f2l_pairs(
            cube,
            plain_cross_end,
            &plain_cross,
            required_pairs,
            self.config.f2l_strategy,
            self.config.efficiency,
            self.config.max_depth,
            self.config.probe_limit,
            &mut search_metrics,
        )?;
        let advanced_end = pair_plan.cross_end;
        let cross_algorithm = pair_plan.cross_algorithm;
        let cross_optimized = pair_plan.cross_optimized;
        let remaining_pair_steps = pair_plan.remaining_steps;
        let last_layer = pair_plan.last_layer;
        let route_decision_id = pair_plan.decision_id;
        let cross_pairs = f2l_pairs(f2l_pair_mask(&advanced_end));
        let cross_stage = self.config.cross_mode.stage();
        if !stage_is_solved(cross_stage, &advanced_end) {
            return Err(Error::Solver(format!(
                "{} stage did not satisfy its cross-and-pair invariant",
                self.config.cross_mode
            )));
        }
        let cross_message = if cross_pairs.is_empty() {
            format!(
                "selected a {}-move {} branch at {} efficiency",
                cross_algorithm.len(),
                self.config.cross_mode,
                self.config.efficiency
            )
        } else {
            format!(
                "selected a {}-move {} branch with {} solved F2L pair(s): {} at {} efficiency",
                cross_algorithm.len(),
                self.config.cross_mode,
                cross_pairs.len(),
                format_pairs(&cross_pairs),
                self.config.efficiency
            )
        };
        record_phase(
            &self.config,
            &mut stages,
            &mut trace,
            &mut exploration,
            search_metrics.decisions,
            route_decision_id,
            cross_stage,
            cube,
            advanced_end,
            cross_algorithm.clone(),
            cross_pairs,
            micros(cross_started.elapsed().as_micros()),
            cross_repaired && !cross_optimized,
            cross_optimized,
            cross_message,
        )?;
        complete.extend(cross_algorithm.moves().iter().copied());
        cube = advanced_end;

        for step in remaining_pair_steps {
            let solved_after = f2l_pair_mask(&step.end).count_ones();
            let message = format!(
                "selected {} moves for F2L pair {}/4 ({}) using {} at {} efficiency",
                step.algorithm.len(),
                solved_after,
                format_pairs(&step.pairs),
                self.config.f2l_strategy,
                self.config.efficiency
            );
            record_phase(
                &self.config,
                &mut stages,
                &mut trace,
                &mut exploration,
                search_metrics.decisions,
                step.decision_id,
                Stage::F2lPair,
                cube,
                step.end,
                step.algorithm.clone(),
                step.pairs,
                step.elapsed_micros,
                step.repaired,
                step.optimized,
                message,
            )?;
            complete.extend(step.algorithm.moves().iter().copied());
            cube = step.end;
        }

        if !stage_is_solved(Stage::F2l, &cube) {
            return Err(Error::Solver(
                "pair-level F2L stages did not finish the first two layers".into(),
            ));
        }

        let message = decision_message(Stage::Oll, &last_layer.oll);
        record_phase(
            &self.config,
            &mut stages,
            &mut trace,
            &mut exploration,
            search_metrics.decisions,
            route_decision_id,
            Stage::Oll,
            cube,
            last_layer.after_oll,
            last_layer.oll.clone(),
            Vec::new(),
            last_layer.oll_elapsed_micros,
            last_layer.oll_repaired,
            false,
            message,
        )?;
        complete.extend(last_layer.oll.moves().iter().copied());
        cube = last_layer.after_oll;

        let message = decision_message(Stage::Pll, &last_layer.pll);
        record_phase(
            &self.config,
            &mut stages,
            &mut trace,
            &mut exploration,
            search_metrics.decisions,
            route_decision_id,
            Stage::Pll,
            cube,
            last_layer.solved,
            last_layer.pll.clone(),
            Vec::new(),
            last_layer.pll_elapsed_micros,
            last_layer.pll_repaired,
            false,
            message,
        )?;
        complete.extend(last_layer.pll.moves().iter().copied());
        cube = last_layer.solved;

        if !cube.is_solved() {
            return Err(Error::Solver(
                "CFOP stages ended in a non-solved state".into(),
            ));
        }
        Ok(SolveResult {
            input_state: String::new(),
            method: SolveMethod::Cfop,
            cross_mode: self.config.cross_mode,
            f2l_strategy: self.config.f2l_strategy,
            efficiency: self.config.efficiency,
            solution: Algorithm::new(complete).simplified(),
            verified: false,
            stages,
            trace,
            exploration,
            decisions: None,
            alternatives: Vec::new(),
            stats: SolveStats {
                method: SolveMethod::Cfop,
                elapsed_micros: 0,
                move_count: 0,
                exploration_states: 0,
                f2l_candidates_evaluated: search_metrics.f2l_candidates_evaluated,
                exact_target_searches: search_metrics.exact_target_searches,
                last_layer_evaluations: search_metrics.last_layer_evaluations,
            },
        })
    }

    fn solve_fast(&self, input: &Cube, decisions: &mut DecisionRecorder) -> Result<SolveResult> {
        let start = Instant::now();
        let solution = if input.is_solved() {
            Algorithm::default()
        } else {
            fast_primary(input, self.config.max_depth, self.config.probe_limit)?
        };
        let end = input.after_algorithm(&solution);
        let elapsed_micros = micros(start.elapsed().as_micros());
        decisions.record(|index| DecisionRecord {
            index,
            parent_id: None,
            stage: Stage::Fast,
            domain: DecisionDomain::TwoPhase,
            kind: DecisionKind::Select,
            depth: 0,
            outcome: DecisionOutcome::Selected,
            reason: DecisionReason::TwoPhaseSolution,
            message: "selected the two-phase solution found within the configured probe limit"
                .into(),
            f2l_pairs: Vec::new(),
            algorithm: Some(solution.clone()),
            start_state: Some(input.facelets()),
            state: Some(end.facelets()),
            score: Some(DecisionScore {
                path_moves: Some(solution.len()),
                probe_limit: Some(self.config.probe_limit),
                ..DecisionScore::default()
            }),
        });
        let trace = if self.config.trace {
            vec![
                TraceEvent {
                    index: 0,
                    stage: Stage::Fast,
                    kind: TraceKind::Decision,
                    message:
                        "two-phase search selected the shortest result found within the probe limit"
                            .into(),
                    f2l_pairs: Vec::new(),
                    state: Some(input.facelets()),
                    algorithm: Some(solution.clone()),
                },
                TraceEvent {
                    index: 1,
                    stage: Stage::Fast,
                    kind: TraceKind::PhaseComplete,
                    message: format!("completed fast search in {} moves", solution.len()),
                    f2l_pairs: Vec::new(),
                    state: Some(end.facelets()),
                    algorithm: Some(solution.clone()),
                },
            ]
        } else {
            Vec::new()
        };
        Ok(SolveResult {
            input_state: String::new(),
            method: SolveMethod::Fast,
            cross_mode: self.config.cross_mode,
            f2l_strategy: self.config.f2l_strategy,
            efficiency: self.config.efficiency,
            solution: solution.clone(),
            verified: false,
            stages: vec![StageResult {
                stage: Stage::Fast,
                f2l_pairs: Vec::new(),
                start_state: input.facelets(),
                end_state: end.facelets(),
                move_count: solution.len(),
                algorithm: solution,
                elapsed_micros,
                repaired: false,
                optimized: false,
            }],
            trace,
            exploration: Vec::new(),
            decisions: None,
            alternatives: Vec::new(),
            stats: SolveStats {
                method: SolveMethod::Fast,
                elapsed_micros: 0,
                move_count: 0,
                exploration_states: 0,
                f2l_candidates_evaluated: 0,
                exact_target_searches: 0,
                last_layer_evaluations: 0,
            },
        })
    }
}

#[derive(Debug, Clone)]
struct F2lStep {
    end: Cube,
    algorithm: Algorithm,
    pairs: Vec<F2lPair>,
    elapsed_micros: u64,
    repaired: bool,
    optimized: bool,
    decision_id: Option<usize>,
}

#[derive(Debug, Clone)]
struct F2lCandidate {
    end: Cube,
    end_mask: u8,
    algorithm: Algorithm,
    pairs: Vec<F2lPair>,
    estimated_total: usize,
    repaired: bool,
    auf: u8,
}

struct SearchMetrics<'a> {
    f2l_candidates_evaluated: usize,
    exact_target_searches: usize,
    last_layer_evaluations: usize,
    f2l_cache: HashMap<(CubeKey, u8, bool), Option<F2lCandidate>>,
    exact_cache: HashMap<ExactSearchKey, Option<Algorithm>>,
    last_layer_cache: HashMap<CubeKey, LastLayerPlan>,
    decisions: &'a mut DecisionRecorder,
}

impl<'a> SearchMetrics<'a> {
    fn new(decisions: &'a mut DecisionRecorder) -> Self {
        Self {
            f2l_candidates_evaluated: 0,
            exact_target_searches: 0,
            last_layer_evaluations: 0,
            f2l_cache: HashMap::new(),
            exact_cache: HashMap::new(),
            last_layer_cache: HashMap::new(),
            decisions,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CubeKey([u8; 46]);

impl From<Cube> for CubeKey {
    fn from(cube: Cube) -> Self {
        Self(cube.cubie_feature_ids())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExactSearchKey {
    from: CubeKey,
    goal: CubeKey,
    max_depth: usize,
    probe_limit: u64,
}

#[derive(Debug, Clone)]
struct F2lPlanNode {
    cube: Cube,
    steps: Vec<F2lStep>,
    move_count: usize,
    priority: usize,
    last_layer: Option<LastLayerPlan>,
    decision_id: Option<usize>,
}

#[derive(Debug)]
struct F2lPlan {
    steps: Vec<F2lStep>,
    last_layer: LastLayerPlan,
    decision_id: Option<usize>,
}

#[derive(Debug)]
struct F2lRoute {
    cross_end: Cube,
    cross_algorithm: Algorithm,
    cross_optimized: bool,
    remaining_steps: Vec<F2lStep>,
    last_layer: LastLayerPlan,
    move_count: usize,
    decision_id: Option<usize>,
}

#[derive(Debug, Clone)]
struct LastLayerPlan {
    after_oll: Cube,
    oll: Algorithm,
    oll_elapsed_micros: u64,
    oll_repaired: bool,
    solved: Cube,
    pll: Algorithm,
    pll_elapsed_micros: u64,
    pll_repaired: bool,
    combined: Algorithm,
}

struct LastLayerEvaluator {
    oll: OLLSolver,
    pll: PLLSolver<'static>,
}

impl Default for LastLayerEvaluator {
    fn default() -> Self {
        let solved = SOLVED_CUBIE_CUBE;
        Self {
            oll: OLLSolver::new(solved),
            pll: PLLSolver::new(solved),
        }
    }
}

impl LastLayerEvaluator {
    fn evaluate(&mut self, cube: Cube) -> Result<LastLayerPlan> {
        let oll_started = Instant::now();
        let (oll_engine_end, oll) = catch_unwind(AssertUnwindSafe(|| {
            self.oll.cube = cube.inner;
            let moves = self.oll.solve();
            (
                Cube {
                    inner: self.oll.cube,
                },
                Algorithm::from_engines(moves),
            )
        }))
        .map_err(|_| Error::Solver("oll engine panicked".into()))?;
        let (after_oll, oll, oll_repaired) =
            reconcile_stage_transition(Stage::Oll, cube, oll_engine_end, oll)?;
        let oll_elapsed_micros = micros(oll_started.elapsed().as_micros());

        let pll_started = Instant::now();
        let (pll_engine_end, pll) = catch_unwind(AssertUnwindSafe(|| {
            self.pll.cube = after_oll.inner;
            let moves = self.pll.solve();
            (
                Cube {
                    inner: self.pll.cube,
                },
                Algorithm::from_engines(moves),
            )
        }))
        .map_err(|_| Error::Solver("pll engine panicked".into()))?;
        let (solved, pll, pll_repaired) =
            reconcile_stage_transition(Stage::Pll, after_oll, pll_engine_end, pll)?;
        let pll_elapsed_micros = micros(pll_started.elapsed().as_micros());

        let mut moves = oll.moves().to_vec();
        moves.extend(pll.moves().iter().copied());
        Ok(LastLayerPlan {
            after_oll,
            oll,
            oll_elapsed_micros,
            oll_repaired,
            solved,
            pll,
            pll_elapsed_micros,
            pll_repaired,
            combined: Algorithm::new(moves).simplified(),
        })
    }
}

thread_local! {
    static LAST_LAYER_EVALUATOR: RefCell<LastLayerEvaluator> =
        RefCell::new(LastLayerEvaluator::default());
}

impl F2lRoute {
    fn move_count(&self) -> usize {
        self.move_count
    }
}

#[allow(clippy::too_many_arguments)]
fn record_phase(
    config: &SolveConfig,
    stages: &mut Vec<StageResult>,
    trace: &mut Vec<TraceEvent>,
    exploration: &mut Vec<ExplorationEvent>,
    decisions: &mut DecisionRecorder,
    parent_decision_id: Option<usize>,
    stage: Stage,
    start: Cube,
    end: Cube,
    algorithm: Algorithm,
    f2l_pairs: Vec<F2lPair>,
    elapsed_micros: u64,
    repaired: bool,
    optimized: bool,
    mut decision: String,
) -> Result<()> {
    if start.after_algorithm(&algorithm) != end {
        return Err(Error::Solver(format!(
            "{stage}: recorded phase moves do not replay to the recorded end state"
        )));
    }
    if repaired {
        decision.push_str(
            "; independently reconstructed the transition after an engine-state mismatch",
        );
    }
    if optimized {
        decision.push_str("; exact-target search shortened the selected branch");
    }

    let decision_depth = stages.len();
    decisions.record(|index| DecisionRecord {
        index,
        parent_id: parent_decision_id,
        stage,
        domain: DecisionDomain::Phase,
        kind: DecisionKind::Select,
        depth: decision_depth,
        outcome: DecisionOutcome::Selected,
        reason: DecisionReason::PhaseCommitted,
        message: decision.clone(),
        f2l_pairs: f2l_pairs.clone(),
        algorithm: Some(algorithm.clone()),
        start_state: Some(start.facelets()),
        state: Some(end.facelets()),
        score: Some(DecisionScore {
            path_moves: Some(algorithm.len()),
            solved_pairs: matches!(
                stage,
                Stage::XCross | Stage::XXCross | Stage::XXXCross | Stage::F2l | Stage::F2lPair
            )
            .then_some(f2l_pair_mask(&end).count_ones() as u8),
            ..DecisionScore::default()
        }),
    });

    if config.exploration && exploration.len() < config.max_exploration_states {
        append_stage_exploration(
            exploration,
            stage,
            &start,
            algorithm.moves().first().copied(),
            config.max_exploration_states,
        );
    }

    if config.trace {
        let pair_suffix = if f2l_pairs.is_empty() {
            String::new()
        } else {
            format!(" ({})", format_pairs(&f2l_pairs))
        };
        trace.push(TraceEvent {
            index: trace.len(),
            stage,
            kind: TraceKind::PhaseStart,
            message: format!("begin {stage}{pair_suffix} from the current canonical state"),
            f2l_pairs: f2l_pairs.clone(),
            state: Some(start.facelets()),
            algorithm: None,
        });
        trace.push(TraceEvent {
            index: trace.len(),
            stage,
            kind: TraceKind::Decision,
            message: decision,
            f2l_pairs: f2l_pairs.clone(),
            state: None,
            algorithm: Some(algorithm.clone()),
        });
        trace.push(TraceEvent {
            index: trace.len(),
            stage,
            kind: TraceKind::PhaseComplete,
            message: format!(
                "completed {stage}{pair_suffix} in {} moves",
                algorithm.len()
            ),
            f2l_pairs: f2l_pairs.clone(),
            state: Some(end.facelets()),
            algorithm: Some(algorithm.clone()),
        });
    }

    stages.push(StageResult {
        stage,
        f2l_pairs,
        start_state: start.facelets(),
        end_state: end.facelets(),
        move_count: algorithm.len(),
        algorithm,
        elapsed_micros,
        repaired,
        optimized,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn plan_f2l_pairs(
    cross_start: Cube,
    plain_cross_end: Cube,
    plain_cross: &Algorithm,
    required_pairs: usize,
    strategy: F2lStrategy,
    efficiency: Efficiency,
    max_depth: usize,
    probe_limit: u64,
    metrics: &mut SearchMetrics,
) -> Result<F2lRoute> {
    plan_f2l_pairs_until(
        cross_start,
        plain_cross_end,
        plain_cross,
        required_pairs,
        strategy,
        efficiency,
        max_depth,
        probe_limit,
        4,
        metrics,
    )
}

#[allow(clippy::too_many_arguments)]
fn plan_f2l_pairs_until(
    cross_start: Cube,
    cube: Cube,
    plain_cross: &Algorithm,
    required_pairs: usize,
    strategy: F2lStrategy,
    efficiency: Efficiency,
    max_depth: usize,
    probe_limit: u64,
    target_pair_count: usize,
    metrics: &mut SearchMetrics,
) -> Result<F2lRoute> {
    let started = Instant::now();
    let mut variants = vec![(false, 1usize, true)];
    if efficiency != Efficiency::Fast {
        variants.push((false, 1, false));
        if strategy == F2lStrategy::Lookahead {
            variants.push((true, 1, false));
            if matches!(efficiency, Efficiency::Thorough | Efficiency::Max) {
                variants.push((true, 4, false));
            }
            if efficiency == Efficiency::Max {
                variants.push((true, 64, false));
            }
        }
    }

    let mut plans = Vec::with_capacity(variants.len());
    let mut first_error = None;
    for (all_aufs, beam_width, single_pair_only) in variants {
        let variant_id = metrics.decisions.record(|index| DecisionRecord {
            index,
            parent_id: None,
            stage: Stage::F2l,
            domain: DecisionDomain::F2lVariant,
            kind: DecisionKind::SearchStart,
            depth: 0,
            outcome: DecisionOutcome::Considered,
            reason: DecisionReason::VariantStarted,
            message: format!(
                "started F2L variant: all_aufs={all_aufs}, beam_width={beam_width}, single_pair_only={single_pair_only}"
            ),
            f2l_pairs: Vec::new(),
            algorithm: None,
            start_state: None,
            state: Some(cube.facelets()),
            score: Some(DecisionScore {
                solved_pairs: Some(f2l_pair_mask(&cube).count_ones() as u8),
                beam_width: Some(beam_width),
                ..DecisionScore::default()
            }),
        });
        match beam_f2l_plan(
            cube,
            strategy,
            all_aufs,
            single_pair_only,
            beam_width,
            target_pair_count,
            variant_id,
            metrics,
        ) {
            Ok(mut plan) => {
                let completed_id = metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id: plan.decision_id.or(variant_id),
                    stage: Stage::F2l,
                    domain: DecisionDomain::F2lVariant,
                    kind: DecisionKind::SearchEnd,
                    depth: plan.steps.len(),
                    outcome: DecisionOutcome::Accepted,
                    reason: DecisionReason::VariantCompleted,
                    message: "completed an F2L search variant with a solved last layer plan".into(),
                    f2l_pairs: plan
                        .steps
                        .iter()
                        .flat_map(|step| step.pairs.iter().copied())
                        .collect(),
                    algorithm: None,
                    start_state: None,
                    state: plan.steps.last().map(|step| step.end.facelets()),
                    score: Some(DecisionScore {
                        path_moves: Some(plan.steps.iter().map(|step| step.algorithm.len()).sum()),
                        last_layer_moves: Some(plan.last_layer.combined.len()),
                        beam_width: Some(beam_width),
                        ..DecisionScore::default()
                    }),
                });
                plan.decision_id = completed_id.or(plan.decision_id).or(variant_id);
                plans.push(plan);
            }
            Err(error) => {
                metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id: variant_id,
                    stage: Stage::F2l,
                    domain: DecisionDomain::F2lVariant,
                    kind: DecisionKind::SearchEnd,
                    depth: 0,
                    outcome: DecisionOutcome::Failed,
                    reason: DecisionReason::VariantFailed,
                    message: error.to_string(),
                    f2l_pairs: Vec::new(),
                    algorithm: None,
                    start_state: None,
                    state: None,
                    score: Some(DecisionScore {
                        beam_width: Some(beam_width),
                        ..DecisionScore::default()
                    }),
                });
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    let exact_budgets = efficiency.exact_budgets(probe_limit);
    for plan in &mut plans {
        let mut step_start = cube;
        for step in &mut plan.steps {
            let folded_into_cross =
                (f2l_pair_mask(&step_start).count_ones() as usize) < required_pairs;
            if !folded_into_cross && step.algorithm.len() > 1 {
                let original_len = step.algorithm.len();
                if let Some(candidate) = first_exact_candidate(&exact_budgets, |budget| {
                    cached_fast_to(
                        step_start,
                        step.end,
                        Stage::F2lPair,
                        max_depth,
                        budget,
                        step.decision_id,
                        metrics,
                    )
                }) {
                    if candidate.len() < step.algorithm.len()
                        && step_start.after_algorithm(&candidate) == step.end
                    {
                        let selected_id = metrics.decisions.record(|index| DecisionRecord {
                            index,
                            parent_id: step.decision_id,
                            stage: Stage::F2lPair,
                            domain: DecisionDomain::ExactTarget,
                            kind: DecisionKind::Select,
                            depth: f2l_pair_mask(&step.end).count_ones() as usize,
                            outcome: DecisionOutcome::Selected,
                            reason: DecisionReason::ExactShorter,
                            message: format!(
                                "replaced a {original_len}-move F2L transition with an exact {}-move transition",
                                candidate.len()
                            ),
                            f2l_pairs: step.pairs.clone(),
                            algorithm: Some(candidate.clone()),
                            start_state: Some(step_start.facelets()),
                            state: Some(step.end.facelets()),
                            score: Some(DecisionScore {
                                path_moves: Some(candidate.len()),
                                estimated_total: Some(original_len),
                                solved_pairs: Some(f2l_pair_mask(&step.end).count_ones() as u8),
                                ..DecisionScore::default()
                            }),
                        });
                        step.algorithm = candidate;
                        step.optimized = true;
                        step.decision_id = selected_id.or(step.decision_id);
                    } else {
                        metrics.decisions.record(|index| DecisionRecord {
                            index,
                            parent_id: step.decision_id,
                            stage: Stage::F2lPair,
                            domain: DecisionDomain::ExactTarget,
                            kind: DecisionKind::Evaluation,
                            depth: f2l_pair_mask(&step.end).count_ones() as usize,
                            outcome: DecisionOutcome::Rejected,
                            reason: DecisionReason::ExactNotShorter,
                            message: "kept the original F2L transition because exact search did not shorten it".into(),
                            f2l_pairs: step.pairs.clone(),
                            algorithm: Some(candidate),
                            start_state: Some(step_start.facelets()),
                            state: Some(step.end.facelets()),
                            score: Some(DecisionScore {
                                path_moves: Some(original_len),
                                ..DecisionScore::default()
                            }),
                        });
                    }
                }
            }
            step_start = step.end;
        }
    }

    let mut routes = Vec::with_capacity(plans.len());
    for plan in plans {
        routes.push(build_f2l_route(
            cross_start,
            cube,
            plain_cross,
            required_pairs,
            plan,
            max_depth,
            &exact_budgets,
            metrics,
        ));
    }
    let selected_index = routes
        .iter()
        .enumerate()
        .min_by_key(|(_, route)| route.move_count())
        .map(|(index, _)| index)
        .ok_or_else(|| {
            first_error.unwrap_or_else(|| {
                Error::Solver("F2L planner produced no complete pair route".into())
            })
        })?;
    for (index, route) in routes.iter().enumerate() {
        if index == selected_index {
            continue;
        }
        metrics.decisions.record(|record_index| DecisionRecord {
            index: record_index,
            parent_id: route.decision_id,
            stage: Stage::F2l,
            domain: DecisionDomain::F2lVariant,
            kind: DecisionKind::Backtrack,
            depth: route.remaining_steps.len(),
            outcome: DecisionOutcome::Rejected,
            reason: DecisionReason::HigherTotalMoves,
            message: "discarded a completed route with a higher total move count".into(),
            f2l_pairs: Vec::new(),
            algorithm: None,
            start_state: None,
            state: Some(route.cross_end.facelets()),
            score: Some(DecisionScore {
                route_moves: Some(route.move_count),
                ..DecisionScore::default()
            }),
        });
    }
    let mut selected = routes.remove(selected_index);
    let selected_id = metrics.decisions.record(|index| DecisionRecord {
        index,
        parent_id: selected.decision_id,
        stage: Stage::F2l,
        domain: DecisionDomain::F2lVariant,
        kind: DecisionKind::Select,
        depth: selected.remaining_steps.len(),
        outcome: DecisionOutcome::Selected,
        reason: DecisionReason::LowerTotalMoves,
        message: "selected the completed CFOP route with the lowest total move count".into(),
        f2l_pairs: Vec::new(),
        algorithm: None,
        start_state: None,
        state: Some(selected.cross_end.facelets()),
        score: Some(DecisionScore {
            route_moves: Some(selected.move_count),
            ..DecisionScore::default()
        }),
    });
    selected.decision_id = selected_id.or(selected.decision_id);

    let planning_micros = micros(started.elapsed().as_micros());
    let timing_share = planning_micros / selected.remaining_steps.len().max(1) as u64;
    for step in &mut selected.remaining_steps {
        step.elapsed_micros = timing_share;
    }
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
fn build_f2l_route(
    cross_start: Cube,
    plain_cross_end: Cube,
    plain_cross: &Algorithm,
    required_pairs: usize,
    mut plan: F2lPlan,
    max_depth: usize,
    exact_budgets: &[u64],
    metrics: &mut SearchMetrics,
) -> F2lRoute {
    let mut cross_end = plain_cross_end;
    let mut split_index = 0;
    while split_index < plan.steps.len()
        && (f2l_pair_mask(&cross_end).count_ones() as usize) < required_pairs
    {
        cross_end = plan.steps[split_index].end;
        split_index += 1;
    }

    let mut cross_moves = plain_cross.moves().to_vec();
    for step in &plan.steps[..split_index] {
        cross_moves.extend(step.algorithm.moves().iter().copied());
    }
    let mut cross_algorithm = Algorithm::new(cross_moves).simplified();
    let mut cross_optimized = false;
    if required_pairs > 0 {
        let cross_stage = match required_pairs {
            1 => Stage::XCross,
            2 => Stage::XXCross,
            3 => Stage::XXXCross,
            _ => Stage::Cross,
        };
        let original_len = cross_algorithm.len();
        if let Some(candidate) = first_exact_candidate(exact_budgets, |budget| {
            cached_fast_to(
                cross_start,
                cross_end,
                cross_stage,
                max_depth,
                budget,
                plan.decision_id,
                metrics,
            )
        }) {
            if candidate.len() < cross_algorithm.len()
                && cross_start.after_algorithm(&candidate) == cross_end
            {
                let selected_id = metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id: plan.decision_id,
                    stage: cross_stage,
                    domain: DecisionDomain::ExactTarget,
                    kind: DecisionKind::Select,
                    depth: required_pairs,
                    outcome: DecisionOutcome::Selected,
                    reason: DecisionReason::ExactShorter,
                    message: format!(
                        "replaced a {original_len}-move advanced-cross transition with an exact {}-move transition",
                        candidate.len()
                    ),
                    f2l_pairs: f2l_pairs(f2l_pair_mask(&cross_end)),
                    algorithm: Some(candidate.clone()),
                    start_state: Some(cross_start.facelets()),
                    state: Some(cross_end.facelets()),
                    score: Some(DecisionScore {
                        path_moves: Some(candidate.len()),
                        estimated_total: Some(original_len),
                        solved_pairs: Some(f2l_pair_mask(&cross_end).count_ones() as u8),
                        ..DecisionScore::default()
                    }),
                });
                cross_algorithm = candidate;
                cross_optimized = true;
                plan.decision_id = selected_id.or(plan.decision_id);
            } else {
                metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id: plan.decision_id,
                    stage: cross_stage,
                    domain: DecisionDomain::ExactTarget,
                    kind: DecisionKind::Evaluation,
                    depth: required_pairs,
                    outcome: DecisionOutcome::Rejected,
                    reason: DecisionReason::ExactNotShorter,
                    message:
                        "kept the advanced-cross transition because exact search did not shorten it"
                            .into(),
                    f2l_pairs: f2l_pairs(f2l_pair_mask(&cross_end)),
                    algorithm: Some(candidate),
                    start_state: Some(cross_start.facelets()),
                    state: Some(cross_end.facelets()),
                    score: Some(DecisionScore {
                        path_moves: Some(original_len),
                        ..DecisionScore::default()
                    }),
                });
            }
        }
    }

    let remaining_steps = plan.steps.split_off(split_index);
    let mut route_moves = cross_algorithm.moves().to_vec();
    for step in &remaining_steps {
        route_moves.extend(step.algorithm.moves().iter().copied());
    }
    route_moves.extend(plan.last_layer.combined.moves().iter().copied());
    let route_algorithm = Algorithm::new(route_moves).simplified();
    let move_count = route_algorithm.len();
    let decision_id = metrics.decisions.record(|index| DecisionRecord {
        index,
        parent_id: plan.decision_id,
        stage: Stage::F2l,
        domain: DecisionDomain::F2lVariant,
        kind: DecisionKind::Candidate,
        depth: plan.steps.len(),
        outcome: DecisionOutcome::Accepted,
        reason: DecisionReason::VariantCompleted,
        message: "assembled a complete cross, F2L, OLL, and PLL route".into(),
        f2l_pairs: f2l_pairs(f2l_pair_mask(&cross_end)),
        algorithm: Some(route_algorithm),
        start_state: Some(cross_start.facelets()),
        state: Some(plan.last_layer.solved.facelets()),
        score: Some(DecisionScore {
            route_moves: Some(move_count),
            last_layer_moves: Some(plan.last_layer.combined.len()),
            solved_pairs: Some(f2l_pair_mask(&cross_end).count_ones() as u8),
            ..DecisionScore::default()
        }),
    });
    F2lRoute {
        cross_end,
        cross_algorithm,
        cross_optimized,
        remaining_steps,
        last_layer: plan.last_layer,
        move_count,
        decision_id: decision_id.or(plan.decision_id),
    }
}

#[allow(clippy::too_many_arguments)]
fn beam_f2l_plan(
    cube: Cube,
    strategy: F2lStrategy,
    all_aufs: bool,
    single_pair_only: bool,
    beam_width: usize,
    target_pair_count: usize,
    variant_id: Option<usize>,
    metrics: &mut SearchMetrics,
) -> Result<F2lPlan> {
    if target_pair_count != 4 {
        return Err(Error::Solver(
            "F2L beam search currently requires a complete four-pair route".into(),
        ));
    }
    if f2l_pair_mask(&cube).count_ones() as usize >= target_pair_count {
        let (last_layer, last_layer_id) = cached_last_layer_plan(cube, variant_id, metrics)?;
        return Ok(F2lPlan {
            steps: Vec::new(),
            last_layer,
            decision_id: last_layer_id.or(variant_id),
        });
    }

    let mut frontier = vec![F2lPlanNode {
        cube,
        steps: Vec::new(),
        move_count: 0,
        priority: 0,
        last_layer: None,
        decision_id: variant_id,
    }];
    let mut complete = Vec::new();
    let mut first_error = None;

    for _ in 0..4 {
        let mut next = Vec::new();
        for node in frontier {
            let before = f2l_pair_mask(&node.cube);
            match f2l_candidates(
                node.cube,
                strategy,
                all_aufs,
                single_pair_only,
                node.decision_id,
                metrics,
            ) {
                Ok(candidates) => {
                    for candidate in candidates {
                        let after = candidate.end_mask;
                        if after & before != before || after.count_ones() <= before.count_ones() {
                            metrics.decisions.record(|index| DecisionRecord {
                                index,
                                parent_id: node.decision_id,
                                stage: Stage::F2lPair,
                                domain: DecisionDomain::F2lBeam,
                                kind: DecisionKind::Prune,
                                depth: node.steps.len() + 1,
                                outcome: DecisionOutcome::Rejected,
                                reason: DecisionReason::PairRegression,
                                message: "rejected an F2L candidate that failed to preserve cumulative pair progress".into(),
                                f2l_pairs: candidate.pairs.clone(),
                                algorithm: Some(candidate.algorithm.clone()),
                                start_state: Some(node.cube.facelets()),
                                state: Some(candidate.end.facelets()),
                                score: Some(DecisionScore {
                                    path_moves: Some(node.move_count + candidate.algorithm.len()),
                                    estimated_total: Some(candidate.estimated_total),
                                    solved_pairs: Some(after.count_ones() as u8),
                                    beam_width: Some(beam_width),
                                    auf: Some(candidate.auf),
                                    ..DecisionScore::default()
                                }),
                            });
                            continue;
                        }
                        let candidate_id = metrics.decisions.record(|index| DecisionRecord {
                            index,
                            parent_id: node.decision_id,
                            stage: Stage::F2lPair,
                            domain: DecisionDomain::F2lBeam,
                            kind: DecisionKind::Candidate,
                            depth: node.steps.len() + 1,
                            outcome: DecisionOutcome::Accepted,
                            reason: DecisionReason::PairProgress,
                            message: format!(
                                "accepted an F2L candidate that increased the solved-pair count to {}",
                                after.count_ones()
                            ),
                            f2l_pairs: candidate.pairs.clone(),
                            algorithm: Some(candidate.algorithm.clone()),
                            start_state: Some(node.cube.facelets()),
                            state: Some(candidate.end.facelets()),
                            score: Some(DecisionScore {
                                path_moves: Some(node.move_count + candidate.algorithm.len()),
                                estimated_total: Some(candidate.estimated_total),
                                solved_pairs: Some(after.count_ones() as u8),
                                beam_width: Some(beam_width),
                                auf: Some(candidate.auf),
                                ..DecisionScore::default()
                            }),
                        });
                        let step = F2lStep {
                            end: candidate.end,
                            algorithm: candidate.algorithm,
                            pairs: candidate.pairs,
                            elapsed_micros: 0,
                            repaired: candidate.repaired,
                            optimized: false,
                            decision_id: candidate_id.or(node.decision_id),
                        };
                        let move_count = node.move_count + step.algorithm.len();
                        let mut steps = node.steps.clone();
                        steps.push(step);
                        let expanded = F2lPlanNode {
                            cube: candidate.end,
                            steps,
                            move_count,
                            priority: node.move_count + candidate.estimated_total,
                            last_layer: None,
                            decision_id: candidate_id.or(node.decision_id),
                        };
                        if after.count_ones() as usize >= target_pair_count {
                            complete.push(expanded);
                        } else {
                            next.push(expanded);
                        }
                    }
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by_key(|node| (node.priority, node.move_count));
        let mut states = HashSet::new();
        let mut unique = Vec::with_capacity(next.len());
        for node in next {
            if states.insert(CubeKey::from(node.cube)) {
                unique.push(node);
            } else {
                metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id: node.decision_id,
                    stage: Stage::F2lPair,
                    domain: DecisionDomain::F2lBeam,
                    kind: DecisionKind::Prune,
                    depth: node.steps.len(),
                    outcome: DecisionOutcome::Pruned,
                    reason: DecisionReason::DuplicateState,
                    message: "pruned a beam node whose cube state was already represented".into(),
                    f2l_pairs: node
                        .steps
                        .last()
                        .map_or_else(Vec::new, |step| step.pairs.clone()),
                    algorithm: node.steps.last().map(|step| step.algorithm.clone()),
                    start_state: Some(
                        node.steps
                            .len()
                            .checked_sub(2)
                            .and_then(|index| node.steps.get(index))
                            .map_or(cube, |step| step.end)
                            .facelets(),
                    ),
                    state: Some(node.cube.facelets()),
                    score: Some(DecisionScore {
                        path_moves: Some(node.move_count),
                        estimated_total: Some(node.priority),
                        solved_pairs: Some(f2l_pair_mask(&node.cube).count_ones() as u8),
                        beam_width: Some(beam_width),
                        ..DecisionScore::default()
                    }),
                });
            }
        }
        if unique.len() > beam_width {
            for node in &unique[beam_width..] {
                metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id: node.decision_id,
                    stage: Stage::F2lPair,
                    domain: DecisionDomain::F2lBeam,
                    kind: DecisionKind::Prune,
                    depth: node.steps.len(),
                    outcome: DecisionOutcome::Pruned,
                    reason: DecisionReason::BeamWidth,
                    message: "pruned a valid F2L node outside the configured beam width".into(),
                    f2l_pairs: node
                        .steps
                        .last()
                        .map_or_else(Vec::new, |step| step.pairs.clone()),
                    algorithm: node.steps.last().map(|step| step.algorithm.clone()),
                    start_state: Some(
                        node.steps
                            .len()
                            .checked_sub(2)
                            .and_then(|index| node.steps.get(index))
                            .map_or(cube, |step| step.end)
                            .facelets(),
                    ),
                    state: Some(node.cube.facelets()),
                    score: Some(DecisionScore {
                        path_moves: Some(node.move_count),
                        estimated_total: Some(node.priority),
                        solved_pairs: Some(f2l_pair_mask(&node.cube).count_ones() as u8),
                        beam_width: Some(beam_width),
                        ..DecisionScore::default()
                    }),
                });
            }
            unique.truncate(beam_width);
        }
        frontier = unique;
    }

    complete.sort_by_key(|node| node.move_count);
    let mut terminal_states = HashSet::new();
    let mut unique_complete = Vec::with_capacity(complete.len());
    for node in complete {
        if terminal_states.insert(CubeKey::from(node.cube)) {
            unique_complete.push(node);
        } else {
            metrics.decisions.record(|index| DecisionRecord {
                index,
                parent_id: node.decision_id,
                stage: Stage::F2l,
                domain: DecisionDomain::F2lBeam,
                kind: DecisionKind::Prune,
                depth: node.steps.len(),
                outcome: DecisionOutcome::Pruned,
                reason: DecisionReason::DuplicateState,
                message: "pruned a duplicate terminal F2L state".into(),
                f2l_pairs: Vec::new(),
                algorithm: None,
                start_state: None,
                state: Some(node.cube.facelets()),
                score: Some(DecisionScore {
                    path_moves: Some(node.move_count),
                    solved_pairs: Some(f2l_pair_mask(&node.cube).count_ones() as u8),
                    ..DecisionScore::default()
                }),
            });
        }
    }
    let mut complete = Vec::with_capacity(unique_complete.len());
    for mut node in unique_complete {
        match cached_last_layer_plan(node.cube, node.decision_id, metrics) {
            Ok((plan, decision_id)) => {
                node.last_layer = Some(plan);
                node.decision_id = decision_id.or(node.decision_id);
                complete.push(node);
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    let selected_index = complete
        .iter()
        .enumerate()
        .min_by_key(|(_, node)| {
            (
                node.move_count
                    + node
                        .last_layer
                        .as_ref()
                        .map_or(0, |plan| plan.combined.len()),
                node.move_count,
            )
        })
        .map(|(index, _)| index)
        .ok_or_else(|| {
            first_error.unwrap_or_else(|| {
                Error::Solver("F2L beam search did not reach the requested pair count".into())
            })
        })?;
    for (index, node) in complete.iter().enumerate() {
        if index == selected_index {
            continue;
        }
        let last_layer_moves = node
            .last_layer
            .as_ref()
            .map_or(0, |plan| plan.combined.len());
        metrics.decisions.record(|record_index| DecisionRecord {
            index: record_index,
            parent_id: node.decision_id,
            stage: Stage::F2l,
            domain: DecisionDomain::F2lBeam,
            kind: DecisionKind::Backtrack,
            depth: node.steps.len(),
            outcome: DecisionOutcome::Rejected,
            reason: DecisionReason::HigherTotalMoves,
            message: "backtracked from a complete F2L branch with a higher projected total".into(),
            f2l_pairs: Vec::new(),
            algorithm: None,
            start_state: None,
            state: Some(node.cube.facelets()),
            score: Some(DecisionScore {
                path_moves: Some(node.move_count),
                last_layer_moves: Some(last_layer_moves),
                estimated_total: Some(node.move_count + last_layer_moves),
                solved_pairs: Some(f2l_pair_mask(&node.cube).count_ones() as u8),
                ..DecisionScore::default()
            }),
        });
    }
    let node = complete.remove(selected_index);
    let last_layer = node
        .last_layer
        .expect("complete F2L nodes have a last-layer plan");
    let selection_id = metrics.decisions.record(|index| DecisionRecord {
        index,
        parent_id: node.decision_id,
        stage: Stage::F2l,
        domain: DecisionDomain::F2lBeam,
        kind: DecisionKind::Select,
        depth: node.steps.len(),
        outcome: DecisionOutcome::Selected,
        reason: DecisionReason::LowerTotalMoves,
        message: "selected the complete F2L branch with the lowest F2L plus last-layer total"
            .into(),
        f2l_pairs: Vec::new(),
        algorithm: None,
        start_state: None,
        state: Some(node.cube.facelets()),
        score: Some(DecisionScore {
            path_moves: Some(node.move_count),
            last_layer_moves: Some(last_layer.combined.len()),
            estimated_total: Some(node.move_count + last_layer.combined.len()),
            solved_pairs: Some(f2l_pair_mask(&node.cube).count_ones() as u8),
            ..DecisionScore::default()
        }),
    });
    Ok(F2lPlan {
        steps: node.steps,
        last_layer,
        decision_id: selection_id.or(node.decision_id),
    })
}

fn cached_last_layer_plan(
    cube: Cube,
    parent_id: Option<usize>,
    metrics: &mut SearchMetrics,
) -> Result<(LastLayerPlan, Option<usize>)> {
    let key = CubeKey::from(cube);
    if let Some(plan) = metrics.last_layer_cache.get(&key).cloned() {
        let decision_id = metrics.decisions.record(|index| DecisionRecord {
            index,
            parent_id,
            stage: Stage::LastLayer,
            domain: DecisionDomain::LastLayer,
            kind: DecisionKind::CacheHit,
            depth: 0,
            outcome: DecisionOutcome::Cached,
            reason: DecisionReason::CacheHit,
            message: "reused a cached OLL and PLL evaluation for this terminal F2L state".into(),
            f2l_pairs: Vec::new(),
            algorithm: Some(plan.combined.clone()),
            start_state: Some(cube.facelets()),
            state: Some(plan.solved.facelets()),
            score: Some(DecisionScore {
                last_layer_moves: Some(plan.combined.len()),
                ..DecisionScore::default()
            }),
        });
        return Ok((plan, decision_id.or(parent_id)));
    }
    metrics.last_layer_evaluations += 1;
    let plan = match LAST_LAYER_EVALUATOR.with(|evaluator| evaluator.borrow_mut().evaluate(cube)) {
        Ok(plan) => plan,
        Err(error) => {
            metrics.decisions.record(|index| DecisionRecord {
                index,
                parent_id,
                stage: Stage::LastLayer,
                domain: DecisionDomain::LastLayer,
                kind: DecisionKind::Evaluation,
                depth: 0,
                outcome: DecisionOutcome::Failed,
                reason: DecisionReason::VariantFailed,
                message: error.to_string(),
                f2l_pairs: Vec::new(),
                algorithm: None,
                start_state: None,
                state: Some(cube.facelets()),
                score: None,
            });
            return Err(error);
        }
    };
    let decision_id = metrics.decisions.record(|index| DecisionRecord {
        index,
        parent_id,
        stage: Stage::LastLayer,
        domain: DecisionDomain::LastLayer,
        kind: DecisionKind::Evaluation,
        depth: 0,
        outcome: DecisionOutcome::Accepted,
        reason: DecisionReason::LastLayerSolved,
        message: "evaluated OLL and PLL for a complete F2L candidate".into(),
        f2l_pairs: Vec::new(),
        algorithm: Some(plan.combined.clone()),
        start_state: Some(cube.facelets()),
        state: Some(plan.solved.facelets()),
        score: Some(DecisionScore {
            last_layer_moves: Some(plan.combined.len()),
            ..DecisionScore::default()
        }),
    });
    metrics.last_layer_cache.insert(key, plan.clone());
    Ok((plan, decision_id.or(parent_id)))
}

fn f2l_candidates(
    cube: Cube,
    strategy: F2lStrategy,
    all_aufs: bool,
    single_pair_only: bool,
    parent_id: Option<usize>,
    metrics: &mut SearchMetrics,
) -> Result<Vec<F2lCandidate>> {
    let auf_count = if strategy == F2lStrategy::Lookahead && all_aufs {
        4
    } else {
        1
    };
    let mut candidates = Vec::with_capacity(auf_count);
    let mut first_error = None;
    for auf in 0..auf_count {
        match cached_f2l_candidate(cube, auf as u8, !single_pair_only, parent_id, metrics) {
            Ok(Some(candidate)) => candidates.push(candidate),
            Ok(None) => {
                metrics.decisions.record(|index| DecisionRecord {
                    index,
                    parent_id,
                    stage: Stage::F2lPair,
                    domain: DecisionDomain::F2lBeam,
                    kind: DecisionKind::Prune,
                    depth: f2l_pair_mask(&cube).count_ones() as usize + 1,
                    outcome: DecisionOutcome::Rejected,
                    reason: DecisionReason::NoPairProgress,
                    message: format!("AUF {auf} produced no cumulative F2L pair transition"),
                    f2l_pairs: Vec::new(),
                    algorithm: None,
                    start_state: None,
                    state: None,
                    score: Some(DecisionScore {
                        solved_pairs: Some(f2l_pair_mask(&cube).count_ones() as u8),
                        auf: Some(auf as u8),
                        ..DecisionScore::default()
                    }),
                });
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    candidates.sort_by_key(|candidate| (candidate.estimated_total, candidate.algorithm.len()));
    let mut states = HashSet::new();
    candidates.retain(|candidate| {
        let unique = states.insert(CubeKey::from(candidate.end));
        if !unique {
            metrics.decisions.record(|index| DecisionRecord {
                index,
                parent_id,
                stage: Stage::F2lPair,
                domain: DecisionDomain::F2lBeam,
                kind: DecisionKind::Prune,
                depth: f2l_pair_mask(&candidate.end).count_ones() as usize,
                outcome: DecisionOutcome::Pruned,
                reason: DecisionReason::DuplicateState,
                message: "pruned an AUF candidate that reached a duplicate cube state".into(),
                f2l_pairs: candidate.pairs.clone(),
                algorithm: Some(candidate.algorithm.clone()),
                start_state: Some(cube.facelets()),
                state: Some(candidate.end.facelets()),
                score: Some(DecisionScore {
                    estimated_total: Some(candidate.estimated_total),
                    solved_pairs: Some(candidate.end_mask.count_ones() as u8),
                    auf: Some(candidate.auf),
                    ..DecisionScore::default()
                }),
            });
        }
        unique
    });
    if candidates.is_empty() {
        Err(first_error.unwrap_or_else(|| {
            Error::Solver("F2L engine produced no cumulative pair transition".into())
        }))
    } else {
        Ok(candidates)
    }
}

fn cached_f2l_candidate(
    cube: Cube,
    auf: u8,
    full_estimate: bool,
    parent_id: Option<usize>,
    metrics: &mut SearchMetrics,
) -> Result<Option<F2lCandidate>> {
    let key = (CubeKey::from(cube), auf, full_estimate);
    if let Some(candidate) = metrics.f2l_cache.get(&key).cloned() {
        metrics.decisions.record(|index| DecisionRecord {
            index,
            parent_id,
            stage: Stage::F2lPair,
            domain: DecisionDomain::F2lBeam,
            kind: DecisionKind::CacheHit,
            depth: f2l_pair_mask(&cube).count_ones() as usize + 1,
            outcome: DecisionOutcome::Cached,
            reason: DecisionReason::CacheHit,
            message: format!("reused the cached F2L candidate for AUF {auf}"),
            f2l_pairs: candidate
                .as_ref()
                .map_or_else(Vec::new, |candidate| candidate.pairs.clone()),
            algorithm: candidate
                .as_ref()
                .map(|candidate| candidate.algorithm.clone()),
            start_state: candidate.as_ref().map(|_| cube.facelets()),
            state: candidate.as_ref().map(|candidate| candidate.end.facelets()),
            score: Some(DecisionScore {
                estimated_total: candidate
                    .as_ref()
                    .map(|candidate| candidate.estimated_total),
                solved_pairs: candidate
                    .as_ref()
                    .map(|candidate| candidate.end_mask.count_ones() as u8),
                auf: Some(auf),
                ..DecisionScore::default()
            }),
        });
        return Ok(candidate);
    }
    metrics.f2l_candidates_evaluated += 1;
    let candidate = f2l_candidate(cube, auf, full_estimate)?;
    metrics.f2l_cache.insert(key, candidate.clone());
    Ok(candidate)
}

fn f2l_candidate(cube: Cube, auf: u8, full_estimate: bool) -> Result<Option<F2lCandidate>> {
    let prefix_move = match auf {
        0 => None,
        1 => Move::from_engine(EngineMove::U),
        2 => Move::from_engine(EngineMove::U2),
        3 => Move::from_engine(EngineMove::U3),
        _ => return Err(Error::Solver(format!("invalid F2L AUF index {auf}"))),
    };
    let adjusted = prefix_move.map_or(cube, |cube_move| cube.after_move(cube_move));
    let (remainder, repaired) = if full_estimate {
        let (_, algorithm, repaired) = run_cfop_stage(Stage::F2l, adjusted)?;
        (algorithm, repaired)
    } else {
        next_f2l_pair_algorithm(adjusted)?
    };
    let mut all_moves = prefix_move.into_iter().collect::<Vec<_>>();
    all_moves.extend(remainder.moves().iter().copied());
    let full = Algorithm::new(all_moves).simplified();
    let estimated_total = full.len();
    let before = f2l_pair_mask(&cube);
    let mut replay = cube;
    let mut prefix_moves = Vec::new();
    let mut fallback = None;

    for &cube_move in full.moves() {
        prefix_moves.push(cube_move);
        replay.apply_move(cube_move);
        let after = f2l_pair_mask(&replay);
        let retained = after & before == before;
        let gained = (after & !before).count_ones();
        if retained && gained > 0 && stage_is_solved(Stage::Cross, &replay) {
            let algorithm = Algorithm::new(prefix_moves.clone()).simplified();
            let candidate = F2lCandidate {
                end: replay,
                end_mask: after,
                algorithm,
                pairs: f2l_pairs(after & !before),
                estimated_total,
                repaired,
                auf,
            };
            if gained == 1 {
                return Ok(Some(candidate));
            }
            if fallback.is_none() {
                fallback = Some(candidate);
            }
        }
    }
    Ok(fallback)
}

fn f2l_pair_mask(cube: &Cube) -> u8 {
    crate::features::solved_f2l_pair_mask(cube)
}

fn f2l_pairs(mask: u8) -> Vec<F2lPair> {
    crate::features::f2l_pairs(mask)
}

fn format_pairs(pairs: &[F2lPair]) -> String {
    pairs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn fill_identity_alternatives(
    cube: &Cube,
    primary: &Algorithm,
    alternatives: &mut Vec<AlternativeSolution>,
    requested: usize,
) {
    const FACES: [&str; 6] = ["U", "R", "F", "D", "L", "B"];
    let mut variant_index = 0usize;
    while alternatives.len() < requested {
        let face = FACES[variant_index % FACES.len()];
        let repetitions = variant_index / FACES.len() + 1;
        let detour: Algorithm = format!("{face} {face}'").parse().expect("static detour");
        let mut moves = Vec::with_capacity(detour.len() * repetitions + primary.len());
        for _ in 0..repetitions {
            moves.extend(detour.moves().iter().copied());
        }
        moves.extend(primary.moves().iter().copied());
        let algorithm = Algorithm::new(moves);
        let notation = algorithm.to_string();
        let unique = notation != primary.to_string()
            && alternatives
                .iter()
                .all(|existing| existing.algorithm.to_string() != notation);
        if unique {
            let verified = cube.after_algorithm(&algorithm).is_solved();
            if verified {
                alternatives.push(AlternativeSolution {
                    source: "verified_identity_variant".into(),
                    move_count: algorithm.len(),
                    algorithm,
                    verified,
                });
            }
        }
        variant_index += 1;
    }
}

fn run_cfop_stage(stage: Stage, cube: Cube) -> Result<(Cube, Algorithm, bool)> {
    let outcome = catch_unwind(AssertUnwindSafe(|| match stage {
        Stage::Setup
        | Stage::XCross
        | Stage::XXCross
        | Stage::XXXCross
        | Stage::F2lPair
        | Stage::LastLayer
        | Stage::Fast => unreachable!(),
        Stage::Cross => {
            let mut solver = CrossSolver { cube: cube.inner };
            let moves = solver.solve();
            if !solver.is_solved() {
                return Err("cross search did not reach its goal".to_owned());
            }
            Ok((Cube { inner: solver.cube }, Algorithm::from_engines(moves)))
        }
        Stage::F2l => {
            let mut solver = F2LSolver { cube: cube.inner };
            let moves = solver.solve();
            if !solver.is_solved() {
                return Err("F2L search did not reach its goal".to_owned());
            }
            Ok((Cube { inner: solver.cube }, Algorithm::from_engines(moves)))
        }
        Stage::Oll => {
            let mut solver = OLLSolver::new(cube.inner);
            let moves = solver.solve();
            if !solver.is_solved() {
                return Err("OLL recognition did not reach its goal".to_owned());
            }
            Ok((Cube { inner: solver.cube }, Algorithm::from_engines(moves)))
        }
        Stage::Pll => {
            let mut solver = PLLSolver::new(cube.inner);
            let moves = solver.solve();
            if !solver.is_solved() {
                return Err("PLL recognition did not solve the cube".to_owned());
            }
            Ok((Cube { inner: solver.cube }, Algorithm::from_engines(moves)))
        }
    }));
    match outcome {
        Ok(Ok((engine_state, algorithm))) => {
            reconcile_stage_transition(stage, cube, engine_state, algorithm)
        }
        Ok(Err(message)) => Err(Error::Solver(format!("{stage}: {message}"))),
        Err(_) => Err(Error::Solver(format!("{stage} engine panicked"))),
    }
}

fn next_f2l_pair_algorithm(cube: Cube) -> Result<(Algorithm, bool)> {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut solver = F2LSolver { cube: cube.inner };
        let moves = solver.solve_to_next_pair_boundary();
        (Cube { inner: solver.cube }, Algorithm::from_engines(moves))
    }))
    .map_err(|_| Error::Solver("F2L pair engine panicked".into()))?;
    let (engine_end, algorithm) = outcome;
    let before = f2l_pair_mask(&cube);
    let made_progress = |candidate: &Cube| {
        let after = f2l_pair_mask(candidate);
        stage_is_solved(Stage::Cross, candidate)
            && after & before == before
            && after.count_ones() > before.count_ones()
    };
    let replayed = cube.after_algorithm(&algorithm);
    if made_progress(&replayed) {
        return Ok((algorithm, false));
    }
    if made_progress(&engine_end) {
        let repaired = fast_to(&cube, &engine_end, 30, 1_000_000)?;
        if cube.after_algorithm(&repaired) == engine_end {
            return Ok((repaired, true));
        }
    }
    Err(Error::Solver(
        "F2L pair engine did not make cumulative progress".into(),
    ))
}

fn reconcile_stage_transition(
    stage: Stage,
    start: Cube,
    engine_end: Cube,
    algorithm: Algorithm,
) -> Result<(Cube, Algorithm, bool)> {
    let replayed = start.after_algorithm(&algorithm);
    if stage_is_solved(stage, &replayed) {
        // Some upstream stage routines probe by mutating their internal cube.
        // Only the independently replayed move sequence is authoritative.
        return Ok((replayed, algorithm, false));
    }
    if stage_is_solved(stage, &engine_end) {
        let repaired = fast_to(&start, &engine_end, 30, 1_000_000)?;
        let replayed = start.after_algorithm(&repaired);
        if replayed == engine_end && stage_is_solved(stage, &replayed) {
            return Ok((replayed, repaired, true));
        }
    }
    Err(Error::Solver(format!(
        "{stage}: replaying the returned moves did not satisfy the phase invariant"
    )))
}

fn stage_is_solved(stage: Stage, cube: &Cube) -> bool {
    match stage {
        Stage::Setup => cube.has_canonical_orientation(),
        Stage::Cross => CrossSolver { cube: cube.inner }.is_solved(),
        Stage::XCross => {
            CrossSolver { cube: cube.inner }.is_solved() && f2l_pair_mask(cube).count_ones() >= 1
        }
        Stage::XXCross => {
            CrossSolver { cube: cube.inner }.is_solved() && f2l_pair_mask(cube).count_ones() >= 2
        }
        Stage::XXXCross => {
            CrossSolver { cube: cube.inner }.is_solved() && f2l_pair_mask(cube).count_ones() >= 3
        }
        Stage::F2l => {
            let solved = SOLVED_CUBIE_CUBE;
            cube.inner.center == solved.center
                && cube.inner.cp[4..8] == solved.cp[4..8]
                && cube.inner.co[4..8] == solved.co[4..8]
                && cube.inner.ep[4..12] == solved.ep[4..12]
                && cube.inner.eo[4..12] == solved.eo[4..12]
        }
        Stage::F2lPair => CrossSolver { cube: cube.inner }.is_solved() && f2l_pair_mask(cube) != 0,
        Stage::Oll => {
            let solved = SOLVED_CUBIE_CUBE;
            (0..4).all(|index| {
                cube.inner.cp[0..4].contains(&solved.cp[index])
                    && cube.inner.co[index] == 0
                    && cube.inner.ep[0..4].contains(&solved.ep[index])
                    && cube.inner.eo[index] == 0
            })
        }
        Stage::Pll | Stage::LastLayer => cube.is_canonically_solved(),
        Stage::Fast => cube.is_solved(),
    }
}

fn fast_primary(cube: &Cube, max_depth: usize, probe_limit: u64) -> Result<Algorithm> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut solver = TwoPhaseSolver::default();
        solver.solve(&cube.facelets(), max_depth, probe_limit, 0, 0)
    }));
    match result {
        Ok(Ok(formula)) => Ok(Algorithm::from_engines(formula.moves).simplified()),
        Ok(Err(error)) => Err(Error::Solver(format!("two-phase search: {error}"))),
        Err(_) => Err(Error::Solver("two-phase engine panicked".into())),
    }
}

fn fast_to(from: &Cube, goal: &Cube, max_depth: usize, probe_limit: u64) -> Result<Algorithm> {
    if from == goal {
        return Ok(Algorithm::default());
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut solver = TwoPhaseSolver::default();
        solver.solve_to(
            &from.facelets(),
            &goal.facelets(),
            max_depth,
            probe_limit,
            0,
            0,
        )
    }));
    match result {
        Ok(Ok(formula)) => Ok(Algorithm::from_engines(formula.moves).simplified()),
        Ok(Err(error)) => Err(Error::Solver(format!(
            "could not reconstruct stage transition: {error}"
        ))),
        Err(_) => Err(Error::Solver(
            "stage transition reconstruction engine panicked".into(),
        )),
    }
}

fn cached_fast_to(
    from: Cube,
    goal: Cube,
    stage: Stage,
    max_depth: usize,
    probe_limit: u64,
    parent_id: Option<usize>,
    metrics: &mut SearchMetrics,
) -> Option<Algorithm> {
    let key = ExactSearchKey {
        from: CubeKey::from(from),
        goal: CubeKey::from(goal),
        max_depth,
        probe_limit,
    };
    if let Some(candidate) = metrics.exact_cache.get(&key).cloned() {
        metrics.decisions.record(|index| DecisionRecord {
            index,
            parent_id,
            stage,
            domain: DecisionDomain::ExactTarget,
            kind: DecisionKind::CacheHit,
            depth: f2l_pair_mask(&goal).count_ones() as usize,
            outcome: DecisionOutcome::Cached,
            reason: DecisionReason::CacheHit,
            message: "reused the result of an exact-target search".into(),
            f2l_pairs: f2l_pairs(f2l_pair_mask(&goal) & !f2l_pair_mask(&from)),
            algorithm: candidate.clone(),
            start_state: candidate.as_ref().map(|_| from.facelets()),
            state: Some(goal.facelets()),
            score: Some(DecisionScore {
                path_moves: candidate.as_ref().map(Algorithm::len),
                solved_pairs: Some(f2l_pair_mask(&goal).count_ones() as u8),
                probe_limit: Some(probe_limit),
                ..DecisionScore::default()
            }),
        });
        return candidate;
    }
    metrics.exact_target_searches += 1;
    let result = fast_to(&from, &goal, max_depth, probe_limit);
    let candidate = result.ok();
    metrics.decisions.record(|index| DecisionRecord {
        index,
        parent_id,
        stage,
        domain: DecisionDomain::ExactTarget,
        kind: DecisionKind::Evaluation,
        depth: f2l_pair_mask(&goal).count_ones() as usize,
        outcome: if candidate.is_some() {
            DecisionOutcome::Accepted
        } else {
            DecisionOutcome::Failed
        },
        reason: if candidate.is_some() {
            DecisionReason::ExactFound
        } else {
            DecisionReason::ExactUnavailable
        },
        message: if candidate.is_some() {
            "exact-target search found a replayable transition".into()
        } else {
            "exact-target search found no transition within this probe budget".into()
        },
        f2l_pairs: f2l_pairs(f2l_pair_mask(&goal) & !f2l_pair_mask(&from)),
        algorithm: candidate.clone(),
        start_state: candidate.as_ref().map(|_| from.facelets()),
        state: Some(goal.facelets()),
        score: Some(DecisionScore {
            path_moves: candidate.as_ref().map(Algorithm::len),
            solved_pairs: Some(f2l_pair_mask(&goal).count_ones() as u8),
            probe_limit: Some(probe_limit),
            ..DecisionScore::default()
        }),
    });
    metrics.exact_cache.insert(key, candidate.clone());
    candidate
}

fn first_exact_candidate(
    budgets: &[u64],
    mut search: impl FnMut(u64) -> Option<Algorithm>,
) -> Option<Algorithm> {
    // min2phase returns the first deterministic solution when probe_min is zero.
    // A larger ceiling can only help after ProbeLimitExceeded, so stop after the
    // first success instead of restarting the same successful search.
    budgets.iter().find_map(|&budget| search(budget))
}

fn fast_alternatives(
    cube: &Cube,
    max_depth: usize,
    probe_limit: u64,
    requested: usize,
    exclude: Option<&Algorithm>,
) -> Result<Vec<Algorithm>> {
    if cube.is_solved() || requested == 0 {
        return Ok(Vec::new());
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut solver = TwoPhaseSolver::default();
        let first = solver
            .solve(&cube.facelets(), max_depth, probe_limit, 0, 0)
            .map(|formula| Algorithm::from_engines(formula.moves).simplified());
        let mut candidates = Vec::new();
        if let Ok(first) = first {
            candidates.push(first);
            for _ in 0..requested.saturating_mul(4).max(requested) {
                match solver.next(probe_limit.min(250_000), 0, 0) {
                    Ok(formula) => {
                        candidates.push(Algorithm::from_engines(formula.moves).simplified())
                    }
                    Err(_) => break,
                }
                if candidates.len() >= requested + 2 {
                    break;
                }
            }
        }
        candidates
    }));
    let candidates =
        result.map_err(|_| Error::Solver("alternative search engine panicked".into()))?;
    let excluded = exclude.map(ToString::to_string);
    let mut seen = HashSet::new();
    let mut alternatives = Vec::new();
    for candidate in candidates {
        let notation = candidate.to_string();
        if excluded.as_deref() == Some(notation.as_str()) || !seen.insert(notation) {
            continue;
        }
        alternatives.push(candidate);
        if alternatives.len() == requested {
            break;
        }
    }
    Ok(alternatives)
}

fn append_stage_exploration(
    target: &mut Vec<ExplorationEvent>,
    stage: Stage,
    cube: &Cube,
    selected: Option<Move>,
    limit: usize,
) {
    const OUTER_MOVES: [&str; 18] = [
        "U", "U2", "U'", "R", "R2", "R'", "F", "F2", "F'", "D", "D2", "D'", "L", "L2", "L'", "B",
        "B2", "B'",
    ];
    for notation in OUTER_MOVES {
        if target.len() >= limit {
            break;
        }
        let cube_move: Move = notation.parse().expect("static move");
        let state = cube.after_move(cube_move);
        let accepted = selected == Some(cube_move);
        target.push(ExplorationEvent {
            index: target.len(),
            parent_id: None,
            stage,
            kind: if accepted {
                ExplorationKind::Selected
            } else {
                ExplorationKind::Backtrack
            },
            depth: 1,
            via_move: cube_move,
            state: state.facelets(),
            score: stage_score(stage, &state),
            accepted,
            reason: if accepted {
                "selected as the first move of the committed stage branch".into()
            } else {
                "one-ply alternative recorded for inspection; the stage chose another branch".into()
            },
        });
    }
}

fn stage_score(stage: Stage, cube: &Cube) -> u16 {
    let solved = SOLVED_CUBIE_CUBE;
    let cross = (4..8)
        .filter(|&index| cube.inner.ep[index] == solved.ep[index] && cube.inner.eo[index] == 0)
        .count();
    let f2l_corners = (4..8)
        .filter(|&index| cube.inner.cp[index] == solved.cp[index] && cube.inner.co[index] == 0)
        .count();
    let f2l_edges = (8..12)
        .filter(|&index| cube.inner.ep[index] == solved.ep[index] && cube.inner.eo[index] == 0)
        .count();
    let oriented = (0..4).filter(|&index| cube.inner.co[index] == 0).count()
        + (0..4).filter(|&index| cube.inner.eo[index] == 0).count();
    let permuted = (0..4)
        .filter(|&index| cube.inner.cp[index] == solved.cp[index])
        .count()
        + (0..4)
            .filter(|&index| cube.inner.ep[index] == solved.ep[index])
            .count();
    match stage {
        Stage::Setup => 0,
        Stage::Cross => cross as u16,
        Stage::XCross | Stage::XXCross | Stage::XXXCross | Stage::F2l | Stage::F2lPair => {
            (cross + f2l_corners + f2l_edges) as u16
        }
        Stage::Oll => (cross + f2l_corners + f2l_edges + oriented) as u16,
        Stage::Pll | Stage::LastLayer | Stage::Fast => {
            (cross + f2l_corners + f2l_edges + oriented + permuted) as u16
        }
    }
}

fn decision_message(stage: Stage, algorithm: &Algorithm) -> String {
    match stage {
        Stage::Setup => format!(
            "normalized the centre frame in {} rotations",
            algorithm.len()
        ),
        Stage::Cross => format!("selected a {}-move bottom-cross branch", algorithm.len()),
        Stage::XCross => format!("selected a {}-move X-cross branch", algorithm.len()),
        Stage::XXCross => format!("selected a {}-move XX-cross branch", algorithm.len()),
        Stage::XXXCross => format!("selected a {}-move XXX-cross branch", algorithm.len()),
        Stage::F2l | Stage::F2lPair => format!(
            "paired and inserted all four F2L slots in {} moves",
            algorithm.len()
        ),
        Stage::Oll => format!(
            "recognized and applied an OLL case in {} moves",
            algorithm.len()
        ),
        Stage::Pll => format!(
            "recognized the PLL permutation and finished in {} moves",
            algorithm.len()
        ),
        Stage::LastLayer => format!(
            "selected a combined last-layer finish in {} moves",
            algorithm.len()
        ),
        Stage::Fast => format!("selected a {}-move two-phase solution", algorithm.len()),
    }
}

fn micros(value: u128) -> u64 {
    value.min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_solver_solves_and_returns_alternative() {
        let solver = Solver::new(SolveConfig {
            method: SolveMethod::Fast,
            alternatives: 1,
            ..SolveConfig::default()
        });
        let cube = Cube::from_scramble("R U F2 L' D").unwrap();
        let result = solver.solve(&cube).unwrap();
        assert!(result.verified);
        assert!(cube.after_algorithm(&result.solution).is_solved());
        assert_eq!(result.alternatives.len(), 1);
        for alternative in result.alternatives {
            assert!(cube.after_algorithm(&alternative.algorithm).is_solved());
        }
    }

    #[test]
    fn cfop_solver_has_phase_boundaries_and_bounded_exploration() {
        let solver = Solver::new(SolveConfig {
            trace: true,
            exploration: true,
            max_exploration_states: 10,
            ..SolveConfig::default()
        });
        let cube = Cube::from_scramble("R U F").unwrap();
        let result = solver.solve(&cube).unwrap();
        assert_eq!(result.stages.len(), 4);
        assert!(result.exploration.len() <= 10);
        assert!(cube.after_algorithm(&result.solution).is_solved());
        for adjacent in result.stages.windows(2) {
            assert_eq!(adjacent[0].end_state, adjacent[1].start_state);
        }
    }

    #[test]
    fn solved_cube_still_fulfils_the_alternative_count() {
        let result = Solver::new(SolveConfig {
            alternatives: 3,
            ..SolveConfig::default()
        })
        .solve(&Cube::solved())
        .unwrap();
        assert!(result.solution.is_empty());
        assert_eq!(result.alternatives.len(), 3);
        assert!(result.alternatives.iter().all(|item| item.verified));
    }

    #[test]
    fn exact_budget_fallback_stops_after_the_first_success() {
        let mut visited = Vec::new();
        let result = first_exact_candidate(&[100, 500, 1_000], |budget| {
            visited.push(budget);
            (budget == 500).then(Algorithm::default)
        });
        assert!(result.is_some());
        assert_eq!(visited, [100, 500]);
    }

    #[test]
    fn decisions_are_default_disabled_and_absent_from_json() {
        let result = Solver::new(SolveConfig::default())
            .solve(&Cube::solved())
            .unwrap();
        assert!(result.decisions.is_none());
        let encoded = serde_json::to_value(result).unwrap();
        assert!(encoded.get("decisions").is_none());
    }

    #[test]
    fn decision_records_are_real_replayable_and_deterministic() {
        let config = SolveConfig {
            decisions: true,
            max_decision_records: 2_048,
            ..SolveConfig::default()
        };
        let cube = Cube::from_scramble("R U F").unwrap();
        let first = Solver::new(config.clone()).solve(&cube).unwrap();
        let second = Solver::new(config).solve(&cube).unwrap();
        let first_trace = first.decisions.as_ref().unwrap();
        let second_trace = second.decisions.as_ref().unwrap();

        assert_eq!(first_trace.schema, DECISION_TRACE_SCHEMA);
        assert!(!first_trace.records.is_empty());
        assert_eq!(
            serde_json::to_value(first_trace).unwrap(),
            serde_json::to_value(second_trace).unwrap()
        );
        for (index, record) in first_trace.records.iter().enumerate() {
            assert_eq!(record.index, index);
            if let Some(parent_id) = record.parent_id {
                assert!(parent_id < record.index);
            }
        }
        assert!(first_trace
            .records
            .iter()
            .any(|record| record.domain == DecisionDomain::LastLayer));
        assert!(first_trace
            .records
            .iter()
            .any(|record| record.domain == DecisionDomain::Phase));

        let candidate = first_trace
            .records
            .iter()
            .find(|record| {
                record.domain == DecisionDomain::F2lBeam
                    && record.kind == DecisionKind::Candidate
                    && record.parent_id.is_some()
                    && record.algorithm.is_some()
                    && record.start_state.is_some()
                    && record.state.is_some()
            })
            .expect("F2L search should emit a real candidate transition");
        let replayed = Cube::from_facelets(candidate.start_state.as_deref().unwrap())
            .unwrap()
            .after_algorithm(candidate.algorithm.as_ref().unwrap());
        assert_eq!(replayed.facelets(), candidate.state.as_deref().unwrap());

        for record in &first_trace.records {
            if let Some(algorithm) = &record.algorithm {
                let start = record
                    .start_state
                    .as_deref()
                    .expect("every decision carrying moves must carry its replay origin");
                let end = record
                    .state
                    .as_deref()
                    .expect("every decision carrying moves must carry its endpoint");
                assert_eq!(
                    Cube::from_facelets(start)
                        .unwrap()
                        .after_algorithm(algorithm)
                        .facelets(),
                    end,
                    "decision {} is not replayable",
                    record.index
                );
            }
        }
    }

    #[test]
    fn decision_record_cap_truncates_observation_not_search() {
        let cube = Cube::from_scramble("R U F").unwrap();
        let result = Solver::new(SolveConfig {
            decisions: true,
            max_decision_records: 4,
            ..SolveConfig::default()
        })
        .solve(&cube)
        .unwrap();
        assert!(result.verified);
        let trace = result.decisions.unwrap();
        assert_eq!(trace.records.len(), 4);
        assert!(trace.truncated);
        assert!(trace.dropped_records > 0);
        assert!(trace
            .records
            .iter()
            .any(|record| record.kind == DecisionKind::Select));
        assert_eq!(
            trace
                .records
                .iter()
                .filter(|record| record.domain == DecisionDomain::Phase)
                .next_back()
                .map(|record| record.stage),
            Some(Stage::Pll)
        );
        assert_eq!(
            trace
                .records
                .iter()
                .filter(|record| {
                    record.domain != DecisionDomain::Phase
                        && matches!(record.kind, DecisionKind::Select | DecisionKind::SearchEnd)
                })
                .next_back()
                .map(|record| (&record.kind, record.stage)),
            Some((&DecisionKind::Select, Stage::F2l))
        );
        let retained_ids = trace
            .records
            .iter()
            .map(|record| record.index)
            .collect::<HashSet<_>>();
        assert!(trace
            .records
            .windows(2)
            .all(|pair| pair[0].index < pair[1].index));
        assert!(trace.records.iter().all(|record| record
            .parent_id
            .is_none_or(|parent| parent < record.index && retained_ids.contains(&parent))));
    }
}
