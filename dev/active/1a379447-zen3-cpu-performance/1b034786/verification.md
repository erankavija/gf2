# M4RM Gray-table wrapper argument checks (jit:1b034786)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95.0`. No command takes a timing window, the host lock, or
an ignored test.

## Builders

The workspace builds a flat Gray table in three places, and one check,
`gf2_kernels_simd::m4rm::assert_gray_build_lengths`, runs in each before its
first store:

- `gf2_kernels_simd::m4rm_gray_build_scalar`, the portable reference of type
  `M4rmGrayBuildFn` for any stride;
- `m4rm_gray_build4_fn` and `m4rm_gray_build8_fn`, the specialized builders
  `LogicalFns` publishes (`crates/gf2-kernels-simd/src/x86/avx2.rs`);
- `gf2_core::alg::m4rm::build_gray_table_flat`, which takes the table as
  `2^k_block` entries over the rows of a matrix and dispatches to the
  specialized builders or to its own bounds-checked walks. It rejects a
  `k_block` whose power overflows `usize` before the shared check.

`M4rmGrayBuildFn`'s rustdoc lists the conditions under Panics, and
`build_gray_table_flat`'s lists its own.

## Argument contract (REQ-01, REQ-02)

`gf2_kernels_simd::m4rm::contract` holds the cases. `kernel_builders` lists the
reference at five strides, which is present on every host, and the specialized
builders of the detected bundle; `assert_contract` refuses an empty list,
prints one line per builder and case, and fails after the report. A rejection
case requires a panic that opens with the builder's own name, carries the
expected condition, and leaves a sentinel-filled buffer as it found it. The
kernel crate's unit test runs the contract over `kernel_builders`, and
`crates/gf2-core/tests/m4rm_gray_build_contract.rs` runs it over the same list
and `build_gray_table_flat` at one stride per route it takes, through an
adapter of the same call shape. A builder that derives its table size and panel
from a matrix cannot be handed a free table size or row count, so the adapter
skips the cases that need one; the `Builder::takes_sizes` field states the
rule.

`survey/run-contract.sh <label>` runs both tests with every feature and the
`gf2-core` test without `simd`, and writes the two logs.

| Tree | Build | Result |
|---|---|---|
| Reference and matrix-level builder without the shared check | every feature | Exit 100; [log](survey/test-logs/all-builders-before-simd.txt). |
| Same | without `simd` | Exit 100; [log](survey/test-logs/all-builders-before-scalar.txt). |
| Every builder with the shared check | every feature | Exit 0; [log](survey/test-logs/all-builders-after-simd.txt). |
| Same | without `simd` | Exit 0; [log](survey/test-logs/all-builders-after-scalar.txt). |

In the first two rows each `FAILED` line states what the builder does: the
reference accepts a zero table size, a table size that is not a power of two
and an overflowing panel length, and meets a short buffer, a short panel and an
overflowing table length with a bounds-check panic after its first stores; the
matrix-level builder meets an overflowing table length with a bounds-check
panic or its debug assertion, and accepts a `k_block` of the word size. Every
one of those runs stays inside its buffers, because both builders index with
bounds checks.

The specialized builders' behaviour without their assertions is recorded by an
earlier form of the test, at the tree where the first line of each log names
its command: [before](survey/test-logs/before-fix.txt), exit 100, and
[after](survey/test-logs/after-fix.txt), exit 0. Their overflowing-table case,
`table_size = usize::MAX / stride_words + 1`, is absent from the first of those
logs: without the assertions the wrapped product passes the length check and
the kernel stores outside the buffer, which ended the test process with
`SIGSEGV` in two uncommitted runs.

## Valid arguments (REQ-03)

The contract's last two cases compare each builder with the XOR of the panel
rows an entry's index selects, for a one-entry table and for every row count of
a 32-entry table.
`dev/scripts/run-kernel-suites.sh <this directory>/survey/test-logs` runs the
fast-tier suites that reach the kernel crate with and without the `simd`
feature; the commands are the first line of each log.

| Build | Result |
|---|---|
| `simd` | Exit 0; [log](survey/test-logs/simd-build.txt). |
| without `simd` | Exit 0; [log](survey/test-logs/scalar-build.txt). |

## Listing (REQ-04)

`python3 -B dev/scripts/regen-asm-listings.py crates/gf2-kernels-simd/src/x86/asm/avx2.asm.txt`
regenerates the module's listing in each commit that changes `avx2.rs`. The
listing names the kernels; the wrappers are nested functions of `avx2::fns`
and outside its symbols. `./scripts/asm-artefact-present.sh` exits 0 at each
of those commits.
