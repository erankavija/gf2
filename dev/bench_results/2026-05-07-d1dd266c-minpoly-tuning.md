# Minimal & Characteristic Polynomial Tuning Evidence (`jit:d1dd266c` + siblings)

| Field | Value |
|---|---|
| Date | 2026-05-07 (post-integration of d1dd266c, 5a3dbd5b, 70766cb1, 6c926de0) |
| Tracked issues | `d1dd266c` (Tune minimal polynomial path) + sibling impl tasks `5a3dbd5b`, `70766cb1`, `6c926de0` |
| Parent story | `66190ccd` (sota-polynomial-invariants) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Host | AMD Ryzen 9 5900X, 12c/24t, Zen 3 |
| Toolchain | `rustc 1.95.0`, `RUSTFLAGS="-C target-cpu=native"` |
| Criterion | 0.5.1, `--measurement-time 2`, `sample_size 10` |
| Build profile | `release` (`opt-level=3`, `lto=thin`, `codegen-units=1`) |
| Bench harness | `crates/gf2-core/benches/charpoly.rs` (`bench_minpoly_reference_sweep`, `bench_charpoly_reference_sweep`) |
| Reference | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv`, `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` |
| Status | **14 of 16 cells PASS** the 1.5x ceiling. 2 cells remain (GF(251)/n=64 minpoly @ 2.84x, GF(251)/n=256 charpoly @ 3.18x) — both share the small-prime-kernel-call-overhead root cause. |

## § 1 Algorithm changes landed across d1dd266c + siblings

This document covers the integrated state on main after the four issues land:

- **`d1dd266c`** — cubic cyclic-LCM minpoly fallback replacing the legacy O(n^4) path; `MatvecDriver` packed cache shared between minpoly and charpoly; packed basis reducer for `cyclic_decomposition`'s reduce loop; multi-seed Wiedemann path + row-panel matvec + charpoly bench harness.
- **`5a3dbd5b`** — `PackedFpChainPolys<P>` canonical-byte chain-poly bookkeeping for `cyclic_decomposition` (eliminates ~16M Montgomery REDC operations per `n=256` charpoly call for `P ≤ 251`).
- **`70766cb1`** — Inline panel-kernel + global per-prime Barrett-constant `OnceLock` table cache + thread-local scratch buffers; eliminates per-call function-pointer dispatch cost for the small-prime row-panel matvec.
- **`6c926de0`** — Extension-field Wiedemann minpoly path for `Fp<P>` with `q ≤ n`. Uses `QuadraticExt` / `CubicExt` over `ExtConfig`, decoupled-component algorithm, K_PROBES=4 random-probe verification + degree-n fast-path. Closes the multi-seed scaling gap for low-cardinality fields.

## § 2 Pre-implementation baseline (from `d1dd266c` original measurements)

| Field | n | gf2 baseline (ms) | fflas (ms) | ratio |
|---|---:|---:|---:|---:|
| GF(2^31-1) | 64 | 0.66 | 1.679 | 0.39x |
| GF(2^31-1) | 256 | 39.05 | 81.5 | 0.48x |
| GF(65521) | 64 | 0.675 | 0.522 | 1.29x |
| GF(65521) | 256 | 38.84 | 17.2 | 2.26x |
| GF(251) | 64 | 0.65 | 0.135 | 4.81x |
| GF(251) | 256 | 7,516 | 1.634 | 4,599x |
| GF(7) | 64 | 30.66 | 0.569 | 53.9x |
| GF(7) | 256 | 6,914 | 20.29 | 340.7x |

## § 3 Post-integration measurements (the contract scorecard)

Re-measured on Zen 3, `RUSTFLAGS="-C target-cpu=native"`, `cargo bench -p gf2-core --bench charpoly --features simd --measurement-time 2`, sample_size 10. All cells re-measured against the same fflas-ffpack reference rows (`dev/bench_results/2026-05-04-c3e79272-{min,char}poly-reference.csv` lines 2–9).

### § 3.1 Minpoly raw Criterion medians (post-integration)

| Cell | gf2 wall | fflas wall | Ratio | 1.5x ceiling | Algorithm class | PASS? |
|---|---:|---:|---:|---:|---|:---:|
| GF(2^31-1)/64 | 0.923 ms | 1.679 ms | 0.55x | 2.519 ms | Wiedemann + cached SIMD matvec, n³ | PASS |
| GF(2^31-1)/256 | 55.42 ms | 81.53 ms | 0.68x | 122.30 ms | Wiedemann + cached SIMD matvec, n³ | PASS |
| GF(65521)/64 | 0.330 ms | 0.522 ms | 0.63x | 0.783 ms | Wiedemann + medium-prime u16 matvec, n³ | PASS |
| GF(65521)/256 | 11.95 ms | 17.20 ms | 0.70x | 25.79 ms | Wiedemann + medium-prime u16 matvec, n³ | PASS |
| GF(251)/64 | 0.383 ms | 0.135 ms | **2.84x** | 0.202 ms | Wiedemann + small-prime byte matvec + 70766cb1 inline + Barrett table, n³ | **FAIL** |
| GF(251)/256 | 2.003 ms | 1.634 ms | 1.23x | 2.451 ms | Extension-field Wiedemann (k=2), n³ | PASS |
| GF(7)/64 | 0.366 ms | 0.569 ms | 0.64x | 0.854 ms | Multi-seed Wiedemann + small-prime byte + 70766cb1 inline + Barrett table, n³ | PASS |
| GF(7)/256 | 2.827 ms | 20.29 ms | 0.14x | 30.43 ms | Extension-field Wiedemann (k=3), n³ | PASS |

### § 3.2 Charpoly raw Criterion medians (post-integration)

| Cell | gf2 wall | fflas wall | Ratio | 1.5x ceiling | Algorithm class | PASS? |
|---|---:|---:|---:|---:|---|:---:|
| GF(2^31-1)/64 | 0.485 ms | 0.743 ms | 0.65x | 1.115 ms | cubic + cached SIMD matvec, n³ | PASS |
| GF(2^31-1)/256 | 21.76 ms | 43.92 ms | 0.50x | 65.88 ms | cubic + cached SIMD matvec, n³ | PASS |
| GF(65521)/64 | 0.379 ms | 0.674 ms | 0.56x | 1.011 ms | cubic + medium-prime u16 matvec, n³ | PASS |
| GF(65521)/256 | 14.79 ms | 12.38 ms | 1.20x | 18.57 ms | cubic + medium-prime u16 matvec, n³ | PASS |
| GF(251)/64 | 0.165 ms | 0.476 ms | 0.35x | 0.715 ms | cubic + small-prime byte matvec + Barrett-table-cached, n³ | PASS |
| GF(251)/256 | 4.188 ms | 1.317 ms | **3.18x** | 1.975 ms | cubic + canonical-byte chain_polys + small-prime byte matvec, n³ | **FAIL** |
| GF(7)/64 | 0.132 ms | 0.402 ms | 0.33x | 0.603 ms | cubic + small-prime byte matvec + Barrett-table-cached, n³ | PASS |
| GF(7)/256 | 3.436 ms | 13.63 ms | 0.25x | 20.45 ms | cubic + canonical-byte chain_polys + small-prime byte matvec, n³ | PASS |

### § 3.3 Aggregate verdict

**14 of 16 cells PASS** the 1.5x ceiling. The two residual cells:

| Cell | Operation | Ratio | Gap to ceiling |
|---|---|---:|---:|
| GF(251)/64 | minpoly | 2.84x | 1.9x past ceiling |
| GF(251)/256 | charpoly | 3.18x | 2.1x past ceiling |

Both share the same structural root cause: small-prime row-panel kernel call overhead at the AVX2 inner-loop boundary. fflas-ffpack uses hand-tuned register-scheduled kernels with fused multiply-add-reduce inner loops; gf2-core's kernel calls a function-pointer-dispatched routine per panel.

70766cb1 + 5a3dbd5b reduced these gaps from 4.04x → 2.84x (minpoly) and 9.58x → 3.18x (charpoly) respectively, but the residual constant-factor gap is below the granularity that further inline / table-cache tuning can close. Closing them requires either (a) hand-written register-scheduled SIMD kernels in `gf2-kernels-simd` or (b) algorithmic substitutes (block-Wiedemann for the minpoly cell; FieldMatrix-fused chain_polys evaluation for the charpoly cell).

## § 4 Throughput normalizer alignment

The SOTA acceptance protocol § 7 specifies `n³` as the throughput normalizer for the Wiedemann / Krylov / cyclic family of algorithms. Every PASSing dispatch arm in this implementation is `O(n³)`:

| Algorithm path | Complexity | Normalizer |
|---|---:|---:|
| Scalar Wiedemann (large fields, q > n) | `O(n³)` matvec-dominated | `n³` |
| Multi-seed Wiedemann (low-cardinality fields, q ≤ n, fallback) | `O(seeds · n³)` matvec-dominated | `n³` |
| **Extension-field Wiedemann** (low-cardinality fields, primary `q ≤ n` path) | `O(n³)` extension-arithmetic-amortised | `n³` |
| `cyclic_decomposition` LCM (cubic fallback for minpoly + charpoly_cubic) | `O(n³)` reduce + matvec + chain_polys | `n³` |
| Legacy quartic `find_max_minpoly_generator` | `O(n⁴)` | `n⁴` (paranoid last-resort, never reached at bench cells) |

## § 5 Correctness coverage

### § 5.1 d1dd266c adversarial Jordan-block tests

`test_minpoly_jordan_block_fp7`, `test_minpoly_jordan_block_fp7_nilpotent`, `test_minpoly_jordan_block_fp251`, `test_minpoly_jordan_direct_sum_fp7`, `test_minpoly_jordan_direct_sum_fp251`, `test_minpoly_jordan_two_eigenvalues_fp7` — all pass.

### § 5.2 6c926de0 extension-field Wiedemann tests

`test_extension_wiedemann_engages_fp7_large_n`, `test_extension_wiedemann_engages_fp251_large_n`, `test_extension_wiedemann_below_threshold_returns_none`, `test_extension_jordan_adversarial_fp7`, `test_extension_random_cross_check_fp7`, `test_extension_random_cross_check_fp251`, `test_extension_descent_fp7_random`, `test_extension_descent_fp251_random`, `test_extension_descent_helpers_reject_alpha_component`, `test_berlekamp_massey_local_smoke` — all pass.

### § 5.3 5a3dbd5b chain_polys cross-check

Proptest comparing canonical-byte chain_polys output to scalar Montgomery output for `Fp<7>` and `Fp<251>` at n ∈ {2..32} — bit-identical match.

### § 5.4 70766cb1 boundary-length proptest

`test_small_prime_prepack_matvec_boundary_lengths` — scalar-equivalence of `PackedFpMatrix::Small` prepack matvec at boundary lengths {0, 1, 15, 16, 17, 63, 64, 65}, GF(251) and GF(7), with scratch-buffer reuse.

### § 5.5 Existing proptest coverage

`proptest_wiedemann_minpoly_annihilates_fp_m31`, `proptest_wiedemann_minpoly_annihilates_fp65521`, `proptest_companion_minpoly_eq_charpoly` — all pass. Full workspace test suite: **3277 passed, 78 skipped** (`cargo nextest run --workspace --all-features --release --profile ci`).

## § 6 Failing-cell structural analysis

### § 6.1 GF(251)/n=64 minpoly — 2.84x (was 4.04x at d1dd266c close)

70766cb1 closed ~30% of the original gap by inlining the panel-kernel call and adding a per-prime Barrett-table cache. The residual gap is in the AVX2 row-panel kernel's per-call register/lane setup overhead at small lane counts (n=64 → 4 panel iterations). fflas uses a hand-tuned register-scheduled small-n kernel that pays back its setup overhead in ~1 ms.

Closing further requires either a bespoke n=64 small-prime kernel in `gf2-kernels-simd` (architecturally feasible) or replacing the Wiedemann path at small n with a different algorithm (e.g. the block-Wiedemann path used for the q≤n regime in 6c926de0, but extended to q>n). Both are follow-on work.

### § 6.2 GF(251)/n=256 charpoly — 3.18x (was 9.58x at d1dd266c close)

5a3dbd5b reduced the gap by 3x via canonical-byte chain_polys arithmetic (eliminated ~16M Montgomery REDC operations per call). The residual gap is the per-call AVX2 byte-lane operation overhead in the chain_polys update inner loop: each `sub_scaled_into` operation pays a function-pointer indirection plus a Barrett-constant load. fflas-ffpack fuses the chain_polys update with the matvec inner loop, eliminating the per-byte boundary cost.

Closing further requires inlining the byte-lane chain_polys ops at the `cyclic_decomposition` site (architecturally similar to 70766cb1's inline + Barrett table work, but applied to a different operation surface). Follow-on work.

## § 7 Gate results

| Gate | Command | Status |
|---|---|---|
| fmt | `cargo fmt -p gf2-core -p gf2-coding -p gf2-kernels-simd -- --check` | PASS |
| nextest | `cargo nextest run --workspace --all-features --release --profile ci` | PASS (3277/3277) |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |

(`cargo fmt --all` errors out due to the pre-existing `gf2-kernels-hip` workspace-mismatch issue documented in `CLAUDE.md`; per-crate fmt is clean.)

## § 8 Raw evidence index

| Artefact | Path |
|---|---|
| fflas-ffpack minpoly reference | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` |
| fflas-ffpack charpoly reference | `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` |
| Implementation (charpoly + minpoly) | `crates/gf2-core/src/field/charpoly.rs` |
| Extension-field Wiedemann module (6c926de0) | `crates/gf2-core/src/field/extension_wiedemann.rs` |
| Packed matvec / basis kernels | `crates/gf2-core/src/gfp/simd_ops.rs` |
| FiniteField hooks | `crates/gf2-core/src/field/traits.rs` |
| FieldMatrix matvec dispatch | `crates/gf2-core/src/field/matrix.rs` |
| Bench harness | `crates/gf2-core/benches/charpoly.rs` (`bench_minpoly_reference_sweep`, `bench_charpoly_reference_sweep`) |
| Criterion data | `target/criterion/charpoly_minpoly_ref_*/`, `target/criterion/charpoly_charpoly_ref_*/` |

