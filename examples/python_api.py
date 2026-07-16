import cfop_solver as cfop

cube = cfop.Cube.from_scramble("R U2 F' L D")
solver = cfop.Solver(
    method="cfop",
    trace=True,
    exploration=True,
    max_exploration_states=12,
    alternatives=2,
)
result = solver.solve(cube)

assert result["verified"]
assert cfop.verify_solution(cube.state, result["solution"])
print(result)
