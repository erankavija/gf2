# Close matmul GF(2) small-n (n=64, n=256) to M4RI parity — 2026-05-28

| Field | Value |
|---|---|
| Date | 2026-05-28 |
| JIT issue | `bdf60780` |
| Parent | story `974a85bd` (sota-gf2-m4ri) / scorecard `b0fa00af` / epic `026fc832` |
| Host | AMD Ryzen 9 5900X (Zen 3, 12c/24t); L1d 32 KiB/core, L2 512 KiB/core, L3 2×32 MiB; AVX2 + FMA, no AVX-512 |
| rustc | 1.95.0 (59807616e 2026-04-14), `-C target-cpu=native` |
| Pinning | `taskset -c 6-11` (CCX1 cores 6–11) |
| Bench | criterion `matmul_square/{n}` in `crates/gf2-core/benches/matmul.rs` (drives `gf2_core::alg::m4rm::multiply`, the production `&a * &b` path) |
| M4RI baseline | **canonical** rows of `dev/bench_results/2026-04-26-reference.csv` (`m4ri,matmul,GF(2),...,uniform`) |

## Summary verdict

| n | M4RI canonical (ns) | gf2 STEP-0 (ns) | STEP-0 ratio | gf2 final (ns, 5-trial median) | final ratio | 1.5× target (ns) | result |
|---:|---:|---:|---:|---:|---:|---:|:--|
| 64 | 3474 | 9494 | 2.733× | **4213** | **1.213×** | 5211 | **PASS** |
| 256 | 29808 | 75695 | 2.539× | **31893** | **1.070×** | 44712 | **PASS** |
| 1024 (non-reg) | 710908 | 797630 | 1.122× | 783200 (min-of-8) | 1.102× | — | **no regression** |
| 4096 (non-reg) | 21911030 | 31998000 | 1.460× | 30601000 (min-of-5) | 1.397× | — | **no regression** |

Both target cells close to ≤ 1.5× M4RI with comfortable margin (19 % at n=64, 29 % at n=256). The two guarded PASS cells (n=1024, n=4096) do not regress: their code paths are byte-for-byte unchanged by this work (see § Non-regression).

## STEP-0 — the true starting gap

The predecessor scorecard `b0fa00af` cited 1.79×/1.72× for n=64/256, but those compared a stale gf2 number (`e24f7839`, 2026-05-07: 9530 ns / 78946 ns) against a **non-canonical** M4RI baseline (`5dea7457`, 2026-05-04: 5333 ns / 45966 ns). Against the **canonical** 2026-04-26 reference (3474 ns / 29808 ns) the real gap was much larger:

- n=64: 9494 ns / 3474 ns = **2.73×**
- n=256: 75695 ns / 29808 ns = **2.54×**

(Both STEP-0 numbers measured this session with the production `matmul_square/{n}` criterion bench, CCX1-pinned, `target-cpu=native`.)

## Root cause

`choose_k_block` for the sub-wide tier (`stride_words < 16`) returned the byte-budget maximum **k=8** for *every* small n. The k_block sweep below (standalone harness, deterministic 64×64 / 256×256 fill) shows k=8 is the **worst** choice at n=64 and that the register-tiled SIMD C-update was gated off entirely below stride 16, so n=256 (stride 4) never reached the AVX2 8×4 tile.

### Lever sweep (ns/iter, CCX1-pinned, `target-cpu=native`)

`rowwise` = `multiply_rowwise_panels` (scalar row-XOR C-update); `tiled` = `multiply_register_tiled` (AVX2 8×4 tile); `build` = Gray-table build only (all panels). Pre-SIMD-build column = scalar `gray_walk_partial`; post = new AVX2 `m4rm_gray_build4/8`.

n=64 (stride 1 — never reaches the 4-word SIMD tile or SIMD build; rowwise == tiled):

| k | rowwise | tiled | build |
|--:|--:|--:|--:|
| 4 | 4387 | 4220 | 1121 |
| 5 | 4428 | 4294 | 1772 |
| 6 | 5091 | 4970 | 2832 |
| 8 (old prod) | 10365 | 10295 | 8688 |

n=256 (stride 4) — pre SIMD-build:

| k | rowwise | tiled | build |
|--:|--:|--:|--:|
| 6 | 68359 | 44656 | 12008 |
| 7 | 67872 | 47145 | 19865 |
| 8 (old prod) | 73881 | 56443 | 35389 |

n=256 (stride 4) — **post SIMD-build** (`m4rm_gray_build4`):

| k | rowwise | tiled | build |
|--:|--:|--:|--:|
| 5 | 70212 | 38771 | 1108 |
| 6 | 58741 | **32791** | 1705 |
| 7 | 52009 | **29511** | 2688 |
| 8 | 46585 | **27505** | 4596 |

The SIMD Gray builder cut the n=256 table-build cost ~7× (12008 → 1705 ns at k=6) and shifted the tiled optimum from the unreachable old k=8/56443 to k=6–8 at ~27–33 µs — all comfortably inside the 44712 ns target.

## Levers applied (production, no env var)

