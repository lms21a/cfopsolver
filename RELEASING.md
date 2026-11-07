# Release checklist

The repository can be prepared and tested while private. Changing its visibility,
publishing packages, and creating a release are separate maintainer actions.

1. Confirm `Cargo.toml`, `pyproject.toml`, and both lockfiles have the intended version.
   Set the changelog entry's date when publishing. Verify that the PyPI and crates.io
   names are available to the publishing account; package names are not reserved by
   this repository.
2. Read [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), preserve its notices, and run
   a full-history secret scan before first making the repository public. The
   preparation scan used Gitleaks 8.30.1 with no findings; rerun it after later changes.
3. Require a green `CI` workflow on the exact commit. It checks the Rust minimum and
   current toolchains, Python versions, Linux/macOS/Windows, dependency advisories,
   wheel contents, installed wheels, and a source-distribution rebuild. Review any
   platform failures before claiming that platform is supported.
4. Run the local checks in [CONTRIBUTING.md](CONTRIBUTING.md), then build into a clean
   output directory:

   ```bash
   uv lock --check
   uv build --wheel --sdist --out-dir dist/release
   uv run --locked python scripts/check_distributions.py dist/release
   uv run --locked python scripts/test_wheel.py dist/release
   cargo package --locked
   ```

5. Review the archives. They must contain licenses and third-party notices, and no
   virtual environments, generated corpora, secrets, or build caches. Rebuild the
   sdist in a clean directory. Validate PyPI metadata with `twine check` and use
   `maturin build --release --compatibility manylinux2014` in an appropriate manylinux
   builder for Linux wheels. Local `linux_x86_64` wheels are local test artifacts and
   are not portable PyPI Linux wheels.
6. Enable GitHub private vulnerability reporting and repository security features
   where available. Configure required CI checks and review protections for `main`.
7. When authorized, change repository visibility and publish the reviewed version.
   Use PyPI trusted publishing or an appropriately scoped token, and `cargo publish
   --locked`. Publish wheels for the platforms being advertised, together with the
   sdist. Never put credentials in the repository or workflow files.
8. Tag the exact reviewed commit as `v<version>`, create release notes from the
   changelog, and verify a fresh install from each public registry after publishing.

The CI workflow only builds, tests, audits, and uploads workflow artifacts.
It does not publish to PyPI/crates.io, create a GitHub release, or change visibility.
