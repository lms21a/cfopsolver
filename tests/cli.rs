use std::process::Command;

fn cfop(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cfop"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn cli_rejects_ambiguous_and_mistyped_options() {
    for (args, diagnostic) in [
        (vec!["solve", "--efficency", "fast"], "unknown option"),
        (vec!["solve", "--max-depth"], "requires a value"),
        (vec!["solve", "--trace", "false"], "does not take a value"),
        (
            vec!["solve", "--method", "fast", "--method", "cfop"],
            "duplicate option",
        ),
        (
            vec![
                "solve",
                "--state",
                cfop_solver::SOLVED_FACELETS,
                "--scramble",
                "R",
            ],
            "mutually exclusive",
        ),
        (
            vec!["solve", "--max-depth", "31"],
            "max_depth must be between",
        ),
    ] {
        let result = cfop(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(diagnostic),
            "{args:?}"
        );
    }
}

#[test]
fn cli_reports_version_help_and_verification_exit_codes() {
    let version = cfop(&["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
    assert!(cfop(&["solve", "--help"]).status.success());
    assert_eq!(
        cfop(&["verify", "--scramble", "R", "--solution", "R'"])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(
        cfop(&["verify", "--scramble", "R", "--solution", "U"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        cfop(&["verify", "--solution", "not-a-move"]).status.code(),
        Some(2)
    );
}

#[test]
fn invalid_generation_options_do_not_truncate_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("keep.jsonl");
    std::fs::write(&path, "existing data\n").unwrap();
    let result = cfop(&[
        "generate",
        "--output",
        path.to_str().unwrap(),
        "--max-depth",
        "31",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "existing data\n");
}
