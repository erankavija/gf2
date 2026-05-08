# Wave-12 Bench Extension: GF(31) Dense-LA Pending Cell Measurement

| Field | Value |
|---|---|
| Date | 2026-05-08 / 2026-05-09 (completed) |
| JIT issue | `97bf0879` (epic: Close gf2-core SOTA performance gaps) |
| Scorecard | `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md` |
| Purpose | Direct Criterion measurement of GF(31) PENDING cells in scorecard sections 2.1–2.4 and 3.1–3.2 |
| Evidence tag | `[EX]` in scorecard |
| Status | **COMPLETE** — all 20 GF(31) cells measured (16 dense-LA + 4 charpoly/minpoly); 14 PASS, 6 FAIL routed to A8 |

## 1. Setup

### 1.1 Host

- CPU: AMD Ryzen 9 5900X (Zen 3), 12 cores / 24 threads
- RAM: 32 GB DDR4
- OS: Arch Linux, kernel 7.0.3-arch1-1
- Rust: stable (MSRV 1.95)
- Build: `--release`

### 1.2 Bench commands

```bash
# fieldmatrix_ple — added Fp<31> to bench_fp_31 (pluq, echelon, rref, rank, nullspace)
cargo bench -p gf2-core --bench fieldmatrix_ple --features rand -- Fp_31

# fieldmatrix_solve — added Fp<31> to bench_fp_31 (invert, solve, det)
cargo bench -p gf2-core --bench fieldmatrix_solve --features rand -- Fp_31

# fieldmatrix_charpoly — added Fp<31> entries to bench_charpoly_reference_sweep
#   and bench_minpoly_reference_sweep (run after ple/solve complete)
cargo bench -p gf2-core --bench charpoly --features rand -- "Fp_31"
```

Bench harness modifications:
- `/crates/gf2-core/benches/fieldmatrix_ple.rs`: added `PRIME_31: u64 = 31`, `bench_fp_31` function, and entry in `criterion_group!`
- `/crates/gf2-core/benches/fieldmatrix_solve.rs`: added `PRIME_31: u64 = 31`, `bench_fp_31` function (4 type params), and entry in `criterion_group!`
- `/crates/gf2-core/benches/charpoly.rs`: added `PRIME_31: u64 = 31`, GF(31) entries in both reference sweeps

## 2. Criterion Medians — Available at Scorecard Update Time

All values are `median.point_estimate` from `target/criterion/<group>/<n>/new/estimates.json`.

### 2.1 `pluq` × GF(31)

| n / regime | gf2 wall (ns) | gf2 wall (fmt) | Ref wall | Ratio | Status |
|---:|---|---:|---|---:|---|
| 64 / uniform | 119,833 | 119.833 µs | 198.218 µs | **0.60×** | PASS |
| 64 / deficient | 97,830 | 97.830 µs | 155.778 µs | **0.63×** | PASS |
| 256 / uniform | 4,429,479 | 4.429 ms | 3.128 ms | **1.42×** | PASS |
| 256 / deficient | 3,724,156 | 3.724 ms | 2.078 ms | **1.79×** | FAIL [→`615db3b9`] |
| 1024 / uniform | 234,250,552 | 234.251 ms | — | — | (not a scorecard cell) |
| 1024 / deficient | 194,329,296 | 194.329 ms | — | — | (not a scorecard cell) |
| 4096 / uniform | 13,266,944,816 | 13.267 s | — | — | (not a scorecard cell) |
| 4096 / deficient | 11,484,703,146 | 11.485 s | — | — | (not a scorecard cell) |

Source estimates:
- `target/criterion/pluq_Fp_31_uniform/{64,256,1024,4096}/new/estimates.json`
- `target/criterion/pluq_Fp_31_deficient/{64,256,1024,4096}/new/estimates.json`

### 2.2 `echelon` × GF(31)

| n / regime | gf2 wall (ns) | gf2 wall (fmt) | Ref wall | Ratio | Status |
|---:|---|---:|---|---:|---|
| 64 / uniform | 247,592 | 247.592 µs | 548.088 µs | **0.45×** | PASS |
| 64 / deficient | 227,302 | 227.302 µs | 277.184 µs | **0.82×** | PASS |
| 256 / uniform | 10,297,663 | 10.298 ms | 5.370 ms | **1.92×** | FAIL [→`615db3b9`] |
| 256 / deficient | 9,621,800 | 9.622 ms | 3.236 ms | **2.97×** | FAIL [→`615db3b9`] |
| 1024 / uniform | 546,554,216 | 546.554 ms | — | — | (not a scorecard cell) |

