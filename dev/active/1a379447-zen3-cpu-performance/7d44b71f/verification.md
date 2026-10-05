# Kernel safety-contract verification (jit:7d44b71f)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95.0`. Script paths are relative to this directory. No
command takes a timing window, the host lock, or an ignored test.

## Baselines

Four trees are identified by content; `survey/locate.py` names them. Each
`survey/<stage>-baseline.json` holds the SHA-256 of every file of
`gf2-kernels-simd` in that tree, and `survey/inputs/<stage>/` holds the bytes
of each path the working tree changes, checked against its baseline digest on
use. `python3 -B survey/freeze-anchor.py <stage> <commit>` writes a baseline
and its snapshots; the commit id is an informational field that no generator
reads.

- [`anchor`](survey/anchor-baseline.json): the unedited sources, with every
  listing regenerated from them by
  `python3 -B dev/scripts/regen-asm-listings.py`, so both sides of a listing
  comparison come from one toolchain and one procedure.
- [`before-1b034786`](survey/before-1b034786-baseline.json): the tree holding
  this task's contracts for every boundary except the two M4RM Gray-build
  wrapper calls.
- [`after-1b034786`](survey/after-1b034786-baseline.json): the tree the code
  change of jit:1b034786 leaves, in which those two wrappers assert the
  conditions their kernels need.
- [`contracts-complete`](survey/contracts-complete-baseline.json): that tree
  with the contracts of the two wrapper calls.

This task's change is two steps: `anchor` to `before-1b034786`, and
`after-1b034786` to `contracts-complete`. The step between them is the code change
of jit:1b034786, which its own
[record](../1b034786/verification.md) covers; the comparisons below leave it
out and state so in their `excluded_step` field. Changes after
`contracts-complete` are outside them as well; the inventory alone reads the
working tree.

## Unsafe-boundary inventory (REQ-01, REQ-02)

`python3 -B survey/make-unsafe-inventory.py` writes the
[inventory](survey/unsafe-inventory.json): one row per `unsafe fn` and per
`unsafe` block of the package's Rust sources with its path, line, enclosing
item and whether its contract is present under the record's `rule`. The
`anchor` object holds the same rows for the anchor sources and `current` those
of the working tree; each carries its totals under `counts`. The script exits
nonzero while a working-tree row has `contract: false`;
`--self-test` checks the rule on a fixture.

Every `unsafe fn` row and every `unsafe` block row of `current` has
`contract: true`. The calls of `avx2_m4rm_gray_build4` and
`avx2_m4rm_gray_build8` in `m4rm_gray_build4_fn` and `m4rm_gray_build8_fn`
take their comment in the second step: both kernels store entry 0
unconditionally and index the table by a Gray code of `i < table_size`, which
stays below `table_size` only for a power of two, and the wrappers assert that
condition from jit:1b034786 on.

`cargo clippy -p gf2-kernels-simd --all-targets --all-features -- -D warnings
-W clippy::undocumented_unsafe_blocks` exits 0.

A `// SAFETY:` comment names one of three discharges:

- the detection function that publishes the calling wrapper or bundle, for the
  target features;
- the feature probe at the top of a test, or the helper that runs the test
  closure after it. The GF(2^m) tests probe `avx2` and `vpclmulqdq`, some
  with `pclmulqdq`; their comments cite the target-feature hierarchy, under
  which a function enabling those features calls one requiring `pclmulqdq`
  and `sse4.1` without `unsafe`. `survey/check-feature-hierarchy.sh` compiles
  [that probe](survey/feature-hierarchy.rs) and records the
  [result](survey/feature-hierarchy.txt);
- the caller condition documented on the published function-pointer type,
  where the kernel leaves a length to its caller: `SmallPrimeSpmmRowFn`,
  `MediumPrimeSpmmRowFn`, `SmallPrimePlePanelBaseFn`,
  `MediumPrimePlePanelBaseFn`, and the bipedal, F_5 and F_7 kernel types.
  `MediumPrimeSpmmRowFn`, `F5UnaryKernelFn` and `F7UnaryKernelFn` gain the
  sentence stating the condition their wrappers rely on. jit:bfc2ceab owns
  these caller-trusted entry points.

`avx2_shift_left_words` and `avx2_shift_right_words` alternate between `buf`
and a raw pointer derived from it earlier; their contracts place no aliasing
obligation on the caller.

## Instruction text (REQ-03)

`python3 -B survey/make-asm-comparison.py` writes the
[assembly comparison](survey/asm-comparison.json), one entry of `steps` per
step. A step's `sources` list classes every package path whose bytes differ
across it under the record's `classifier_rule`; the script fails when a Rust
source is not `comment-or-blank-only`. Its `listings` list splits each listing
of either side per symbol and compares the instruction text under the record's
`comparison_rule`; `source_class_counts`, `symbol_count` and
`differing_symbol_count` total the step, and the script fails when a symbol
differs. Every symbol row of both steps carries `instruction_text: same`.

A listing with `regenerated: false` holds the bytes of its step's first tree. In the first step these are
`bipedal_avx512.asm.txt`, which records no symbol, and
`_lto_opacity_callsites.asm.txt`. That listing is exempt from regeneration: its
banner records rustc 1.95.0 and an extraction from the assembly of the `gf2-core`
example `lto_opacity_audit`, its two symbols are `gf2-core` call sites and not
functions of this package, `regen-asm.sh` does not write it, and
`asm-artefact-present.sh` maps no kernel source to it. Its rows compare equal
bytes and show nothing about this change. `gf2m_common.asm.txt` records no symbol either, because every
function of that module is `#[inline(always)]` and is emitted inside the
`gf2m_batch` and `gf2m_gemm` kernels. The second step regenerates the `avx2` listing
alone. A regenerated listing differs from its
predecessor in its banner, which carries the commit and time of regeneration, and in
compiler-numbered local names.

The listings cover the symbols they name. For every other function the crate
emits, `python3 -B survey/make-crate-functions.py freeze <stage>` builds the
package with `--emit=asm` on a tree that holds that baseline's digests and
writes the digest of each function's instruction text under the record's
`rule`. `make-crate-functions.py compare` checks each of the four records
against its baseline's source digests and joins them across the same two steps
in the [function comparison](survey/crate-function-comparison.json); each
step's `function_count` and `differing_function_count` total it. Every row of
both steps carries `instruction_text: same`.

## Shared suites (REQ-03)

`dev/scripts/run-kernel-suites.sh <this directory>/survey/test-logs` runs both
rows and writes the logs. Each log opens with
the toolchain and command and closes with the exit status; its nextest
`Summary` line carries the test counts.

| Build | Command | Result |
|---|---|---|
| `simd` | `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-kernels-simd -p gf2-core -p gf2-algebra -p gf2-coding --all-features --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/simd-build.txt). |
| without `simd` | `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --no-default-features --features rand,io,parallel,visualization,tuning-profile,test-support --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/scalar-build.txt). |

The second row covers `gf2-core` alone and names each of its features except
`simd`. The test builds of `gf2-algebra` and `gf2-coding` enable their default
`simd` feature through their own `test-support` dev-dependency, so a test build
of either always carries the feature (jit:dd359005).

## Gates

`./scripts/asm-artefact-present.sh` exits 0 at each of the two commits that
edit the kernel sources and reports each changed SIMD source covered by its
listing;
the gate inspects the latest commit only and passes vacuously at later
commits. `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh` exits 0 from the
worktree root with every step reported `ok`.
