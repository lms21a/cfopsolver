use std::collections::HashSet;

use cfop_solver::{
    Algorithm, CrossMode, Cube, Efficiency, F2lStrategy, PhaseBoundary, PhaseInvariantStatus,
    SolveConfig, SolveMethod, Solver, Stage, TraceEvent, TrainingVerifier, VerificationConfig,
    VerificationInput,
};

#[test]
fn invalid_search_limits_fail_before_search_even_for_solved_cubes() {
    for config in [
        SolveConfig {
            max_depth: 0,
            ..SolveConfig::default()
        },
        SolveConfig {
            max_depth: 31,
            ..SolveConfig::default()
        },
        SolveConfig {
            max_depth: usize::MAX,
            ..SolveConfig::default()
        },
        SolveConfig {
            probe_limit: 0,
            ..SolveConfig::default()
        },
    ] {
        assert!(matches!(
            Solver::new(config).solve(&Cube::solved()),
            Err(cfop_solver::Error::InvalidConfig(_))
        ));
    }
}

#[test]
fn every_supported_move_matches_golden_and_round_trips() {
    let goldens = [
        (
            "U",
            "UUUUUUUUUBBBRRRRRRRRRFFFFFFDDDDDDDDDFFFLLLLLLLLLBBBBBB",
        ),
        (
            "U2",
            "UUUUUUUUULLLRRRRRRBBBFFFFFFDDDDDDDDDRRRLLLLLLFFFBBBBBB",
        ),
        (
            "U'",
            "UUUUUUUUUFFFRRRRRRLLLFFFFFFDDDDDDDDDBBBLLLLLLRRRBBBBBB",
        ),
        (
            "R",
            "UUFUUFUUFRRRRRRRRRFFDFFDFFDDDBDDBDDBLLLLLLLLLUBBUBBUBB",
        ),
        (
            "R2",
            "UUDUUDUUDRRRRRRRRRFFBFFBFFBDDUDDUDDULLLLLLLLLFBBFBBFBB",
        ),
        (
            "R'",
            "UUBUUBUUBRRRRRRRRRFFUFFUFFUDDFDDFDDFLLLLLLLLLDBBDBBDBB",
        ),
        (
            "F",
            "UUUUUULLLURRURRURRFFFFFFFFFRRRDDDDDDLLDLLDLLDBBBBBBBBB",
        ),
        (
            "F2",
            "UUUUUUDDDLRRLRRLRRFFFFFFFFFUUUDDDDDDLLRLLRLLRBBBBBBBBB",
        ),
        (
            "F'",
            "UUUUUURRRDRRDRRDRRFFFFFFFFFLLLDDDDDDLLULLULLUBBBBBBBBB",
        ),
        (
            "D",
            "UUUUUUUUURRRRRRFFFFFFFFFLLLDDDDDDDDDLLLLLLBBBBBBBBBRRR",
        ),
        (
            "D2",
            "UUUUUUUUURRRRRRLLLFFFFFFBBBDDDDDDDDDLLLLLLRRRBBBBBBFFF",
        ),
        (
            "D'",
            "UUUUUUUUURRRRRRBBBFFFFFFRRRDDDDDDDDDLLLLLLFFFBBBBBBLLL",
        ),
        (
            "L",
            "BUUBUUBUURRRRRRRRRUFFUFFUFFFDDFDDFDDLLLLLLLLLBBDBBDBBD",
        ),
        (
            "L2",
            "DUUDUUDUURRRRRRRRRBFFBFFBFFUDDUDDUDDLLLLLLLLLBBFBBFBBF",
        ),
        (
            "L'",
            "FUUFUUFUURRRRRRRRRDFFDFFDFFBDDBDDBDDLLLLLLLLLBBUBBUBBU",
        ),
        (
            "B",
            "RRRUUUUUURRDRRDRRDFFFFFFFFFDDDDDDLLLULLULLULLBBBBBBBBB",
        ),
        (
            "B2",
            "DDDUUUUUURRLRRLRRLFFFFFFFFFDDDDDDUUURLLRLLRLLBBBBBBBBB",
        ),
        (
            "B'",
            "LLLUUUUUURRURRURRUFFFFFFFFFDDDDDDRRRDLLDLLDLLBBBBBBBBB",
        ),
        (
            "M",
            "UBUUBUUBURRRRRRRRRFUFFUFFUFDFDDFDDFDLLLLLLLLLBDBBDBBDB",
        ),
        (
            "M2",
            "UDUUDUUDURRRRRRRRRFBFFBFFBFDUDDUDDUDLLLLLLLLLBFBBFBBFB",
        ),
        (
            "M'",
            "UFUUFUUFURRRRRRRRRFDFFDFFDFDBDDBDDBDLLLLLLLLLBUBBUBBUB",
        ),
        (
            "E",
            "UUUUUUUUURRRFFFRRRFFFLLLFFFDDDDDDDDDLLLBBBLLLBBBRRRBBB",
        ),
        (
            "E2",
            "UUUUUUUUURRRLLLRRRFFFBBBFFFDDDDDDDDDLLLRRRLLLBBBFFFBBB",
        ),
        (
            "E'",
            "UUUUUUUUURRRBBBRRRFFFRRRFFFDDDDDDDDDLLLFFFLLLBBBLLLBBB",
        ),
        (
            "S",
            "UUULLLUUURURRURRURFFFFFFFFFDDDRRRDDDLDLLDLLDLBBBBBBBBB",
        ),
        (
            "S2",
            "UUUDDDUUURLRRLRRLRFFFFFFFFFDDDUUUDDDLRLLRLLRLBBBBBBBBB",
        ),
        (
            "S'",
            "UUURRRUUURDRRDRRDRFFFFFFFFFDDDLLLDDDLULLULLULBBBBBBBBB",
        ),
        (
            "Uw",
            "UUUUUUUUUBBBBBBRRRRRRRRRFFFDDDDDDDDDFFFFFFLLLLLLLLLBBB",
        ),
        (
            "Uw2",
            "UUUUUUUUULLLLLLRRRBBBBBBFFFDDDDDDDDDRRRRRRLLLFFFFFFBBB",
        ),
        (
            "Uw'",
            "UUUUUUUUUFFFFFFRRRLLLLLLFFFDDDDDDDDDBBBBBBLLLRRRRRRBBB",
        ),
        (
            "Rw",
            "UFFUFFUFFRRRRRRRRRFDDFDDFDDDBBDBBDBBLLLLLLLLLUUBUUBUUB",
        ),
        (
            "Rw2",
            "UDDUDDUDDRRRRRRRRRFBBFBBFBBDUUDUUDUULLLLLLLLLFFBFFBFFB",
        ),
        (
            "Rw'",
            "UBBUBBUBBRRRRRRRRRFUUFUUFUUDFFDFFDFFLLLLLLLLLDDBDDBDDB",
        ),
        (
            "Fw",
            "UUULLLLLLUURUURUURFFFFFFFFFRRRRRRDDDLDDLDDLDDBBBBBBBBB",
        ),
        (
            "Fw2",
            "UUUDDDDDDLLRLLRLLRFFFFFFFFFUUUUUUDDDLRRLRRLRRBBBBBBBBB",
        ),
        (
            "Fw'",
            "UUURRRRRRDDRDDRDDRFFFFFFFFFLLLLLLDDDLUULUULUUBBBBBBBBB",
        ),
        (
            "Dw",
            "UUUUUUUUURRRFFFFFFFFFLLLLLLDDDDDDDDDLLLBBBBBBBBBRRRRRR",
        ),
        (
            "Dw2",
            "UUUUUUUUURRRLLLLLLFFFBBBBBBDDDDDDDDDLLLRRRRRRBBBFFFFFF",
        ),
        (
            "Dw'",
            "UUUUUUUUURRRBBBBBBFFFRRRRRRDDDDDDDDDLLLFFFFFFBBBLLLLLL",
        ),
        (
            "Lw",
            "BBUBBUBBURRRRRRRRRUUFUUFUUFFFDFFDFFDLLLLLLLLLBDDBDDBDD",
        ),
        (
            "Lw2",
            "DDUDDUDDURRRRRRRRRBBFBBFBBFUUDUUDUUDLLLLLLLLLBFFBFFBFF",
        ),
        (
            "Lw'",
            "FFUFFUFFURRRRRRRRRDDFDDFDDFBBDBBDBBDLLLLLLLLLBUUBUUBUU",
        ),
        (
            "Bw",
            "RRRRRRUUURDDRDDRDDFFFFFFFFFDDDLLLLLLUULUULUULBBBBBBBBB",
        ),
        (
            "Bw2",
            "DDDDDDUUURLLRLLRLLFFFFFFFFFDDDUUUUUURRLRRLRRLBBBBBBBBB",
        ),
        (
            "Bw'",
            "LLLLLLUUURUURUURUUFFFFFFFFFDDDRRRRRRDDLDDLDDLBBBBBBBBB",
        ),
        (
            "x",
            "FFFFFFFFFRRRRRRRRRDDDDDDDDDBBBBBBBBBLLLLLLLLLUUUUUUUUU",
        ),
        (
            "x2",
            "DDDDDDDDDRRRRRRRRRBBBBBBBBBUUUUUUUUULLLLLLLLLFFFFFFFFF",
        ),
        (
            "x'",
            "BBBBBBBBBRRRRRRRRRUUUUUUUUUFFFFFFFFFLLLLLLLLLDDDDDDDDD",
        ),
        (
            "y",
            "UUUUUUUUUBBBBBBBBBRRRRRRRRRDDDDDDDDDFFFFFFFFFLLLLLLLLL",
        ),
        (
            "y2",
            "UUUUUUUUULLLLLLLLLBBBBBBBBBDDDDDDDDDRRRRRRRRRFFFFFFFFF",
        ),
        (
            "y'",
            "UUUUUUUUUFFFFFFFFFLLLLLLLLLDDDDDDDDDBBBBBBBBBRRRRRRRRR",
        ),
        (
            "z",
            "LLLLLLLLLUUUUUUUUUFFFFFFFFFRRRRRRRRRDDDDDDDDDBBBBBBBBB",
        ),
        (
            "z2",
            "DDDDDDDDDLLLLLLLLLFFFFFFFFFUUUUUUUUURRRRRRRRRBBBBBBBBB",
        ),
        (
            "z'",
            "RRRRRRRRRDDDDDDDDDFFFFFFFFFLLLLLLLLLUUUUUUUUUBBBBBBBBB",
        ),
    ];
    for (algorithm, expected) in goldens {
        let parsed: cfop_solver::Algorithm = algorithm.parse().unwrap();
        let moved = Cube::solved().after_algorithm(&parsed);
        assert_eq!(
            moved.facelets(),
            expected,
            "move semantics changed {algorithm}"
        );
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
        "L' R' B2 U2 L' R L B U' F B' L2 B2 U2 B U' L2 R U2 F2",
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
                    >= adjacent[0].stats.f2l_candidates_evaluated
            );
            assert!(
                adjacent[1].stats.exact_target_searches >= adjacent[0].stats.exact_target_searches
            );
            assert!(
                adjacent[1].stats.last_layer_evaluations
                    >= adjacent[0].stats.last_layer_evaluations
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

#[test]
fn training_verifier_accepts_real_cfop_stage_boundaries() {
    let cube = Cube::from_scramble("D R2 B2 L2 F L B' R' L D F U' B2 D' L R2 F2 L2 F' B'").unwrap();
    let result = Solver::new(SolveConfig {
        cross_mode: CrossMode::XXCross,
        efficiency: Efficiency::Thorough,
        ..SolveConfig::default()
    })
    .solve(&cube)
    .unwrap();

    let mut moves = Vec::new();
    let mut boundaries = Vec::new();
    for stage in &result.stages {
        moves.extend(stage.algorithm.moves().iter().copied());
        boundaries.push(PhaseBoundary {
            stage: stage.stage,
            after_move: moves.len(),
            f2l_pairs: stage.f2l_pairs.clone(),
        });
    }
    let input = VerificationInput {
        id: Some("real-cfop".into()),
        state: cube.facelets(),
        moves: Algorithm::new(moves).to_string(),
        known_solutions: vec![result.solution.to_string()],
        phase_boundaries: boundaries,
    };
    let verified = TrainingVerifier::new(VerificationConfig::default())
        .unwrap()
        .verify(&input);
    assert!(verified.parse.valid);
    assert!(verified.solved);
    assert_eq!(
        verified.phase_invariants.status,
        PhaseInvariantStatus::Passed
    );
    assert!(verified.phase_invariants.first_violation.is_none());
    assert_eq!(verified.known_solutions.verified, 1);
}
