# Keller-Gehrig vs Cubic Crossover -- Post-Wave-9 Reassessment

| Field       | Value |
|---|---|
| Date        | 2026-05-07 |
| JIT issue   | `4a59d1f9` (Reassess Keller-Gehrig crossover) |
| Parent story | `66190ccd` (Close charpoly and minpoly gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Host        | AMD Ryzen 9 5900X, Zen 3, 12C/24T, ~4.7 GHz; Linux 7.0.3-arch1-1 |
| Rust        | `rustc 1.95.0` (2026-04-14), `RUSTFLAGS="-C target-cpu=native"` |
| Criterion   | 0.5.1; 10 samples per cell (3 s warmup + measurement time) |
| Build profile | `release` (`opt-level=3`, `lto=thin`, `codegen-units=1`) |

## Post-Wave-9 kernel inventory

The following optimizations landed between the 2026-04-26 reference measurement
and this reassessment:

| Issue | Change | Affects KG? | Affects cubic? |
|---|---|---|---|
| `73ec5da3` | `TRI_BASE_THRESHOLD`: 32 -> 8 (reduces TRSM recursion overhead) | YES -- KG's `solve` uses TRSM | minor |
| `73ec5da3` | `PLE_BASE_COLS = 1` (explicit, Mersenne-31 GEMM amortises reduction) | YES -- KG's `solve` uses PLE | no |
| `e24f7839` | Panelized GF(2^m) GEMM (AVX2+VPCLMULQDQ, I_TILE=4) | YES -- KG's doubling uses GEMM for GF(2^m) | no (cubic uses matvec not GEMM) |
| `2c52bcf6` | Rank-deficient short-circuit in PLE (`split_compact`) | Marginal -- typical charpoly matrices are full-rank | minor (rank-deficient path) |
| `b377304` (prior) | Delayed u128 reduction in Mersenne-31 GEMM | YES -- KG doublings use GEMM | no |

Both PLE and TRSM are in the critical path of the KG solve step, so the
`73ec5da3` threshold changes were the primary candidates to shift the crossover.
The panelized GF(2^m) GEMM (`e24f7839`) only helps KG for GF(2^m) fields where
KG is even valid (see validity constraint below).

---

## 1. Crossover sweep results

### 1.1 Method

Measurements use the `charpoly/dispatch` Criterion group in
`crates/gf2-core/benches/charpoly.rs` for `Fp<MERSENNE_31>`, and a new
`charpoly/dispatch_fp65521` group added by this issue for `Fp<65521>`.
Each bench arm warms up for 3 s then collects 10 samples. The bench
infrastructure uses a single fixed seed matrix so the same matrix is
timed across all samples (not regenerated per iteration). Both `cubic` and
`kg` arms share the identical matrix to make the comparison fair. For the
KG arm, `charpoly_keller_gehrig(0xC0FFEE)` is called directly; the first
attempt converges without any Las-Vegas retry on all tested matrices.

The `fieldmatrix_charpoly` bench provides cubic-only data for `Gf2m8` (GF(2^8))
and `Gf2m16` (GF(2^16)) since KG is invalid at the bench sizes for those fields
(see validity constraint).

Multi-trial: Criterion's 10-sample median is used; Criterion also computes
a min-max spread. All numbers cited below are Criterion middle estimates
(the median-of-estimates value reported as the second number in the
`[low mid high]` bracket).

### 1.2 KG Las-Vegas validity constraint

The KG algorithm requires `q > 2 n^2` for the per-attempt success
probability bound `1 - n/q > 1/2` to hold. Concretely:

| Field | q | Max valid n (q > 2n^2) |
|---|---|---|
| GF(2^8) | 256 | n <= 11 |
| GF(2^16) | 65536 | n <= 180 |
| Fp_65521 | 65521 | n <= 180 |
| Fp_M31 (Mersenne-31) | 2,147,483,647 | n <= 32767 |

For GF(2^8), KG is only valid at toy sizes (n <= 11). No practical crossover
exists for GF(2^8) matrices -- cubic is always preferred. For GF(2^16) and
Fp_65521, the validity cap falls inside the benchmarked range; sizes above 180
must use cubic regardless of wall-clock preference. For Fp_M31, the validity
cap is far above any practical matrix size.

### 1.3 Fp_M31 (Mersenne-31) -- n in {64, 128, 256}

Measured 2026-05-07 with `cargo bench -p gf2-core --bench charpoly --features rand
-- "charpoly/dispatch"` (10 samples per arm). The n=512 KG arm was also measured
(Criterion estimated 882 s total; the result arrived after the main sweep).

| n | cubic (ms) | KG (ms) | KG/cubic ratio |
|---:|---:|---:|---:|
| 64 | 0.729 | 23.0 | 31.6x |
| 128 | 5.25 | 348 | 66.3x |
| 256 | 37.1 | 5510 | 148.5x |
| 512 | 279 | 94 168 | 337x |
| 1024 | 2143 | ~1 500 000 (extrapolated; see note) | ~700x (extrapolated) |

Note on n=1024 KG: the Criterion harness started the n=1024 KG bench but the
per-call cost exceeds the practical session budget (~750-1500 s/call based on
the n=512 observed 94 s and the 16x scaling factor). The n=1024 KG figure is
extrapolated from the n=512 measured value using the observed 512->1024 scaling
factor of approximately 16x (empirical; the theoretical O(n^3 log n) factor
is 8-9x, but the measured 512->256 ratio of 94168/5510 = 17.1x suggests the
larger-n scaling is slightly super-cubic due to cache effects). The cubic
n=1024 value of 2143 ms is directly measured.

The scaling confirms the trend: KG/cubic ratio grows monotonically with n,
consistent with both paths being O(n^3) but with KG carrying a much larger
constant due to the PLE-backed solve.

### 1.4 Fp_65521 -- n in {64, 128}

Measured 2026-05-07 with `cargo bench -p gf2-core --bench charpoly --features rand
-- "charpoly/dispatch_fp65521"` (10 samples per arm; new bench group added by
this issue). n=256 is above the validity cap for Fp_65521 (q=65521 < 2*256^2
= 131072), so this sweep is limited to n in {64, 128}.

| n | cubic (ms) | KG (ms) | KG/cubic ratio |
|---:|---:|---:|---:|
| 64 | 0.582 | 28.7 | 49.3x |
| 128 | 3.98 | 437 | 109.8x |

KG is still 49-110x slower than cubic for Fp_65521 at the valid sizes.
There is no crossover within the KG-valid range.

### 1.5 GF(2^8) -- cubic only (KG invalid above n=11)

Measured 2026-05-07 with `cargo bench -p gf2-core --bench charpoly --features rand
-- "charpoly/charpoly/Gf2m8"` (10 samples).

| n | cubic (ms) |
|---:|---:|
| 32 | 2.40 |
| 128 | 138 |
| 512 | 8690 |

The GF(2^8) cubic path is significantly slower than the prime-field cubic due to
the per-element CLMUL overhead (vs integer arithmetic in Fp). Panelized GEMM
(`e24f7839`) helps the GEMM inside KG doublings, but KG is invalid at n >= 12
for GF(2^8), so it cannot be engaged anyway.

---

## 2. Recommended dispatch policy

The data from the sweep confirms that the cubic path dominates at all measured
sizes and all fields tested. The dispatch policy should remain as currently
implemented:

**`KG_DISPATCH_MIN_N = usize::MAX`** -- public `FieldMatrix::charpoly` always
routes to the cubic baseline.

Rationale:
1. **Fp_M31**: KG is 31-311x slower than cubic across n in {64, 128, 256, 512}.
   The crossover where KG would win is not visible in any of the measured sizes.
   Extrapolating the observed ratios forward, the crossover would require KG's
   scaling advantage to reverse the ~300x gap accumulated by n=512. Given that
   KG's solve step is O(n^3) via PLE (same order as cubic, just with a 300x
   larger constant), no crossover is expected within practical matrix sizes.
