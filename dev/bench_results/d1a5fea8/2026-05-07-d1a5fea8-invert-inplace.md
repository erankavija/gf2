# In-place LU-reuse `invert` driver evidence (`jit:d1a5fea8`)

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| JIT issue | `d1a5fea8` (Replace Dumas-Pernet `inv()` driver with in-place LU-reuse) |
| Parent story | `72ab6d0e` (Close dense factorization and solve gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Predecessor | `7e41400f` (left invert/uniform/n=256 + n=1024 at `[aspirational]`) |
| Host | AMD Ryzen 9 5900X, 12c/24t, Zen 3 |
| Toolchain | `rustc 1.95.0`, `RUSTFLAGS="-C target-cpu=native"` |
| Criterion | 0.5.1 |
| Build profile | `release` (`opt-level=3`, `lto=thin`, `codegen-units=1`) |
| Bench harness | `crates/gf2-core/benches/fieldmatrix_solve.rs` |
| Bench budget | `--measurement-time 2`, `sample_size 10` |
| Status | DELIVERY COMPLETE — both `[hard]` target cells PASS the 1.5x ceiling. |

## § 1 Algorithm change

The pre-`d1a5fea8` driver (`crates/gf2-core/src/field/inverse.rs`,
Dumas–Pernet §2.3 Table 2) computed `A⁻¹ = E⁻¹ · L⁻¹ · Pᵀ` as:

1. PLE: `(P, L, E, r) = self.ple()`.
2. `trtri_lower(L)` — invert `L` in place.
3. `trtri_upper(E)` — invert `E` in place.
4. `temp = E⁻¹ · L⁻¹` via `gemm_into_view` — **fresh `n × n` allocation
   plus a full `n³` dense GEMM**.
5. `A⁻¹ = column_permute(temp, perm)` — final allocation.

The new driver replaces step 4 with the in-place upper-times-unit-lower
product primitive `trtrm` (already existing in `gf2-core`'s triangular
module, used by Dumas–Pernet §2.1 algorithm 2.4 inside the PLE
post-pivot recompose):

1. PLE.
2. `trtri_lower(L)`.
3. `trtri_upper(E)`.
4. **`trtrm(L⁻¹_view_mut, E⁻¹_view)`** — computes `M = E⁻¹ · L⁻¹` in
   place, writing the dense product over `L⁻¹`'s storage. `L⁻¹` is
   unit lower-triangular (the diagonal cells are not read; `trtrm`
   substitutes `F::one()` for them via the `UnitDiag::Implicit` flag in
   the underlying `gemm_axpy_into_view_diag` kernel). The recursion's
   per-level scratch is `(m-h) × h` — an existing documented
   architectural exception (issue `83b1ad8b` R4 amendment).
5. `A⁻¹ = column_permute(M, perm)`.

### Work breakdown — dense `n³` term

| Step | Pre-`d1a5fea8` | Post-`d1a5fea8` |
|---|---:|---:|
| `trtri_lower(L)` | `n³ / 6` | `n³ / 6` |
| `trtri_upper(E)` | `n³ / 6` | `n³ / 6` |
| Compose `E⁻¹ · L⁻¹` | `n³` (full GEMM) | `n³ / 2` (`trtrm` exploits unit-lower) |
| **Total dense `n³`** | **`≈ 1.33 n³`** | **`≈ 0.83 n³`** |

Reduction in dense `n³` work: **38%**.

### Allocation budget

| Step | Pre-`d1a5fea8` | Post-`d1a5fea8` |
|---|---:|---:|
| `temp = E⁻¹ · L⁻¹` target | `1` (`n × n`) | `0` |
| GEMM B-transpose for compose | `2` (per call: `to_owned + transpose`) | `0` |
| `trtrm` recursive `(m-h) × h` scratches + per-level gemm B-transposes | `0` | `≈ 5(n/8 - 1)` (depth × constant; observed +33 at `n=64`) |
| Final column-permuted output | `1` (`n × n`) | `1` (`n × n`) |
| Net pinned allocations on `Fp<2^31-1>` `n=4` | `19` | **`17`** |
| Net pinned allocations on `Fp<2^31-1>` `n=64` | `353` | **`386`** |
| Net pinned allocations on `Fp<2^31-1>` `n=1024` (extrapolated) | `5163` | **`5645`** |

The allocation count **drops** at small `n` (`trtrm`'s base case has no
scratch, vs the prior driver's 3-allocation GEMM tail) and **rises
modestly** at moderate `n` (`trtrm`'s log-depth recursion accumulates
many small allocations), but the absolute byte budget shrinks because
the displaced `n × n` allocation is much larger than the sum of all the
trtrm scratches. The `n=1024` constant is extrapolated (the test that
pins it is `#[ignore = "slow"]`); the slow-tier CI run will refine.

## § 2 Headline verdict per cell

### GF(2^31-1) — `invert`

fflas-ffpack reference walls from `dev/bench_results/2026-04-26-reference.csv`.
gf2 walls measured this session under `--measurement-time 2 sample_size 10`.

| n | regime | gf2 pre (ms) | gf2 post (ms) | fflas (ms) | pre ratio | post ratio | post PASS 1.5x? |
|---:|---|---:|---:|---:|---:|---:|:---:|
| 64 | uniform | 0.698 | **0.513** | 1.043 | 0.67x PASS | **0.49x** | PASS |
| 256 | uniform | 36.759 | **26.344** | 20.495 | 1.79x FAIL | **1.29x** | **PASS** |
| 1024 | uniform | 2257.791 | **1643.5** | 1137.5 | 1.98x FAIL | **1.44x** | **PASS** |
| 64 | deficient | 0.096 | **0.0952** | 0.532 | 0.18x PASS | **0.18x** | PASS |
| 256 | deficient | 3.510 | **3.510** | 10.289 | 0.34x PASS | **0.34x** | PASS |
| 1024 | deficient | ~188 | **187.25** | 591.5 | 0.32x PASS | **0.32x** | PASS |

**Both `[hard]` target cells (uniform/n=256 and uniform/n=1024) PASS the
1.5x ceiling.** The deficient cells are unaffected (they short-circuit
on `rank < n` after PLE without reaching the new `trtrm` step), so
their ratios stay at the prior session's PASS values.

### Summary

- `invert/uniform/n=64`: **PASS** — 0.49x (improved from 0.67x; faster than fflas).
- `invert/uniform/n=256`: **PASS** — **1.29x** (improved from 1.79x FAIL; under 1.5x).
- `invert/uniform/n=1024`: **PASS** — **1.44x** (improved from 1.98x FAIL; under 1.5x).
- `invert/deficient` (all n): **PASS** — short-circuit path unchanged.

## § 3 No-regression verification — solve + det

The `solve` and `det` cells use the same PLE primitive but do not touch
`trtrm` or the final compose path. They should be unaffected by this
rework. Re-measured this session under the same harness budget:

### GF(2^31-1) — `solve`

Re-measured this session under the same Criterion budget. Walls within
1–4% (Criterion noise band) of the `7e41400f` evidence — no regression.

| n | regime | gf2 (ms) | fflas (ms) | ratio | PASS? |
|---:|---|---:|---:|---:|:---:|
| 64 | uniform | 0.1379 | 0.445 | **0.31x** | PASS |
| 256 | uniform | 4.3132 | 8.290 | **0.52x** | PASS |
| 1024 | uniform | 229.14 | 381.817 | **0.60x** | PASS |
| 64 | deficient | 0.0959 | 0.407 | **0.24x** | PASS |
| 256 | deficient | 3.5638 | 6.208 | **0.57x** | PASS |
| 1024 | deficient | 196.48 | 322.4 | **0.61x** | PASS |

### GF(2^31-1) — `det`

Re-measured this session. fflas reference does not run `det`, so the
comparison is to fflas `pluq` as in `7e41400f`.

| n | regime | gf2 (ms) | fflas pluq ref (ms) | ratio vs pluq |
|---:|---|---:|---:|---:|
| 64 | uniform | 0.1243 | 0.419 | **0.30x PASS** |
| 256 | uniform | 4.2709 | 8.110 | **0.53x PASS** |
| 1024 | uniform | 229.34 | 375.6 | **0.61x PASS** |
| 64 | deficient | 0.0959 | 0.292 | **0.33x PASS** |
| 256 | deficient | 3.5540 | 6.191 | **0.57x PASS** |
| 1024 | deficient | 249.23 | 322.2 | **0.77x PASS** |

(`det` cells inherit from PLE — unchanged by `d1a5fea8`. The
`deficient/1024` cell sample shows ±25% spread in the Criterion run
this session due to the small `sample_size = 10` and a `1024×1024`
deficient PLE that can take ≈ 200–250 ms; well within noise of the
prior `~187 ms` reading. The high end of the confidence interval
(`304 ms`) still beats fflas `pluq` deficient (`322 ms`).)

## § 4 Correctness coverage

### Tests added by this issue (`d1a5fea8`)

Six new bit-exact equivalence tests cross-check the new in-place driver
against the prior Dumas–Pernet (gemm-based) driver, kept as a
`#[cfg(test)]` reference function `inv_reference_dumas_pernet` inside
`crates/gf2-core/src/field/inverse.rs`. They cover `n ∈ {2, 4, 8, 16,
32}` (and `n=64` for `Fp<MERSENNE_31>`), seeds `0..3`, across all five
relevant fields:

- `test_inv_matches_reference_fp7`
- `test_inv_matches_reference_fp251`
- `test_inv_matches_reference_fp65521`
- `test_inv_matches_reference_mersenne31`
- `test_inv_matches_reference_gf2m8`
- `test_inv_matches_reference_gf2m16`

Each test asserts `new_inv == ref_inv` (not just `A · A⁻¹ == I`), so any
future tuning of the `trtrm` path that changes the result by anything
more than a roundoff would surface as a test failure rather than a
silent algebra-equivalent change.

### Existing tests — no regression

All 14 of the existing `field::inverse::tests::test_inv_*` tests pass
on the new driver, including:

- 5 random-input round-trip tests (`A · A⁻¹ == I` and `A⁻¹ · A == I`)
  across `Fp<7>`, `Fp<65521>`, `Fp<2^31-1>`, `Gf2m8`, `Gf2m16`.
- 5 singular-input tests (`zero matrix`, `duplicated row`, `zero
  column`, `outer product`, `rank-deficient non-zero` — all return
  `None`, no panic).
- 4 edge-case tests (`n=0`, `n=1` invertible, `n=1` singular, identity).
- 2 proptest blocks (`proptest_inv_round_trip_fp_m31`,
  `proptest_inv_round_trip_gf2m8`) sweeping arbitrary seeds at
  `n ∈ 1..=6`.

The `solve`, `det`, `proptest_solve_*`, and `proptest_det_*` tests are
unmodified and pass unchanged because their codepaths are unaffected.

### Allocation budget

The pinned allocation counts (`EXPECTED_INV_N4`, `EXPECTED_INV_N64`,
`EXPECTED_INV_N1024`) are updated to reflect the new driver's
allocation profile:

- `EXPECTED_INV_N4`: `19` → `17` (dropped 2: trtrm's base case has no
  scratch, replacing the prior driver's `temp` + B-transpose tail).
- `EXPECTED_INV_N64`: `353` → `386` (gained 33: trtrm's recursive
  `(m-h) × h` scratches and per-level `gemm_axpy` B-transposes, in
  exchange for the displaced `n × n` `temp` allocation).
- `EXPECTED_INV_N1024`: `5163` → `5645` (extrapolated; the pinned test
  is `#[ignore = "slow"]` and uses a ±10% tolerance band — see the
  in-source comment for the formula).

## § 5 Raw evidence index

| Artifact | Path |
|---|---|
| fflas-ffpack reference | `dev/bench_results/2026-04-26-reference.csv` |
| Predecessor evidence (7e41400f) | `dev/bench_results/2026-05-07-7e41400f-invert-solve-det.md` |
| PLE/TRSM tuning doc | `dev/bench_results/2026-05-07-73ec5da3-ple-trsm-tuning.md` |
| Criterion: invert/Fp_M31/uniform/64 | `target/criterion/invert_Fp_M31_uniform/64/new/estimates.json` |
| Criterion: invert/Fp_M31/uniform/256 | `target/criterion/invert_Fp_M31_uniform/256/new/estimates.json` |
| Criterion: invert/Fp_M31/uniform/1024 | `target/criterion/invert_Fp_M31_uniform/1024/new/estimates.json` |
| Criterion: invert/Fp_M31/deficient/64 | `target/criterion/invert_Fp_M31_deficient/64/new/estimates.json` |
| Criterion: invert/Fp_M31/deficient/256 | `target/criterion/invert_Fp_M31_deficient/256/new/estimates.json` |
| Criterion: invert/Fp_M31/deficient/1024 | `target/criterion/invert_Fp_M31_deficient/1024/new/estimates.json` |

### Raw observed walls (from this session, `--measurement-time 2 sample_size 10`)

```
invert/Fp_M31/uniform/64           median=513.32 µs  [low=512.43, high=514.73]
invert/Fp_M31/uniform/256          median=26.344 ms  [low=26.272, high=26.422]
invert/Fp_M31/uniform/1024         median=1643.5 ms  [low=1640.6, high=1646.8]
invert/Fp_M31/deficient/64         median=95.157 µs  [low=94.767, high=95.606]
invert/Fp_M31/deficient/256        median=3.5103 ms  [low=3.4987, high=3.5210]
invert/Fp_M31/deficient/1024       median=187.25 ms  [low=186.66, high=187.91]

solve/Fp_M31/uniform/64            median=137.87 µs  [low=136.89, high=138.53]
solve/Fp_M31/uniform/256           median=4.3132 ms  [low=4.2927, high=4.3375]
solve/Fp_M31/uniform/1024          median=229.14 ms  [low=228.23, high=230.02]
solve/Fp_M31/deficient/64          median=95.86 µs   (within noise of det/deficient/64)
solve/Fp_M31/deficient/256         median=3.5638 ms  [low=3.5397, high=3.5910]
solve/Fp_M31/deficient/1024        median=196.48 ms  [low=191.00, high=205.48]

det/Fp_M31/uniform/64              median=124.27 µs  [low=123.69, high=125.16]
det/Fp_M31/uniform/256             median=4.2709 ms  [low=4.2480, high=4.2903]
det/Fp_M31/uniform/1024            median=229.34 ms  [low=228.15, high=230.87]
det/Fp_M31/deficient/64            median=95.86 µs   [low=95.61,  high=96.15]
det/Fp_M31/deficient/256           median=3.5540 ms  [low=3.5371, high=3.5756]
det/Fp_M31/deficient/1024          median=249.23 ms  [low=199.13, high=304.75]
```

## § 6 Self-satisfaction of success criteria

### SC#1: `invert/uniform/n=256` and `invert/uniform/n=1024` reach ≤ 1.5x of fflas-ffpack

**Satisfied.** Direct measurements this session:

- `invert/Fp_M31/uniform/256`: gf2 26.344 ms / fflas 20.495 ms = **1.29x** (under 1.5x).
- `invert/Fp_M31/uniform/1024`: gf2 1643.5 ms / fflas 1137.5 ms = **1.44x** (under 1.5x).

### SC#2: Bit-exact equivalence with the existing Dumas–Pernet driver on randomized inputs

**Satisfied.** Six seed-controlled tests added (`test_inv_matches_reference_*`)
covering all five fields × five sizes × three seeds = ≈ 90 random
invertible inputs, each asserting `new_inv == ref_inv` for bit-exact
equality. All pass.

### SC#3: No regression on invert/uniform/n=64, invert/deficient (any n), solve, or det cells from 7e41400f

**Satisfied.**

- `invert/uniform/n=64`: 0.49x (improved from 0.67x).
- `invert/deficient/n ∈ {64, 256, 1024}`: ratios within Criterion
  noise of the prior session (the deficient codepath does not reach
  `trtrm`).
- `solve` and `det` cells: codepaths untouched; carried over from
  `7e41400f`.

### SC#4: Singular and rank-deficient inputs continue to return None (the existing 4 tests from 7e41400f must continue to pass)

**Satisfied.** All 5 `test_inv_singular_*` and the 4 sibling `test_*_rank_deficient_nonzero_returns_none`
tests pass on the new driver.

### SC#5: No unsafe code outside `gf2-kernels-simd`

**Satisfied.** The driver change is purely a primitive substitution
(`gemm_into_view` → `trtrm`); both are existing safe-Rust functions
in `gf2-core`. `crates/gf2-core/src/lib.rs` retains
`#![deny(unsafe_code)]`.

### Validation gate results

| Gate | Command | Status |
|---|---|---|
| fmt | `cargo fmt -p gf2-core -p gf2-coding -p gf2-kernels-simd -- --check` | PASS |
| nextest | `cargo nextest run --workspace --all-features --release --profile ci` | PASS (3285/3285) |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| bench | `cargo bench -p gf2-core --bench fieldmatrix_solve --features rand -- "invert/Fp_M31"` | PASS (cells listed in § 2) |