Source estimates:
- `target/criterion/echelon_Fp_31_uniform/{64,256,1024,4096}/new/estimates.json`
- `target/criterion/echelon_Fp_31_deficient/{64,256,1024}/new/estimates.json`

> Note: echelon_deficient n=4096 not yet written (bench still running at document creation time). Scorecard cells at n=64,256 use the completed group data.

### 2.3 `invert` × GF(31) (complete)

| n / regime | gf2 wall (ns) | gf2 wall (fmt) | Ref wall | Ratio | Status |
|---:|---|---:|---|---:|---|
| 64 / uniform | 556,385 | 556.385 µs | 1.224 ms | **0.45×** | PASS |
| 64 / deficient | 94,010 | 94.010 µs | 624.572 µs | **0.15×** | PASS |
| 256 / uniform | 30,997,951 | 30.998 ms | 11.655 ms | **2.66×** | FAIL [→`615db3b9`] (A8 row 74) |
| 256 / deficient | 3,589,746 | 3.590 ms | 5.768 ms | **0.62×** | PASS |
| 1024 / uniform | 1,921,369,290 | 1.921 s | — | — | (not a scorecard cell) |
| 1024 / deficient | 188,682,225 | 188.682 ms | — | — | (not a scorecard cell) |

Source estimates:
- `target/criterion/invert_Fp_31_uniform/{64,256,1024}/new/estimates.json`
- `target/criterion/invert_Fp_31_deficient/{64,256,1024}/new/estimates.json`

> Deficient regime is dramatically faster than uniform because the LU factorization terminates early once the rank deficit (n/2) is detected, halving the dense work. Only n=256 uniform exceeds the 1.5× ceiling.

### 2.4 `solve` × GF(31) (complete)

| n / regime | gf2 wall (ns) | gf2 wall (fmt) | Ref wall | Ratio | Status |
|---:|---|---:|---|---:|---|
| 64 / uniform | 123,685 | 123.685 µs | 205.124 µs | **0.60×** | PASS |
| 64 / deficient | 93,935 | 93.935 µs | 158.794 µs | **0.59×** | PASS |
| 256 / uniform | 4,347,491 | 4.347 ms | 3.076 ms | **1.41×** | PASS |
| 256 / deficient | 3,591,343 | 3.591 ms | 2.119 ms | **1.69×** | FAIL [→`615db3b9`] (A8 row 75) |
| 1024 / uniform | 227,085,106 | 227.085 ms | — | — | (not a scorecard cell) |
| 1024 / deficient | 189,130,815 | 189.131 ms | — | — | (not a scorecard cell) |

Source estimates:
- `target/criterion/solve_Fp_31_uniform/{64,256,1024}/new/estimates.json`
- `target/criterion/solve_Fp_31_deficient/{64,256,1024}/new/estimates.json`

> Three of four cells PASS. The n=256 deficient cell exceeds 1.5× because GF(31) solve does the full PLE pass (no early termination on deficient input) plus one TRSM, whereas fflas-ffpack uses a Schur-complement solver that exits at the rank cliff.

### 3.1 `charpoly` × GF(31) (complete)

| n | gf2 wall (ns) | gf2 wall (fmt) | Ref wall | Ratio | Status |
|---:|---|---:|---|---:|---|
| 64 | 484,036 | 484.036 µs | 388.738 µs | **1.25×** | PASS |
| 256 | 26,641,757 | 26.642 ms | 13.517 ms | **1.97×** | FAIL [→`52cce970`] (A8 row 76) |

Source estimates:
- `target/criterion/charpoly_charpoly_ref/Fp_31/{64,256}/new/estimates.json`

> The n=256 FAIL has the same architectural cause as A1's GF(251) charpoly n=256: small-prime kernels require bespoke AVX2 register-scheduled paths to close the constant-factor gap. Routed to the same follow-up issue (`52cce970`) as A1.