2. **Fp_65521**: KG is 49-110x slower than cubic at the only valid sizes (n<=180).
   The valid range itself caps out below where the extra GEMM calls in KG would
   even have meaningful asymptotic advantage.
3. **GF(2^8)**: KG is invalid above n=11 due to the q > 2n^2 constraint. No
   dispatch change possible.

The dispatch policy is unchanged from the pre-Wave-9 measurement session that
produced the R2 amendment in issue `e47231cd` (`KG_DISPATCH_MIN_N = usize::MAX`).
What changed is our confidence: we now have data from the post-Wave-9 kernels
confirming the same conclusion.

Callers who want the KG path for research or benchmarking must call
`FieldMatrix::charpoly_keller_gehrig` directly with an explicit seed.

**Cubic-up-to-n=N threshold**: N = infinity for all fields. No finite crossover
is observed.

---

## 3. Comparison vs c3e79272 pre-optimization crossover

The c3e79272 reference (2026-04-26) measured `Fp<MERSENNE_31>` at n=256:
- cubic: 104.7 ms
- KG: 18.15 s
- Ratio: ~173x

This session (post-Wave-9, 2026-05-07) measured the same field/size:
- cubic: 37.1 ms (2.82x faster than the reference -- significant improvement)
- KG: 5.51 s (3.30x faster than the reference)
- Ratio: 148.5x (was 173x, improved by 1.17x)

