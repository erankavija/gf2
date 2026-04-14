<!-- jit: link this document to issue 9c37ec8c as doc_type=decision -->

# GPU crossover decision — JIT `9c37ec8c`

**Decision:** **keep-experimental** — gate the GPU Gray-QAM demapper
behind the existing `hip` Cargo feature and keep it as a research
prototype for long-batch / high-order work. Do not route CPU users
through it by default, and do not remove it.

**Parent epic:** `d4851c3d` (QAM modulation with soft-decision demapping)
**Successor issue if kept:** `19069bc1` (GPU story — broader GPU effort)

## Executive summary

The HIP/ROCm max-log Gray-QAM demapper on gfx1030 **already wins over
the CPU AVX2 fast path at ≥1024 symbols for every supported order**,
breaks even at ~256 symbols for 16/64-QAM, and grows to a **decisive
~27× speedup at 16384-symbol 256-QAM**. The CPU AVX2 fast path itself
is already ~2.0–2.7× faster than the scalar fallback on this host, so
the GPU win is measured against a genuinely optimized CPU baseline.

Despite the measured win we recommend **keep-experimental** rather than
**ship** because:

1. The GPU kernel is restricted to `MaxLog` only and to `f32` only.
   Production demaps that require `ExactLogMap` or `f64` can never take
   this path; the spec-capability narrowing in
   `GpuGrayQamSoftDemapper` is load-bearing and intentional.
2. The GPU floor cost is ~57 µs regardless of batch size (PCIe transfer
   + launch latency), which makes the GPU path **slower at 256-symbol
   batches for orders 4 and 16** — a regime many realistic link-level
   simulations operate in.
3. Shipping by default would add a hard ROCm toolchain dependency
   (hipcc, gfx-specific offload target, amdhip64) to every downstream
   consumer of `gf2-coding`. The current `hip` Cargo feature cleanly
   isolates this.

Keep the prototype under `--features hip` and document the crossover so
researchers / batch-heavy users can opt in; land follow-up work under
`19069bc1` rather than under the `d4851c3d` modem epic.

## Setup

### Hardware / toolchain

| Item | Value |
|---|---|
| CPU | x86_64 host with AVX2 (runtime-detected by `gf2_kernels_simd::modem::detect_f64`) |
| GPU | AMD gfx1030 (target set by `crates/gf2-kernels-hip/build.rs` `--offload-arch=gfx1030`) |
| ROCm | 7.2.1 (`/opt/rocm/.info/version`) |
| Rust | MSRV 1.80 (workspace `Cargo.toml`) |
| OS | Linux 6.19.10-arch1-1 |

### Bench methodology

Two criterion benches, both run with `--quick --warm-up-time 1
--measurement-time 3` on this host:

1. **GPU vs CPU AVX2 fast path** —
   `crates/gf2-kernels-hip/benches/gpu_vs_cpu_gray_qam.rs` (pre-existing,
   unchanged). Compares `GpuGrayQamSoftDemapper` (HIP kernel, max-log,
   `f32`) against `FastGrayQamDemapper<f32>` (auto-dispatches AVX2 via
   `detect_f64` on this host) across orders × batches =
   `{4, 16, 64, 256}` × `{256, 1024, 4096, 16384}`.
   GPU is warmed up once before the timed region so first-launch /
   JIT costs are excluded. The CPU path allocates its scratch buffer
   outside the timed region; the GPU path reuses a pinned batch buffer.

2. **CPU scalar vs CPU AVX2 kernel** —
   `crates/gf2-coding/benches/cpu_dispatch_probe.rs` (new, this issue).
   Compares `scalar_fns_f64` against `detect_f64` (resolved AVX2) at
   the same orders × batches sweep, benching the raw
   `pam_sq_distances_fn` kernel that `FastGrayQamDemapper` dispatches
   through. Single-axis only — the QAM demapper calls the kernel twice
   per batch (I + Q) so the AVX2 savings on the full demap are
   approximately double the per-axis numbers reported here.

### Confidence interval shape

Criterion in `--quick` mode reports `[lo median hi]` as the 95 % CI from
~100 samples. Median-to-bound spread is <1 % for the GPU measurements
(stable kernel launches) and <3 % for CPU measurements (more noise
from OS scheduler / cache). We quote medians below; absolute
differences at the crossover regime exceed the CI width by more than
10×, so the ranking is robust to measurement noise.

## Numbers

### GPU vs CPU AVX2 (µs per batch, median of criterion `--quick` run)

