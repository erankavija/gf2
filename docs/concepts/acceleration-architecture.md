# Acceleration architecture

A gf2 computation takes one of three accelerated forms: a SIMD kernel bound at
run time, a HIP kernel the caller or the simulation executor selects, or a
rayon fan-out over independent work. Each has a scalar or CPU equivalent with
the same observable result within its declared numerical contract
(`@/inv/backend-behavioral-equivalence`).
Hardware requirements and the Cargo features that enable each path are listed
in [Supported configurations](../reference/supported-configurations.md).

## Crate layering

All production `unsafe` code lives in `gf2-kernels-simd` and `gf2-kernels-hip`
(`@/inv/unsafe-kernel-isolation`). Both expose safe entry points, and the
library crates decide when to call them:

| Layer | SIMD | HIP |
|---|---|---|
| Kernels | `gf2-kernels-simd`: `#[target_feature]` kernels behind safe detection | `gf2-kernels-hip`: device code, FFI, and the `host` resource layer |
| Selection | `gf2-core`, `gf2-coding`, `gf2-algebra` | `gf2-coding`, `gf2-algebra`, `gf2-sim` |
| Fallback policy | per bundle, at detection | per consumer |

`gf2-core` links `gf2-kernels-simd` in every build; its `simd` feature gates
only detection and dispatch.

## SIMD dispatch

The kernel families of `gf2-kernels-simd` export `detect` functions that
return bundles of function pointers, such as `LogicalFns` for bitwise and
population-count operations or `gf2m_batch::Gf2mBatchFns` for batched
$\mathrm{GF}(2^m)$ multiplication. Most return `Option`, where `None` means
the host lacks an instruction set extension the bundle requires, and the
caller runs its scalar code. The `modem` and `transpose` detectors return a
bundle on every host, with scalar entries where no vector kernel applies.
Each detection result is cached in a process-wide static, so a cache probes
the CPU once per process. In `gf2-core` the caches sit behind `maybe_*`
accessors (`maybe_simd`, `maybe_gf2m_batch`, `maybe_fp_small` and their
siblings); `gf2-coding` holds its own for the LLR, modem and BCH-encode
bundles, and the bipedal detectors cache inside the kernel crate.

Bundles are detected independently. A host can bind the logical bundle and
lack the extensions of the $\mathrm{GF}(2^m)$ batch bundle, in which case bit
operations run vectorized and $\mathrm{GF}(2^m)$ batches run scalar.

### Route selection

Where a route depends on operand shape, a selector decides whether the
detected bundle is used:

- **Compile-time word thresholds.**
  `gf2_core::kernels::select_backend_for_size` and
  `gf2_core::matrix::matvec_route` compare a word count with a constant. The
  default build uses the conservative table; building with
  `RUSTFLAGS="--cfg gf2_tuning_baked"` substitutes the values of the committed
  tuning profile. A runtime profile does not move these boundaries.
- **Runtime tuning selectors.** Algorithm schedules (M4RM tiers and panel
  widths, PLE, triangular solves, polynomial arithmetic, BCH encode family,
  parallel thresholds) read the process-wide tuning authority in
  `gf2_core::tuning`. `tuning::install` accepts one `PreparedEnvelope` of
  crate-owned sections (`CoreTuning`, `CodingTuning`, `AlgebraTuning`) before
  first access; a section read before installation resolves to its owner's
  conservative defaults for the rest of the process.

Route reporters run the dispatcher's own selector and report its decision
without executing it: `gf2_core::matrix::matvec_route`,
`gf2_core::alg::m4rm::m4rm_schedule_route` and
`gf2_core::gfp::simd_ops::prime_gemm_route`. Hot loops that repeat one
operation width resolve the route once, as
`gf2_core::kernels::ops::resolve_xor_inplace` does for row XOR.

### Equivalence evidence

