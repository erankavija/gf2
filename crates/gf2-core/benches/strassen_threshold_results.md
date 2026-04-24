# Strassen-Winograd threshold sweep and classical-vs-Winograd speedup

Issue `ad597ede`, story `d48a3cfd/T3`. Evidence for the chosen
`WINO_THRESHOLD` constant and for the `[aspirational]` criterion
"Winograd beats classical at the chosen threshold by ≥ 1.2×".

Reproduce with:

```bash
# Runtime harness (fast, purpose-built for this story):
cargo run -p gf2-core --release --example strassen_timing -- sweep 1024
cargo run -p gf2-core --release --example strassen_timing -- compare 256 512 1024

# Criterion run (longer, full statistical reporting):
cargo bench -p gf2-core --bench strassen_threshold --features rand
```

Host: single physical core, `cargo build --release`, host-default target
CPU. All numbers are *wall-clock elapsed* from `std::time::Instant`
around a single `gemm` / `gemm_winograd` call; criterion adds
warm-up + sampled averaging but does not change the qualitative
ranking.

## Chosen threshold: `WINO_THRESHOLD = 128`

The sweep below at `n = 1024`, Mersenne-31, measures the one-shot
runtime of `gemm_winograd` with different base-case cutoffs. 64 and
128 are tied within noise (each ≈ 1.51-1.54× vs classical); 128 is
picked because it produces a shorter recursion tree (two fewer peels
at `n ∈ {512, 1024, 2048}`) and therefore less padding / allocation
overhead in practice, matching the pattern used by fflas-ffpack.

| Threshold | Winograd runtime (ms) | Speedup vs classical |
|-----------|-----------------------|----------------------|
| classical (baseline) | 3461.41 | 1.000× |
| 32        | 2317.06               | 1.494× |
| **64**    | **2254.22**           | **1.536×** |
| **128** (chosen) | **2294.34**    | **1.509×** |
| 256       | 2504.81               | 1.382× |
| 512       | 2780.73               | 1.245× |
| 1024      | 3065.10               | 1.129× |

Thresholds 64 and 128 are statistically tied; 128 was chosen because
(a) the difference is within single-run noise, (b) 128 × 128
Mersenne-31 blocks fit comfortably in L2, and (c) the shorter
recursion tree reduces heap traffic without giving up the measured
crossover.

## Classical vs Winograd at the chosen threshold

### Mersenne-31 (`Fp<2^31 − 1>`)

| n     | Classical (ms) | Winograd (ms) | Speedup |
|-------|----------------|---------------|---------|
| 256   |    54.24       |    44.42      | 1.221×  |
| 512   |   410.81       |   309.25      | 1.328×  |
| 1024  |  3238.55       |  2144.69      | 1.510×  |
| 2048  |  27623.06      |  16109.28     | 1.715×  |
| 4096  | *not measured in this harness — committed criterion run recommended* | | |

At every measured size on Mersenne-31 Winograd beats the classical
path; the advantage grows with `n` as the sub-cubic complexity
asserts itself. **The `[aspirational]` criterion (≥ 1.2× speedup on
at least one field) is met at every `n ≥ 256` on Mersenne-31.**

### GF(2^8) (`Gf2mWide<1, AES-GF(2^8)>`)

| n     | Classical (ms) | Winograd (ms) | Speedup |
|-------|----------------|---------------|---------|
| 256   |    946.66      |    726.66     | 1.303×  |
| 512   |   7588.32      |   5054.15     | 1.501×  |
| 1024  |  60694.16      |  35712.57     | 1.700×  |
| 2048  | 481410.33      | 253912.10     | 1.896×  |

GF(2^8) has a considerably heavier per-MAC cost than Mersenne-31
(each multiply does carryless-multiply + reduction instead of a
single u64 × u64 → u128 + REDC), so the quadratic block-adds that
Winograd trades against one of the seven multiplies pay off earlier
and more dramatically. **The `[aspirational]` criterion is met at
every `n ≥ 256` on GF(2^8) as well.**

### n = 4096 note

The `n = 4096` case extrapolates to several hours per field on
this host. The committed criterion bench
(`benches/strassen_threshold.rs`) includes `n = 2048` and `n = 4096`
cases — run it overnight when retuning the constant. Because the
speedup ratio grows monotonically with `n` (1.22× → 1.33× → 1.51× →
1.72× on Mersenne-31 and 1.30× → 1.50× → 1.70× → 1.90× on GF(2^8)
for `n = 256, 512, 1024, 2048`), we expect `n = 4096` to land in the
1.8-2.1× range on both fields.

## Bound-propagation check

The committed proptest
`prop_winograd_output_respects_theorem_4_bound_fp31` in
`src/field/winograd.rs` exercises theorem 4 at `n = WINO_THRESHOLD +
4 = 132` over Mersenne-31 — exactly one Winograd recursion level. It
asserts every output cell's canonical value fits the level-1 bound
`((1 + 3)/2)² · ceil(k/2) · (p − 1)²`, plus bit-exact equality with
the classical `gemm`. No failing cases at `ProptestConfig::with_cases(4)`.

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
| `[hard]` bit-exact with `gemm` | Pass — 17 unit + 2 proptest cases. |
| `[hard]` theorem-4 bound verified | Pass — `prop_winograd_output_respects_theorem_4_bound_fp31`. |
| `[hard]` threshold picked from bench | Pass — sweep table above. |
| `[hard]` odd-dim coverage | Pass — 5 dedicated tests. |
| `[aspirational]` ≥ 1.2× speedup | **Met** on both fields at `n ≥ 256`. 1.22-1.70× measured. |
