"""End-to-end smoke tests for the uv-managed Python extension."""

import tempfile

import cfop_solver as cfop


def test_python_api() -> None:
    cube = cfop.Cube.from_scramble("R U2 F' L D")
    assert cube.verify("D' L' F U2 R'")

    result = cfop.Solver(
        method="cfop",
        cross="xcross",
        f2l="lookahead",
        efficiency="thorough",
        trace=True,
        exploration=True,
        max_exploration_states=8,
        alternatives=1,
    ).solve(cube)
    assert result["verified"]
    assert len(result["alternatives"]) == 1
    assert result["alternatives"][0]["verified"]
    assert result["stages"][0]["stage"] == "xcross"
    assert result["stages"][-2]["stage"] == "oll"
    assert result["stages"][-1]["stage"] == "pll"
    assert result["efficiency"] == "thorough"
    assert result["stats"]["f2l_candidates_evaluated"] > 0
    assert result["stats"]["exact_target_searches"] > 0
    assert result["stats"]["last_layer_evaluations"] > 0
    pairs = [pair for stage in result["stages"] for pair in stage.get("f2l_pairs", [])]
    assert len(pairs) == len(set(pairs)) == 4
    assert all("moves" in event for event in result["trace"] if event["kind"] != "phase_start")
    assert cfop.verify_solution(cube.state, result["solution"])

    records = cfop.generate(
        10,
        seed=7,
        difficulty="easy",
        solutions_per_scramble=2,
        method="cfop",
        threads=2,
        cross="xcross",
        efficiency="thorough",
        probe_limit=200_000,
    )
    assert all(solution["verified"] for row in records for solution in row["solutions"])
    assert all(
        solution["efficiency"] == "thorough" for row in records for solution in row["solutions"]
    )
    assert records[0]["solutions"][0]["stats"]["f2l_candidates_evaluated"] > 0
    assert records[0]["solutions"][0]["stats"]["exact_target_searches"] > 0
    assert all(
        solution["stats"]["move_count"] == solution["move_count"]
        for row in records
        for solution in row["solutions"]
    )

    with tempfile.NamedTemporaryFile(suffix=".jsonl") as output:
        cfop.generate_jsonl(output.name, 12, seed=8, batch_size=5, threads=2)
        assert sum(1 for _ in open(output.name, encoding="utf-8")) == 12


if __name__ == "__main__":
    test_python_api()
