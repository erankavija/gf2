# Kernel safety-contract verification (jit:7d44b71f)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95.0`. Script paths are relative to this directory. No
command takes a timing window, the host lock, or an ignored test.

## Anchor

The task anchor is identified by content. The
[anchor baseline](survey/anchor-baseline.json) holds the SHA-256 of every file
of `gf2-kernels-simd` at the anchor, and `survey/inputs/anchor/` holds the
anchor bytes of each path this task changes, checked against its baseline
digest on use. The anchor is the tree whose listings
`python3 -B dev/scripts/regen-asm-listings.py` regenerated from the unedited
sources, so both sides of the listing comparison come from one toolchain and
one procedure. `python3 -B survey/freeze-anchor.py <commit>` writes the
baseline and the snapshots; the commit id is an informational field that no
generator reads.

## Unsafe-boundary inventory (REQ-01, REQ-02)

`python3 -B survey/make-unsafe-inventory.py` writes the
[inventory](survey/unsafe-inventory.json): one row per `unsafe fn` and per
`unsafe` block of the package's Rust sources with its path, line, enclosing
item and whether its contract is present under the record's `rule`. The
`anchor` object holds the same rows for the anchor sources and `current` those
of the working tree; each carries its totals under `counts`. The script exits
nonzero while a working-tree row has `contract: false`;
`--self-test` checks the rule on a fixture.

Every `unsafe fn` row of `current` has `contract: true`. Two `unsafe` block
rows have `contract: false`: the calls of `avx2_m4rm_gray_build4` and
`avx2_m4rm_gray_build8` in `m4rm_gray_build4_fn` and `m4rm_gray_build8_fn`
(`crates/gf2-kernels-simd/src/x86/avx2.rs`). Both kernels store entry 0
unconditionally and index the table by a Gray code of `i < table_size`, which
stays below `table_size` only for a power of two; their `# Safety` sections
state that condition. The wrappers assert the stride and the two buffer
lengths and leave `table_size` unconstrained, and `M4rmGrayBuildFn` documents
no such caller condition, so no gate or documented invariant discharges the
contract and no comment is written there. REQ-02 is unmet for these two rows.

`cargo clippy -p gf2-kernels-simd --all-targets --all-features -- -D warnings
-W clippy::undocumented_unsafe_blocks` reports the same two blocks and no
other.

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
  sentence stating the condition their wrappers rely on.

## Instruction text (REQ-03)

`python3 -B survey/make-asm-comparison.py` writes the
[assembly comparison](survey/asm-comparison.json). Its `sources` list classes
every package path whose bytes differ from the anchor under the record's
`classifier_rule`; the script fails when a Rust source is not
`comment-or-blank-only`. Its `listings` list splits each listing of the anchor
and of the working tree per symbol and compares the instruction text under the
record's `comparison_rule`; `source_class_counts`, `symbol_count` and
`differing_symbol_count` total the record, and the script fails when a symbol
differs. Every symbol row carries `instruction_text: same`.

A listing with `regenerated: false` holds its anchor bytes:
`bipedal_avx512.asm.txt` records no symbol, and
`_lto_opacity_callsites.asm.txt` is produced from a `gf2-core` example outside
`regen-asm.sh`. `gf2m_common.asm.txt` records no symbol either, because every
function of that module is `#[inline(always)]` and is emitted inside the
`gf2m_batch` and `gf2m_gemm` kernels. A regenerated listing differs from its
anchor in its banner, which carries the commit and time of regeneration, and in
compiler-numbered local names.

The listings cover the symbols they name. For every other function the crate
emits, `python3 -B survey/make-crate-functions.py current` builds the package
with `--emit=asm`, digests each function's instruction text under the record's
`rule`, and joins the result with the
[anchor digests](survey/crate-functions-anchor.json) in the
[function comparison](survey/crate-function-comparison.json), whose
`function_count` and `differing_function_count` total it. Every row carries
`instruction_text: same`. `make-crate-functions.py anchor` writes the anchor
record and refuses to run unless every package file other than a listing holds
its baseline digest.

## Shared suites (REQ-03)

`survey/run-suites.sh` runs both rows and writes the logs. Each log opens with
the toolchain and command and closes with the exit status; its nextest
`Summary` line carries the test counts.

| Build | Command | Result |
|---|---|---|
| `simd` | `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-kernels-simd -p gf2-core -p gf2-algebra -p gf2-coding --all-features --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/simd-build.txt). |
| without `simd` | `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --no-default-features --features rand,io,parallel,visualization,tuning-profile,test-support --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/scalar-build.txt). |

The second row covers `gf2-core` alone and names each of its features except
`simd`. The test builds of `gf2-algebra` and `gf2-coding` enable their default
`simd` feature through their own `test-support` dev-dependency, so a test build
of either always carries the feature.

## Gates

`./scripts/asm-artefact-present.sh` exits 0 at the commit that edits the
kernel sources and reports each changed SIMD source covered by its listing;
the gate inspects the latest commit only and passes vacuously at later
commits. `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh` exits 0 from the
worktree root with every step reported `ok`.
