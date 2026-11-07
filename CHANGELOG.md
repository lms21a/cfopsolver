# Changelog

## 0.5.0 - Unreleased

Public-release preparation after the private 0.4.0 milestone.

- Reject impossible corner sticker combinations that previously passed facelet
  validation and could be reconstructed as a different cube.
- Reject unmatched parentheses in algorithms and invalid search limits before search
  or generation output is opened. Python configuration errors raise `ValueError`.
- Reject unknown, duplicate, missing-value, and conflicting CLI options; add command
  help and version output.
- Upgrade PyO3 to 0.30.1 and rand to 0.8.6 to clear known advisories. Replace
  `static_init` with standard-library `LazyLock`, removing its unmaintained dependency.
- Raise the minimum Rust version from 1.70 to 1.85 and the minimum Python version
  from 3.8 to 3.9. Use the CPython 3.9 stable ABI for wheels.
- Add local checks for installed wheels, source distributions, and dependency
  notices, plus contribution, security, packaging, and release documentation.
- Include the full retained engine provenance and upstream MIT notices in
  distributed artifacts.

## 0.4.0 - Private milestone

- Add optional real search decisions, structured cube features, and the ordered
  batched training verifier with phase, pair, parse, and move-excess diagnostics.

## 0.3.2 - Private milestone

- Extract the required rcuber engine into the repository and remove the patched
  path dependency.

## 0.3.1 - Private milestone

- Improve throughput across the CFOP efficiency profiles.

## 0.3.0 - Private milestone

- Add configurable search efficiency, advanced crosses, and pair-level F2L traces.
