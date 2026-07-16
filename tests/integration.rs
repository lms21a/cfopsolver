use std::collections::HashSet;

use cfop_solver::{
    CrossMode, Cube, Efficiency, F2lStrategy, SolveConfig, SolveMethod, Solver, Stage, TraceEvent,
};

#[test]
fn every_supported_move_round_trips_with_its_inverse() {
    let bases = [
        "U", "R", "F", "D", "L", "B", "M", "E", "S", "Uw", "Rw", "Fw", "Dw", "Lw", "Bw", "x", "y",
        "z",
    ];
    for base in bases {
        for suffix in ["", "2", "'"] {
            let algorithm = format!("{base}{suffix}");
            let parsed: cfop_solver::Algorithm = algorithm.parse().unwrap();
            let moved = Cube::solved().after_algorithm(&parsed);
            let restored = Cube::from_facelets(&moved.facelets()).unwrap();
            assert_eq!(restored, moved, "serialization changed {algorithm}");
            assert!(
                moved
                    .after_algorithm(&parsed.inverse())
                    .is_canonically_solved(),
                "inverse failed for {algorithm}"
            );
        }
    }
}

#[test]
fn parsing_is_transactional_for_model_verification() {
    let mut cube = Cube::from_scramble("R U F").unwrap();
    let before = cube.facelets();
    assert!(cube.apply("R U definitely-not-a-move F").is_err());
    assert_eq!(cube.facelets(), before);
    assert!(cube.apply("[R,U]").is_err());
    assert_eq!(cube.facelets(), before);
}

#[test]
fn fast_solver_normalizes_and_reports_a_rotated_center_frame() {
    let cube = Cube::from_scramble("x R U2 F L'").unwrap();
    assert!(!cube.has_canonical_orientation());
    let result = Solver::new(SolveConfig {
        method: SolveMethod::Fast,
        ..SolveConfig::default()
    })
    .solve(&cube)
    .unwrap();
    assert_eq!(result.stages[0].stage, Stage::Setup);
    assert!(cube.after_algorithm(&result.solution).is_solved());
}

#[test]
fn staged_cfop_replays_phase_boundaries_on_varied_scrambles() {
    let solver = Solver::new(SolveConfig::default());
    let scrambles = [
        "R U2 F' L2 D B2 R' F U L' D2 B R2 U' F2",
        "D2 L B' U2 R2 F D' R U B2 L' F2 R' U2",
        "B R2 U' F2 D L2 U B' R F' D2 L U2 R'",
        "F2 U L2 B' D R2 U' F L' D2 B2 R U F'",
    ];
    for scramble in scrambles {
        let cube = Cube::from_scramble(scramble).unwrap();
        let result = solver.solve(&cube).unwrap();
        assert_eq!(result.stages.first().unwrap().stage, Stage::Cross);
        assert_eq!(result.stages[result.stages.len() - 2].stage, Stage::Oll);
        assert_eq!(result.stages.last().unwrap().stage, Stage::Pll);
        let pairs = result
            .stages
            .iter()
            .flat_map(|stage| stage.f2l_pairs.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs.iter().copied().collect::<HashSet<_>>().len(), 4);
        assert!(cube.after_algorithm(&result.solution).is_solved());
        for stage in &result.stages {
            let replay = Cube::from_facelets(&stage.start_state)
                .unwrap()
                .after_algorithm(&stage.algorithm);
            assert_eq!(replay.facelets(), stage.end_state);
        }
        for adjacent in result.stages.windows(2) {
            assert_eq!(adjacent[0].end_state, adjacent[1].start_state);
        }
    }
}

