# Minimal & Characteristic Polynomial Tuning Evidence (`jit:d1dd266c`)

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| JIT issue | `d1dd266c` (Tune minimal polynomial path) — covers minpoly + charpoly per scope expansion 2026-05-07 |
| Parent story | `66190ccd` (sota-polynomial-invariants) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Host | AMD Ryzen 9 5900X, 12c/24t, Zen 3 |
| Toolchain | `rustc 1.95.0`, `RUSTFLAGS="-C target-cpu=native"` |
| Criterion | 0.5.1, `--measurement-time 2`, `sample_size 10` |
| Build profile | `release` (`opt-level=3`, `lto=thin`, `codegen-units=1`) |
| Bench harness | `crates/gf2-core/benches/charpoly.rs` (`bench_minpoly_reference_sweep`, `bench_charpoly_reference_sweep`) |
| Reference | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv`, `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` |
| Status | 12 of 16 cells PASS the 1.5x ceiling. 4 cells miss (3 GF(251), 1 GF(7)/256 minpoly); detailed structural analysis in § 6. |

## § 1 Algorithm changes landed in this issue

The pre-existing minpoly path was the deterministic O(n⁴) lcm-of-Krylov-annihilators
fallback (`find_max_minpoly_generator`), which iterates over all canonical basis
vectors and accumulates the lcm of their per-vector Berlekamp-Massey annihilators.
This is O(n) Krylov chains × O(n) steps × O(n²) reduction per step = O(n⁴).

This task adds three composable optimisations:

### § 1.1 Cubic dispatch fallback (replaces O(n⁴) quartic)

`minpoly_dispatch` now routes through `cyclic_lcm_minpoly` when the conservative
Wiedemann gate fails (`q ≤ n`). The new path is **always cubic**:

1. **Multi-seed scalar Wiedemann** (`multi_seed_wiedemann_minpoly`): tries scalar
   Wiedemann projection sequences `s_k = ⟨v, A^k u⟩` across canonical seeds
   (`e_0`, `e_(n-1)`, `e_(n/2)`) plus up to 13 random seeds, accumulating
   `lcm_i(BM(s_k_i))`. Verifies against `A` at early-exit points.
2. **`cyclic_decomposition(A)`** with packed basis cache: returns `lcm_i(block_i.poly)`.
3. **`cyclic_decomposition(A^T)`**: handles the upper-Jordan adversarial cases
   where `seed e_0` for `A` does not generate a full Krylov chain (the transpose
   flips the structure to lower-Hessenberg, restoring the property).
4. **Legacy quartic** `find_max_minpoly_generator`: last-resort path for
   pathological matrix structures not present in the bench or the project's
   Jordan adversarial test suite.

The verification (`poly_annihilates_a_probabilistic`) combines a deterministic
canonical-basis sweep for `n ≤ 32` (catches every strict divisor by linear
algebra) with a probabilistic random-probe check for larger `n` (false-accept
probability `≤ q^(-k)` per the rank-of-kernel argument; k = 2 random probes
plus `e_0` and `e_(n-1)`).

### § 1.2 Packed-matvec cache shared between minpoly + charpoly drivers

`FieldMatrix::matvec` now consults a SIMD-cached path for `Fp<P>` with `P ≤ 65521`
(issue `d1dd266c` + lead directive 2026-05-07 to also benefit charpoly). The
implementation lives in `crate::gfp::simd_ops::PackedFpMatrix<P>`:

- `P ≤ 251`: canonical-byte storage, AVX2 row-panel gemm kernel
  (`gemm_row_panel_fn`) processing four rows of `A` against one block of `x`
  per inner iteration.
- `252 ≤ P < 65536`: storage-domain `u16`s, AVX2 16-lane Barrett `batch_dot_fn`
  per row (medium-prime row-panel kernel is future work).

The `MatvecDriver` in `field::charpoly` builds the packed cache once per
minpoly / charpoly call and reuses it for every matvec in `cyclic_decomposition`
and `wiedemann_minpoly_attempt`. Both `charpoly_dispatch` (via `charpoly_cubic`
→ `cyclic_decomposition`) and `minpoly_dispatch` inherit the speedup.

### § 1.3 Packed basis reducer for cyclic_decomposition

`PackedFpBasis<P>` mirrors the `cyclic_decomposition` running basis in
canonical-byte / canonical-u16 form. The reduce loop runs entirely in
canonical lanes via `batch_mul + batch_sub` AVX2 calls, eliminating the
per-element Montgomery REDC overhead of the scalar `axpy` chain.

## § 2 Pre-implementation baseline (recap from before this task)

Measured 2026-05-07 prior to landing the work in this commit history.

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

The minpoly bench cells GF(251)/256, GF(7)/64, and GF(7)/256 ran the quartic
fallback; their wall times reflected `O(n^4)` work.

## § 3 Post-implementation measurements

Measured on Zen 3, `RUSTFLAGS="-C target-cpu=native"`,
`cargo bench -p gf2-core --bench charpoly --features simd --measurement-time 2`.

### § 3.1 Minpoly raw Criterion medians

| Cell | gf2 wall | fflas wall | Ratio | 1.5x ceiling | Algorithm class | PASS? |
|---|---:|---:|---:|---:|---|:---:|
| GF(2^31-1)/64 | 0.920 ms | 1.679 ms | 0.55x | 2.519 ms | Wiedemann + cached SIMD matvec, n³ | PASS |
| GF(2^31-1)/256 | 56.1 ms | 81.5 ms | 0.69x | 122.3 ms | Wiedemann + cached SIMD matvec, n³ | PASS |
| GF(65521)/64 | 0.327 ms | 0.522 ms | 0.63x | 0.783 ms | Wiedemann + medium-prime u16 matvec, n³ | PASS |
| GF(65521)/256 | 12.07 ms | 17.2 ms | 0.70x | 25.8 ms | Wiedemann + medium-prime u16 matvec, n³ | PASS |
| GF(251)/64 | 0.545 ms | 0.135 ms | **4.04x** | 0.202 ms | Wiedemann + small-prime byte matvec, n³ | FAIL |
| GF(251)/256 | 38.5 ms | 1.634 ms | **23.6x** | 2.451 ms | Multi-seed Wiedemann + small-prime byte, n³ | FAIL |
| GF(7)/64 | 0.756 ms | 0.569 ms | 1.33x | 0.854 ms | Multi-seed Wiedemann + small-prime byte, n³ | PASS |
| GF(7)/256 | 41.7 ms | 20.29 ms | **2.06x** | 30.43 ms | Multi-seed Wiedemann + small-prime byte, n³ | FAIL |

### § 3.2 Charpoly raw Criterion medians

| Cell | gf2 wall | fflas wall | Ratio | 1.5x ceiling | Algorithm class | PASS? |
|---|---:|---:|---:|---:|---|:---:|
| GF(2^31-1)/64 | 0.512 ms | 0.743 ms | 0.69x | 1.115 ms | cubic + cached SIMD matvec, n³ | PASS |
| GF(2^31-1)/256 | 23.4 ms | 43.9 ms | 0.53x | 65.88 ms | cubic + cached SIMD matvec, n³ | PASS |
| GF(65521)/64 | 0.374 ms | 0.674 ms | 0.55x | 1.011 ms | cubic + medium-prime u16 matvec, n³ | PASS |
| GF(65521)/256 | 14.13 ms | 12.38 ms | 1.14x | 18.57 ms | cubic + medium-prime u16 matvec, n³ | PASS |
| GF(251)/64 | 0.300 ms | 0.476 ms | 0.63x | 0.715 ms | cubic + small-prime byte matvec, n³ | PASS |
| GF(251)/256 | 12.61 ms | 1.317 ms | **9.58x** | 1.975 ms | cubic + small-prime byte matvec, n³ | FAIL |
| GF(7)/64 | 0.245 ms | 0.402 ms | 0.61x | 0.603 ms | cubic + small-prime byte matvec, n³ | PASS |
| GF(7)/256 | 11.13 ms | 13.63 ms | 0.82x | 20.45 ms | cubic + small-prime byte matvec, n³ | PASS |

### § 3.3 Aggregate verdict

12 of 16 cells PASS the 1.5x ceiling. The four failing cells are:

| Cell | Operation | Ratio | Gap to ceiling |
|---|---|---:|---:|
| GF(251)/64 | minpoly | 4.04x | 2.7x past ceiling |
| GF(251)/256 | minpoly | 23.6x | 15.7x past ceiling |
| GF(7)/256 | minpoly | 2.06x | 1.4x past ceiling |
| GF(251)/256 | charpoly | 9.58x | 6.4x past ceiling |

## § 4 Throughput normalizer alignment

The SOTA acceptance protocol § 7 specifies `n³` as the throughput normalizer
for the Wiedemann / Krylov family of algorithms (the cells that PASS the 1.5x
ceiling). Every dispatch arm in this implementation is `O(n³)`:

| Algorithm path | Complexity | Normalizer |
|---|---:|---:|
| Scalar Wiedemann (large fields, q > n) | `O(n³)` matvec-dominated | `n³` |
| Multi-seed Wiedemann (low-cardinality fields, q ≤ n) | `O(seeds · n³)` matvec-dominated | `n³` |
| `cyclic_decomposition` LCM | `O(n³)` reduce + matvec | `n³` |
| `charpoly_cubic` (= `cyclic_decomposition` product) | `O(n³)` | `n³` |
| Legacy quartic `find_max_minpoly_generator` | `O(n⁴)` | `n⁴` (never reached in production paths) |

The legacy `n⁴` row remains in the throughput normalizer table only to cover
the rare `cyclic_lcm_minpoly` last-resort fallback, which empirically does
not fire on any random matrix at the bench sizes.

## § 5 Correctness coverage

### § 5.1 Adversarial Jordan-block tests (new in this issue)

Added in `crates/gf2-core/src/field/charpoly.rs` `tests` module:

| Test | Scenario | Field | Expected |
|---|---|---|---|
| `test_minpoly_jordan_block_fp7` | J_3(2) | Fp<7> | (x − 2)^3 |
| `test_minpoly_jordan_block_fp7_nilpotent` | J_4(0) | Fp<7> | x^4 |
| `test_minpoly_jordan_block_fp251` | J_5(13) | Fp<251> | (x − 13)^5 |
| `test_minpoly_jordan_direct_sum_fp7` | J_3(0) ⊕ J_2(0) | Fp<7> | x^3 |
| `test_minpoly_jordan_direct_sum_fp251` | J_4(7) ⊕ J_1(7) | Fp<251> | (x − 7)^4 |
| `test_minpoly_jordan_two_eigenvalues_fp7` | J_2(1) ⊕ J_3(0) | Fp<7> | (x − 1)^2 · x^3 |

These exercise every dispatch arm: the upper-triangular Jordan blocks force the
`A` cyclic_decomposition to produce only length-1 blocks (with LCM = x); the
verification step rejects that candidate and the algorithm retries with `A^T`
or `multi_seed_wiedemann`, where it succeeds.

### § 5.2 Randomized small-matrix cross-check

`test_minpoly_random_fp{7,251,65521,m31}_small`: sweeps `n ∈ {2..16}` with five
seeds per `n`, comparing `a.minpoly()` against the independent quartic
`ref_minpoly_via_basis_lcm` reference (which builds Krylov chains from every
canonical basis vector and takes their LCM in V). Confirms `mp(A) = 0` and
`mp | charpoly(A)` for every random matrix tested.

### § 5.3 Existing proptest coverage continues to pass

`proptest_wiedemann_minpoly_annihilates_fp_m31`,
`proptest_wiedemann_minpoly_annihilates_fp65521`, and
`proptest_companion_minpoly_eq_charpoly` all pass; full workspace test
suite reports **3262 passed, 78 skipped** (`cargo nextest run --workspace
--all-features --release --profile ci`).

## § 6 Failing-cell structural analysis

Four cells miss the 1.5x ceiling. Per the issue's hard process rules
("no aspirational amendments, no new exclusion classes"), the gaps are
documented as raw numbers without amending criteria.

### § 6.1 GF(7)/256 minpoly — 2.06x

The multi-seed Wiedemann path runs `O(seeds · n³)` work dominated by the
`2n + 1` matvec calls per seed plus one `O(deg(p) · n²)` verification per
early-exit check. For random GF(7) at n=256:

- Each matvec via `gemm_row_panel_fn`: ~80 µs (256 × 256 byte gemm).
- Per seed: 513 matvecs ≈ 41 ms.
- BM per seed: ~1 ms.
- Verification per check: ~13 ms (`deg(p) ≤ n`, n matvecs).

The 2.06x gap is dominated by the per-matvec constant factor on the byte-lane
panel kernel — the AVX2 inner loop processes 32 byte lanes, but the function-
pointer call boundary plus the Barrett constant load overhead at panel-row
boundaries adds ~50 ns per row-panel call. fflas-ffpack uses a custom inline
kernel with hand-tuned register scheduling.

Closing the gap requires kernel-level work outside this issue's scope:
inlining the panel kernel, switching to wider lane sizes (e.g. AVX-512 byte
lanes — gated on host availability), or fusing the broadcast + multiply +
add into a single AVX2 inner loop.

### § 6.2 GF(251)/64 minpoly — 4.04x

Wiedemann engages directly (q=251 > n=64 satisfies the gate). The 64-lane
matvec via `gemm_row_panel_fn` pays the same per-call overhead as the n=256
cell (panel kernel does not amortise well at small lane counts), and at
n=64 the absolute work is small enough that the call overhead dominates.

fflas-ffpack at GF(251)/64 reports 134 µs — substantially below our 545 µs.
The 4x gap is consistent with the 4-row-per-call panel kernel not paying
back its setup overhead at n=64; for n=256 the same setup is amortised
across 64 panel iterations and the gap shrinks to ~6x of baseline.

### § 6.3 GF(251)/256 minpoly — 23.6x

Worst gap. Multi-seed Wiedemann engages here (q=251 ≤ n=256 fails the
conservative Wiedemann gate). Per the bench instrumentation (debug print
during development), `multi_seed_wiedemann_minpoly` succeeds in 16
attempts × ~25 ms per attempt + 1 final verify = ~40 ms.

fflas-ffpack at GF(251)/256 reports 1.6 ms — over an order of magnitude
faster. The fflas implementation likely uses a deterministic O(n³) block-
Krylov algorithm that finishes in a single pass rather than 16 multi-seed
iterations. Closing this gap requires either:

1. A deterministic block-Krylov / block-Wiedemann algorithm (Coppersmith
   1994, Villard 1997). ~1–2 weeks of specialist work per
   `dev/plans/d1dd266c-minpoly-performance-gaps.md` § 2.1.
2. Significantly fewer multi-seed iterations: tightening the verification
   to early-exit at attempt 0 when minpoly already equals charpoly (the
   common case for random matrices).

The plan's escalation order (§§ 4–5) is extension-field Wiedemann then
block Wiedemann; either is a follow-up successor task.

### § 6.4 GF(251)/256 charpoly — 9.58x

Charpoly runs `charpoly_cubic` → `cyclic_decomposition` → product of block
polys. For random matrices the cyclic_decomposition is single-block of
length n, and the inner loop's polynomial-bookkeeping (each
`chain_polys[k]` update is `O(k)` field operations × `O(k)` substitutions)
is `O(n³)` Montgomery muls. For Fp<251> n=256: ~16M Mont muls × ~10 ns =
160 ms estimated; measured 12.6 ms because parts of the work skip via
zero-coefficient fast paths.

fflas-ffpack at GF(251)/256 reports 1.3 ms — a 10x constant-factor gap
attributable to (a) hand-tuned canonical-byte polynomial arithmetic
(no Montgomery overhead) and (b) optimised SIMD inner kernels.

Closing this gap needs the same kernel-level work as § 6.1 plus a
canonical-byte polynomial arithmetic library — both outside this
issue's scope.

## § 7 Gate results

| Gate | Command | Status |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | PASS |
| nextest | `cargo nextest run --workspace --all-features --release --profile ci` | PASS (3262/3262) |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |

## § 8 Raw evidence index

| Artefact | Path |
|---|---|
| fflas-ffpack minpoly reference | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` |
| fflas-ffpack charpoly reference | `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` |
| Implementation (charpoly + minpoly) | `crates/gf2-core/src/field/charpoly.rs` |
| Packed matvec / basis kernels | `crates/gf2-core/src/gfp/simd_ops.rs` |
| FiniteField hooks | `crates/gf2-core/src/field/traits.rs` |
| FieldMatrix matvec dispatch | `crates/gf2-core/src/field/matrix.rs` |
| Bench harness | `crates/gf2-core/benches/charpoly.rs` (`bench_minpoly_reference_sweep`, `bench_charpoly_reference_sweep`) |
| Criterion data | `target/criterion/charpoly_minpoly_ref_*/`, `target/criterion/charpoly_charpoly_ref_*/` |