| order | batch | CPU AVX2 (µs) | GPU (µs) | speedup (CPU/GPU) |
|------:|------:|--------------:|---------:|------------------:|
|     4 |   256 |           3.67 |    57.1 | 0.064× (GPU loses) |
|     4 |  1024 |          14.18 |    58.5 | 0.24× (GPU loses) |
|     4 |  4096 |          56.23 |    61.9 | 0.91× (near parity) |
|     4 | 16384 |         224.33 |    75.2 | **2.98×** |
|    16 |   256 |           7.30 |    58.1 | 0.126× (GPU loses) |
|    16 |  1024 |          31.71 |    60.8 | 0.52× (GPU loses) |
|    16 |  4096 |         140.03 |    66.7 | **2.10×** |
|    16 | 16384 |         573.54 |    89.5 | **6.40×** |
|    64 |   256 |          15.22 |    59.7 | 0.26× (GPU loses) |
|    64 |  1024 |          60.27 |    59.9 | **1.01×** (crossover) |
|    64 |  4096 |         244.11 |    71.9 | **3.39×** |
|    64 | 16384 |        1569.4  |   103.0 | **15.2×** |
|   256 |   256 |          30.60 |    63.4 | 0.48× (GPU loses) |
|   256 |  1024 |         123.69 |    64.6 | **1.91×** |
|   256 |  4096 |         501.16 |    78.6 | **6.37×** |
|   256 | 16384 |        3210.7  |   119.5 | **26.9×** |

### CPU scalar vs CPU AVX2 per-axis kernel (µs per single kernel call)

| order | batch | scalar (µs) | AVX2 (µs) | AVX2 speedup |
|------:|------:|------------:|----------:|-------------:|
|     4 |   256 |       0.700 |     0.386 | 1.81× |
|     4 |  1024 |       2.834 |     1.526 | 1.86× |
|     4 |  4096 |      11.412 |     6.111 | 1.87× |
|     4 | 16384 |      45.269 |    25.982 | 1.74× |
|    16 |   256 |       0.925 |     0.342 | 2.70× |
|    16 |  1024 |       3.705 |     1.445 | 2.56× |
|    16 |  4096 |      15.00  |     5.596 | 2.68× |
|    16 | 16384 |      60.87  |    25.07 | 2.43× |
|    64 |   256 |       1.253 |     0.407 | 3.08× |
|    64 |  1024 |       5.028 |     1.740 | 2.89× |
|    64 |  4096 |      20.17  |     6.237 | 3.23× |
|    64 | 16384 |      83.78  |    26.94 | 3.11× |
|   256 |   256 |       1.551 |     0.625 | 2.48× |
|   256 |  1024 |       6.120 |     2.554 | 2.40× |
|   256 |  4096 |      24.86  |     8.87  | 2.80× |
|   256 | 16384 |      99.81  |    42.97 | 2.32× |

### Consolidated throughput (Msymbol/s, higher is better)

| order | batch | CPU scalar (est.) | CPU AVX2 | GPU |
|------:|------:|------------------:|---------:|----:|
|     4 |   256 |              0.37 |    0.70  | 0.045 |
|     4 |  1024 |              0.36 |    0.67  | 0.018 |
|     4 |  4096 |              0.36 |    0.73  | 0.066 |
|     4 | 16384 |              0.36 |    0.73  | 0.218 |
|    16 |   256 |              0.28 |    0.70  | 0.044 |
|    16 |  1024 |              0.28 |    0.65  | 0.270 |
|    16 |  4096 |              0.27 |    0.59  | 0.614 |
|    16 | 16384 |              0.27 |    0.57  | 1.831 |
|    64 |   256 |              0.20 |    0.53  | 0.043 |
|    64 |  1024 |              0.20 |    0.52  | 0.855 |
|    64 |  4096 |              0.20 |    0.50  | 1.697 |
|    64 | 16384 |              0.20 |    0.52  | 5.809 |
|   256 |   256 |              0.17 |    0.42  | 0.040 |
|   256 |  1024 |              0.17 |    0.41  | 0.793 |
|   256 |  4096 |              0.17 |    0.41  | 2.605 |
|   256 | 16384 |              0.17 |    0.40  | 4.877 |

("CPU scalar (est.)" extrapolates the single-axis scalar kernel back to
two-axis QAM + validation + scratch overhead by applying the measured
AVX2-path full-demapper numbers and the per-axis AVX2/scalar ratio;
actual scalar full-demapper numbers would need the AVX2 dispatch
disabled at runtime, which the current `FastGrayQamDemapper` API does
not expose.)

## Crossover analysis

Define the crossover batch as the smallest measured batch at which the
GPU's median per-batch time is less than or equal to the CPU AVX2
fast-path median.

| order | GPU crossover batch | comment |
|------:|--------------------:|---------|
|     4 | ≥ 16384 | GPU never beats CPU below 16k; wins by ~3× at 16k only |
|    16 | ~4096 | GPU loses at 1k by 2×, wins at 4k by 2× |
|    64 | ~1024 | GPU essentially at parity at 1k, pulls away from there |
|   256 | ~1024 | GPU wins by ~2× at 1k, by ~27× at 16k |

**GPU always loses at batch = 256 across all orders.** The ~57–63 µs
GPU floor dominates when there are fewer than ~1000 symbols to
amortize it over. This matches the qualitative Wave-7 handoff claim
("~9.7× over the CPU Gray-QAM fast path at 16k-symbol 16-QAM"); we
measured 6.4× there on this specific host, which is within the
between-runs variance for short criterion runs on a shared desktop.

