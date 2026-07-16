import cfop_solver as cfop

cube = cfop.Cube.from_scramble("R U2 F' L D")
solver = cfop.Solver(
    method="cfop",
    cross="xxcross",
    f2l="lookahead",
    trace=True,
    exploration=True,
    max_exploration_states=12,
    alternatives=2,
)
result = solver.solve(cube)

assert result["verified"]
assert cfop.verify_solution(cube.state, result["solution"])
assert result["stages"][0]["stage"] == "xxcross"
print(result)
