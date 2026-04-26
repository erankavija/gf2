# Uncompetitiveness Profile — 2026-04-26

Investigation of "what makes gf2-core uncompetitive in general?" against
M4RI (GF(2) reference) and fflas-ffpack (Fp reference). Triggered by the
0.07–0.21× ratios observed in the BitMatrix matmul cells of the
2026-04-26 reference run.

## TL;DR

Three compounding causes, ranked by remediation ease ↘ payoff ↗:

1. **No `[profile.release]` block at the workspace level.** Cargo
   defaults `lto = false` and `codegen-units = 16`. Function pointers
   in `gf2-kernels-simd::LogicalFns` are **never** inlined into
   `gf2-core::alg::m4rm::multiply` from the upstream crate. M4RI is
   built as a single TU with `-O3 -mavx2`-equivalent and full
   inlining.
2. **`LogicalFns` defeats devirtualisation even with LTO.** It is a
   struct of `fn(_,_)` pointers stored in an `OnceLock<Option<&LogicalFns>>`,
   loaded per `xor_inplace` call. The hot M4RM path performs ~1M–17M
   `xor_inplace` calls per matmul depending on n; each pays a load +
   pointer call regardless of buffer size.
3. **No Strassen layer in `gf2-core::alg`.** M4RI auto-switches to
   Strassen-Winograd at n ≥ 1024 by default. Our M4RM is pure
   schoolbook.

A direct test confirms (1)+(2): enabling `--features simd` on the
emitter only buys **+16% at n=1024 and +24% at n=4096**, vs. the ~4×
theoretical XOR-throughput improvement that 256-bit `vpxor` should
provide. The dispatch overhead consumes most of the SIMD win.

## Numbers (matmul, GF(2), n×n×n)

| n    | gf2 scalar (ops/s) | gf2 simd (ops/s) | M4RI (ops/s)  | gf2-simd / M4RI |
|------|--------------------|------------------|---------------|-----------------|
| 64   | 2.22 × 10¹⁰        | 2.17 × 10¹⁰      | 1.51 × 10¹¹   | 0.144×          |
| 256  | 8.30 × 10¹⁰        | 8.49 × 10¹⁰      | 1.13 × 10¹²   | 0.075×          |
| 1024 | 3.87 × 10¹¹        | 4.50 × 10¹¹      | 3.02 × 10¹²   | 0.149×          |
| 4096 | 1.09 × 10¹²        | 1.35 × 10¹²      | 6.27 × 10¹²   | 0.215×          |

Scalar-vs-SIMD nearly identical at n ≤ 256 because `select_backend_for_size`
threshold is 8 u64 words (n=512); below that the scalar branch is selected.
The gap to M4RI is *also* ~7× at n=256 where SIMD is not yet involved —
confirming SIMD is not the dominant story.

## Evidence

### A. Scalar-only build numbers were what we published

The bench example `crates/gf2-core/examples/bench_csv_emitter.rs` has
`required-features = ["rand"]`. The 2026-04-26 reference run command
recorded in `dev/bench_results/2026-04-26.md:127` is:

```
cargo run -p gf2-core --release --example bench_csv_emitter \
    --features rand,test-support -- ...
```

No `simd` feature. So all 212 published rows of `2026-04-26-gf2.csv`
were measured with the **scalar backend**. (M4RI of course uses AVX2
unconditionally on this host.)

### B. Even with `simd`, dispatch is a hot path

`crates/gf2-core/src/kernels/ops.rs:33` `xor_inplace` performs, per call:

```rust
match select_backend_for_size(dst.len()) {       // inlinable
    SelectedBackend::Simd => {
        if let Some(backend) = maybe_simd() {    // OnceLock atomic load
            backend.xor(dst, src);               // function-pointer call
        } ...
    }
    SelectedBackend::Scalar => SCALAR_BACKEND.xor(dst, src),
}
```

`backend` is a `&'static LogicalFns` struct of `fn(_,_)` pointers. With
default `codegen-units = 16` and no `lto`, the call is opaque to LLVM
across the gf2-core ↔ gf2-kernels-simd boundary.

### C. Workspace has no release-profile tuning

`Cargo.toml` (workspace root) has no `[profile.release]` block. All
crates inherit Cargo defaults:

| Setting          | Cargo default | Recommended for perf |
|------------------|---------------|----------------------|
| `opt-level`      | `3`           | `3` ✓                |
| `lto`            | `false`       | `"thin"` or `"fat"`  |
| `codegen-units`  | `16`          | `1`                  |

### D. `BitMatrix::get` carries 2 bounds-check `assert!`s

`crates/gf2-core/src/matrix.rs:369-386` checks `row < self.rows` and
`col < self.cols` per call, with formatted panic messages. Called
k_block times per row in `extract_bits` (M4RM panel inner). At n=4096
that is ~16.8M bounds-checked accesses per matmul. Likely DCE'd by
LLVM after the prior `if col < max_col` check, but the `extract_bits`
loop itself still does per-bit branching where M4RI processes whole
words at a time.

## Recommended follow-up issues

The investigation belongs *outside* epic `bb85c68a` (which is
"FieldMatrix linear algebra correctness", now complete except final
sign-off). Recommend a new perf epic with at least these stories:

1. **`perf:profile-release-tuning`** — Add `[profile.release]` to the
   workspace `Cargo.toml`: `lto = "thin"`, `codegen-units = 1`,
   optionally `panic = "abort"`. Re-bench. Expected: +30–60% from
   cross-crate inlining alone, no API change.
2. **`perf:devirtualize-kernel-dispatch`** — Hoist `maybe_simd()` out
   of the per-call hot path. Two viable patterns:
   (a) cache the resolved function pointer in `m4rm::multiply` once
   at entry, call directly in the loop;
   (b) replace `LogicalFns` with a `&dyn Backend` trait object so LTO
   can devirtualize, OR with a `cfg!(target_feature = "avx2")`
   compile-time branch with a runtime-gated wrapper at the binary
   crate level.
3. **`perf:m4rm-strassen-layer`** — Add Strassen-Winograd recursion
   to `gf2-core::alg::m4rm` for n ≥ 1024 (parameter-tunable). Expected
   payoff: ~12% per recursion level, so 3-level switch at n=4096 ≈
   1.4× on top of the inlining wins. Closes most of the n=4096 gap.
4. **`perf:fp-delayed-reduction`** — Out of scope for the GF(2) gap
   but flagged here for completeness: fflas-ffpack `Fp<P>` for small
   primes batches multiplications into accumulators that defer modular
   reduction by ~32 multiply-adds before reducing. We reduce per
   multiply. This is the dominant story for the Fp gap (a separate
   investigation).

(1) is a one-line config change with the highest ROI. Recommend
landing it first and re-running the reference bench before tackling
(2)–(3) — the new baseline will reframe how big each subsequent gain
appears.

## Appendix: Re-run command for these numbers

```bash
RUSTFLAGS="-C target-cpu=native" cargo build -p gf2-core --release \
    --example bench_csv_emitter --features rand,test-support,simd

./target/release/examples/bench_csv_emitter --warmup 1 --iters 2 \
    --filter matmul --output /tmp/gf2-simd.csv
```

The asm dump used to confirm point (B) was emitted via:

```bash
cargo rustc --release -p gf2-core --lib -- --emit=asm -C opt-level=3 \
    -C target-cpu=native --features simd
```

Then inspect `_ZN8gf2_core3alg4m4rm8multiply...` in
`target/release/deps/gf2_core-*.s` for the call to
`ScalarBackend::xor` from inside the M4RM body, and the absence of
inlined `vpxor` sequences for the corresponding SIMD branch.