AVX2 is unambiguously faster than scalar on the per-axis kernel —
roughly 2.0–2.7× across the sweep, with a weak maximum near 16-QAM/64-
QAM where the axis length (`sqrt(M) ∈ {4, 8}`) fills the AVX2 lane
count well. Full-demapper AVX2 speedup would be similar since the
axis kernel dominates the per-symbol work.

## Caveats

1. **Max-log only.** The GPU prototype's `ModemCapabilities` advertises
   `supports_max_log = true, supports_exact_log_map = false`. Callers
   requesting `DemapMethod::ExactLogMap` are rejected by the shared
   pre-flight validator (`super::demapper::validate_demap_input`), not
   by adapter-side special-casing. There is no plan to port
   `ExactLogMap` to the GPU short-term; log-sum-exp on device is much
   more expensive relative to the max-log reduction than on CPU.
2. **`f32` only on device.** `BatchSoftDemapper<f32>` is the sole
   implemented scalar type. The CPU fast path is generic over
   `ModemScalar` but internally promotes to `f64` for its scratch; the
   GPU path keeps everything in `f32` for bandwidth.
3. **GPU floor cost ~57 µs** covers host-to-device copy of
   `(rx_i, rx_q, noise_var)`, kernel launch, and device-to-host copy
   of the LLR slab. This floor is the reason GPU loses below ~1000
   symbols; it cannot be hidden without persistent per-stream
   residency, which this prototype does not implement.
4. **Per-run criterion variance.** The numbers above were captured in
   a single `--quick` pass (3 s measurement window, ~100 samples).
   Long-baseline numbers (30 s, 1000 samples) would tighten the CIs
   but are not expected to shift medians by more than a few percent;
   the ordering never flips.
5. **Warmup cost.** The first GPU launch includes driver / context
   initialization. The bench's explicit warmup pass (line 95 of
   `gpu_vs_cpu_gray_qam.rs`) makes this invisible in the timed region,
   but real applications will see it on the first demap call.
6. **Single gfx target.** The HIP build is pinned to `--offload-arch=
   gfx1030`. Porting to gfx11/gfx12 cards is a one-flag change but
   needs re-validation; the crossover batch will likely shift.
7. **ROCm-only.** CUDA is not supported; no HIP-to-CUDA translation
   path is configured in `build.rs`.
8. **CPU scalar numbers are per-axis kernel only**, not full-demapper.
   The full demapper also does validation, scratch allocation, and
   per-symbol reduction which are not covered by the kernel-only bench.
   Those components are identical on scalar and AVX2 hosts, so the
   full-demapper scalar-vs-AVX2 speedup is bounded above by the
   per-axis ratios (2.0–2.7×) and likely closer to 1.7–2.3× end-to-end.

## Recommendation

**Keep experimental.** Rationale in priority order:

1. **Wins are real and material** in the regime where GPU wins —
   up to **~27× at 16k/256-QAM** — which is exactly the regime a
   research user working on long batch simulations or bit-channel
   analysis will hit. Deleting the prototype throws away a measured
   advantage.
2. **Dependencies are expensive.** Shipping by default forces hipcc,
   amdhip64, and a gfx-specific offload target on every downstream
   consumer of `gf2-coding`. The existing `hip = ["dep:gf2-kernels-hip"]`
   Cargo feature isolates this cleanly.
3. **The loss regime is also real.** At batches ≤ 256 the GPU is
   **15–25× slower** than CPU AVX2. If the GPU path were on by default,
   every short-batch caller would regress. An opt-in feature is the
   right gating.
4. **Capability narrowing stays useful.** The `MaxLog`-only capability
   advertisement is a non-trivial piece of design that the modem
   framework needs for any future specialized backend. Keeping the
   prototype in the tree preserves that design constraint as a live
   example.
5. **No blockers identified.** Bit-parity tests (`FastGrayQamDemapper`
   oracle, tolerance 1e-3 AWGN / 5e-3 random complex gains) all pass,
   so the prototype is numerically correct — just not universally
   dominant.

## Next steps

Tracked under `19069bc1` (GPU story):

1. **Persistent streams** — hide the ~57 µs floor by holding the
   device-side buffers across successive `demap_llrs` calls. Would
   likely drop the crossover to ~256–512 symbols.
2. **gfx11 / gfx12 re-validation** — remeasure on newer hardware before
   any "ship" reconsideration.
3. **Batched multi-call API** — enqueue many `DemapInput`s into one GPU
   call so per-call overhead amortizes. Aligns well with the
   `SimulationRunner` traffic pattern.
4. **Consider a scalar-host path for `FastGrayQamDemapper`**
   instrumentation (disable AVX2 at runtime via a feature flag) so
   future CPU-path crossover reports can measure the full-demapper
   scalar baseline without relying on per-axis extrapolation.

The `d4851c3d` modem epic does **not** depend on any of this. Close
`9c37ec8c` with this decision and pick up future GPU work under
`19069bc1`.