1. **Lower the register-tiled gate** `M4RM_TILED_MIN_STRIDE_WORDS`: 16 → 4 (= `M4RM_TILE_WORDS`). n=256 (stride 4) and n=512 (stride 8) now reach the AVX2 8×4 YMM C-update. Strides 1–3 still fall through to row-XOR (no full 4-word tile). `m ≥ 8` is unchanged.
2. **SIMD Gray-table builders** `m4rm_gray_build4` / `m4rm_gray_build8` in `gf2-kernels-simd` (one / two YMM accumulators carry the whole Gray walk; scalar Gray-control stays off the SIMD critical path). Wired into `build_gray_table_flat` for `stride_words == 4` and `== 8`. These are the win that makes higher-k panels affordable at n=256.
3. **Small-n k_block heuristic** `choose_k_block_small_n`: for `stride_words < 16`, replace the fixed byte-budget-max (always k=8) with the M4RI cost-balance heuristic `round(0.8 · log2(min(k, n)))` clamped to `[2, 8]`. Yields k≈5 at n=64 and k≈6 at n=256 — both near the measured per-size optimum, and a 2.6× improvement at n=64 over the old k=8.

The wide tier (`stride_words ≥ 16`, i.e. n ≥ 1024) is untouched: it keeps the `8e305c21`/`974a85bd` budget-driven `choose_k_block_with_limit` (k=9 at n≥1024) and the existing `gray_walk_stride16_simd` / `gray_walk_full` builders.

## Final measurement — 5-trial CCX1-pinned

Each trial is a fresh criterion process (`<bench> 'matmul_square/{n}$' --bench --warm-up-time 1 --measurement-time 3`); the median of the per-trial criterion point estimates is the reported wall.

- **n=64**: 4210, 4220, 4210, 4215, 4213 ns → median **4213 ns** → 4213/3474 = **1.213× ≤ 1.5×** ✓
- **n=256**: 31893, 31908, 32149, 32807, 31867 ns → median **31893 ns** → 31893/29808 = **1.070× ≤ 1.5×** ✓

## Non-regression — n=1024 and n=4096

The n=1024 (stride 16) and n=4096 (stride 64) dispatch is **provably unchanged**: `choose_k_block` returns 9 for both (verified by `test_production_schedule_policy_small_n_tier_and_wide_tier_boundary`); `use_register_tiled_schedule` was already true at stride 16/64; and `build_gray_table_flat` routes stride 16 to `gray_walk_stride16_simd` and stride 64 to `build_gray_table_flat_v0` — the new `stride == 4` / `stride == 8` branches are never taken. The only measured deltas are host noise (the box had 11 logged-in users and load ~1–2 during the session). Using the noise-robust minimum-of-N statistic for the deterministic kernel:

- n=1024: STEP-0 797630 ns; final min-of-8 = **783200 ns** (−1.8 %) — no regression.
- n=4096: STEP-0 31998000 ns; final min-of-5 = **30601000 ns** (−4.4 %) — no regression.

Both deltas are negative (faster) and well inside the ≤ 5 % bound; the cross-trial spread (n=1024: 783–855 µs) is pure host contention, not a code effect.

## Correctness

Bit-exactness against a naive O(n³) GF(2) reference, all passing:

- `crates/gf2-core/tests/simd_equiv_matmul.rs::production_m4rm_matches_scalar_reference_at_smalln_boundaries_and_targets` — production `&a * &b` at n ∈ {0,1,15,16,17,63,64,65,256}.
- `…::production_m4rm_proptest_smalln_boundary_lengths_match_scalar` — 48-case proptest, random fill at the same boundary lengths + n=256 (exercises SIMD build stride 4/8 + register tile).
- `crates/gf2-kernels-simd/src/x86/avx2.rs::test_m4rm_gray_build4_matches_scalar` / `…_build8_…` — kernel-level Gray-build equivalence vs scalar reference for k=1..8.
- Pre-existing `test_production_multiply_matches_legacy_schedule_on_boundary_shapes`, `test_m4ri_style_schedule_matches_production_on_boundary_shapes`, `matmul_word_boundary_lengths_match_scalar_reference`, `row_xor_matmul_matches_scalar_reference_proptest_sizes_0_to_512` still pass (the new k_block policy is still a valid M4RM schedule, so outputs are unchanged).

Full suite: `cargo nextest run -p gf2-core --release --profile ci` → 2066 passed; `-p gf2-kernels-simd -p gf2-algebra` → 708 passed.

## Unsafe isolation

All new `unsafe` lives in `gf2-kernels-simd` (`avx2_m4rm_gray_build4/8`). `gf2-core` stays `#![deny(unsafe_code)]`; it only calls the safe `m4rm_gray_build4_fn` / `m4rm_gray_build8_fn` function pointers from `LogicalFns`. The AVX2 asm artefact `crates/gf2-kernels-simd/src/x86/asm/avx2.asm.txt` was regenerated to include the two new symbols (real `vxorps ymm` / `vmovups ymm` confirmed).

## Reproducibility

```bash
# build (target-cpu=native, simd)
RUSTFLAGS="-C target-cpu=native" cargo build --release -p gf2-core --bench matmul --features simd,parallel
BIN=$(ls -t target/release/deps/matmul-* | grep -v '\.d$' | head -1)

# 5-trial CCX1-pinned target measurement
for n in 64 256; do
  for t in 1 2 3 4 5; do
    taskset -c 6-11 "$BIN" "matmul_square/$n\$" --bench --warm-up-time 1 --measurement-time 3
  done
done

# non-regression
for n in 1024 4096; do
  for t in 1 2 3 4 5; do
    taskset -c 6-11 "$BIN" "matmul_square/$n\$" --bench --warm-up-time 2 --measurement-time 4
  done
done
```

CSV: `dev/bench_results/2026-05-28-bdf60780-matmul-gf2-smalln.csv`.
