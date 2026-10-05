# Producing-closure check modes (81afdf8e)

`make-external-producing-inputs.py` and `make-shift-producing-inputs.py` take `--check [output]`: it writes nothing and exits non-zero, listing the paths in which the closure at `output` (default: the family's committed closure) differs from the tree. Both write the closure without the option, to the default path or to the first positional argument, as the launchers call them. The shared mechanics live in `dev/scripts/producing_closure.py`.

`check-family-producing-closures.py` selects both by the option and reports the number of closures checked; its `--self-test` stages each generator in a fixture repository, adds an enumerated source after the closure is written, and asserts that the check rejects the closure and that no check leaves the fixture changed.

## Closure state before the change

Each closure lacked sources that entered the tree after the closure was written. The paths below are the regenerated closures' additions.

| Closure | Added source | Entered tree in |
|---|---|---|
| external | `crates/gf2-kernels-simd/src/x86/popcount.rs` | `cfa318418` |
| external, shift | `crates/gf2-kernels-simd/src/shift_funnel.rs`, `crates/gf2-kernels-simd/src/x86/shift_funnel.rs` | `e2e3d8ff3` |
| external, shift | `dev/tools/tuning-campaign-support/src/arm.rs` | `6ec3afbf3` |
| external, shift | `dev/tools/tuning-campaign-support/src/scratch.rs` | `a4876dede` |
| external, shift | `dev/tools/tuning-campaign-support/src/repository.rs` | `bd270c1c6` |
| external, shift | `crates/gf2-kernels-simd/src/m4rm.rs` | `6d03804a2` |
| shift | `crates/gf2-core/src/residual_shift.rs` | `e2e3d8ff3` |
| shift | `crates/gf2-core/src/gf2m/byte_table.rs` | `47ff752cf` |
| shift | `crates/gf2-core/src/test_scratch.rs` | `a4876dede` |
| shift | `crates/gf2-core/src/dispatch_contract.rs` | `fccfcc6e4` |

Both closures also gain `dev/scripts/producing_closure.py` and `dev/scripts/repository_files.py`, which the generators import.

## Committed evidence

A receipt snapshots the closure and generator bytes its campaign ran with under its own `inputs/producing/`; those snapshots and the receipts keep their bytes. The regenerated closures apply to campaigns launched after this change.