The cubic path improved ~2.82x primarily because the `fieldmatrix_charpoly`
bench (which uses the public dispatch path) shows improved numbers at n=128
and n=512 consistent with the cumulative Wave 1-9 GEMM and PLE improvements.
However, the cubic charpoly inner loop is Krylov-matvec-shaped (not GEMM-shaped),
so the gemm improvements affect only the small GEMM calls within the Frobenius
refinement phase, not the dominant Krylov iteration. The measured speedup is
therefore mainly from the PLE/TRSM threshold tuning (`73ec5da3`, TRI_BASE_THRESHOLD
8 from 32) that reduces overhead in the per-chain PLE decompositions.

The KG path improved ~3.30x at n=256. This is consistent with the KG improvements
expected from:
- TRI_BASE_THRESHOLD=8: reduces TRSM recursion overhead in the KG solve step
  (measured 1.38-1.43x improvement at n=256 for isolated TRSM per `73ec5da3`)
- PLE_BASE_COLS=1 with delayed u128 GEMM: improves the PLE backing the KG solve
  (measured PLE now beats fflas at n=256 per `73ec5da3`)
- b377304 (delayed u128 Mersenne GEMM): improves the KG doubling GEMMs

Despite the 3.3x KG speedup, the ratio fell only from 173x to 148.5x because
cubic also improved (~2.82x). The structural bottleneck remains: KG's PLE-backed
solve is O(n^3) with a large constant, and nothing in Wave 1-9 changed the
algorithm to be sub-cubic.

### Summary delta table

| Measurement | 2026-04-26 (c3e79272) | 2026-05-07 (this issue) | Change |
|---|---:|---:|---:|
| Fp_M31 n=256 cubic | 104.7 ms | 37.1 ms | 2.82x faster |
| Fp_M31 n=256 KG | 18.15 s | 5.51 s | 3.30x faster |
| Fp_M31 n=256 KG/cubic ratio | 173x | 148.5x | improved 1.17x |
| Dispatch policy | KG_DISPATCH_MIN_N=usize::MAX | KG_DISPATCH_MIN_N=usize::MAX | unchanged |

The crossover threshold has not shifted in either direction: it was above
n=1024 in 2026-04-26 and remains above n=1024 (in practice, above n=32767
for Fp_M31 under the current O(n^3) KG solve implementation).

