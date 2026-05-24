# Route A: in-Rust GF(251) f32/FMA cascade rework — design note

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `68cdf4c8` (Prototype in-Rust GF(251) f32/FMA cascade) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 1, item 1 |
| Predecessor | `a70b1c70` (Phase 0 baseline measurements) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2+FMA, no AVX-512 |

This note records the design decisions for reworking the dormant Candidate F
(in-Rust f32/FMA cascade) to compete with Candidate C on GF(251) at
n ∈ {256, 1024}, per the verbatim Phase 1 item 1 in
`dev/active/615db3b9-finite-field-la-sota-plan.md`:

> Rework the existing Candidate F around the precise GF(251) target:
> k-chunking at 268 (= ⌊2²⁴ / 250²⌋), vectorized output reduction,
> lower pack cost, and comparison at n=256/1024 rather than broad
> all-prime dispatch.

## 1. Baseline (from Phase 0)

From `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 5 (drift
check at HEAD) and the predecessor 5-trial sweep
`dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv`:

| cell | Candidate C Gop/s (median) | Existing Candidate F Gop/s | fflas Gop/s |
|---|---:|---:|---:|
| GF(251)/n=256 | 58.98 (drift 61.03) | 54.04 | 128.48 |
| GF(251)/n=1024 | 70.89 (drift 72.44) | 66.04 | 138.32 |

The 1.5x-of-fflas threshold (`gf2/fflas ≥ 0.667`) requires:

- n=256: gf2 ≥ 128.48 × 0.667 = **85.65 Gop/s**
- n=1024: gf2 ≥ 138.32 × 0.667 = **92.21 Gop/s**

Existing F is at 54.04 / 66.04, i.e. 42.0 % / 47.7 % of fflas. The plan's
question is whether the levers identified (k-chunking at 268, vectorized
output reduction, lower pack cost) close the gap to 1.5x or whether the
in-Rust f32 route is structurally below the BLAS-cascade ceiling on
this host class.

## 2. Levers applied

### 2.1 Lower pack cost via direct f32 lookup table

The current dispatch in `crates/gf2-core/src/gfp/simd_ops.rs` lines
519–522 packs `a` and `bt` from `Fp<P>` to `f32` via
`a.iter().map(|x| x.value() as f32).collect()`. Each `.value()` call is
a Montgomery REDC (one `mulx`, one `mul`, one subtract, one cmov ≈ 8
cycles on Zen 3). For n=1024, that's `2 · 1024 · 1024 = 2 097 152` REDC
calls in the pack loop alone — ≈ 16.7 ms at 5 GHz, vs. the inner FMA
work of ~14 ms at 138 Gop/s. The pack cost is comparable to the entire
FMA budget.

Replace with a per-prime f32 lookup table:

```text
from_mont_f32[raw_storage] = (canonical value as f32),  for raw ∈ [0, P)
```

The table has 251 entries × 4 bytes = 1 004 bytes, fitting in 16 cache
lines. Each pack lane becomes a single L1 load + integer-to-f32 store
(no REDC, no integer conversion). Estimated saving ≈ 14 ms at n=1024,
≈ 850 µs at n=256.

The table is built once per prime per process, alongside the existing
`SmallPrimeTables { from_mont, to_mont, barrett_mu }` in
`crates/gf2-core/src/gfp/simd_ops.rs`. The new field is
`from_mont_f32: Vec<f32>`.

### 2.2 Lower output unpack cost via to_mont table

The current dispatch in lines 523–525 unpacks output via
`Fp::<P>::new(byte as u64)` per cell — another REDC call per output.
At n=1024 that's 1 048 576 REDCs ≈ 8.4 ms.

Replace with the existing `to_mont: Vec<u64>` table (already built and
cached): each cell becomes one u64 load + `Fp::from_raw_storage`. The
unpack path is shared with Candidate C's already-cached fast unpack
(simd_ops.rs lines 580–585) — we reuse it verbatim.

### 2.3 Vectorized output reduction in the kernel

The current Candidate F kernel
(`crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` lines 580–585) does
scalar `% p` per output cell after `store_and_reduce_tile`. For the
4×24 tile that's 96 `idiv` calls (~22-25 cycles each on Zen 3) = ~2 300
cycles per tile.

Replace with an AVX2 Barrett-reduction sequence on the 8-lane i32
accumulators *before* storing to scratch:

For an i32 lane value `v ∈ [0, k_max · (p-1)²]` ≤ 268 · 62 500 ≈
16.75 million < 2^24, we have v < 2^24. The Barrett reduction
`v mod 251` proceeds:

1. `q := (v * MAGIC) >> 32` with `MAGIC = ⌈2^32 / 251⌉ = 17 111 423`
   (one `vpmuldq` lane-pair giving the high 32 bits).
2. `r := v - q * 251` (one `vpmullw`/`vpsubd` chain).
3. Correction: `r := if r >= 251 { r - 251 } else { r }`
   (one `vpsubd` + `vpminud`).

This is roughly 4-5 instructions per 8 i32 lanes vs. 8 `idiv` calls.
On Zen 3 the SIMD-Barrett path takes ~6 cycles per 8 lanes instead of
~200 cycles for the scalar `% p`. For 12 accumulator vectors per tile,
that's ~72 cycles vs. ~2 400 — saving ~2 300 cycles per tile.

At n=1024 with `m/M_R · n/N_R = 256 · 43 = 11 008` tiles, the per-tile
saving is ~110 µs total. This is small compared to the pack-cost win
but it's a clear improvement and matches the plan's call-out.

### 2.4 K-chunking at 268 (per-prime k_max for p=251)

The current kernel already computes `k_max = floor(2^24 / (p-1)²)` =
268 for p=251, and the chunk size is `min(k, k_max, K_CHUNK_CAP)`. For
GF(251) at n=1024 this gives 4 chunks (256, 256, 256, 256 = 1024 with
the last partial chunk having 1024-3·268 = 220 < 268, ok). For n=256,
one chunk (256 < 268). This part of the plan is already satisfied by
the existing code; we keep it unchanged.

### 2.5 Inner-kernel register schedule (unchanged)

The existing 4×24 tile uses 12 FMA accumulators + 3 B-tile registers +
1 a-broadcast = 16/16 ymm registers on Zen 3. Two FMA execution ports
on Zen 3 each retire one `_mm256_fmadd_ps` per cycle, so the inner body
is back-end-bound at 6 cycles per 12-FMA step (2 ports × 6 cycles = 12
FMAs / step). This matches the theoretical peak; no register-schedule
change is warranted. Alternative schedules (6×16, 8×8) give lower ILP
or lower lane utilisation.

## 3. Toggle mechanism

The criterion says "non-default dispatch toggle (cargo feature OR
runtime debug switch)". A runtime env-var toggle keeps the change
local to `crates/gf2-core/src/gfp/simd_ops.rs` and avoids adding a
Cargo feature that would propagate across the workspace.

```rust
fn select_f32_path<const P: u64>(_m: usize, _k: usize, _n: usize) -> bool {
    // Production: N_THRESH_PRIME=252 keeps F dormant.
    if P >= N_THRESH_PRIME && P <= 251 { return true; }
    // Route A toggle (issue 68cdf4c8): opt-in measurement path for GF(251).
    if P == 251 && route_a_enabled() { return true; }
    false
}

