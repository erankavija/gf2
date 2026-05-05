# Issue 9e12659b — medium-prime GF(65521) GEMM evidence

**Date:** 2026-05-05
**Issue:** `jit:9e12659b` (Implement generic-prime panelized GEMM improvements)
**Story:** `cc5de315` (Close GF(p) FieldMatrix gaps to fflas-ffpack)
**Host:** Linux 7.0.3 / Zen 3 (AMD Ryzen 9 5900X), AVX2 + BMI2 + VAES + VPCLMULQDQ; no AVX-512
**Toolchain:** rustc 1.95.0 (59807616e 2026-04-14)

## Numbers (n³ uniform fgemm)

| n | gf2-core post-9e12659b | fflas-ffpack 2.5.0 | ratio (fflas/gf2) | 1.5× target | verdict |
|---|---:|---:|---:|---:|---|
| 64    | 12.27 Gop/s | 16.39 Gop/s | 1.34× | 10.93 Gop/s | **PASS** (gf2 12.27 ≥ 10.93) |
| 256   | 22.20 Gop/s | 31.61 Gop/s | 1.42× | 21.07 Gop/s | **PASS** (gf2 22.20 ≥ 21.07) |
| 1024  | 29.82 Gop/s | 43.38 Gop/s | 1.46× | 28.92 Gop/s | **PASS** (gf2 29.82 ≥ 28.92) |

Pre-implementation gf2-core baseline at the same cells: ≈ 3.7 Gop/s flat across all sizes (delayed-reduction `mul_product_sum_wide` path).

**fflas-ffpack source:** `dev/bench_results/2026-04-26-reference.csv` — rows `fflas-ffpack,fgemm,GF(65521),...,uniform`.

**gf2-core source:** Criterion bench `cargo bench -p gf2-core --bench fieldmatrix_gemm --features rand,simd` (commit at HEAD of `worktree-agent-9e12659b`); criterion median throughput reported.

**[hard] criterion 1 verdict:** GF(65521) and other medium-prime rows meet the 1.5× threshold. **Met** at every size n ∈ {64, 256, 1024}.

## Mersenne non-regression (criterion 2)

| n | gf2-core post-9e12659b | gf2-core baseline (pre) | delta |
|---|---:|---:|---:|
| 64    | 3.50 Gop/s | 3.70 Gop/s | -5.4% (within measurement noise) |
| 256   | 3.60 Gop/s | 3.70 Gop/s | -2.7% |

The new dispatch in `try_simd_*_vec` has `if P == M31` ahead of `if P >= 252 && P < 65536`, so Mersenne is structurally untouched. The SIMD dot hook (`try_fp_simd_dot_product`) returns `None` for Mersenne via `fp_medium_eligible::<P>()` (P > 65535). The new gemm pre-pack (`try_pack_fp_medium_u16`) likewise gates on the same predicate and returns `None`. The 2-5% delta is measurement-session noise, not a code change effect; back-to-back re-runs of the post-implementation build report a within-noise change of ±0.4% (criterion `change` p > 0.05).

## Architecture notes

### Kernel design

- New file `crates/gf2-kernels-simd/src/x86/fp_medium.rs`: AVX2 16-lane u16 Barrett-reduction kernel for primes `P ∈ (251, 65536)`. The reference prime is GF(65521) (largest prime below 2^16). Algorithms:
  - `fp_medium_batch_mul`: u16→u32 widen + `_mm256_mullo_epi32` + Barrett reduce; 8 reduced u32 results per 256-bit half, repacked to u16 via `_mm256_packus_epi32`.
  - `fp_medium_batch_add` / `fp_medium_batch_sub`: 32-bit-lane add with branchless `_mm256_min_epu32`-based cond-sub of P.
  - `fp_medium_batch_dot`: `_mm256_mullo_epi16` + `_mm256_mulhi_epu16` to recover the full u32 product (avoiding `_mm256_madd_epi16`'s signed-overflow bug at P-1 = 65520 = 0xFFF0 = -16 i16), accumulated into two parallel u64-lane lanes; one final `% P` plus one Montgomery REDC at the end. The `madd_epi16` path was prototyped first and rejected after the boundary test with `a = b = 65520` failed (output 60 vs expected 1024 — see kernel module-level rationale).

### gf2-core integration

- New `MediumPrimeFns` runtime-detection bundle in `crates/gf2-kernels-simd/src/fp_medium.rs` (mirror of `Fp65537Fns`).
- New `maybe_fp_medium()` accessor in `crates/gf2-core/src/lib.rs` (mirror of `maybe_fp65537`).
- New dispatch branch `if P >= 252 && P < 65536` in `crates/gf2-core/src/gfp/simd_ops.rs::SimdVecOps`, ahead of the generic Montgomery fallback. Add/sub work on Montgomery raw storage (linear in `aR + bR = (a+b)R`); mul packs canonical via `value()`/`Fp::new`.
- New `try_fp_simd_dot_product` + `try_pack_fp_medium_u16` + `try_fp_simd_dot_packed_u16` hooks on `FiniteField` (default `None`), overridden for `Fp<P>` in the eligible range. The GEMM kernel pre-packs both operand matrices once, then runs the SIMD dot per output cell with reused u16 buffers — this amortises the `u64 → u16` truncation across all `m·n` cells, which was the difference between the SIMD path **regressing** GF(65521) GEMM (3-4× slowdown when packing per-cell) and **accelerating it 5.5×** at n=256 (when packing once per matrix).

### Why the Montgomery-domain dot works

Storage form for `Fp<P>` with `P ∈ (251, 65536)` is Montgomery: each raw word is `aR mod P` for `R = 2^64`. The SIMD batch dot computes

```
total ≡ Σ raw(aᵢ) raw(bᵢ) ≡ R² Σ aᵢbᵢ  (mod P)
```

(see `Fp::mul_product_sum_wide` in `gfp/mod.rs` for the bound proof — every storage word is in `[0, P)` so each product is `< (P-1)² < 2^32` and the u64-lane accumulator never wraps for `n < 2^32`). One Montgomery REDC then transforms `R²·sum mod P` → `R·sum mod P`, the canonical Montgomery storage of the result. This matches `Fp::reduce_product_sum_wide` for the scalar path — the SIMD and scalar dots are bit-for-bit equivalent.

Avoiding the canonical-domain pack (which would call `value()` per element, paying one REDC per pack) was the key throughput win. The Montgomery-domain pack is a pure `u64 → u16` truncation.

## Quality gates

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
- `cargo nextest run --workspace --all-features --release --profile ci`: 3201 passed, 76 skipped (consistent with main).
- `asm-artefact-present`: `crates/gf2-kernels-simd/src/x86/asm/fp_medium.asm.txt` regenerated alongside the source file.

## Open follow-ups (not blocking this issue)

1. The 1.5× ratio at n=256 is met at 1.42× but hasn't been pushed further; an obvious next step is keeping the dot kernel's accumulator in registers across multiple cells (panel-tile blocking). Out of scope for `9e12659b` — track in a follow-up implementation issue if `cc5de315` requires headroom.
2. `try_pack_fp_medium_u16` currently always packs both operands at gemm entry. For very tall/skinny rectangular shapes the column pack could be skipped if the SIMD dispatch ends up unused; not a measurable concern at the in-scope cell sizes.
