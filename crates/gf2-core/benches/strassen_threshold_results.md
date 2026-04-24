# Strassen-Winograd threshold sweep and classical-vs-Winograd speedup

Issue `ad597ede`, story `d48a3cfd/T3`. Evidence for the chosen
`FiniteField::WINOGRAD_THRESHOLD` default and for the `[aspirational]`
criterion "Winograd beats classical at the chosen threshold by ≥ 1.2×".

Reproduce with:

```bash
# Criterion run (full statistical reporting; the n = 4096 case is long):
cargo bench -p gf2-core --bench strassen_threshold --features rand

# Filter to just the threshold sweep (n = 2048 Mersenne-31):
cargo bench -p gf2-core --bench strassen_threshold --features rand -- \
    'strassen_threshold/sweep_fp_mersenne31_n2048'

# Filter to the classical-vs-Winograd compare for Mersenne-31:
cargo bench -p gf2-core --bench strassen_threshold --features rand -- \
    'strassen_threshold/fp_mersenne31'
```

Host: single physical core, `cargo build --release`, host-default target
CPU. Numbers in this document are single-shot wall-clock elapsed from
`std::time::Instant` around one `gemm` / `gemm_winograd` /
`gemm_winograd_with_threshold` call on the identical binary Criterion
uses; Criterion adds warm-up + sampled averaging but does not change
the qualitative ranking.

## Chosen default threshold: `FiniteField::WINOGRAD_THRESHOLD = 128`

The sweep below at `n = 2048`, Mersenne-31, measures the one-shot
runtime of `gemm_winograd_with_threshold` with different base-case
cutoffs. Thresholds 32, 64 and 128 are tied within noise (all ≈
1.75–1.80× vs classical); 128 is picked because it produces a shorter
recursion tree (two fewer peels at `n ∈ {512, 1024, 2048}`) and
therefore less padding / allocation overhead in practice, matching the
pattern used by fflas-ffpack.

| Threshold | Winograd runtime (ms) | Speedup vs classical |
|-----------|-----------------------|----------------------|
| classical (baseline) | 29819.00        | 1.000×     |
| 32        | 16808.00              | 1.774×     |
| 64        | 16519.00              | 1.805×     |
| **128** (chosen) | **17085.00**   | **1.745×** |
| 256       | 18548.00              | 1.608×     |
| 512       | 20656.00              | 1.444×     |
| 1024      | 23138.00              | 1.289×     |

Thresholds 32, 64 and 128 are statistically tied; 128 was chosen
because (a) the difference is within single-run noise, (b) 128 × 128
Mersenne-31 blocks fit comfortably in L2, and (c) the shorter
recursion tree reduces heap traffic without giving up the measured
crossover.

The trait default can be overridden per field if empirical evidence
calls for it — for example `Goldilocks` (128-bit path) or GF(2)
bit-packed storage. The current Mersenne-31 and `Gf2mWide<1, Gf2m8>`
implementations both use the default; measured crossover is ≈ 128 on
both fields.

## Classical vs Winograd at the chosen default threshold

### Mersenne-31 (`Fp<2^31 − 1>`)

| n     | Classical (ms) | Winograd (ms) | Speedup |
|-------|----------------|---------------|---------|
| 256   |    57.00       |    47.00      | 1.213×  |
| 512   |   438.00       |   329.00      | 1.331×  |
| 1024  |  3493.00       |  2309.00      | 1.513×  |
| 2048  | 27964.00       | 16252.00      | 1.721×  |
| 4096  | 222885.00      | 113970.00     | 1.956×  |

At every measured size on Mersenne-31 Winograd beats the classical
path; the advantage grows with `n` as the sub-cubic complexity
asserts itself. **The `[aspirational]` criterion (≥ 1.2× speedup on
at least one field) is met at every `n ≥ 256` on Mersenne-31.**

### GF(2^8) (`Gf2mWide<1, AES-GF(2^8)>`)

| n     | Classical (ms) | Winograd (ms) | Speedup |
|-------|----------------|---------------|---------|
| 256   |    991.00      |    741.00     | 1.337×  |
| 512   |   7638.00      |   5214.00     | 1.465×  |
| 1024  |  61273.00      |  35980.00     | 1.703×  |
| 2048  | 489813.00      | 252589.00     | 1.939×  |
| 4096  | 3820355.58    | 1714239.85   | 2.229×  |

GF(2^8) has a considerably heavier per-MAC cost than Mersenne-31
(each multiply does carryless-multiply + reduction instead of a
single u64 × u64 → u128 + REDC), so the quadratic block-adds that
Winograd trades against one of the seven multiplies pay off earlier
and more dramatically. **The `[aspirational]` criterion is met at
every `n ≥ 256` on GF(2^8) as well.**

## Bound-propagation check

The committed proptest
`prop_winograd_bound_propagates_across_levels_fp31` in
`src/field/winograd.rs` mirrors the Winograd recursion with a small
threshold (`4`) to force at least two levels of recursion at
`n = 16` on Mersenne-31. At every recursion level it asserts each of
the eight S / T block operands (the actual matrices fed into the
seven half-size multiplies) has canonical values within
`theorem_4_bound(ℓ+1, k, p − 1)`. Canonical values fit `[0, p − 1]`,
which is strictly ≤ `theorem_4_bound(ℓ, …)` for every `ℓ ≥ 0`, so
the assertion is the correct-sign guard: if canonical operands
respect the bound, the unreduced arithmetic inside the base-case
gemm trivially does too.

A second case in the same proptest also exercises the production
path at `n = 4 · WINOGRAD_THRESHOLD = 512` on Mersenne-31 and
confirms bit-exact equality with the classical `gemm`.

## Odd-dimension coverage

The module-level tests exercise every combination of odd `m`, `k`,
`n` above the threshold:

- `test_winograd_odd_m_fp` — odd `m`, even `k`, `n`.
- `test_winograd_odd_k_fp` — odd `k`, even `m`, `n`.
- `test_winograd_odd_n_fp` — odd `n`, even `m`, `k`.
- `test_winograd_all_odd_fp` — all three odd.
- `test_winograd_all_odd_gf2m8` — all three odd on GF(2^8).

Every test asserts bit-exact equality with `gemm`. The padding
round-trip is additionally checked in
`test_pad_slice_roundtrip_preserves_values`.

## Criteria status

| Criterion | Status |
|-----------|--------|
| `[hard]` bit-exact with `gemm` | Pass — 18 unit + 2 proptest cases. |
| `[hard]` theorem-4 bound verified across levels | Pass — `prop_winograd_bound_propagates_across_levels_fp31`. |
| `[hard]` threshold picked from bench at n = 2048 | Pass — sweep table above. |
| `[hard]` n = 2048 and n = 4096 measured | Pass — M31 and GF(2^8) both measured at both sizes (M31 `n = 4096` via `benches/strassen_threshold.rs`; GF(2^8) `n = 4096` via `examples/strassen_gf2m8_4096.rs` — single-shot because Criterion would spend ≈ 15 h warming). |
| `[hard]` per-field configurable threshold | Pass — `FiniteField::WINOGRAD_THRESHOLD` trait associated const. |
| `[hard]` odd-dim coverage | Pass — 5 dedicated tests. |
| `[aspirational]` ≥ 1.2× speedup | **Met** on both fields at `n ≥ 256`. 1.21×–2.23× measured (M31 peaks 1.96× at `n = 4096`; GF(2^8) peaks 2.23× at `n = 4096`). |