SIMD and scalar routes run shared behavioral suites, for example
[`tests/simd_equiv/`](../../crates/gf2-core/tests/simd_equiv/mod.rs) and
[`popcount_routes.rs`](../../crates/gf2-core/tests/popcount_routes.rs) in
`gf2-core`. Test binaries compiled without `simd`, such as
[`m4rm_tiled_effective_no_simd.rs`](../../crates/gf2-core/tests/m4rm_tiled_effective_no_simd.rs),
pin the scalar route's behavior.
[`scripts/scalar-lane-tests.sh`](../../scripts/scalar-lane-tests.sh) runs the
`gf2-algebra` and `gf2-coding` suites with their default features without
`simd` and prints the feature list Cargo resolves for the build.

## HIP backend

`gf2-kernels-hip` holds the device kernels, listed in its
[crate documentation](../../crates/gf2-kernels-hip/src/lib.rs), and a `host`
layer of safe wrappers: `HipStreamPool`, `DeviceBuffer`, `PinnedHostBuffer`,
`LaunchDims` and `GfxTarget`. `LaunchDims` derives grid and block dimensions
from the problem size alone, so launch geometry is independent of device
occupancy. `GfxTarget::detect`
matches the device's `gcnArchName` against the code objects `build.rs`
compiled.

A GPU path runs only when the caller or the pipeline configuration selects it.
`HipError::is_recoverable` divides the failure vocabulary into recoverable
cases (device memory exhaustion, an architecture without a code object) and
fatal ones, and the consumers differ in how they act on it:

- `gf2_sim::gpu::map_hip_error` maps recoverable errors to
  `RecoverableError` and fatal ones to `FatalError`.
  `gf2_sim::executor::failure::dispatch_with_fallback` then runs the stage's
  CPU fallback for a recoverable error, or, with `PipelineConfig::strict_gpu`,
  promotes out-of-memory to fatal. A fatal error writes a JSON diagnostic dump
  and aborts the run (`@/inv/accelerator-safe-fallback`).
- The BCH batch syndrome evaluator in `gf2-coding` computes rows on the CPU
  after a recoverable error and returns the fatal ones.
- `gf2_algebra::gpu` batch permanents and the product-code GPU BCJR engine
  selected by `TurboDecoderConfig::use_gpu_bcjr` panic on a HIP failure.

### Hybrid executor

In `gf2-sim`, every stage declares an `ExecutionClass` (`CpuOnly`, `GpuOnly`,
`Hybrid`) and a `FallbackKind`. The `Scheduler` builds a rayon pool of
`parallelism` threads and, with the `hip` feature, one HIP stream per worker.
Each worker owns a strided partition of the frame indices and processes it in
batches: it prepares batch `N+1` on the CPU while batch `N` runs on its own
stream, synchronizing per stream rather than per device.
[`gpu_hybrid.rs`](../../crates/gf2-sim/examples/gpu_hybrid.rs) runs one chain
on both paths and compares the results.

## Parallelism

Each crate's `parallel` feature fans independent work out over rayon, with
the fan-out threshold or chunk size read from a tuning selector where one
exists. Examples are extension-field batch arithmetic in `gf2-core` above
`soa_batch.parallel_min_len`, and `permanent_bipedal3_parallel` in
`gf2-algebra`, which splits the Gray-code subset walk into chunks of
`permanent.gray_chunk_subsets` subsets. Library calls run on rayon's global
pool, which `RAYON_NUM_THREADS` sizes; `gf2-sim` builds its own pool from
`parallelism`.

Results do not depend on worker count or scheduling
(`@/inv/deterministic-seeded-execution`):

- Library reductions combine exact finite-field or integer values, so their
  order does not change the result.
- In `gf2-sim`, each frame's ChaCha20 stream is seeked to
  `worker_offset(seed, snr_idx, 0, g)` for global frame index `g`, so a
  frame's noise and decode outcome are independent of the worker that runs
  it, and per-worker counters reduce in worker-index order.

The `gf2-sim` [crate documentation](../../crates/gf2-sim/src/lib.rs) states
which result columns are byte-identical across worker counts, across
checkpoint resume and between CPU and GPU runs.
[`determinism.rs`](../../crates/gf2-sim/tests/determinism.rs),
[`parallel_determinism.rs`](../../crates/gf2-sim/tests/parallel_determinism.rs)
and the `gpu_*_byte_identity.rs` tests in the same directory check them.