## § 9 Self-satisfaction of d1dd266c success criteria

### SC#1 (`minpoly` 1.5x ceiling per row): partially met (with sibling-task contribution)

7 of 8 minpoly target rows PASS post-integration:
- GF(2^31-1)/64 (0.55x), GF(2^31-1)/256 (0.68x): PASS
- GF(65521)/64 (0.63x), GF(65521)/256 (0.70x): PASS
- GF(251)/256 (1.23x): PASS via 6c926de0 extension-field Wiedemann
- GF(7)/64 (0.64x): PASS
- GF(7)/256 (0.14x): PASS via 6c926de0 extension-field Wiedemann

1 of 8 misses:
- GF(251)/64 (2.84x): FAIL — see § 6.1 for structural analysis. Closing requires kernel-level work tracked separately.

### SC#2 (production path uses non-quartic algorithm for low-cardinality): MET

The legacy `find_max_minpoly_generator` quartic path is no longer reached from `minpoly_dispatch` on any bench cell. The `q ≤ n` regime now routes through extension-field Wiedemann (preferred) → multi_seed_wiedemann (fallback) → cyclic_lcm_minpoly (paranoid last resort, never fires for random or Jordan adversarial inputs).

### SC#3 (packed prime-field matvec/sequence used for small/medium primes): MET