#[test]
fn advanced_cross_modes_include_the_requested_number_of_pairs() {
    let cube = Cube::from_scramble("D R2 B2 L2 F L B' R' L D F U' B2 D' L R2 F2 L2 F' B'").unwrap();
    for (mode, expected_stage, minimum_pairs) in [
        (CrossMode::Cross, Stage::Cross, 0),
        (CrossMode::XCross, Stage::XCross, 1),
        (CrossMode::XXCross, Stage::XXCross, 2),
        (CrossMode::XXXCross, Stage::XXXCross, 3),
    ] {
        let result = Solver::new(SolveConfig {
            cross_mode: mode,
            trace: true,
            ..SolveConfig::default()
        })
        .solve(&cube)
        .unwrap();
        let cross = &result.stages[0];
        assert_eq!(cross.stage, expected_stage);
        assert!(cross.f2l_pairs.len() >= minimum_pairs);

        let all_pairs = result
            .stages
            .iter()
            .flat_map(|stage| stage.f2l_pairs.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(all_pairs.len(), 4);
        assert_eq!(all_pairs.iter().copied().collect::<HashSet<_>>().len(), 4);
        assert!(cube.after_algorithm(&result.solution).is_solved());
        assert_trace_moves_are_attached(&result.trace);
    }
}

#[test]
fn lookahead_reduces_f2l_moves_on_the_reference_scramble() {
    let cube = Cube::from_scramble("D R2 B2 L2 F L B' R' L D F U' B2 D' L R2 F2 L2 F' B'").unwrap();
    let solve = |f2l_strategy| {
        Solver::new(SolveConfig {
            f2l_strategy,
            ..SolveConfig::default()
        })
        .solve(&cube)
        .unwrap()
    };
    let standard = solve(F2lStrategy::Standard);
    let lookahead = solve(F2lStrategy::Lookahead);
    let f2l_moves = |result: &cfop_solver::SolveResult| {
        result
            .stages
            .iter()
            .filter(|stage| stage.stage == Stage::F2lPair)
            .map(|stage| stage.move_count)
            .sum::<usize>()
    };
    assert!(f2l_moves(&lookahead) < f2l_moves(&standard));
}

#[test]
fn efficiency_profiles_trade_search_work_for_solution_quality() {
    let scrambles = [
        "D R2 B2 L2 F L B' R' L D F U' B2 D' L R2 F2 L2 F' B'",
        "B' F D' U2 B F R2 F' B' F B2 R' F U' B R' U' L2 U' R2",
    ];
    for scramble in scrambles {
        let cube = Cube::from_scramble(scramble).unwrap();
        let results = [
            Efficiency::Fast,
            Efficiency::Balanced,
            Efficiency::Thorough,
            Efficiency::Max,
        ]
        .iter()
        .copied()
        .map(|efficiency| {
            Solver::new(SolveConfig {
                cross_mode: CrossMode::XXXCross,
                efficiency,
                ..SolveConfig::default()
            })
            .solve(&cube)
            .unwrap()
        })
        .collect::<Vec<_>>();

        assert!(results.iter().all(|result| result.verified));
        for adjacent in results.windows(2) {
            assert!(adjacent[1].stats.move_count <= adjacent[0].stats.move_count);
            assert!(
                adjacent[1].stats.f2l_candidates_evaluated
                    > adjacent[0].stats.f2l_candidates_evaluated
            );
            assert!(
                adjacent[1].stats.exact_target_searches > adjacent[0].stats.exact_target_searches
            );
            assert!(
                adjacent[1].stats.last_layer_evaluations > adjacent[0].stats.last_layer_evaluations
            );
        }
        assert_eq!(results[0].stats.exact_target_searches, 0);
    }
}

fn assert_trace_moves_are_attached(trace: &[TraceEvent]) {
    assert!(!trace.is_empty());
    for event in trace {
        if !matches!(event.kind, cfop_solver::solver::TraceKind::PhaseStart) {
            assert!(event.algorithm.is_some(), "missing moves on {event:?}");
        }
    }
    let encoded = serde_json::to_value(trace).unwrap();
    for event in encoded.as_array().unwrap() {
        if event["kind"] != "phase_start" {
            assert!(event.get("moves").is_some());
            assert!(event.get("algorithm").is_none());
        }
    }
}
