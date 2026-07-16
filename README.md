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
- Configurable `cross`, `xcross`, `xxcross`, and `xxxcross` openings followed by
  independently traced F2L pair insertions, OLL, and PLL.
- Standard and dynamic-lookahead F2L strategies with independently replayed and
  checked phase invariants.
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
use cfop_solver::{CrossMode, Cube, Efficiency, F2lStrategy, SolveConfig, Solver};

let cube = Cube::from_scramble("R U2 F' L D")?;

// Immutable application is convenient for verification.
let next = cube.after("D' L' F U2 R'")?;
assert!(next.is_solved());

// Mutable application avoids extra copies in a step loop.
let mut cursor = cube;
cursor.apply("D' L'")?.apply("F U2 R'")?;
assert!(cursor.is_solved());

let result = Solver::new(SolveConfig {
    cross_mode: CrossMode::XXCross,
    f2l_strategy: F2lStrategy::Lookahead,
    efficiency: Efficiency::Thorough,
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

`CrossMode::XCross`, `XXCross`, and `XXXCross` require the opening phase to finish
the bottom cross together with at least one, two, or three F2L pairs. Exact-target
search may shorten that combined phase while preserving its chosen endpoint.
`F2lStrategy::Lookahead` evaluates alternative U-face alignments before each pair and
is the default; `Standard` keeps the simpler deterministic route.

`Efficiency` controls how much additional search is spent reducing moves. Every level
retains the plans considered by lower levels. Candidate plans are exact-shortened and
scored as complete, simplified CFOP solutions before selection, so increasing the
profile cannot discard the best lower-profile route already found:

| Profile | F2L planning | Exact-target optimization |
|---|---|---|
| `Fast` | Single deterministic route | Disabled |
| `Balanced` | Adds four-AUF greedy lookahead | Up to 100,000 probes |
| `Thorough` | Adds a four-state beam | Up to 500,000 probes |
| `Max` | Adds a 64-state pair beam | Up to `probe_limit` |

`Balanced` is the default. `probe_limit` remains the hard ceiling, so lowering it also
caps the thorough and max profiles. The profile affects staged CFOP solving;
`SolveMethod::Fast` continues to use its existing two-phase probe settings. AUF and
beam expansion require `F2lStrategy::Lookahead`; with `Standard`, higher profiles only
increase exact-target budgets. Serialized `efficiency` values record the requested
configuration, including on non-primary fast alternatives where the profile is not
applied.

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
    cross="xxcross",
    f2l="lookahead",
    efficiency="thorough",
    probe_limit=500_000,
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

Tracing is off by default. `trace=True` emits phase starts, the selected moves,
the reason for the decision, phase completion, timings, and optional canonical states.
F2L emits a separate `f2l_pair` phase for every remaining pair; each event identifies
the exact `f2l_pairs` involved. Pairs already completed by an advanced cross are listed
on that opening phase.

Exploration is a separate option because it can be much larger. When enabled, the
solver records bounded one-ply alternatives for the stage, their move, resulting state,
progress score, and whether the branch was selected or backtracked. Set
`max_exploration_states` to enforce the desired cap. No exploratory state strings are
created on the disabled path.

Each phase has `start_state`, `end_state`, `moves`, `move_count`, timing,
`f2l_pairs`, `optimized`, and a `repaired` marker. Trace decisions and completions use
the same `moves` key, so phase actions do not need to be recovered from prose. The
`repaired` marker is set only if a mutable upstream stage diverges from its returned
move list; `optimized` means exact-target search found a shorter route to the same
verified endpoint. Dataset records using this expanded structure declare
`schema: "cfop-dataset/v3"`.

Solve results expose `stats.f2l_candidates_evaluated`,
`stats.exact_target_searches`, and `stats.last_layer_evaluations`, making the cost of
an efficiency profile visible instead of encoding it only in elapsed time. These are
unique physical evaluations after per-solve cache hits. Generated primary solutions
retain the counters with wall-clock timing zeroed for deterministic output.

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

For traced CFOP training data, the same options are available during generation:

```python
records = cfop.generate(
    10_000,
    seed=42,
    method="cfop",
    cross="xxcross",
    f2l="lookahead",
    efficiency="thorough",
    trace=True,
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
cfop solve --scramble "R U F" --method cfop \
  --cross xxcross --f2l lookahead --efficiency thorough --trace --explore --json
cfop apply --state "$STATE" --moves "R U R' U'"
cfop verify --scramble "R U F" --solution "F' U' R'"
cfop generate --count 1000 --output data.jsonl --method cfop \
  --efficiency thorough --probe-limit 500000 --solutions 2 --threads 4
```

## Performance snapshot

On the development machine, three release runs over the same 1,000 hard, 20-move
scrambles (`seed=918273`) using CFOP, XXX-cross, lookahead F2L, four generation
threads, batches of 128, and no trace produced:

| Efficiency | Mean solution | v0.3.0 | v0.3.1 median | Throughput | Speedup |
|---|---:|---:|---:|---:|---:|
| `fast` | 64.17 moves | 0.77 s | 0.72 s | ~1,390 records/s | 1.07x |
| `balanced` | 50.62 moves | 2.47 s | 1.90 s | ~526 records/s | 1.30x |
| `thorough` | 48.72 moves | 6.16 s | 2.27 s | ~441 records/s | 2.71x |
| `max` | 47.99 moves | 11.85 s | 2.45 s | ~408 records/s | 4.84x |

All 4,000 profile results verified, every higher profile was non-regressing on every
scramble, and each profile's JSONL output was byte-identical across the three runs.
Treat these as a local snapshot; hardware, selected cross mode, trace output, and
probe limits materially affect throughput.

The v0.3.2 private-engine extraction reproduced all four v0.3.1 JSONL files
byte-for-byte. Three default-feature release runs measured median times of 0.72 s,
1.86 s, 2.25 s, and 2.54 s respectively, keeping throughput within normal local
variance while removing the patched path dependency. A separate 1,000-record direct
two-phase corpus and a repeated-alternative corpus also matched v0.3.1 byte-for-byte.

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
The private cube engine retains the required MIT-licensed portions of `rcuber` 0.7.20
inside `src/engine/`, including the pair-boundary performance extension. Unused solver
families and packaging code are excluded; see `THIRD_PARTY_NOTICES.md` for attribution.
