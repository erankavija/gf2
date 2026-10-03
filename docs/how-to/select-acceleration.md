# Select SIMD, GPU and parallel execution

This page enables and checks the SIMD, HIP and rayon paths for `gf2-core` and
`gf2-coding` workloads. The dispatch model is in
[Acceleration architecture](../concepts/acceleration-architecture.md); CPU and
GPU requirements and each crate's feature defaults are in
[Supported configurations](../reference/supported-configurations.md).

## Enable the features

Each path is a Cargo feature:

```toml
[dependencies]
gf2-core = { git = "https://github.com/erankavija/gf2", features = ["simd", "parallel"] }
gf2-coding = { git = "https://github.com/erankavija/gf2", features = ["parallel", "hip"] }
```

Cargo unifies features across the build, so a feature that any dependency
enables in `gf2-core` applies to every user of `gf2-core`. `hip` requires
ROCm at build time
([workspace exclusion](../reference/supported-configurations.md#workspace-exclusion)).

## SIMD

### Select the instruction set

No build option selects an instruction set. Each kernel bundle probes the CPU
on first use and binds for the life of the process, or leaves its operations
on scalar code
([SIMD dispatch](../concepts/acceleration-architecture.md#simd-dispatch)).
In `gf2-core` and for `gf2-coding`'s LLR operations, detection runs only with
the `simd` feature; the `gf2-coding` Gray-QAM demapper and BCH encode kernels
detect in every build. The `avx2` and `avx512` features of `gf2-kernels-simd`
gate no code. To pin scalar code on a SIMD-capable host, build with no
dependency enabling `gf2-core/simd` (for `gf2-coding`,
`default-features = false`), construct the demapper with
`FastGrayQamDemapper::new_with_scalar_kernel`, and keep the conservative BCH
encode section.

`RUSTFLAGS="-C target-cpu=native"` lets the compiler use the build host's
extensions in code outside the dispatched kernels and produces a binary that
requires those extensions. For LLR de-rate-matching, a native build and a
baseline `x86-64` build measure the same
([evidence](../reference/performance-evidence.md#nr-llr-derate)).

### Check what the host binds

The bundle accessors in `gf2_core::kernels::simd` return `None` when the host
lacks an extension the bundle requires, and the route reporters return the
arm a dispatcher takes for an operand size:

```rust
use gf2_core::kernels::simd::{maybe_gf2m_batch, maybe_simd};
use gf2_core::kernels::Backend;
use gf2_core::matrix::matvec_route;

println!("logical bundle: {:?}", maybe_simd().map(|backend| backend.name()));
println!("GF(2^m) batch bundle: {}", maybe_gf2m_batch().is_some());
println!("BitMatrix::matvec at 16 words per row: {:?}", matvec_route(16));
```

The [route selection](../concepts/acceleration-architecture.md#route-selection)
section lists the other reporters.

### Install a tuning profile

Some selectors, including the word-count thresholds, are compile-time
constants; building with `RUSTFLAGS="--cfg gf2_tuning_baked"` substitutes the
committed calibrated values. The others read a process-wide tuning envelope.
`gf2_core::tuning::install` accepts one envelope, once, before the first
tuning read; after a read it returns `AlreadyResolved`, and the process keeps
the conservative defaults. Insert every section the process needs into that
one envelope.

To install a committed `gf2-core` profile, enable `gf2-core`'s
`tuning-profile` feature and decode a file from
`crates/gf2-core/data/tuning-profiles/`. A calibrated profile records the CPU
model and toolchain it was measured on in its `measurement` object, and
`from_json` rejects one whose harness schema the codec does not support.

```rust
use gf2_core::tuning::{self, CoreTuning, CoreTuningCodec, ProfileRegistryBuilder};

let registry = ProfileRegistryBuilder::new()
    .register::<CoreTuning, CoreTuningCodec>()?
    .build()?;
tuning::install(registry.from_json(&std::fs::read_to_string(profile_path)?)?)?;
```

### Admit the BCH encode families

Under the conservative `gf2-coding` section, batch BCH encoding runs the
scalar reference recurrence for every code. The table, bit-sliced and
carry-less-multiply fold families, the last two through the
`gf2_kernels_simd::bch_encode` kernels, run only from the bounds an installed
`CodingTuning` sets. Every family writes the same codeword bits
([algorithm families](../../crates/gf2-coding/src/bch/encode.rs)). No committed
receipt measures their crossovers, so take the bounds from a measurement on
the target host:

```rust
use gf2_coding::tuning::{CodingTuning, EncodeSelectors};
use gf2_core::tuning::{self, CompiledProfileProvenance, PreparedEnvelope, ProfileId};

// Table family from redundancy 32 at any batch length; bit-sliced and fold
// families from 64 messages per batch.
let encode = EncodeSelectors::try_new(32, 1, 64, 64)?;
let id = ProfileId::parse("bch-encode-local")?;
let prepared = PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
    .insert(CodingTuning::from_selectors(encode))?
    .build()?;
tuning::install(prepared)?;
```

## Thread parallelism

With `parallel`, batch entry points distribute independent items over rayon,
for example `CpuBackend::batch_matvec` and `BatchExtField` multiplication in
`gf2-core`, and `LdpcDecoder::decode_batch_with_config` and BCH batch encoding
in `gf2-coding`; without it they process the same items on the calling thread.
`BatchExtField` batches fan out only from `soa_batch.parallel_min_len`
elements, which `gf2_core::compute::field::soa_parallel_route` reports.
Results do not depend on the thread count
([parallelism](../concepts/acceleration-architecture.md#parallelism)).

Library calls use the rayon pool of the calling context: the global pool,
sized by `RAYON_NUM_THREADS`, or a pool the caller installs:

```rust
let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build()?;
let results = pool.install(|| {
    LdpcDecoder::decode_batch_with_config(&code, &frames, 50, config)
});
```

The repository's `.cargo/config.toml` sets `RAYON_NUM_THREADS=4` for every
process Cargo launches unless the environment sets it; a binary run directly
from `target/` uses rayon's default of one thread per available CPU.

For simulations, `sim_runner --parallel` simulates SNR points concurrently
([run a simulation campaign](run-simulation-campaigns.md)). A `gf2-sim`
pipeline builds its own pool of `parallelism` workers, set through the
preset builder's `parallelism`; `dvb_t2_awgn_campaign` sets it to
`std::thread::available_parallelism`, which follows the process's CPU
affinity.

## GPU offload

GPU paths run only where the caller selects them:

| Workload | Selection |
|---|---|
| BCH syndromes and decoding | `BinaryBchDecoder::compute_syndromes_batch_gpu` or `correct_batch_gpu` in place of the CPU batch calls |
| Max-log soft demapping of Gray-QAM presets | `GpuGrayQamSoftDemapper::new(spec, max_batch)` as the `BatchSoftDemapper<f32>` |
| Product-code SISO decoding | `TurboDecoderConfig::use_gpu_bcjr`, or `use_gpu_bcjr = true` in the `turbo` table of a `sim_runner` curve |
| DVB-T2 LDPC decoding and demapping in `gf2-sim` | `with_gpu(true)` on the `gf2_sim::presets::dvb_t2` builder, or `dvb_t2_awgn_campaign --gpu` |

The consumers handle HIP failures differently
([HIP backend](../concepts/acceleration-architecture.md#hip-backend)).
`GpuGrayQamSoftDemapper::new` returns the `HipError` of a failed allocation or
upload, and a failed demap launch panics. In `gf2-sim`, `strict_gpu` on the
pipeline configuration, or `--strict-gpu`, makes device memory exhaustion
fatal.

`gf2_kernels_hip::host::device_mem_info` returns `Ok` when a device is
visible. Check the device paths against the CPU on the target device:

```bash
./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features hip \
    --cargo-profile ci-test --profile ci -E 'test(/::gpu::/)'
./scripts/cargo-budget.sh cargo run -p gf2-sim --release --features hip --example gpu_hybrid
```

The tests compare BCH device decoding with the CPU path; those that need a
device return early without one. `gpu_hybrid` runs one DVB-T2 SNR point on the CPU and the
hybrid path and asserts equal `frames`, `errors` and `fer`.
