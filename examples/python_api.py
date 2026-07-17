import cfop_solver as cfop

cube = cfop.Cube.from_scramble("R U2 F' L D")
solver = cfop.Solver(
    method="cfop",
    cross="xxcross",
    f2l="lookahead",
    efficiency="thorough",
    trace=True,
    exploration=True,
    max_exploration_states=12,
    decisions=True,
    max_decision_records=128,
    alternatives=2,
)
result = solver.solve(cube)

assert result["verified"]
assert cfop.verify_solution(cube.state, result["solution"])
assert result["stages"][0]["stage"] == "xxcross"

features = cube.features("both")
assert features is not None

candidate = " ".join(stage["moves"] for stage in result["stages"])
offset = 0
boundaries = []
for stage in result["stages"]:
    offset += stage["move_count"]
    boundaries.append(
        {
            "stage": stage["stage"],
            "after_move": offset,
            "f2l_pairs": stage.get("f2l_pairs", []),
        }
    )

verification = cfop.verify_batch(
    [
        {
            "id": "demo",
            "state": cube.state,
            "moves": candidate,
            "known_solutions": [result["solution"]],
            "phase_boundaries": boundaries,
        }
    ],
    threads=2,
    features="cubie",
)["results"][0]

print("solution:", result["solution"])
print("stages:", [(stage["stage"], stage["moves"]) for stage in result["stages"]])
print("decision records:", len(result["decisions"]["records"]))
print("cubie IDs:", cube.feature_vector("cubie_ids"))
print("verification:", verification)
