# Contributing

Use Rust 1.85 or newer and Python 3.9 or newer. The checkout pins Rust 1.85.0
and Python 3.12 for development. Install Rust with rustup and install uv, then run:

```bash
uv sync --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo test --locked --doc
uv run --locked pytest
uv run --locked ruff check .
uv run --locked ruff format --check .
uv run --locked mypy
```

Rebuild the editable extension with `uv sync --reinstall-package cfop-solver`
after changing Rust code. A cached Python extension does not automatically rebuild
when its source changes.

For a bug report, include the version, platform, exact scramble or serialized state,
solver options, and expected/actual behavior. For generation, include the seed and
record index. Keep security reports private; see [SECURITY.md](SECURITY.md).

Changes to moves, cube parsing, search, or phase logic should have a regression that
independently replays the affected transitions. Check deterministic generation across
thread counts when changing ordering or randomness. The 54-character facelet state
is authoritative; feature arrays are derived representations. Version serialized
schemas when changing their meaning, and retain the default-disabled behavior of
optional instrumentation.

Do not commit generated datasets, build outputs, virtual environments, or credentials.
Preserve the provenance and licenses documented in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
when changing extracted engine code. Contributions are provided under the repository's
MIT license.
