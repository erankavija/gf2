# profile.release delta — 2026-04-26

Measured impact of adding `[profile.release]` (`lto = "thin"`,
`codegen-units = 1`) to the workspace `Cargo.toml`. Issue
`c7791a20` under epic `babcf05e` (gf2-core PPC-spiral).

## Host & toolchain

- Host: `Linux fraktaali 6.19.11-arch1-1 #1 SMP PREEMPT_DYNAMIC Thu, 02 Apr 2026 23:33:01 +0000 x86_64 GNU/Linux`
- CPU: AMD Ryzen 9 5900X 12-Core Processor (Zen 3, 24 logical cores)
- Rustc: `rustc 1.91.0 (f8297e351 2025-10-28)` (host default toolchain)
- MSRV: 1.80 (set in `crates/gf2-core/Cargo.toml`)
- `RUSTFLAGS="-C target-cpu=native"` for both runs
- Before commit: `3d07769ac9d305aba66903d1c74b223db919a455`
- After commit: this change (workspace `Cargo.toml` only)

## Methodology

Bench harness: `cargo run -p gf2-core --release --features rand,test-support,simd
--example bench_csv_emitter -- --warmup 2 --iters 5 --filter matmul`. Each run
emits one CSV row per (n, rank_regime). To bound noise, the harness was
rerun four times in each configuration; statistics reported are min and
median across the four runs.

Build steps:

```bash
# baseline (no [profile.release] block)
RUSTFLAGS="-C target-cpu=native" cargo build -p gf2-core --release \
    --example bench_csv_emitter --features rand,test-support,simd

./target/release/examples/bench_csv_emitter \
    --warmup 2 --iters 5 --filter matmul --output /tmp/before.csv
```

Then add `[profile.release] { lto = "thin", codegen-units = 1 }` and
re-run identical commands.

## Numbers

BitMatrix matmul, GF(2), wall-time in microseconds (lower = better),
uniform regime. Four runs each, sorted ascending.

| n    | before runs (µs)               | before med | after runs (µs)               | after med | ratio (before/after) |
|------|--------------------------------|------------|-------------------------------|-----------|----------------------|
| 64   | 17.16, 17.16, 21.07, 21.39     | 19.12      | 15.25, 15.41, 15.46, 19.91    | 15.44     | **1.24×**            |
| 256  | 330.79, 335.22, 386.77, 448.96 | 360.99     | 339.06, 360.06, 363.85, 394.20| 361.96    | **1.00×**            |
| 1024 | 4630, 4869, 4876, 5116         | 4872       | 5156, 5296, 5371, 5384        | 5334      | **0.91×**            |
| 4096 | 90094, 90344, 90888, 91010     | 90616      | 96588, 99314, 99874, 106868   | 99594     | **0.91×**            |

(All values in µs of wall time per matmul. Uniform regime; deficient
regime tracked as a sanity check, results within ±2% of uniform.)

## PASS/FAIL on success criteria

| Criterion (verbatim) | Result |
|---|---|
| Workspace `Cargo.toml` has `[profile.release]` with `lto = "thin"`, `codegen-units = 1`. `panic = "abort"` OPTIONAL. | **PASS** — added; `panic = "abort"` deliberately omitted (default unwind preserved). |
| `cargo build --workspace --all-features --release` completes without warnings. | **PASS** — no warnings. |
| `cargo nextest run --workspace --all-features --release --profile ci` passes within 60 s. | **PASS** — 3024 tests in 6.4 s. |
| BitMatrix matmul at n=1024 (`--features simd`) shows ≥1.3× improvement over no-LTO baseline. | **FAIL** — measured ratio is **0.91× (slight regression)**. See "Empirical finding" below. |
| No regression on existing benches (criterion baselines ±5%). | **MIXED** — n=64 improves +24%; n=256 unchanged; n=1024 regresses 9%; n=4096 regresses 10% on the matmul SIMD path. |

## Empirical finding (key result for the lead)

**Thin LTO + codegen-units=1 does not deliver the predicted ≥1.3× win at
n=1024 with `--features simd`.** The hypothesis in the issue description
was that ThinLTO would inline `gf2_kernels_simd::LogicalFns` function
pointers across the gf2-core ↔ gf2-kernels-simd boundary, devirtualising
the per-call dispatch in `xor_inplace` / `popcount`. Empirically, ThinLTO
does not see through `OnceLock<Option<&'static LogicalFns>>` of `fn(_,_)`
pointers — the load + indirect call survives optimisation, and the
slightly different inlining heuristics actually cost a few percent on
the SIMD-bound large-n path.

The win **is** real on the small-n / scalar-bound path (+24% at n=64),
where the compiler can now fully inline the scalar XOR and bounds checks
across the dispatcher. But the ROI predicted in the uncompetitiveness
profile (+30–60% from cross-crate inlining alone) does **not**
materialise on this bench at the sizes we care about for the M4RI gap
(n ≥ 1024).

A `lto = "fat"` experiment was also performed (median 5.07 ms at
n=1024 uniform across 3 runs) — also below the spec'd 1.3×. So the
issue is not "thin LTO is too weak" but "function-pointer dispatch
in `LogicalFns` is opaque even to fat LTO, exactly as the
uncompetitiveness profile (point B) predicted".

This makes the dispatch-hoist follow-up task `b2ecd2ff` the primary
performance lever, not a polish optimisation. The Cargo.toml change
remains foundational — it pins the criterion baselines for I1
(`b2ecd2ff`) and the rest of the PPC spiral on a single, reproducible
profile rather than Cargo defaults — but the headline 1.3× claim
needs to be amended (or the criterion needs to be rewritten as a
correctness-of-build-profile criterion rather than a perf-delta one).

## Other regressions checked

The doc covers only the matmul bench (filter=matmul) since that was
the criterion focus. Other criterion baselines (under `target/criterion/`
on commit `0d2f284`) were not re-run as part of this change because:

1. The repo's `target/criterion/` does not contain a checked-in
   baseline from `0d2f284` (criterion baselines are local-only).
2. The PPC-spiral epic plan calls for I1 (`b2ecd2ff`) to pin baselines
   *after* this task lands.

A spot-check on `cargo bench -p gf2-core --bench matmul` was deferred
because it would have re-built the entire workspace at full LTO twice
(15+ min) for a result already covered by the `bench_csv_emitter` data.

## Conclusion

The Cargo.toml change is **landed correctly** and does what was asked
mechanically (LTO + single CGU). The **performance hypothesis behind it
is not validated by the empirical data** at the sizes specified by the
[hard] criterion. Recommend the lead either:

- (a) accept the change as a profile-stability move and amend the
  ≥1.3× criterion (mark it as superseded with reason "function-pointer
  dispatch is opaque to LTO; see b2ecd2ff for the actual lever"), or
- (b) reject and roll back, then sequence b2ecd2ff first and re-measure.

(a) is the cheaper option since b2ecd2ff is already on the wave plan
and the profile change is needed regardless to give I1 a stable
baseline.
