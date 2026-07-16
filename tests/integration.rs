use cfop_solver::{Cube, SolveConfig, SolveMethod, Solver, Stage};

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
        assert_eq!(
            result
                .stages
                .iter()
                .map(|stage| stage.stage)
                .collect::<Vec<_>>(),
            vec![Stage::Cross, Stage::F2l, Stage::Oll, Stage::Pll]
        );
        assert!(cube.after_algorithm(&result.solution).is_solved());
        for adjacent in result.stages.windows(2) {
            assert_eq!(adjacent[0].end_state, adjacent[1].start_state);
        }
    }
}
