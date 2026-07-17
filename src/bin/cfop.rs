use std::{collections::HashMap, env, process};

use cfop_solver::{
    generate_to_jsonl, CrossMode, Cube, Difficulty, Efficiency, F2lStrategy, FeatureSelection,
    GenerateConfig, SolveConfig, SolveMethod, Solver,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(2);
    }
}

fn run() -> cfop_solver::Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };
    let options = parse_options(args.collect())?;
    match command.as_str() {
        "solve" => {
            let cube = input_cube(&options)?;
            let config = SolveConfig {
                method: option(&options, "method").unwrap_or("cfop").parse()?,
                cross_mode: option(&options, "cross")
                    .unwrap_or("cross")
                    .parse::<CrossMode>()?,
                f2l_strategy: option(&options, "f2l")
                    .unwrap_or("lookahead")
                    .parse::<F2lStrategy>()?,
                efficiency: option(&options, "efficiency")
                    .unwrap_or("balanced")
                    .parse::<Efficiency>()?,
                trace: flag(&options, "trace"),
                exploration: flag(&options, "explore"),
                max_exploration_states: number(&options, "max-exploration", 72)?,
                decisions: flag(&options, "decisions"),
                max_decision_records: number(&options, "max-decisions", 512)?,
                alternatives: number(&options, "alternatives", 0)?,
                max_depth: number(&options, "max-depth", 30)?,
                probe_limit: number(&options, "probe-limit", 1_000_000)?,
            };
            let result = Solver::new(config).solve(&cube)?;
            if flag(&options, "json")
                || !result.trace.is_empty()
                || !result.exploration.is_empty()
                || result.decisions.is_some()
            {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{}", result.solution);
            }
        }
        "apply" => {
            let cube = input_cube(&options)?;
            let moves = required(&options, "moves")?;
            println!("{}", cube.after(moves)?.facelets());
        }
        "verify" => {
            let cube = input_cube(&options)?;
            let solution = required(&options, "solution")?;
            let solved = cube.verifies(solution)?;
            println!("{solved}");
            if !solved {
                process::exit(1);
            }
        }
        "features" => {
            let cube = input_cube(&options)?;
            let selection = option(&options, "encoding")
                .unwrap_or("both")
                .parse::<FeatureSelection>()?;
            let features = cube.features(selection).ok_or_else(|| {
                cfop_solver::Error::Solver("--encoding none does not produce features".into())
            })?;
            println!("{}", serde_json::to_string_pretty(&features)?);
        }
        "generate" => {
            let difficulty = match option(&options, "difficulty").unwrap_or("hard") {
                "easy" => Difficulty::Easy,
                "medium" => Difficulty::Medium,
                "hard" => Difficulty::Hard,
                value => {
                    return Err(cfop_solver::Error::Solver(format!(
                        "bad difficulty {value:?}"
                    )))
                }
            };
            let config = GenerateConfig {
                count: number(&options, "count", 100)?,
                seed: number(&options, "seed", 0)?,
                difficulty,
                scramble_length: number(&options, "length", 0)?,
                solutions_per_scramble: number(&options, "solutions", 1)?,
                method: option(&options, "method")
                    .unwrap_or("fast")
                    .parse::<SolveMethod>()?,
                cross_mode: option(&options, "cross")
                    .unwrap_or("cross")
                    .parse::<CrossMode>()?,
                f2l_strategy: option(&options, "f2l")
                    .unwrap_or("lookahead")
                    .parse::<F2lStrategy>()?,
                efficiency: option(&options, "efficiency")
                    .unwrap_or("balanced")
                    .parse::<Efficiency>()?,
                max_depth: number(&options, "max-depth", 30)?,
                probe_limit: number(&options, "probe-limit", 1_000_000)?,
                trace: flag(&options, "trace"),
                exploration: flag(&options, "explore"),
                max_exploration_states: number(&options, "max-exploration", 72)?,
                decisions: flag(&options, "decisions"),
                max_decision_records: number(&options, "max-decisions", 512)?,
                features: option(&options, "features")
                    .unwrap_or("none")
                    .parse::<FeatureSelection>()?,
                threads: number(&options, "threads", 0)?,
            };
            let output = option(&options, "output").unwrap_or("cfop-data.jsonl");
            let batch_size = number(&options, "batch-size", 1024)?;
            generate_to_jsonl(&config, output, batch_size)?;
            println!("wrote {} records to {output}", config.count);
        }
        "help" | "--help" | "-h" => print_usage(),
        other => {
            return Err(cfop_solver::Error::Solver(format!(
                "unknown command {other:?}"
            )))
        }
    }
    Ok(())
}

fn input_cube(options: &HashMap<String, Option<String>>) -> cfop_solver::Result<Cube> {
    if let Some(state) = option(options, "state") {
        Cube::from_facelets(state)
    } else if let Some(scramble) = option(options, "scramble") {
        Cube::from_scramble(scramble)
    } else {
        Ok(Cube::solved())
    }
}

fn parse_options(values: Vec<String>) -> cfop_solver::Result<HashMap<String, Option<String>>> {
    let mut result = HashMap::new();
    let mut index = 0;
    while index < values.len() {
        let token = &values[index];
        if !token.starts_with("--") {
            return Err(cfop_solver::Error::Solver(format!(
                "expected --option, got {token:?}"
            )));
        }
        let key = token.trim_start_matches("--").to_owned();
        let value = if index + 1 < values.len() && !values[index + 1].starts_with("--") {
            index += 1;
            Some(values[index].clone())
        } else {
            None
        };
        result.insert(key, value);
        index += 1;
    }
    Ok(result)
}

fn option<'a>(options: &'a HashMap<String, Option<String>>, key: &str) -> Option<&'a str> {
    options.get(key).and_then(|value| value.as_deref())
}

fn required<'a>(
    options: &'a HashMap<String, Option<String>>,
    key: &str,
) -> cfop_solver::Result<&'a str> {
    option(options, key)
        .ok_or_else(|| cfop_solver::Error::Solver(format!("--{key} requires a value")))
}

fn flag(options: &HashMap<String, Option<String>>, key: &str) -> bool {
    options.contains_key(key)
}

fn number<T>(
    options: &HashMap<String, Option<String>>,
    key: &str,
    default: T,
) -> cfop_solver::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match option(options, key) {
        Some(value) => value
            .parse()
            .map_err(|error| cfop_solver::Error::Solver(format!("bad --{key}: {error}"))),
        None => Ok(default),
    }
}

fn print_usage() {
    println!(
        "cfop-solver\n\n\
         solve    [--scramble MOVES | --state FACELETS] [--method cfop|fast] [--cross cross|xcross|xxcross|xxxcross] [--f2l standard|lookahead] [--efficiency fast|balanced|thorough|max] [--trace] [--explore] [--decisions] [--max-decisions N] [--alternatives N] [--json]\n\
         apply    [--scramble MOVES | --state FACELETS] --moves MOVES\n\
         verify   [--scramble MOVES | --state FACELETS] --solution MOVES\n\
         features [--scramble MOVES | --state FACELETS] [--encoding cubie|facelet-ids|both]\n\
         generate --count N --output FILE [--difficulty easy|medium|hard] [--cross MODE] [--f2l STRATEGY] [--efficiency PROFILE] [--decisions] [--max-decisions N] [--features none|cubie|facelet-ids|both] [--max-depth N] [--probe-limit N] [--solutions N] [--threads N]"
    );
}