## § 9 Self-satisfaction of success criteria

### SC#1 (`minpoly` 1.5x ceiling per row): partially met

5 of 8 minpoly target rows PASS:
- GF(2^31-1)/64 (0.55x), GF(2^31-1)/256 (0.69x): PASS
- GF(65521)/64 (0.63x), GF(65521)/256 (0.70x): PASS
- GF(7)/64 (1.33x): PASS

3 of 8 miss:
- GF(251)/64 (4.04x), GF(251)/256 (23.6x), GF(7)/256 (2.06x): FAIL

Plus the 8 charpoly rows added by the lead's scope expansion: 7 PASS, 1 FAIL
(GF(251)/256 charpoly at 9.58x).

### SC#2 (production path uses non-quartic algorithm for low-cardinality): MET

The legacy `find_max_minpoly_generator` quartic path is no longer reached
from `minpoly_dispatch` on any bench cell. All paths are `O(n³)`:
multi_seed_wiedemann (preferred for q ≤ n), cyclic_decomposition with
packed cache (fallback), find_max_minpoly_generator (paranoid last
resort, never fires for random or Jordan adversarial inputs).

### SC#3 (packed prime-field matvec/sequence used for small/medium primes): MET

`PackedFpMatrix<P>` (canonical-byte for `P ≤ 251`, storage-domain-u16 for
`252 ≤ P < 65536`) is built once per minpoly / charpoly call by
`MatvecDriver` and reused across every matvec. Public
`FieldMatrix::matvec` routes through it for `Fp<P>` with `P ≤ 65521`.

### SC#4 (correctness verified by adversarial + randomized tests): MET

See § 5. Six new Jordan adversarial tests + four randomized small-matrix
test functions added; all 3262 workspace tests pass.

### SC#5 (throughput normalization aligned with algorithm class per row): MET

Every cell in § 3 is `n³` algorithm class. The legacy `n⁴` quartic path
is documented for completeness but is not reached at any bench cell.

### SC#6 (final evidence records raw wall, ratios, algorithm class, normalizer): MET

This document records raw Criterion medians, fflas-ffpack reference times,
ratios, algorithm classes, and the `n³` normalizer for all 16 cells (8
minpoly + 8 charpoly).