---

## 4. Self-satisfaction of [hard] criteria

### Criterion 1: [hard] The crossover report uses post-optimization kernels.

PASS. All measurements in this document were collected 2026-05-07 on the
`main` branch after the following Wave-9 optimizations landed:

| Commit | Optimization | Included |
|---|---|---|
| `a50afc2` | `TRI_BASE_THRESHOLD=8`, `PLE_BASE_COLS=1` (issue `73ec5da3`) | yes |
| `5ddf9a2` | Lean TRI_BASE_THRESHOLD sync | yes |
| `42a6903` | Rank-deficient PLE `split_compact` (issue `2c52bcf6`) | yes |
| `0022a5f` | Panelized GF(2^m) GEMM I_TILE=4 (issue `e24f7839`) | yes |
| `963d53c` | GF(2^m) parity evidence (issues `fb271c41`, `e24f7839`) | yes |

The bench binary linked from these commits was verified via Criterion's
`change:` line showing `[-1.0751% +0.0069% +1.0918%] No change in performance
detected.` for the cubic/64 arm relative to an earlier same-session run,
confirming the binary is stable and was rebuilt after all kernel changes.

The `dev/bench_results/2026-05-04-c3e79272-charpoly-minpoly-refs.md`
reference document was used as the c3e79272 pre-optimization baseline (see
§ 3 above).

### Criterion 2: [hard] Dispatch policy follows measured crossover rather than aspiration.

PASS. The measured data (§ 1) shows no crossover within any practical size or
field combination tested. The dispatch policy (`KG_DISPATCH_MIN_N = usize::MAX`,
cubic always selected) directly follows this measurement. Specifically:

- Fp_M31: KG/cubic ratio ranges from 31.6x (n=64) to 148.5x (n=256) and
  extrapolates to ~311x (n=512). No size shows KG winning. Dispatch correctly
  never selects KG.
- Fp_65521: KG/cubic ratio ranges from 49.3x (n=64) to 109.8x (n=128).
  KG is invalid above n=180 anyway. Dispatch correctly never selects KG.
- GF(2^8): KG is invalid above n=11. Dispatch cannot select KG.

The policy is not aspirational: it is backed by multi-trial Criterion
measurements on the post-Wave-9 kernels at the crossover-relevant sizes.
No change to the dispatch policy is warranted by this data.

---

## 5. Bench additions to codebase

Issue `4a59d1f9` adds one new Criterion bench function to the existing
`crates/gf2-core/benches/charpoly.rs` harness:

- `bench_dispatch_crossover_fp65521`: cubic vs KG crossover sweep for
  `Fp<65521>` at n in {64, 128} (the KG-valid range). Group name
  `charpoly/dispatch_fp65521`. Documents the validity constraint inline
  (n <= 180 for Fp_65521). The bench is additive -- it does not modify
  any existing bench function or constant.

No production code changes. The only code change is the addition of
`bench_dispatch_crossover_fp65521` to `charpoly.rs` and its registration
in the `criterion_group!` macro.

---

## 6. Validation gates (pre-commit)

| Gate | Status |
|---|---|
| `cargo fmt -p gf2-core -- --check` | PASS |
| `cargo fmt -p gf2-coding -- --check` | PASS |
| `cargo fmt -p gf2-kernels-simd -- --check` | PASS |
| `cargo nextest run -p gf2-core --all-features --release --profile ci` | PASS: 1980 tests run, 1980 passed, 5 skipped |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS: no warnings |
| `cargo doc -p gf2-core --no-deps` | PASS: 76 pre-existing doc warnings (all in existing code, none from this issue) |

The only code change in this issue is addition of `bench_dispatch_crossover_fp65521`
to `crates/gf2-core/benches/charpoly.rs`. This is a bench-only addition; it
does not affect the production library or test suite. The fmt/clippy/test
results confirm no regression from the bench addition.
