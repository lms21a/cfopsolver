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
    F2l,
    Oll,
    Pll,
    Fast,
}

impl fmt::Display for Stage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Setup => "setup",
            Self::Cross => "cross",
            Self::F2l => "f2l",
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
    pub start_state: String,
    pub end_state: String,
    pub algorithm: Algorithm,
    pub move_count: usize,
    pub elapsed_micros: u64,
    /// True when an upstream stage's mutable state diverged from its returned
    /// moves and the transition was independently reconstructed and verified.
    #[serde(default)]
    pub repaired: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveStats {
    pub method: SolveMethod,
    pub elapsed_micros: u64,
    pub move_count: usize,
    pub exploration_states: usize,
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
                    start_state: input_state.clone(),
                    end_state: setup_end.clone(),
                    move_count: setup.len(),
                    algorithm: setup.clone(),
                    elapsed_micros: 0,
                    repaired: false,
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
        let mut stages = Vec::with_capacity(4);
        let mut trace = Vec::new();
        let mut exploration = Vec::new();
        let mut complete = Vec::new();

        for stage in [Stage::Cross, Stage::F2l, Stage::Oll, Stage::Pll] {
            let start = cube;
            if self.config.trace {
                trace.push(TraceEvent {
                    index: trace.len(),
                    stage,
                    kind: TraceKind::PhaseStart,
                    message: format!("begin {stage} from the current canonical state"),
                    state: Some(start.facelets()),
                    algorithm: None,
                });
            }
            let stage_started = Instant::now();
            let (next, algorithm, repaired) = run_cfop_stage(stage, start)?;
            let elapsed_micros = micros(stage_started.elapsed().as_micros());

            if self.config.exploration && exploration.len() < self.config.max_exploration_states {
                append_stage_exploration(
                    &mut exploration,
                    stage,
                    &start,
                    algorithm.moves().first().copied(),
                    self.config.max_exploration_states,
                );
            }

            if self.config.trace {
                trace.push(TraceEvent {
                    index: trace.len(),
                    stage,
                    kind: TraceKind::Decision,
                    message: if repaired {
                        format!(
                            "{}; independently reconstructed the transition after an engine-state mismatch",
                            decision_message(stage, &algorithm)
                        )
                    } else {
                        decision_message(stage, &algorithm)
                    },
                    state: None,
                    algorithm: Some(algorithm.clone()),
                });
                trace.push(TraceEvent {
                    index: trace.len(),
                    stage,
                    kind: TraceKind::PhaseComplete,
                    message: format!("completed {stage} in {} moves", algorithm.len()),
                    state: Some(next.facelets()),
                    algorithm: Some(algorithm.clone()),
                });
            }

            stages.push(StageResult {
                stage,
                start_state: start.facelets(),
                end_state: next.facelets(),
                move_count: algorithm.len(),
                algorithm: algorithm.clone(),
                elapsed_micros,
                repaired,
            });
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
                    state: Some(input.facelets()),
                    algorithm: Some(solution.clone()),
                },
                TraceEvent {
                    index: 1,
                    stage: Stage::Fast,
                    kind: TraceKind::PhaseComplete,
                    message: format!("completed fast search in {} moves", solution.len()),
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
            solution: solution.clone(),
            verified: false,
            stages: vec![StageResult {
                stage: Stage::Fast,
                start_state: input.facelets(),
                end_state: end.facelets(),
                move_count: solution.len(),
                algorithm: solution,
                elapsed_micros,
                repaired: false,
            }],
            trace,
            exploration: Vec::new(),
            alternatives: Vec::new(),
            stats: SolveStats {
                method: SolveMethod::Fast,
                elapsed_micros: 0,
                move_count: 0,
                exploration_states: 0,
            },
        })
    }
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
        Stage::Setup => unreachable!(),
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
        Stage::Fast => unreachable!(),
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
        Stage::F2l => F2LSolver { cube: cube.inner }.is_solved(),
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
        Stage::F2l => (cross + f2l_corners + f2l_edges) as u16,
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
        Stage::F2l => format!(
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
