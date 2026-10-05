# M4RM Gray-table wrapper argument checks (jit:1b034786)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95.0`. No command takes a timing window, the host lock, or
an ignored test.

## Implementations

`M4rmGrayBuildFn` has two implementations, both in
`crates/gf2-kernels-simd/src/x86/avx2.rs`: the wrappers `LogicalFns` publishes
as `m4rm_gray_build4_fn` and `m4rm_gray_build8_fn`. No scalar function has that
type; `gf2-core` builds the tables of other strides through its own
bounds-checked walks. Both wrappers call `assert_m4rm_gray_build_lengths` after
their stride assertion and before their kernel, and the type's rustdoc lists
the conditions under Panics.

## Argument contract (REQ-01, REQ-02)

`crates/gf2-kernels-simd/tests/m4rm_gray_build_contract.rs` drives both
implementations through the bundle `gf2_kernels_simd::detect` returns. Each
rejection case sizes its buffers so that a call the builder accepts stays
inside them, and requires the builder's own panic message.
`survey/run-contract.sh <label>` runs the test and writes the log.

| Tree | Result |
|---|---|
| Before the assertions | Exit 100; the zero, non-power-of-two and overflowing-panel cases report that the builder accepted the arguments; [log](survey/test-logs/before-fix.txt). |
| With the assertions | Exit 0; [log](survey/test-logs/after-fix.txt). |

The overflowing-table case, `table_size = usize::MAX / stride_words + 1`, is
absent from the first row's tree: without the assertions the wrapped product
passes the length check and the kernel stores outside the buffer, which ended
the test process with `SIGSEGV` in two uncommitted runs. The case enters the
test with the assertions.

## Valid arguments (REQ-03)

The contract test's last two cases compare each builder with the XOR of the
panel rows an entry's index selects, for a one-entry table and for every row
count of a 32-entry table.
`dev/scripts/run-kernel-suites.sh <this directory>/survey/test-logs` runs the
fast-tier suites that reach the kernel crate with and without the `simd`
feature; the commands are the first line of each log.

| Build | Result |
|---|---|
| `simd` | Exit 0; [log](survey/test-logs/simd-build.txt). |
| without `simd` | Exit 0; [log](survey/test-logs/scalar-build.txt). |

## Listing (REQ-04)

`python3 -B dev/scripts/regen-asm-listings.py crates/gf2-kernels-simd/src/x86/asm/avx2.asm.txt`
regenerates the module's listing in the commit that changes `avx2.rs`. The
listing names the kernels; the wrappers are nested functions of `avx2::fns`
and outside its symbols. `./scripts/asm-artefact-present.sh` exits 0 at that
commit.
