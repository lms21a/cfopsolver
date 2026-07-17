# CFOP Solver

A Rust-first Rubik's Cube toolkit with Python bindings, a staged CFOP solver,
a faster two-phase solver, real bounded search decisions, optional numeric cube
features, batch training verification, exact state validation, and deterministic
parallel JSONL generation.

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
- Toggleable real search-decision records and separately bounded counterfactual states.
- Optional exact-frame cubie/facelet feature tensors with stable categorical IDs.
- A reusable ordered batch verifier for RL rewards and verifier-model training.
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
    decisions: true,
    max_decision_records: 512,
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
    decisions=True,
    max_decision_records=512,
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

## Trace, real decisions, and exploration output

Tracing is off by default. `trace=True` emits phase starts, the selected moves,
the reason for the decision, phase completion, timings, and optional canonical states.
F2L emits a separate `f2l_pair` phase for every remaining pair; each event identifies
the exact `f2l_pairs` involved. Pairs already completed by an advanced cross are listed
on that opening phase.

`decisions=True` adds the actual choices made inside solver search as
`cfop-decisions/v1`. Records cover F2L variants and beam candidates, accepted/rejected
AUF routes, duplicate and beam pruning, terminal last-layer scoring, losing-branch
backtracking, exact-target attempts, committed phases, and fast two-phase selection.
Every record has deterministic IDs and provenance parents, typed
domain/kind/outcome/reason fields, the relevant moves/state, and structured scores.
`parent_id` explains which prior decision produced the branch; it is not necessarily a
state-transition edge. Records with moves carry an exact `start_state`, and replaying
those moves from it produces the recorded `state`. `max_decision_records` bounds only
the observation: search continues normally, while `truncated` and `dropped_records`
report anything omitted. Small caps reserve room for final selections and phase
commits. No wall-clock values enter this stream, so seeded datasets remain deterministic.
Record IDs may contain gaps when a capped stream evicts earlier low-priority records.

`exploration=True` retains the older bounded one-ply counterfactual snapshot for each
stage. It is useful for local what-if states, but it is not a claim that search actually
visited every listed branch; use `decisions` when real search provenance matters. Set
`max_exploration_states` to cap it. Neither path builds state strings while disabled.

Each phase has `start_state`, `end_state`, `moves`, `move_count`, timing,
`f2l_pairs`, `optimized`, and a `repaired` marker. Trace decisions and completions use
the same `moves` key, so phase actions do not need to be recovered from prose. The
`repaired` marker is set only if a mutable upstream stage diverges from its returned
move list; `optimized` means exact-target search found a shorter route to the same
verified endpoint. Default and phase-traced dataset records remain
`schema: "cfop-dataset/v3"`; decision- or feature-enriched records declare v4.

Solve results expose `stats.f2l_candidates_evaluated`,
`stats.exact_target_searches`, and `stats.last_layer_evaluations`, making the cost of
an efficiency profile visible instead of encoding it only in elapsed time. These are
unique physical evaluations after per-solve cache hits. Generated primary solutions
retain the counters with wall-clock timing zeroed for deterministic output.

## Structured cube features

The 54-character `facelets-urfdlb-v1` string remains the only authoritative serialized
state. Numeric features are derived, versioned, and opt-in:

| Encoding | Shape | Meaning |
|---|---:|---|
| `cubie_ids` | 46 | centers, corner permutation/orientation, edge permutation/orientation |
| `cubie_one_hot` | 292 | factorized one-hot form of those 46 categorical values |
| `facelet_ids` | 54 | `U R F D L B` mapped to `0..5` in facelet order |
| `facelet_one_hot` | 324 | six-way one-hot facelets |

`cubie-factorized-urfdlb-v1` fixes the flat offsets at centers `0..6`, corner
permutation `6..14`, corner orientation `14..22`, edge permutation `22..34`, and
edge orientation `34..46`. Center/color IDs are `U R F D L B = 0..5`; corner
positions and IDs are `URF UFL ULB UBR DFR DLF DBL DRB = 0..7`; edge positions and
IDs are `UR UF UL UB DR DF DL DB FR FL BL BR = 0..11`. Orientations use `0..2` for
corners and `0..1` for edges. Feature payloads include their format identifiers, and
the same widths/cardinalities are exported as Python module constants.

All encodings preserve the exact spatial frame, including `x`, slice, and wide moves;
they never silently rotate the state. The accompanying `cfop_progress` reports
`canonical_orientation`, cross edges, solved F2L pair identities, last-layer oriented
and permuted cubie counts, and the highest sequential milestone.

```python
cube = cfop.Cube.from_scramble("x R U F")
features = cube.features("both")             # cube-features/v1
cubie_ids = cube.feature_vector("cubie_ids") # 46 integers
facelet_ids = cfop.cube_features(cube.state, "facelet_ids")
```

Generation accepts `features="cubie"`, `"facelet_ids"`, or `"both"`. Disabled output
remains byte-compatible `cfop-dataset/v3`; feature- or decision-enabled records use
`cfop-dataset/v4` and attach features once at the sample level.

## Training verifier

`TrainingVerifier` owns and reuses one Rayon pool. A whole Python list crosses the FFI
boundary once, is evaluated in parallel in Rust, and returns in input order. Bad states,
move strings, field types, phase names, missing fields, and unknown/typoed fields become
per-item parse diagnostics instead of aborting neighboring samples.

```python
verifier = cfop.TrainingVerifier(threads=8, features="cubie")
batch = verifier.verify_batch([
    {
        "id": "rollout-0",
        "state": state,
        "moves": candidate,
        "known_solutions": [known_solution],
        "phase_boundaries": [
            {"stage": "cross", "after_move": 7},
            {"stage": "f2l_pair", "after_move": 15, "f2l_pairs": ["fr"]},
        ],
    },
    {"id": "malformed", "state": state, "moves": "not-a-move"},
])
```

Each result includes parse validity, exact final state, physical solved status, literal
move count, Cross/F2L/OLL/PLL progress, preserved/broken/gained F2L pairs, the first
phase-invariant violation, and signed move excess versus the shortest parsed and
verified known solution. Negative excess means the candidate found a shorter solution;
unsolved candidates report `null` rather than receiving a misleading favorable score.
When boundaries are supplied, phase invariants are checked at each declaration and once
at the end, not after every atomic move, because valid algorithms may temporarily
disturb a pair inside a phase. Without boundaries the invariant status is
`not_provided`. The convenience `cfop.verify_batch(...)` builds a pool for one call;
reuse the class for repeated training batches.

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
    decisions=True,
    max_decision_records=512,
    features="cubie",
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
cfop features --scramble "x R U F" --encoding both
cfop generate --count 1000 --output data.jsonl --method cfop \
  --efficiency thorough --decisions --features cubie \
  --probe-limit 500000 --solutions 2 --threads 4
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

The default-disabled v0.4.0 instrumentation was checked against those saved v0.3.2
artifacts: all four 1,000-record profile JSONL files remained byte-identical, and a
warmed fast-profile run took 0.69 s. For the new verifier, three end-to-end Python runs
over repeated three-move candidates (eight verifier threads, including Python JSON
conversion) measured median throughput of about 128,000 samples/s without features
and 81,100 samples/s with cubie features. These simple-candidate figures are a local
throughput sanity check, not a proxy for every rollout distribution.

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
