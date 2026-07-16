# CFOP Solver

A Rust-first Rubik's Cube toolkit with Python bindings, a staged CFOP solver,
a faster two-phase solver, bounded decision/exploration traces, exact state
validation, solution verification, and deterministic parallel JSONL generation.

## What is included

- `Cube`: compact cubie state with exact center-frame tracking.
- One serialized state: 54 `URFDLB` facelets in standard U, R, F, D, L, B order.
- All face, slice, wide, and rotation moves (`U` through `B`, `M/E/S`, `Uw` through
  `Bw`, and `x/y/z`) with prime and double suffixes.
- Mutable, immutable, and stateless move-application APIs.
- A real `cross -> f2l -> oll -> pll` solving pipeline with independently replayed
  and checked phase invariants.
- A fast two-phase solver for high-throughput use.
- Toggleable structured decision traces and separately bounded exploratory states.
- Distinct alternative solutions and deterministic easy/medium/hard scramble generation.
- Parallel in-Rust generation and bounded-memory JSONL streaming.
- A CLI and a PyO3/maturin extension.

## Canonical state

The public state is always exactly 54 uppercase facelets. Positions are nine
row-major stickers for each spatial face in `U R F D L B` order. Sticker letters
identify pieces/colors, and centers are serialized too, so a state after `x`, `M`,
or `Rw` can be loaded again without losing its orientation.

```text
UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB
```

Dataset records declare this as `state_format: "facelets-urfdlb-v1"`. Parsing checks
length, alphabet, counts, unique cubies, corner twist, edge flip, the 24 legal center
frames, and combined corner/edge/center parity.

## Rust

```rust
use cfop_solver::{Cube, SolveConfig, Solver};

let cube = Cube::from_scramble("R U2 F' L D")?;

// Immutable application is convenient for verification.
let next = cube.after("D' L' F U2 R'")?;
assert!(next.is_solved());

// Mutable application avoids extra copies in a step loop.
let mut cursor = cube;
cursor.apply("D' L'")?.apply("F U2 R'")?;
assert!(cursor.is_solved());

let result = Solver::new(SolveConfig {
    trace: true,
    exploration: true,
    alternatives: 2,
    ..SolveConfig::default()
}).solve(&cube)?;

assert!(result.verified);
println!("{}", result.solution);
# Ok::<(), cfop_solver::Error>(())
```

`SolveMethod::Cfop` is the default and preserves phase boundaries. Use
`SolveMethod::Fast` when throughput and shorter solutions matter more than CFOP
interpretability. Every primary and alternative solution is reapplied to the original
state before it is returned. Alternative entries include `source`, `algorithm`,
`move_count`, and `verified`; if continued search cannot produce the requested count,
the solver fills it with explicitly labeled `verified_identity_variant` results rather
than silently returning fewer solutions.

## Python

The Python project and its development tools are managed by
[uv](https://docs.astral.sh/uv/). The first command creates or updates `.venv`,
installs the extension in editable mode, and syncs the versions in `uv.lock`:

```bash
uv sync
uv run python examples/python_api.py
```

The environment remains an ordinary virtual environment, so direct execution works too:

```bash
.venv/bin/python examples/python_api.py
```

```python
import cfop_solver as cfop

cube = cfop.Cube.from_scramble("R U2 F' L D")
assert cube.verify("D' L' F U2 R'")

solver = cfop.Solver(
    method="cfop",
    trace=True,
    exploration=True,
    max_exploration_states=64,
    alternatives=2,
)
result = solver.solve(cube)
assert result["verified"]

# Stateless verification is handy for model outputs.
assert cfop.verify_solution(cube.state, result["solution"])
next_state = cfop.apply_moves(cube.state, "R U")
```

Long solve and generation operations release the Python GIL. Results are plain
dict/list/string values and are immediately JSON-serializable.

## Decision and exploration output

Tracing is off by default. `trace=True` emits phase starts, the selected algorithm,
the reason for the decision, phase completion, timings, and optional canonical states.

Exploration is a separate option because it can be much larger. When enabled, the
solver records bounded one-ply alternatives for the stage, their move, resulting state,
progress score, and whether the branch was selected or backtracked. Set
`max_exploration_states` to enforce the desired cap. No exploratory state strings are
created on the disabled path.

Each phase has `start_state`, `end_state`, `algorithm`, `move_count`, timing, and a
`repaired` marker. The marker is set only if a mutable upstream stage diverges from its
returned move list; in that case the same phase endpoint is reconstructed by an
independent search and replayed before being returned.

## Bulk generation

Python, in memory:

```python
records = cfop.generate(
    10_000,
    seed=42,
    difficulty="hard",
    solutions_per_scramble=2,
    method="fast",
    threads=8,
)
```

Python, streaming JSONL from Rust:

```python
cfop.generate_jsonl(
    "train.jsonl",
    1_000_000,
    seed=42,
    solutions_per_scramble=2,
    threads=8,
    batch_size=4096,
)
```

CLI:

```bash
cargo run --release -- generate \
  --count 100000 --seed 42 --difficulty hard \
  --solutions 2 --method fast --threads 8 --output train.jsonl
```

Record seeds derive only from `(root_seed, global_index)`, so output order and cube/
solution content remain stable across thread counts. Timing fields are zeroed in
datasets to keep output deterministic. With two requested solutions, generation uses
the fast solver result plus the independently verified inverse of the known scramble;
requests beyond two additionally use continued solution search, with verified identity
variants only as an explicitly labeled final fallback.

## CLI

```bash
cfop solve --scramble "R U F" --method cfop --trace --explore --json
cfop apply --state "$STATE" --moves "R U R' U'"
cfop verify --scramble "R U F" --solution "F' U' R'"
cfop generate --count 1000 --output data.jsonl --solutions 2 --threads 4
```

## Performance snapshot

On the development machine, the release binary generated and verified 1,000 hard
records with two solutions each in 0.53 seconds on four threads (about 1,900 records/s).
A 100-scramble staged-CFOP stress run finished in 0.06 seconds on four threads, with
all phase boundaries and final solutions replay-verified. Treat these as a local
snapshot; hardware and scramble/alternative settings materially affect throughput.

## Development

```bash
uv sync
uv lock --check
uv run pytest
uv run ruff check .
uv run ruff format --check .
uv run mypy
uv build --wheel

cargo test
cargo check --features python
```

`uv.lock` controls Python build and development dependencies; `Cargo.lock` controls
Rust dependencies. The checkout currently targets Rust 1.70, publishes an abi3 wheel
for Python 3.8+, and uses Python 3.12 for local development via `.python-version`.
