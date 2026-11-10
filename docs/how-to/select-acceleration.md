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
arm a selector picks for an operand size. `matvec_route` reports the
word-count comparison alone; the dispatcher runs scalar code when the bundle
is absent:

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

The `gf2-coding` section, `CodingTuning`, holds two selector families:
`encode` for BCH batch encoding and `decode` for LDPC batch decoding
([Route LDPC batch decoding](#route-ldpc-batch-decoding)). Under the
conservative section, batch BCH encoding runs the
scalar reference recurrence for every code. The table, bit-sliced and
carry-less-multiply fold families, the last two through the
`gf2_kernels_simd::bch_encode` kernels, run only from the bounds an installed
`CodingTuning` sets. Every family writes the same codeword bits
([algorithm families](../../crates/gf2-coding/src/bch/encode.rs)). Take the
bounds from a measurement on the target host. The sample sets the `encode`
family and takes the `decode` family from the conservative section:

```rust
use gf2_coding::tuning::{CodingSelectors, CodingTuning, EncodeSelectors};
use gf2_core::tuning::{self, CompiledProfileProvenance, PreparedEnvelope, ProfileId};

// Table family from redundancy 32 at any batch length; bit-sliced and fold
// families from 64 messages per batch.
let encode = EncodeSelectors::try_new(32, 1, 64, 64)?;
let id = ProfileId::parse("bch-encode-local")?;
let prepared = PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
    .insert(CodingTuning::from_selectors(CodingSelectors {
        encode,
        ..CodingSelectors::CONSERVATIVE
    }))?
    .build()?;
tuning::install(prepared)?;
```

### Route LDPC batch decoding

`BatchSoftDecoder` decodes a batch of frames, each a slice of `f32` LLRs, into
caller-provided `BitVec` words and `DecodeOutcome`s in input order. Its scope
is soft-decision whole-frame block decoding. `LdpcBatchDecoder` implements it
for `gf2-coding`'s LDPC codes in waves of eight frames (`LANES`), each frame
stopping as the single-frame decoder `LdpcDecoder` stops it. Each wave takes
one of the routes
[`BatchRoute`](../../crates/gf2-coding/src/ldpc/route.rs) names:

- `Lane` runs the wave in the lane kernels, one frame per lane. The kernels are
  the AVX2 ones when `gf2-coding` is built with the `simd` feature and the host
  reports AVX2, and the portable ones otherwise; the two backends write the
  same bits.
- `Qc` decodes each frame on `QcDecoder`, in its AVX2 kernel when `gf2-coding`
  is built with the `simd` feature and the host reports AVX2, and in its
  portable kernel otherwise; the two write the same bits.
- `SingleFrame` decodes each frame on `LdpcDecoder`.

The route follows from the algorithm and the decode family of the coding
section. A wave takes `Lane` when the algorithm is of the min-sum family
(`MinSum`, `NormalizedMinSum`, `OffsetMinSum`), the wave holds at least
`lane_min_frames` frames, the code has at most `lane_max_edges`
Tanner-graph edges, and configured decode concurrency is at most
`lane_max_workers`. An omitted worker bound resolves to one.
`decode_batch` uses one-worker semantics for standalone decoders;
`decode_batch_with_workers` accepts the caller's configured concurrency.
`PooledBatchDecoder` supplies the execution pool's width, preserving a larger
explicit caller width; simulation stages pass their executing pool's width.
`SumProduct` takes `SingleFrame`.

A wave of a code that retains quasi-cyclic structure takes `Qc` instead under
a min-sum rule, within the bounds of `gf2_coding::ldpc::QcRouteBounds`: the
lifting size is at least `min_lifting_size`, configured decode concurrency is
at most `max_workers`, and the lane route is ineligible, the wave holds fewer
than `lane_claim_min_frames` frames or the lifting size exceeds
`lane_claim_max_lifting_size`. The bounds are no tuning selector. By default
they are `QcRouteBounds::structural` of the QC backend: every lifting size and
worker count, with an eligible lane route claiming a full wave of eight frames
at lifting sizes up to `LANE_CLAIM_QC_VECTORS` vectors of the backend's QC
kernels, three vectors of eight `f32` lanes on AVX2 and three positions on the
portable kernels. `DecodeSelectors::with_qc_route` sets other bounds, such as
`QcRouteBounds::OPEN`, which gives every such wave `Qc`, or
`QcRouteBounds::CLOSED`.
`LdpcCode::from_quasi_cyclic` retains the structure;
`LdpcCode::attach_quasi_cyclic` retains it on a code read from elsewhere when
the lifted parity-check matrix equals the code's exactly.

`granules_per_task` is
the number of `batch_granularity` frame groups in a task of
`PooledBatchDecoder` ([Decode LDPC frames in parallel](#decode-ldpc-frames-in-parallel)).
The conservative family closes the lane route and opens the QC route, so a
process that installs no section decodes a code with QC structure on `Qc` and
every other code frame by frame. Take the bounds from a measurement on the
target host; the committed coding envelope under
`crates/gf2-coding/data/tuning-profiles/` records the calibration campaign's
lane selectors with their provenance. With `gf2-coding`'s `tuning-profile`
feature enabled, registering `CodingTuning` with `CodingTuningCodec` in the
profile registry above installs one.
`LdpcBatchDecoder::new` reads the family once at construction;
`with_selectors` takes one explicitly.

`batch_route(frames)` reports the route of a wave of `frames` frames without
decoding: a batch longer than eight frames takes the route of eight frames in
each full wave and of `r` frames in a final wave of `r` frames. `last_wave_route` reports the route the last wave
decoded on. The frames of a wave
decode as in the single-frame decoder
([numerical contract](../reference/ldpc-decoder-numerical-contract.md)), so the
selectors move time only. The sample lets the lane route claim a wave of at
least four frames from the QC route, checks the routes and decodes through the
trait
([rustdoc example](../../crates/gf2-coding/src/ldpc/batch.rs) with the code and
`frames` built):

```rust
let selectors = DecodeSelectors::try_new(1, usize::MAX, 1, 1)?
    .with_qc_route(QcRouteBounds::new(0, NonZeroUsize::MAX, 4, usize::MAX));
let decoder = LdpcBatchDecoder::with_selectors(
    code,
    DecoderConfig::default(),
    20,
    DecodedWord::Codeword,
    selectors,
);
assert!(matches!(decoder.batch_route(frames.len()), BatchRoute::Lane(_)));
assert!(matches!(decoder.batch_route(3), BatchRoute::Qc(_)));

let mut decoder: Box<dyn BatchSoftDecoder> = Box::new(decoder);
let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
let mut decoded = vec![BitVec::new(); views.len()];
let mut outcomes = vec![DecodeOutcome::default(); views.len()];
decoder.decode_batch(&views, &mut decoded, &mut outcomes);
```

`with_portable_lanes()` holds the portable lane and QC kernels on an AVX2 host.

## Thread parallelism

With `parallel`, batch entry points distribute independent items over rayon,
for example `CpuBackend::batch_matvec` and `BatchExtField` multiplication in
`gf2-core` and BCH batch encoding in `gf2-coding`; without it they process the
same items on the calling thread.
`BatchExtField` batches fan out only from `soa_batch.parallel_min_len`
elements, which `gf2_core::compute::field::soa_parallel_route` reports.
Results do not depend on the thread count
([parallelism](../concepts/acceleration-architecture.md#parallelism)).

Library calls use the rayon pool of the calling context: the global pool,
sized by `RAYON_NUM_THREADS`, or a pool the caller installs.

### Decode LDPC frames in parallel

`PooledBatchDecoder` implements `BatchSoftDecoder` over one inner decoder per
worker of the rayon pool current at construction and returns decoded words and
`DecodeOutcome`s in input order. With `LdpcBatchDecoder` as the inner decoder, each worker decodes its waves
on the routes of [Route LDPC batch decoding](#route-ldpc-batch-decoding).
`parallel` spreads the waves over the workers; without it the pool decodes on
the calling thread.

Construct the pool and decode inside the installed rayon pool
([`ldpc_parallel_batch.rs`](../../crates/gf2-coding/examples/ldpc_parallel_batch.rs)
builds the code and frames and runs this under `cargo run -p gf2-coding
--features parallel --example ldpc_parallel_batch`):

```rust
let rayon_pool = rayon::ThreadPoolBuilder::new().num_threads(8).build()?;
let mut decoder = rayon_pool.install(|| {
    PooledBatchDecoder::new(|| {
        LdpcBatchDecoder::new(
            code.clone(),
            DecoderConfig::default(),
            50,
            DecodedWord::Message,
        )
    })
});

let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
let mut decoded = vec![BitVec::new(); views.len()];
let mut outcomes = vec![DecodeOutcome::default(); views.len()];
rayon_pool.install(|| decoder.decode_batch(&views, &mut decoded, &mut outcomes));
```

The repository's `.cargo/config.toml` sets `RAYON_NUM_THREADS` for every
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
| DVB-T2 (`@/citation/Etsi2015`) LDPC decoding and demapping in `gf2-sim` | `with_gpu(true)` on the `gf2_sim::presets::dvb_t2` builder, or `dvb_t2_awgn_campaign --gpu` from `experiments/dvb-t2-awgn` |

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
