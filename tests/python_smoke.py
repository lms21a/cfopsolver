"""End-to-end smoke tests for the uv-managed Python extension."""

import tempfile
from typing import Any

import pytest

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
        decisions=True,
        max_decision_records=32,
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
    assert result["decisions"]["schema"] == "cfop-decisions/v1"
    assert 0 < len(result["decisions"]["records"]) <= 32
    assert any(record["kind"] == "candidate" for record in result["decisions"]["records"])
    pairs = [pair for stage in result["stages"] for pair in stage.get("f2l_pairs", [])]
    assert len(pairs) == len(set(pairs)) == 4
    assert all("moves" in event for event in result["trace"] if event["kind"] != "phase_start")
    assert cfop.verify_solution(cube.state, result["solution"])

    features = cube.features()
    assert features is not None
    assert features["schema"] == "cube-features/v1"
    assert len(features["facelet_ids"]) == 54
    assert len(cube.feature_vector("cubie_ids")) == 46
    assert len(cube.feature_vector("cubie_one_hot")) == 292
    assert cfop.CUBIE_FEATURE_WIDTH == 46
    assert cfop.CUBIE_ONE_HOT_WIDTH == 292
    assert len(cfop.CUBIE_FEATURE_CARDINALITIES) == 46
    with pytest.raises(ValueError):
        cube.features("not-an-encoding")

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
    assert records[0]["schema"] == "cfop-dataset/v3"
    assert "features" not in records[0]
    assert all(
        solution["stats"]["move_count"] == solution["move_count"]
        for row in records
        for solution in row["solutions"]
    )

    enriched = cfop.generate(1, difficulty="easy", decisions=True, features="both")
    assert enriched[0]["schema"] == "cfop-dataset/v4"
    assert enriched[0]["features"]["schema"] == "cube-features/v1"
    assert enriched[0]["solutions"][0]["decisions"]["schema"] == "cfop-decisions/v1"

    batch_items: list[dict[str, Any]] = [
        {
            "id": "valid",
            "state": cfop.Cube.from_scramble("R U F").state,
            "moves": "F' U' R'",
            "known_solutions": ["F' U' R'", "R"],
            "phase_boundaries": [{"stage": "fast", "after_move": 3}],
        },
        {"id": "bad-moves", "state": cfop.SOLVED_STATE, "moves": "not-a-move"},
        {
            "id": "bad-annotation",
            "state": cfop.SOLVED_STATE,
            "moves": "",
            "phase_boundaries": [{"stage": "not-a-stage", "after_move": 0}],
        },
        {"id": "bad-state", "state": "U" * 54, "moves": ""},
    ]
    verified = cfop.verify_batch(batch_items, threads=2, features="cubie")
    assert verified["schema"] == "cfop-verification/v1"
    assert [row["id"] for row in verified["results"]] == [
        "valid",
        "bad-moves",
        "bad-annotation",
        "bad-state",
    ]
    valid = verified["results"][0]
    assert valid["parse"]["valid"] and valid["solved"]
    assert valid["move_count"] == 3
    assert valid["known_solutions"]["best_move_count"] == 3
    assert valid["known_solutions"]["excess_moves"] == 0
    assert valid["phase_invariants"]["status"] == "passed"
    assert valid["features"]["cubie"] is not None
    assert not verified["results"][1]["parse"]["moves_valid"]
    assert not verified["results"][2]["parse"]["input_valid"]
    assert verified["results"][2]["parse"]["state_valid"]
    assert verified["results"][2]["solved"]
    assert verified["results"][2]["final_state"] == cfop.SOLVED_STATE
    assert verified["results"][2]["phase_invariants"]["status"] == "invalid"
    assert verified["results"][3]["parse"]["input_valid"]
    assert not verified["results"][3]["parse"]["state_valid"]

    reusable = cfop.TrainingVerifier(threads=2)
    assert reusable.verify_batch(batch_items)["results"][0]["solved"]

    with tempfile.NamedTemporaryFile(suffix=".jsonl") as output:
        cfop.generate_jsonl(output.name, 12, seed=8, batch_size=5, threads=2)
        assert sum(1 for _ in open(output.name, encoding="utf-8")) == 12


if __name__ == "__main__":
    test_python_api()
