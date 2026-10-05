# Corrections to the frozen logical-candidate instruments

> **Diátaxis Type:** Reference

The [candidate portfolio](portfolio.md) is a producing input the unroll pilot
receipts pin by digest, and keeps its bytes; the receipts hold their own
snapshot of the candidate launcher. This record quotes each statement in them
about the tree that holds the candidate bodies, states what the committed
evidence establishes, and gives the reading the other documents of the study
use.

No correction changes a cell, a margin, a limit, a decision rule or a recorded
outcome.

## C-01: the tree that holds the candidate bodies

Portfolio, "Profile decision":

> A compile-time experimental `--cfg` chooses the candidate body; ordinary
> builds retain the current body.

Portfolio, "Acceptance and code bounds":

> Candidate source is confined to the existing
> `gf2-kernels-simd/src/x86/avx2.rs` XOR body, with at most one new private
> target-feature XOR function and 80 added nonblank production lines.

**Narrowed.** The source that holds the two bodies is the `avx2.rs` each unroll
pilot receipt snapshots under its `inputs/producing` directory, beside the
manifest that declares the two configuration names. The current kernel source
and manifest name neither configuration. The
[reader record](../1a379447-zen3-cpu-performance/fa1d8733/survey/flag-readers.json)
lists each file that names one, with its class.

**Reading used.** "The candidate body" is the body in a pilot receipt's source
snapshot. On the current tree `--cfg gf2_xor_unroll2` and
`--cfg gf2_xor_unroll4` select nothing, and a build with either compiles the
established body.

**Decisions.** Unaffected: each receipt pins its producing sources per file and
its arm executables by digest, so no cell or verdict reads the current kernel
source.

## C-02: the launcher the receipts snapshot

Each pilot receipt snapshots the [candidate launcher](run-candidate.sh) it ran
under, `build_arms`:

> ```
> RUSTFLAGS="${flags}" CARGO_TARGET_DIR="${candidate_target}" \\
>     ./scripts/cargo-budget.sh cargo build --release --manifest-path "${MANIFEST}" \\
>     --bin logical-arm --bin logical-oracle
> ```

**Narrowed.** `flags` is `--cfg gf2_xor_unroll<factor>`, which selects a body
only in the kernel source of the same snapshot. The live launcher holds no
build: its `build` and `smoke` actions refuse, and its `window` and
`profile-window` actions refuse when their receipt or profile summary is
absent and otherwise check the committed one.

**Reading used.** The snapshotted launcher is the record of how the committed
pilots and candidate profiles were launched. The candidate evidence is the
pilot receipts with their snapshots, the candidate disassemblies and the
candidate profiles; the current tree builds no candidate executable.

**Decisions.** Unaffected, for the reason C-01 gives.
