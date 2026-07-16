use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use rayon::{prelude::*, ThreadPoolBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    Algorithm, CrossMode, Cube, Efficiency, Error, ExplorationEvent, F2lStrategy, Result,
    SolveConfig, SolveMethod, SolveStats, Solver, StageResult, TraceEvent,
};

pub const DATASET_SCHEMA: &str = "cfop-dataset/v3";
pub const STATE_FORMAT: &str = "facelets-urfdlb-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub const fn default_scramble_length(self) -> usize {
        match self {
            Self::Easy => 5,
            Self::Medium => 12,
            Self::Hard => 20,
        }
    }
}

impl Default for Difficulty {
    fn default() -> Self {
        Self::Hard
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerateConfig {
    pub count: usize,
    pub seed: u64,
    pub difficulty: Difficulty,
    /// Zero selects the difficulty profile's default.
    pub scramble_length: usize,
    pub solutions_per_scramble: usize,
    pub method: SolveMethod,
    pub cross_mode: CrossMode,
    pub f2l_strategy: F2lStrategy,
    pub efficiency: Efficiency,
    pub max_depth: usize,
    pub probe_limit: u64,
    pub trace: bool,
    pub exploration: bool,
    pub max_exploration_states: usize,
    /// Zero uses the machine's available parallelism.
    pub threads: usize,
}

impl Default for GenerateConfig {
    fn default() -> Self {
        Self {
            count: 100,
            seed: 0,
            difficulty: Difficulty::Hard,
            scramble_length: 0,
            solutions_per_scramble: 1,
            method: SolveMethod::Fast,
            cross_mode: CrossMode::Cross,
            f2l_strategy: F2lStrategy::Lookahead,
            efficiency: Efficiency::Balanced,
            max_depth: 30,
            probe_limit: 1_000_000,
            trace: false,
            exploration: false,
            max_exploration_states: 72,
            threads: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSolution {
    pub source: String,
    pub method: SolveMethod,
    #[serde(default)]
    pub cross_mode: CrossMode,
    #[serde(default)]
    pub f2l_strategy: F2lStrategy,
    #[serde(default)]
    pub efficiency: Efficiency,
    pub moves: Algorithm,
    pub move_count: usize,
    pub verified: bool,
    pub stats: SolveStats,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<StageResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace: Vec<TraceEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exploration: Vec<ExplorationEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSample {
    pub schema: String,
    pub state_format: String,
    pub index: usize,
    /// Hex avoids precision loss in JSON consumers that represent all numbers as doubles.
    pub item_seed: String,
    pub difficulty: Difficulty,
    pub scramble: Algorithm,
    pub scramble_move_count: usize,
    pub state: String,
    pub solutions: Vec<GeneratedSolution>,
}

pub fn generate_dataset(config: &GenerateConfig) -> Result<Vec<GeneratedSample>> {
    validate_config(config)?;
    let pool = if config.threads == 0 {
        ThreadPoolBuilder::new()
    } else {
        ThreadPoolBuilder::new().num_threads(config.threads)
    }
    .build()
    .map_err(|error| Error::Solver(format!("could not build generation thread pool: {error}")))?;

    let records: Vec<Result<GeneratedSample>> = pool.install(|| {
        (0..config.count)
            .into_par_iter()
            .map(|index| {
                generate_one(config, index).map_err(|error| {
                    Error::Solver(format!("generation record {index} failed: {error}"))
                })
            })
            .collect()
    });
    records.into_iter().collect()
}

/// Generate in bounded batches and stream JSONL without retaining the full dataset.
pub fn generate_to_jsonl(
    config: &GenerateConfig,
    path: impl AsRef<Path>,
    batch_size: usize,
) -> Result<()> {
    validate_config(config)?;
    if batch_size == 0 {
        return Err(Error::Solver("batch_size must be at least one".into()));
    }
    let mut writer = BufWriter::new(File::create(path)?);
    let mut offset = 0;
    while offset < config.count {
        let size = batch_size.min(config.count - offset);
        let mut batch_config = config.clone();
        batch_config.count = size;
        let records = generate_range(&batch_config, offset)?;
        for record in records {
            serde_json::to_writer(&mut writer, &record)?;
            writer.write_all(b"\n")?;
        }
        offset += size;
    }
    writer.flush()?;
    Ok(())
}

pub fn write_jsonl(records: &[GeneratedSample], path: impl AsRef<Path>) -> Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    for record in records {
        serde_json::to_writer(&mut writer, record)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn generate_range(config: &GenerateConfig, start_index: usize) -> Result<Vec<GeneratedSample>> {
    let pool = if config.threads == 0 {
        ThreadPoolBuilder::new()
    } else {
        ThreadPoolBuilder::new().num_threads(config.threads)
    }
    .build()
    .map_err(|error| Error::Solver(format!("could not build generation thread pool: {error}")))?;
    let records: Vec<Result<GeneratedSample>> = pool.install(|| {
        (start_index..start_index + config.count)
            .into_par_iter()
            .map(|index| {
                generate_one(config, index).map_err(|error| {
                    Error::Solver(format!("generation record {index} failed: {error}"))
                })
            })
            .collect()
    });
    records.into_iter().collect()
}

fn generate_one(config: &GenerateConfig, index: usize) -> Result<GeneratedSample> {
    let item_seed = derive_seed(config.seed, index as u64);
    let length = if config.scramble_length == 0 {
        config.difficulty.default_scramble_length()
    } else {
        config.scramble_length
    };
    let scramble = random_scramble(length, item_seed);
    let cube = Cube::solved().after_algorithm(&scramble);
    let solve_config = SolveConfig {
        method: config.method,
        cross_mode: config.cross_mode,
        f2l_strategy: config.f2l_strategy,
        efficiency: config.efficiency,
        trace: config.trace,
        exploration: config.exploration,
        max_exploration_states: config.max_exploration_states,
        max_depth: config.max_depth,
        probe_limit: config.probe_limit,
        // A generated record already has a second genuine solution available:
        // the inverse of its known scramble. Reserve the much more expensive
        // continuation search for requests beyond those two solutions.
        alternatives: config.solutions_per_scramble.saturating_sub(2),
    };
    let result = Solver::new(solve_config).solve(&cube)?;
    let mut solutions = Vec::with_capacity(config.solutions_per_scramble);
    let mut primary_stages = result.stages.clone();
    // Wall-clock timings are useful for interactive solve results but would make a
    // seeded dataset differ across runs and thread counts.
    for stage in &mut primary_stages {
        stage.elapsed_micros = 0;
    }
    let mut primary_stats = result.stats.clone();
    primary_stats.elapsed_micros = 0;
    solutions.push(GeneratedSolution {
        source: "solver_primary".into(),
        method: config.method,
        cross_mode: config.cross_mode,
        f2l_strategy: config.f2l_strategy,
        efficiency: config.efficiency,
        moves: result.solution.clone(),
        move_count: result.solution.len(),
        verified: result.verified,
        stats: primary_stats,
        stages: primary_stages,
        trace: result.trace,
        exploration: result.exploration,
    });

    if solutions.len() < config.solutions_per_scramble {
        let inverse = scramble.inverse().simplified();
        push_unique_solution(
            &mut solutions,
            GeneratedSolution {
                source: "scramble_inverse".into(),
                method: SolveMethod::Fast,
                cross_mode: config.cross_mode,
                f2l_strategy: config.f2l_strategy,
                efficiency: config.efficiency,
                move_count: inverse.len(),
                verified: cube.after_algorithm(&inverse).is_solved(),
                stats: generated_stats(SolveMethod::Fast, inverse.len()),
                moves: inverse,
                stages: Vec::new(),
                trace: Vec::new(),
                exploration: Vec::new(),
            },
        );
    }
    for alternative in result.alternatives {
        push_unique_solution(
            &mut solutions,
            GeneratedSolution {
                source: alternative.source,
                method: SolveMethod::Fast,
                cross_mode: config.cross_mode,
                f2l_strategy: config.f2l_strategy,
                efficiency: config.efficiency,
                move_count: alternative.move_count,
                verified: alternative.verified,
                stats: generated_stats(SolveMethod::Fast, alternative.move_count),
                moves: alternative.algorithm,
                stages: Vec::new(),
                trace: Vec::new(),
                exploration: Vec::new(),
            },
        );
    }
    add_verified_identity_variants(&cube, &mut solutions, config.solutions_per_scramble);
    solutions.truncate(config.solutions_per_scramble);

    Ok(GeneratedSample {
        schema: DATASET_SCHEMA.into(),
        state_format: STATE_FORMAT.into(),
        index,
        item_seed: format!("{item_seed:016x}"),
        difficulty: config.difficulty,
        scramble_move_count: scramble.len(),
        scramble,
        state: cube.facelets(),
        solutions,
    })
}

fn random_scramble(length: usize, seed: u64) -> Algorithm {
    const FACES: [&str; 6] = ["U", "R", "F", "D", "L", "B"];
    const SUFFIXES: [&str; 3] = ["", "2", "'"];
    let mut rng = ChaCha12Rng::seed_from_u64(seed);
    let mut moves = Vec::with_capacity(length);
    let mut previous_face = None;
    for _ in 0..length {
        let mut face = rng.gen_range(0..FACES.len());
        while Some(face) == previous_face {
            face = rng.gen_range(0..FACES.len());
        }
        previous_face = Some(face);
        let suffix = SUFFIXES[rng.gen_range(0..SUFFIXES.len())];
        moves.push(
            format!("{}{suffix}", FACES[face])
                .parse()
                .expect("static scramble move"),
        );
    }
    Algorithm::new(moves)
}

fn add_verified_identity_variants(
    cube: &Cube,
    solutions: &mut Vec<GeneratedSolution>,
    requested: usize,
) {
    const DETOURS: [&str; 6] = ["U U'", "R R'", "F F'", "D D'", "L L'", "B B'"];
    let Some(primary) = solutions.first().map(|solution| solution.moves.clone()) else {
        return;
    };
    for detour in DETOURS {
        if solutions.len() >= requested {
            break;
        }
        let mut moves = detour
            .parse::<Algorithm>()
            .expect("static detour")
            .moves()
            .to_vec();
        moves.extend(primary.moves().iter().copied());
        let variant = Algorithm::new(moves);
        push_unique_solution(
            solutions,
            GeneratedSolution {
                source: "verified_identity_variant".into(),
                method: solutions[0].method,
                cross_mode: solutions[0].cross_mode,
                f2l_strategy: solutions[0].f2l_strategy,
                efficiency: solutions[0].efficiency,
                move_count: variant.len(),
                verified: cube.after_algorithm(&variant).is_solved(),
                stats: generated_stats(solutions[0].method, variant.len()),
                moves: variant,
                stages: Vec::new(),
                trace: Vec::new(),
                exploration: Vec::new(),
            },
        );
    }
}

fn push_unique_solution(target: &mut Vec<GeneratedSolution>, candidate: GeneratedSolution) {
    if candidate.verified
        && !target
            .iter()
            .any(|existing| existing.moves.to_string() == candidate.moves.to_string())
    {
        target.push(candidate);
    }
}

fn generated_stats(method: SolveMethod, move_count: usize) -> SolveStats {
    SolveStats {
        method,
        move_count,
        ..SolveStats::default()
    }
}

fn validate_config(config: &GenerateConfig) -> Result<()> {
    if config.solutions_per_scramble == 0 {
        return Err(Error::Solver(
            "solutions_per_scramble must be at least one".into(),
        ));
    }
    Ok(())
}

fn derive_seed(root: u64, index: u64) -> u64 {
    // SplitMix64: cheap domain separation by record index and stable across thread counts.
    let mut value = root ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_across_thread_counts() {
        let base = GenerateConfig {
            count: 4,
            seed: 42,
            difficulty: Difficulty::Easy,
            solutions_per_scramble: 2,
            threads: 1,
            trace: true,
            ..GenerateConfig::default()
        };
        let one = generate_dataset(&base).unwrap();
        let many = generate_dataset(&GenerateConfig { threads: 2, ..base }).unwrap();
        assert_eq!(
            serde_json::to_string(&one).unwrap(),
            serde_json::to_string(&many).unwrap()
        );
        assert!(one.iter().all(|record| record.solutions.len() == 2));
        assert!(one
            .iter()
            .all(|record| !record.solutions[0].trace.is_empty()));
    }

    #[test]
    fn jsonl_has_one_record_per_line() {
        let records = generate_dataset(&GenerateConfig {
            count: 2,
            difficulty: Difficulty::Easy,
            ..GenerateConfig::default()
        })
        .unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        write_jsonl(&records, file.path()).unwrap();
        let contents = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(contents.lines().count(), 2);
    }

    #[test]
    fn every_advanced_cross_mode_generates_verified_records() {
        for cross_mode in [
            CrossMode::Cross,
            CrossMode::XCross,
            CrossMode::XXCross,
            CrossMode::XXXCross,
        ] {
            let records = generate_dataset(&GenerateConfig {
                count: 4,
                seed: 20260715,
                difficulty: Difficulty::Hard,
                method: SolveMethod::Cfop,
                cross_mode,
                threads: 2,
                ..GenerateConfig::default()
            })
            .unwrap();
            assert!(records.iter().all(|record| {
                record.schema == DATASET_SCHEMA
                    && record.solutions[0].verified
                    && record.solutions[0].cross_mode == cross_mode
                    && record.solutions[0].efficiency == Efficiency::Balanced
                    && record.solutions[0].stats.elapsed_micros == 0
                    && record.solutions[0].stats.f2l_candidates_evaluated > 0
            }));
        }
    }

    #[test]
    fn lookahead_reduces_aggregate_f2l_work() {
        let base = GenerateConfig {
            count: 12,
            seed: 99,
            difficulty: Difficulty::Hard,
            method: SolveMethod::Cfop,
            threads: 2,
            ..GenerateConfig::default()
        };
        let standard = generate_dataset(&GenerateConfig {
            f2l_strategy: F2lStrategy::Standard,
            ..base
        })
        .unwrap();
        let lookahead = generate_dataset(&GenerateConfig {
            f2l_strategy: F2lStrategy::Lookahead,
            ..base
        })
        .unwrap();
        let f2l_moves = |records: &[GeneratedSample]| {
            records
                .iter()
                .flat_map(|record| &record.solutions[0].stages)
                .filter(|stage| stage.stage == crate::Stage::F2lPair)
                .map(|stage| stage.move_count)
                .sum::<usize>()
        };
        assert!(f2l_moves(&lookahead) < f2l_moves(&standard));
    }
}