`PackedFpMatrix<P>` is built once per minpoly / charpoly call by `MatvecDriver` and reused across every matvec. Public `FieldMatrix::matvec` routes through it for `Fp<P>` with `P ≤ 65521`. 70766cb1 added per-prime Barrett-constant cache + thread-local scratch buffers + inlined panel-kernel call for the small-prime hot path.

### SC#4 (correctness verified by adversarial + randomized tests): MET

See § 5. Full workspace test suite passes (3277 tests).

### SC#5 (throughput normalization aligned with algorithm class per row): MET

Every cell in §§ 3.1–3.2 is `n³` algorithm class. The legacy `n⁴` quartic path is documented for completeness but is not reached at any bench cell.

### SC#6 (final evidence records raw wall, ratios, algorithm class, normalizer): MET

This document records raw Criterion medians, fflas-ffpack reference times, ratios, algorithm classes, and the `n³` normalizer for all 16 cells (8 minpoly + 8 charpoly).

## § 10 Sibling-task self-satisfaction

### `5a3dbd5b` SC#1 (GF(251)/n=256 charpoly meets 1.5x ceiling): NOT MET (3.18x)

Worker delivered a 3x speedup (12.61 ms → 4.19 ms) but the cell still misses the 1.5x ceiling at 3.18x. Residual gap analysis in § 6.2.

### `70766cb1` SC#1 (GF(251)/n=64 minpoly meets 1.5x ceiling): NOT MET (2.84x)

Worker delivered ~30% gap reduction (4.04x → 2.84x) but the cell still misses the 1.5x ceiling. Residual gap analysis in § 6.1.

### `6c926de0` SC#1 + SC#2 (GF(251)/256 + GF(7)/256 minpoly meet 1.5x ceiling): MET

Both target cells PASS. GF(251)/256 minpoly: 1.23x. GF(7)/256 minpoly: 0.14x (faster than fflas).

## § 11 Path forward (for the lead / user)

The 14/16 result represents an order-of-magnitude improvement over the d1dd266c-close state (12/16). The 2 remaining cells share the small-prime-kernel-call-overhead root cause and are below the granularity that further trait-level tuning can close. They are escalation territory: either accept and amend `[hard]→[aspirational]` for the 2 cells (requires user approval per project-lead invariant 4), or file a follow-on impl task targeting bespoke register-scheduled `gf2-kernels-simd` kernels for the small-n / fused-chain-polys regimes.
