use std::{
    collections::HashSet,
    fmt,
    panic::{catch_unwind, AssertUnwindSafe},
    str::FromStr,
    time::Instant,
};

use rcuber::solver::{
    cfop::{CrossSolver, F2LSolver, OLLSolver, PLLSolver},
    min2phase::solver::Solver as TwoPhaseSolver,
};
use serde::{Deserialize, Serialize};

use crate::{Algorithm, Cube, Error, Move, Result};

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
            _ => Err(Error::Solver(format!("unknown cross mode {value:?}"))),
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
            _ => Err(Error::Solver(format!("unknown F2L strategy {value:?}"))),
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
            _ => Err(Error::Solver(format!(
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
            _ => Err(Error::Solver(format!("unknown solve method {value:?}"))),
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
    /// Number of distinct additional solutions to request.
    pub alternatives: usize,
    pub max_depth: usize,
    pub probe_limit: u64,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<AlternativeSolution>,
    pub stats: SolveStats,
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
        let started = Instant::now();
        let input_state = cube.facelets();
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
        let mut result = match self.config.method {
            SolveMethod::Cfop => self.solve_cfop(&working_cube)?,
            SolveMethod::Fast => self.solve_fast(&working_cube)?,
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
        Ok(result)
    }

    pub fn solve_scramble(&self, scramble: &str) -> Result<SolveResult> {
        self.solve(&Cube::from_scramble(scramble)?)
    }

    fn solve_cfop(&self, input: &Cube) -> Result<SolveResult> {
        let mut cube = *input;
        let mut stages = Vec::with_capacity(8);
        let mut trace = Vec::new();
        let mut exploration = Vec::new();
        let mut complete = Vec::new();
        let mut search_metrics = SearchMetrics::default();

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

        for stage in [Stage::Oll, Stage::Pll] {
            let stage_started = Instant::now();
            let (next, algorithm, repaired) = run_cfop_stage(stage, cube)?;
            let message = decision_message(stage, &algorithm);
            record_phase(
                &self.config,
                &mut stages,
                &mut trace,
                &mut exploration,
                stage,
                cube,
                next,
                algorithm.clone(),
                Vec::new(),
                micros(stage_started.elapsed().as_micros()),
                repaired,
                false,
                message,
            )?;
            complete.extend(algorithm.moves().iter().copied());
            cube = next;
        }

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

    fn solve_fast(&self, input: &Cube) -> Result<SolveResult> {
        let start = Instant::now();
        let solution = if input.is_solved() {
            Algorithm::default()
        } else {
            fast_primary(input, self.config.max_depth, self.config.probe_limit)?
        };
        let end = input.after_algorithm(&solution);
        let elapsed_micros = micros(start.elapsed().as_micros());
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
}

#[derive(Debug)]
struct F2lCandidate {
    end: Cube,
    algorithm: Algorithm,
    pairs: Vec<F2lPair>,
    estimated_total: usize,
    repaired: bool,
}

#[derive(Debug, Default)]
struct SearchMetrics {
    f2l_candidates_evaluated: usize,
    exact_target_searches: usize,
    last_layer_evaluations: usize,
}

#[derive(Debug, Clone)]
struct F2lPlanNode {
    cube: Cube,
    steps: Vec<F2lStep>,
    move_count: usize,
    priority: usize,
    last_layer: Algorithm,
}

#[derive(Debug)]
struct F2lPlan {
    steps: Vec<F2lStep>,
    last_layer: Algorithm,
}

#[derive(Debug)]
struct F2lRoute {
    cross_end: Cube,
    cross_algorithm: Algorithm,
    cross_optimized: bool,
    remaining_steps: Vec<F2lStep>,
    last_layer: Algorithm,
}

impl F2lRoute {
    fn move_count(&self) -> usize {
        let mut moves = self.cross_algorithm.moves().to_vec();
        for step in &self.remaining_steps {
            moves.extend(step.algorithm.moves().iter().copied());
        }
        moves.extend(self.last_layer.moves().iter().copied());
        Algorithm::new(moves).simplified().len()
    }
}

#[allow(clippy::too_many_arguments)]
fn record_phase(
    config: &SolveConfig,
    stages: &mut Vec<StageResult>,
    trace: &mut Vec<TraceEvent>,
    exploration: &mut Vec<ExplorationEvent>,
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
    let mut variants = vec![(false, 1usize)];
    if strategy == F2lStrategy::Lookahead && efficiency != Efficiency::Fast {
        variants.push((true, 1));
        if matches!(efficiency, Efficiency::Thorough | Efficiency::Max) {
            variants.push((true, 4));
        }
        if efficiency == Efficiency::Max {
            variants.push((true, 64));
        }
    }

    let mut plans = Vec::with_capacity(variants.len());
    let mut first_error = None;
    for (all_aufs, beam_width) in variants {
        match beam_f2l_plan(
            cube,
            strategy,
            all_aufs,
            beam_width,
            target_pair_count,
            metrics,
        ) {
            Ok(plan) => plans.push(plan),
            Err(error) => {
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
            if step.algorithm.len() > 1 {
                for &budget in &exact_budgets {
                    metrics.exact_target_searches += 1;
                    if let Ok(candidate) = fast_to(&step_start, &step.end, max_depth, budget) {
                        if candidate.len() < step.algorithm.len()
                            && step_start.after_algorithm(&candidate) == step.end
                        {
                            step.algorithm = candidate;
                            step.optimized = true;
                        }
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
    let mut selected = routes
        .into_iter()
        .min_by_key(F2lRoute::move_count)
        .ok_or_else(|| {
            first_error.unwrap_or_else(|| {
                Error::Solver("F2L planner produced no complete pair route".into())
            })
        })?;

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
        for &budget in exact_budgets {
            metrics.exact_target_searches += 1;
            if let Ok(candidate) = fast_to(&cross_start, &cross_end, max_depth, budget) {
                if candidate.len() < cross_algorithm.len()
                    && cross_start.after_algorithm(&candidate) == cross_end
                {
                    cross_algorithm = candidate;
                    cross_optimized = true;
                }
            }
        }
    }

    let remaining_steps = plan.steps.split_off(split_index);
    F2lRoute {
        cross_end,
        cross_algorithm,
        cross_optimized,
        remaining_steps,
        last_layer: plan.last_layer,
    }
}

fn beam_f2l_plan(
    cube: Cube,
    strategy: F2lStrategy,
    all_aufs: bool,
    beam_width: usize,
    target_pair_count: usize,
    metrics: &mut SearchMetrics,
) -> Result<F2lPlan> {
    if f2l_pair_mask(&cube).count_ones() as usize >= target_pair_count {
        let last_layer = if target_pair_count == 4 {
            metrics.last_layer_evaluations += 1;
            last_layer_algorithm(cube)?
        } else {
            Algorithm::default()
        };
        return Ok(F2lPlan {
            steps: Vec::new(),
            last_layer,
        });
    }

    let mut frontier = vec![F2lPlanNode {
        cube,
        steps: Vec::new(),
        move_count: 0,
        priority: 0,
        last_layer: Algorithm::default(),
    }];
    let mut complete = Vec::new();
    let mut first_error = None;

    for _ in 0..4 {
        let mut next = Vec::new();
        for node in frontier {
            match f2l_candidates(node.cube, strategy, all_aufs, metrics) {
                Ok(candidates) => {
                    for candidate in candidates {
                        let before = f2l_pair_mask(&node.cube);
                        let after = f2l_pair_mask(&candidate.end);
                        if after & before != before || after.count_ones() <= before.count_ones() {
                            continue;
                        }
                        let step = F2lStep {
                            end: candidate.end,
                            algorithm: candidate.algorithm,
                            pairs: candidate.pairs,
                            elapsed_micros: 0,
                            repaired: candidate.repaired,
                            optimized: false,
                        };
                        let move_count = node.move_count + step.algorithm.len();
                        let mut steps = node.steps.clone();
                        steps.push(step);
                        let expanded = F2lPlanNode {
                            cube: candidate.end,
                            steps,
                            move_count,
                            priority: node.move_count + candidate.estimated_total,
                            last_layer: Algorithm::default(),
                        };
                        if after.count_ones() as usize >= target_pair_count {
                            let mut expanded = expanded;
                            if target_pair_count == 4 {
                                metrics.last_layer_evaluations += 1;
                                match last_layer_algorithm(expanded.cube) {
                                    Ok(algorithm) => expanded.last_layer = algorithm,
                                    Err(error) => {
                                        if first_error.is_none() {
                                            first_error = Some(error);
                                        }
                                        continue;
                                    }
                                }
                            }
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
        next.retain(|node| states.insert(node.cube.facelets()));
        next.truncate(beam_width);
        frontier = next;
    }

    complete
        .into_iter()
        .min_by_key(|node| (node.move_count + node.last_layer.len(), node.move_count))
        .map(|node| F2lPlan {
            steps: node.steps,
            last_layer: node.last_layer,
        })
        .ok_or_else(|| {
            first_error.unwrap_or_else(|| {
                Error::Solver("F2L beam search did not reach the requested pair count".into())
            })
        })
}

fn last_layer_algorithm(cube: Cube) -> Result<Algorithm> {
    let (after_oll, oll, _) = run_cfop_stage(Stage::Oll, cube)?;
    let (_, pll, _) = run_cfop_stage(Stage::Pll, after_oll)?;
    let mut moves = oll.moves().to_vec();
    moves.extend(pll.moves().iter().copied());
    Ok(Algorithm::new(moves).simplified())
}

fn f2l_candidates(
    cube: Cube,
    strategy: F2lStrategy,
    all_aufs: bool,
    metrics: &mut SearchMetrics,
) -> Result<Vec<F2lCandidate>> {
    let prefixes: &[&str] = if strategy == F2lStrategy::Lookahead && all_aufs {
        &["", "U", "U2", "U'"]
    } else {
        &[""]
    };
    let mut candidates = Vec::with_capacity(prefixes.len());
    let mut first_error = None;
    for notation in prefixes {
        metrics.f2l_candidates_evaluated += 1;
        match f2l_candidate(cube, notation) {
            Ok(Some(candidate)) => candidates.push(candidate),
            Ok(None) => {}
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    candidates.sort_by_key(|candidate| (candidate.estimated_total, candidate.algorithm.len()));
    let mut states = HashSet::new();
    candidates.retain(|candidate| states.insert(candidate.end.facelets()));
    if candidates.is_empty() {
        Err(first_error.unwrap_or_else(|| {
            Error::Solver("F2L engine produced no cumulative pair transition".into())
        }))
    } else {
        Ok(candidates)
    }
}

fn f2l_candidate(cube: Cube, prefix_notation: &str) -> Result<Option<F2lCandidate>> {
    let prefix: Algorithm = prefix_notation.parse()?;
    let adjusted = cube.after_algorithm(&prefix);
    let (_, remainder, repaired) = run_cfop_stage(Stage::F2l, adjusted)?;
    let mut all_moves = prefix.moves().to_vec();
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
            let end = cube.after_algorithm(&algorithm);
            let candidate = F2lCandidate {
                end,
                algorithm,
                pairs: f2l_pairs(after & !before),
                estimated_total,
                repaired,
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
    let solved = rcuber::cubie::SOLVED_CUBIE_CUBE;
    let mut mask = 0u8;
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

fn f2l_pairs(mask: u8) -> Vec<F2lPair> {
    [F2lPair::Fr, F2lPair::Fl, F2lPair::Bl, F2lPair::Br]
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, pair)| (mask & (1 << index) != 0).then_some(pair))
        .collect()
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
            let replayed = cube.after_algorithm(&algorithm);
            if stage_is_solved(stage, &replayed) {
                // Some upstream stage routines probe by mutating their internal cube.
                // Only the independently replayed move sequence is authoritative.
                return Ok((replayed, algorithm, false));
            }
            if stage_is_solved(stage, &engine_state) {
                let repaired = fast_to(&cube, &engine_state, 30, 1_000_000)?;
                let replayed = cube.after_algorithm(&repaired);
                if replayed == engine_state && stage_is_solved(stage, &replayed) {
                    return Ok((replayed, repaired, true));
                }
            }
            Err(Error::Solver(format!(
                "{stage}: replaying the returned moves did not satisfy the phase invariant"
            )))
        }
        Ok(Err(message)) => Err(Error::Solver(format!("{stage}: {message}"))),
        Err(_) => Err(Error::Solver(format!("{stage} engine panicked"))),
    }
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
        Stage::F2l => F2LSolver { cube: cube.inner }.is_solved(),
        Stage::F2lPair => CrossSolver { cube: cube.inner }.is_solved() && f2l_pair_mask(cube) != 0,
        Stage::Oll => OLLSolver::new(cube.inner).is_solved(),
        Stage::Pll => PLLSolver::new(cube.inner).is_solved(),
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
    let solved = rcuber::cubie::SOLVED_CUBIE_CUBE;
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
        Stage::Pll | Stage::Fast => (cross + f2l_corners + f2l_edges + oriented + permuted) as u16,
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
}