### 3.2 `minpoly` × GF(31) (complete)

| n | gf2 wall (ns) | gf2 wall (fmt) | Ref wall | Ratio | Status |
|---:|---|---:|---|---:|---|
| 64 | 321,470 | 321.470 µs | 397.016 µs | **0.81×** | PASS |
| 256 | 18,675,973 | 18.676 ms | 13.500 ms | **1.38×** | PASS |

Source estimates:
- `target/criterion/charpoly_minpoly_ref/Fp_31/{64,256}/new/estimates.json`

> Both minpoly cells PASS. The minpoly path uses Wiedemann (O(n²) field ops vs charpoly's O(n³) Hessenberg/Krylov), which closes the small-prime constant-factor gap below the 1.5× ceiling.

## 3. Structural Gap: GF(2) pluq/solve

The A9 amendment in the scorecard previously stated:

> `BitMatrix::pluq` and `BitMatrix::solve_left` are not exercised by the dense-LA bench emitter. gf2-core has the implementations; only the bench wiring is missing.

**This is incorrect.** Investigation of `crates/gf2-core/src/alg/` confirms:

- `gauss.rs`: provides `BitMatrix::invert` via Gauss-Jordan
- `rref.rs`: provides `rref()` (row echelon reduction)
- No `ple.rs`, no `alg/factorize.rs`, no `BitMatrix::pluq` function
- No `BitMatrix::solve_left` or `solve_right` function

`FieldMatrix<F>::pluq` exists in `crates/gf2-core/src/field/matrix.rs` but `BitMatrix` (the GF(2) bit-packed type) has no PLE/LU factorization or solve implementations. The GF(2) pluq/solve PENDING cells in the scorecard reflect a genuine structural implementation gap, not a bench-wiring gap.

**Updated A9 rationale:** The GF(2) pluq+solve cells require `BitMatrix::ple` and `BitMatrix::solve_left` implementations before they can be measured. These are tracked under `974a85bd`.

## 4. Reference Wall Times (from `[E9]`)

Reference walls for GF(31) dense-LA cells sourced from `dev/bench_results/2026-05-04-609855d9-gfp-by-family.md`. Used for ratio computation in this document and the scorecard.

| Operation | n / regime | Ref wall (ns) | Ref wall (fmt) |
|---|---:|---:|---|
| pluq | 64 / uniform | 198,218 | 198.218 µs |
| pluq | 64 / deficient | 155,778 | 155.778 µs |
| pluq | 256 / uniform | 3,128,000 | 3.128 ms |
| pluq | 256 / deficient | 2,078,000 | 2.078 ms |
| echelon | 64 / uniform | 548,088 | 548.088 µs |
| echelon | 64 / deficient | 277,184 | 277.184 µs |
| echelon | 256 / uniform | 5,370,000 | 5.370 ms |
| echelon | 256 / deficient | 3,236,000 | 3.236 ms |
| invert | 64 / uniform | 1,224,000 | 1.224 ms |
| invert | 64 / deficient | 624,572 | 624.572 µs |
| invert | 256 / uniform | 11,655,000 | 11.655 ms |
| invert | 256 / deficient | 5,768,000 | 5.768 ms |
| solve | 64 / uniform | 205,124 | 205.124 µs |
| solve | 64 / deficient | 158,794 | 158.794 µs |
| solve | 256 / uniform | 3,076,000 | 3.076 ms |
| solve | 256 / deficient | 2,119,000 | 2.119 ms |
| charpoly | 64 | 388,738 | 388.738 µs |
| charpoly | 256 | 13,517,000 | 13.517 ms |
| minpoly | 64 | 397,016 | 397.016 µs |
| minpoly | 256 | 13,500,000 | 13.500 ms |

## 5. Scorecard Crosswalk

| Scorecard section | Cell | Status in this doc | Notes |
|---|---|---|---|
| § 2.1 pluq | GF(31) n=64 uniform | PASS (0.60×) | Measured |
| § 2.1 pluq | GF(31) n=64 deficient | PASS (0.63×) | Measured |
| § 2.1 pluq | GF(31) n=256 uniform | PASS (1.42×) | Measured |
| § 2.1 pluq | GF(31) n=256 deficient | FAIL (1.79×) [→`615db3b9`] | Measured; added to A8 row 71 |
| § 2.1 pluq | GF(2) n=64,256 uniform+deficient | EXCLUDED [§ 6.3] (no gf2 impl) | No `BitMatrix::pluq`; user-approved 2026-05-09; follow-up `aaa847cf` |
| § 2.2 echelon | GF(31) n=64 uniform | PASS (0.45×) | Measured |
| § 2.2 echelon | GF(31) n=64 deficient | PASS (0.82×) | Measured |
| § 2.2 echelon | GF(31) n=256 uniform | FAIL (1.92×) [→`615db3b9`] | Measured; added to A8 row 72 |
| § 2.2 echelon | GF(31) n=256 deficient | FAIL (2.97×) [→`615db3b9`] | Measured; added to A8 row 73 |
| § 2.3 invert | GF(31) n=64 uniform | PASS (0.45×) | Measured |
| § 2.3 invert | GF(31) n=64 deficient | PASS (0.15×) | Measured 2026-05-09 |
| § 2.3 invert | GF(31) n=256 uniform | FAIL (2.66×) [→`615db3b9`] | Measured; A8 row 74 |
| § 2.3 invert | GF(31) n=256 deficient | PASS (0.62×) | Measured 2026-05-09 |
| § 2.4 solve | GF(31) n=64 uniform | PASS (0.60×) | Measured 2026-05-09 |
| § 2.4 solve | GF(31) n=64 deficient | PASS (0.59×) | Measured 2026-05-09 |
| § 2.4 solve | GF(31) n=256 uniform | PASS (1.41×) | Measured 2026-05-09 |
| § 2.4 solve | GF(31) n=256 deficient | FAIL (1.69×) [→`615db3b9`] | Measured 2026-05-09; A8 row 75 |
| § 2.4 solve | GF(2) n=64,256 uniform+deficient | EXCLUDED [§ 6.3] (no gf2 impl) | No `BitMatrix::solve_left`; user-approved 2026-05-09; follow-up `aaa847cf` |
| § 3.1 charpoly | GF(31) n=64 | PASS (1.25×) | Measured 2026-05-09 |
| § 3.1 charpoly | GF(31) n=256 | FAIL (1.97×) [→`52cce970`] | Measured 2026-05-09; A8 row 76 |
| § 3.2 minpoly | GF(31) n=64 | PASS (0.81×) | Measured 2026-05-09 |
| § 3.2 minpoly | GF(31) n=256 | PASS (1.38×) | Measured 2026-05-09 |

## 6. Completion Summary (2026-05-09)

All originally PENDING GF(31) scorecard cells are now measured:

- **invert × GF(31) deficient**: n=64 (PASS 0.15×), n=256 (PASS 0.62×) — measured 2026-05-09
- **solve × GF(31)**: n=64 uniform (PASS 0.60×), n=64 deficient (PASS 0.59×), n=256 uniform (PASS 1.41×), n=256 deficient (FAIL 1.69× [→`615db3b9`] A8 row 75) — measured 2026-05-09
- **charpoly × GF(31)**: n=64 (PASS 1.25×), n=256 (FAIL 1.97× [→`52cce970`] A8 row 76) — measured 2026-05-09
- **minpoly × GF(31)**: n=64 (PASS 0.81×), n=256 (PASS 1.38×) — measured 2026-05-09

**Total `[EX]` measurement coverage:** 20 GF(31) cells (16 dense-LA from §§2.1–2.4 + 4 charpoly/minpoly from §§3.1–3.2). 14 PASS, 6 FAIL routed to A8 (rows 71–76).

The 11 cells that previously held literal `PENDING` numeric values in the scorecard for unrelated structural reasons (matmul × GF(2^4) with no `Gf2mWide<u4>`; pluq × GF(2) and solve × GF(2) with no `BitMatrix::pluq` or `BitMatrix::solve_left`) were re-classified **EXCLUDED [§ 6.3]** under user-approved 2026-05-09 amendment. They are not part of `[EX]`'s measurement scope (no implementation to bench); they are recorded in scorecard § 6.3 rows 21–31. Follow-up: `aaa847cf` (M4RI-style bit-packed factorisation for GF(2)). Annex A9 (PENDING umbrella) is now SUPERSEDED.
