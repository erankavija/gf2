# Logical-buffer no-change verification (jit:fcb04d66)

The source revision is `cd353653a2aaa7d0f6f82369b8a6d022c1c09cb4` on `worktree-agent-fcb04d66`. The [source evidence ledger](survey/source-evidence.json) pins the inspected code by revision, path, line, verbatim text, and file digest. The production crates `gf2-core` and `gf2-kernels-simd` have no diff from this anchor.

## Commands and results

Run from this issue's worktree, with an empty `RUSTFLAGS` environment and Rust `rustc 1.95.0 (59807616e 2026-04-14)`:

| Command | Result |
|---|---|
| `git diff --exit-code cd353653a2aaa7d0f6f82369b8a6d022c1c09cb4 -- crates/gf2-core crates/gf2-kernels-simd` | Exit 0; no production diff. |
| `RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1 nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features --test simd_equiv_demo --test simd_equiv_dispatch_hoist --cargo-profile ci-test --profile ci` | Exit 0; 8 tests passed. Nextest run `bd3cf2f0-8c31-4825-8d0e-55493bb9fec4`. |
| `RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1 nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features --lib --test matrix --cargo-profile ci-test --profile ci -E 'test(test_boundary_) | test(test_mask_tail_invariant) | test(test_row_words)'` | Exit 0; 10 tests passed. Nextest run `eb4bd9e6-d67e-44dd-b6ce-4e5ce25f9a68`. |

The first selection exercises the existing shared scalar/SIMD XOR oracle, its canonical boundary list, unaligned slice offsets, and the dispatch-hoist route. The second selection exercises packed-bit boundaries, zero tail padding, and public matrix row indexing. The source evidence ledger identifies the dispatch, fallback, feature gate, experimental cfg, and safety-contract lines behind these tests.

## Build scope

The focused tests build the current core path with `--all-features` under Rust 1.95's `ci-test` profile. `--all-features` enables `simd`; ordinary `gf2-core` defaults omit it. Empty `RUSTFLAGS` leaves both factor-specific `gf2_xor_unroll*` cfgs unset, so this verification compiles the established AVX2 body, not an experimental unroll. Existing [premeasurement build evidence](../bc091474/readiness.md) records separate Rust 1.95 **release** builds for that baseline and each private cfg arm. Their [baseline](../bc091474/asm/baseline-avx2-xor.asm.txt), [factor 2](../bc091474/asm/unroll2-avx2-xor.asm.txt), and [factor 4](../bc091474/asm/unroll4-avx2-xor.asm.txt) annotated release disassemblies pin the measured binary identities and flags. This issue modifies no SIMD symbol and requires no new disassembly.

No timed measurement, host lock, ignored test, production build change, or holdout campaign is part of this verification.