fn route_a_enabled() -> bool {
    std::env::var("GF2_GF251_ROUTE_A").map(|v| v == "1").unwrap_or(false)
}
```

This is checked at the GEMM dispatch site only (not inside the inner
loop), so the per-call overhead is one env-var read per GEMM call
(~50 ns); no inner-loop cost. The env var is read every call because
`OnceLock`-caching the value would be a semantic gotcha for
test/bench harnesses that set the var per-call.

The production dispatch (`N_THRESH_PRIME = 252`,
`route_a_enabled() == false`) is unchanged: zero impact on default
behaviour, satisfying the "no production dispatch change" criterion.

## 4. Public references consulted

No fflas-ffpack source code, comments, autotuning tables, or
micro-kernel structure was consulted in designing this rework. The
local fflas checkout at `/home/vkaskivuo/Projects/fflas-ffpack` was
not opened.

Public references that informed the design:

1. **Goto, K., and van de Geijn, R. A.** "Anatomy of High-Performance
   Matrix Multiplication." ACM Trans. Math. Softw., 34(3):12, 2008. —
   The register-blocked micro-kernel structure (M_R × N_R inner tile,
   k-axis as innermost loop, pre-packed panels) follows the GotoBLAS /
   BLIS framework. The 4×24 register tile shape is a standard register
   blocking choice for AVX2 / FMA3 hosts; it was already in the
   existing Candidate F per `dev/plans/small_prime_kernel_strategy.md`
   § 5.5.
2. **AMD Software Optimization Guide for AMD Family 19h Processors
   (Zen 3), revision 3.07** (Publication ID 56665). — FMA latency
   (4 cycles), two FMA ports (Pipe FPU0 / FPU1) each with 1-per-cycle
   throughput, 16 ymm-register file, L1d 32 KB / L2 512 KB per core.
   The 12-accumulator FMA chain stays well above the 4-cycle FMA
   latency (12 chains × 4 cycles = 48 cycle dependency horizon) so the
   inner loop is back-end-bound on the FMA ports, not latency-bound.
3. **Granlund, T., and Möller, N.** "Improved division by invariant
   integers." IEEE Trans. Comput., 60(2):165–175, 2011. — Magic-number
   Barrett reduction for unsigned int divide by an invariant. We use a
   plain 32-bit-magic form here because p=251 fits trivially in the
   simple variant.
4. **Dumas, J.-G., Giorgi, P., and Pernet, C.** "Dense Linear Algebra
   over Word-Size Prime Fields." ACM TOMS 35(3), 2009; arXiv:cs/0601133
   — Bound formula `k_max = ⌊2^24 / (p-1)²⌋` for f32 accumulation
   without rounding loss. This is the same formula already used in
   the existing kernel.
5. **gf2-owned prior art:**
   - `crates/gf2-kernels-simd/src/fp_small_f32.rs` and
     `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` — the existing
     Candidate F we are reworking. Internal author: gf2 project.
   - `crates/gf2-kernels-simd/src/fp_small.rs` and
     `crates/gf2-kernels-simd/src/x86/fp_small.rs` — Candidate C's
     16-bit Barrett-reduction kernel; the Barrett-reduction shape on
     i32 lanes for the output-mod step is derived from C's u16
     Barrett (different lane width, same reduction algebra).
   - `dev/plans/small_prime_kernel_strategy.md` § 4.5 / § 5.5 / § 6.1
     — the existing design note for Candidate F.
   - `dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv`
     — empirical baseline that motivated the rework focus on pack/
     reduction overhead.

## 5. Test plan (TDD)

Per the issue criterion:
> Bit-exact equality vs the existing Candidate C output across n in
> {64, 256, 1024} on canonical seeds (proptest or fixed-seed parity
> test).

The kernel under test is the safe-wrapper
`crate::simd::maybe_fp_small_f32().unwrap().batch_gemm_fn` invoked via
the production `fp_small_try_gemm_classical` path with the route-A
env var set. The reference is the production `fp_small_try_gemm_classical`
path with the env var unset (i.e. Candidate C).

Boundary `n` values exercised by the new tests:

- `n ∈ {0, 1, 15, 16, 17, 63, 64, 65, 255, 256, 257, 1023, 1024}`
- `k ∈ {1, 64, 256, 268, 1024}` covering the k_max=268 boundary
- `m ∈ {1, 4, 8, 33}` covering the M_R=4 boundary
- Fixed seeds: `splitmix64(1)`, `splitmix64(2)` for `(a, bt)`.

Tests are integration-level in
`crates/gf2-core/tests/route_a_parity.rs`. They flip the env var
in-process for the prototype call and clear it back afterwards.

## 6. Non-regression measurement plan

Per the issue criterion:
> No regression on currently-PASSing GF(p) cells (delta ≤ 5% under
> same-session measurement at same commit).

Same-session 5-trial measurement plan after the rework lands:

| Field | n | Owner | Threshold |
|---|---:|---|---|
| GF(7) | 256, 1024 | already-PASS at Candidate C | within 5% of cited 34.46 / 68.17 Gop/s |
| GF(31) | 256, 1024 | already-PASS at Candidate C | within 5% of cited 53.74 / 68.98 Gop/s |
| GF(127) | 256, 1024 | already-PASS at Candidate C | within 5% of cited 54.65 / 71.20 Gop/s |
| GF(251) | 256, 1024 | rework target (route A) | head-to-head route-A vs Candidate C |

Non-regression cells are measured with the env var UNSET — i.e. the
production dispatch path which is untouched by this work — so they
exercise exactly the same code as the Phase 0 baseline.

## 7. Risk and open questions

- **Pack-cost dominance.** If pack cost is the binding constraint
  (likely at n=256 where the inner FMA budget is only ~3.6 ms), the
  table-lookup pack will dominate the rework's win. At n=1024 the
  inner FMA budget is ~14 ms and pack is closer to amortised; the
  win will be smaller in relative terms.
- **BLAS-cascade ceiling.** fflas reaches 128–138 Gop/s by delegating
  to OpenBLAS sgemm, which has been hand-tuned over 15+ years. An
  in-Rust kernel at 80% of OpenBLAS sgemm is plausible (since both
  are FMA-port-bound on the same host) but reaching 138 Gop/s × 0.667
  = 92.21 Gop/s at n=1024 is the explicit threshold question. The
  expected result of this rework is a measurable improvement over
  the existing 66.04 Gop/s; whether it clears 92.21 is the empirical
  question the evidence doc resolves.

## 8. Out of scope (per issue)

- Changing production dispatch (`N_THRESH_PRIME`, `select_f32_path`
  default behaviour).
- Route B (BLAS) or route C (integer panel) prototypes — separate
  sibling tasks under epic `026fc832`.
- AVX-512 / VNNI variants — out of scope for `026fc832`, routed to
  epic `7f809931`.

## 9. References

- `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 1, item 1
- `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 5
- `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 1.1
- `dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv`
- `dev/plans/small_prime_kernel_strategy.md` § 4.5 / § 5.5 / § 6.1
- `crates/gf2-kernels-simd/src/fp_small_f32.rs`
- `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs`
- `crates/gf2-core/src/gfp/simd_ops.rs`
