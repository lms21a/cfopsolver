# Local rcuber patch

This directory vendors `rcuber` 0.7.20, pinned by the root `Cargo.lock`.

The only functional change from the published crate is
`F2LSolver::solve_to_next_pair_boundary` in `src/solver/cfop/f2l.rs`. It follows the
same pair ordering and algorithms as `F2LSolver::solve`, but returns as soon as the
first replayable cumulative pair boundary is reached. CFOPSolver uses this only for
the `fast` efficiency route, where solving every later pair would be discarded by the
pair-level trace planner.

All other files are the unmodified crates.io 0.7.20 source distribution.
