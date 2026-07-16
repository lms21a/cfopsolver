"""Run after installing the maturin wheel into the active Python environment."""

import tempfile

import cfop_solver as cfop


def main() -> None:
    cube = cfop.Cube.from_scramble("R U2 F' L D")
    assert cube.verify("D' L' F U2 R'")

    result = cfop.Solver(
        method="cfop",
        trace=True,
        exploration=True,
        max_exploration_states=8,
        alternatives=1,
    ).solve(cube)
    assert result["verified"]
    assert len(result["alternatives"]) == 1
    assert result["alternatives"][0]["verified"]
    assert [stage["stage"] for stage in result["stages"]] == [
        "cross",
        "f2l",
        "oll",
        "pll",
    ]
    assert cfop.verify_solution(cube.state, result["solution"])

    records = cfop.generate(
        10,
        seed=7,
        difficulty="easy",
        solutions_per_scramble=2,
        threads=2,
    )
    assert all(solution["verified"] for row in records for solution in row["solutions"])

    with tempfile.NamedTemporaryFile(suffix=".jsonl") as output:
        cfop.generate_jsonl(output.name, 12, seed=8, batch_size=5, threads=2)
        assert sum(1 for _ in open(output.name, encoding="utf-8")) == 12


if __name__ == "__main__":
    main()
