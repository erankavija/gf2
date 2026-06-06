# gf2-sim pipeline — Phase 0 design doc

**Issue:** `ec530af9` — Pipeline design doc — gf2-sim
**Parent epic:** `f9717e7e` — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim)
**Author:** `agent:project-lead`
**Created:** 2026-06-07
**Status:** Draft (awaiting Phase 0 milestone approval)

This is the contract for Phase A and Phase B parallel implementation
work. The 15-row locked-architecture table in the epic body is the
input contract; this doc operationalises every cell with implementable
detail. Decisions from the 2026-06-07 adversarial review are folded in.

Cross-references: epic `f9717e7e`, run-book
`dev/active/f9717e7e-project-plan.md`.

---

## §1 Stage / Connector trait shapes

The `gf2-sim` crate exposes three core abstractions.

### `Stage<I, O>`

```rust
pub trait Stage<I, O>: Send + Sync {
    /// Process one batch of frames in this stage's preferred layout.
    fn process(
        &self,
        input: &I,
        scratch: &mut StageScratch,
    ) -> Result<O, StageError>;

    /// Layout preference: SoA (true; default) or AoS.
    fn prefers_soa(&self) -> bool { true }

    /// CPU-only / GPU-bound / hybrid. Used by the executor to assign
    /// rayon workers vs HIP streams.
    fn execution_class(&self) -> ExecutionClass;

    /// For OOM-fallback (§8): a GPU stage may declare a CPU-equivalent
    /// stage that the executor substitutes on `RecoverableError::OutOfMemory`.
    fn cpu_fallback(&self) -> Option<Arc<dyn AnyStage>> { None }
}
```

### `Connector<T>`

Carries layout metadata + numeric precision tag + batch size:

```rust
pub struct Connector<T> {
    pub batch_size: usize,
    pub frame_len_bits: usize,
    _element_type: PhantomData<T>,
}
```

Element types for the DVB-T2 chain: `BitPackedU64`, `LlrF32`,
`SymbolComplexF32`.

### `Pipeline`

Immutable after `build()`:

```rust
pub struct Pipeline {
    stages: Vec<Box<dyn AnyStage>>,
    edges: Vec<Edge>,
    config: PipelineConfig,
}

impl Pipeline {
    pub fn submit(&self, batch: &[Frame]) -> Result<BatchHandle, StageError>;
    pub fn collect(&self, handle: BatchHandle) -> Result<Vec<Frame>, StageError>;
    pub fn run_with_decoder(&self, ...) -> SimulationResults;
    pub fn run_parallel(&self, ..., parallelism: NonZeroUsize) -> SimulationResults;
}
```

### Error type hierarchy

```rust
pub enum StageError {
    Recoverable(RecoverableError),
    Fatal(FatalError),
}

pub enum RecoverableError {
    OutOfMemory { device_id: i32, bytes_requested: usize },
    Transient(Box<dyn std::error::Error + Send + Sync>),
}

pub enum FatalError {
    KernelLaunch { hip_code: i32, kernel: &'static str, args: String },
    DeviceUnavailable,
    BuildError(BuildError),
    CpuFallbackAlsoFailed { original: Box<RecoverableError> },
}
```

`Recoverable::OutOfMemory` triggers the executor's CPU-substitution
path (§8). All other variants hard-fail.

### Module skeleton (eliminates Wave A.2 merge conflicts)

`118a0091` (scaffolding) creates this layout up-front so the four
Wave A.2 workers can fan out without touching each other's files:

```
crates/gf2-sim/
├── Cargo.toml
├── src/
│   ├── lib.rs                    -- re-exports + crate-level docs
│   ├── pipeline.rs               -- Pipeline, build()
│   ├── stage.rs                  -- Stage, Connector, AnyStage, ExecutionClass
│   ├── error.rs                  -- StageError, RecoverableError, FatalError
│   ├── config.rs                 -- PipelineConfig (replaces SimulationConfig)
│   ├── observability.rs          -- tracing setup, checkpoint hooks (stub)
│   ├── parallel/                 -- 3fcb7025 owns this dir
│   │   └── mod.rs                  per-worker dispatch + ChaCha20 seek
│   ├── presets/
│   │   ├── mod.rs                -- typestate builder framework
│   │   └── dvb_t2.rs             -- 81d05bab owns this file
│   ├── graph/                    -- c09d3e95 owns this dir
│   │   └── mod.rs                  graph API + build()
│   ├── channels/                 -- db9836e4 owns this dir
│   │   ├── mod.rs
│   │   ├── awgn.rs
│   │   ├── rayleigh.rs
│   │   └── rician.rs
│   ├── checkpoint/               -- 5f12e7ff owns this dir
│   │   └── mod.rs
│   ├── executor/                 -- Phase C owns this dir
│   │   └── mod.rs                  (Phase A stub)
│   └── gpu/                      -- Phase B + `feature = "hip"` owns this
│       └── mod.rs                  (Phase A stub)
├── examples/                     -- populated in D + E
└── tests/
    └── determinism.rs            -- 48a0db6c owns this
```

This structural pre-allocation is the §1 deliverable that resolves
audit finding M2 (Wave A.2 merge conflict risk).

---

## §2 SoA ↔ AoS conversions at user-visible I/O

User submits frames in AoS: `Vec<Frame>`. Pipeline transposes once at
`submit()` into SoA buffers it owns. Stages operate on SoA. On
`collect()`, the pipeline transposes back to AoS.

```rust
pub struct Frame {
    pub info_bits: BitVec,
    pub tx_symbols: Vec<Complex<f32>>,
    pub rx_symbols: Vec<Complex<f32>>,
    pub rx_llrs: Vec<f32>,
    pub decoded_bits: BitVec,
}
```

Cost: O(batch_size · frame_len_bits) per submit/collect call. For
DVB-T2 n=64800 at batch=256, that's ~16M bits ≈ 2 MB transpose per
call — negligible vs decoder iterations.

Memory ownership: pipeline owns SoA buffers (one Arc-backed buffer per
attribute × stage); user owns AoS. Pipeline acquires/releases SoA
buffers from an internal pool to amortise allocation across batches.

---

## §3 ChaCha20 per-worker seek scheme

Each per-(SNR, worker) tuple owns an independent `ChaCha20Rng`:

```
worker_offset(seed, snr_index, worker_idx, frame_idx_in_worker) =
    seed
  + snr_index * SNR_STRIDE          // 2^48 words per SNR
  + worker_idx * WORKER_STRIDE      // 2^32 words per worker
  + frame_idx_in_worker * FRAME_STRIDE  // 4096 words per frame
```

Constants:

| Constant | Value | Rationale |
|---|---|---|
| `SNR_STRIDE` | `1 << 48` | 2^48 words ≈ 2 PB per SNR; covers any plausible run |
| `WORKER_STRIDE` | `1 << 32` | 2^32 words ≈ 32 GB per worker; 1M+ frames per worker |
| `FRAME_STRIDE` | `4096` | 128 KB of noise per frame; covers DVB-T2 n=64800 + headroom |

A `ChunkWords` constant of 4096 was chosen because the longest in-use
codeword (DVB-T2 LDPC n=64800) needs about 256K bits of noise for the
forward chain, which is 64K bytes = 8K u64 words; 4096 u64 words is
half of that. **Stage authors must assert their per-frame noise draw
≤ `FRAME_STRIDE - 256` words** to leave headroom for the next call.
Violating this constant breaks determinism. The check is enforced via
debug-only `assert!` in `gf2-sim::parallel::Reducer::draw_for_frame()`.

On GPU (`f6004add`), each kernel thread computes the same offset:
`worker_offset(seed, snr_idx, worker_idx, frame_idx)` and seeds its
local ChaCha20 state. The same algorithm runs CPU-side and device-side
so the byte stream matches.

**Aggregation order** (resolves the deterministic-reduction half of
the contract): when summing per-worker counters at the SNR boundary,
the executor iterates workers in `worker_idx` order. The integer
counters (`frames`, `errors`, `total_iterations`, `total_bits`,
`total_bit_errors`) are u64 so the order does not actually matter for
correctness — but specifying it removes a class of "did I aggregate
correctly?" review questions.

---

## §4 Heartbeat-checkpoint schema in the streaming pipeline

Backwards-compatible with the existing `checkpoints/snr_NNNN.json`
schema from issue `fd73e8a8`. Optional new fields enable hybrid resume:

```json
{
  "snr_index": 5,
  "eb_n0_db": 3.24,
  "frames_completed": 37555,
  "errors_accumulated": 0,
  "total_iterations": 65082,
  "total_queries": 37555,
  "total_bits": 1185735384,
  "total_bit_errors": 0,
  "worker_states": [
    {"worker_idx": 0, "frames_in_worker": 1564, "rng_word_pos": 6406144},
    {"worker_idx": 1, "frames_in_worker": 1563, "rng_word_pos": 6402048}
    /* ... */
  ],
  "schema_version": 2
}
```

- `schema_version` distinguishes legacy (`null` / absent → v1) from
  the new format. Legacy checkpoints resume on a single worker.
- `worker_states` is present only for heartbeat checkpoints written
  by the parallel runner; per-SNR-boundary checkpoints may omit it.
- On resume, the runner restores per-worker `rng_word_pos` and
  re-derives the chunk seek from `worker_offset(...)`.

**GPU pipeline drain protocol**: before a heartbeat flush, the
executor signals all HIP streams to complete in-flight batches via
`hipStreamSynchronize()` on each owned stream (NOT
`hipDeviceSynchronize()`, which would block other contexts). Once
every stream reports idle, per-worker counters are aggregated and the
JSON is serialised atomically (write to `.tmp`, fsync, rename).

---

## §5 Crate-boundary diagram

```
            ┌────────────────────────────────┐
            │ gf2-core                       │
            │   primitives: BitVec, fields   │
            └───────────────┬────────────────┘
                            │
            ┌───────────────▼────────────────┐
            │ gf2-coding                     │
            │   codes: BCH, LDPC, QAM, etc.  │
            └───────────────┬────────────────┘
                            │
            ┌───────────────▼────────────────┐
            │ gf2-sim                  ← new │
            │   Pipeline, Stage, executor    │
            │   presets/, graph/, channels/  │
            └─────────────┬──────────────────┘
                          │ feature = "hip"
                          ▼ (optional)
            ┌──────────────────────────────┐
            │ gf2-kernels-hip              │
            │   HIP FFI + kernels (gfx*)   │
            └──────────────────────────────┘
```

`gf2-sim` `Cargo.toml`:

```toml
[dependencies]
gf2-core    = { path = "../gf2-core" }
gf2-coding  = { path = "../gf2-coding" }
gf2-kernels-hip = { path = "../gf2-kernels-hip", optional = true }
rayon            = "1"
rand_chacha      = "0.4"
tracing          = "0.1"
serde            = { version = "1", features = ["derive"] }

[features]
default = []
hip     = ["dep:gf2-kernels-hip"]
llr-f64 = []
```

Public surfaces of `gf2-sim`:

- `gf2_sim::Pipeline`, `gf2_sim::Stage`, `gf2_sim::Connector`,
  `gf2_sim::PipelineConfig`, `gf2_sim::StageError`.
- `gf2_sim::presets::dvb_t2::Pipeline` (typestate builder).
- `gf2_sim::graph::Chain`.
- `gf2_sim::channels::{Awgn, Rayleigh, Rician}`.
- `gf2_sim::gpu::HipDispatcher` (only with `--features hip`).

CI: `cargo test --workspace` works on non-ROCm hosts (HIP feature off
by default, `gf2-kernels-hip` continues to be excluded from the
default workspace member set per CLAUDE.md §Architecture).

---

## §6 Multi-arch HIP dispatch

Compile-time gfx target list (Phase B `36075e4c`):

| Target | Arch family | CI today |
|---|---|---|
| gfx1030 | RDNA2 (RX 6800/6900 XT) | yes |
| gfx1100 | RDNA3 (RX 7900 XT/XTX) | seam only |
| gfx1200 | RDNA4 | seam only |
| gfx90a, gfx940, gfx942 | CDNA2 / MI200 / MI300 | seam only |

Runtime detection: `hipDeviceGetAttribute(major, minor)` returns the
compute capability; the dispatcher maps to a precompiled kernel blob
under `crates/gf2-kernels-hip/kernels/<gfx-target>/`. If no blob
matches, the dispatcher emits a `tracing::warn!` event with the device
ID and falls back to the CPU equivalent stage (consistent with the
OOM policy in §8).

Kernel blob format: one `.co` file per (kernel, gfx-target) compiled
ahead of time by the `gf2-kernels-hip` build script (`build.rs`)
invoking `hipcc --offload-arch=<target>`. The build script gracefully
skips any arch whose toolchain is missing — gfx1030 must always be
compiled successfully; the others are best-effort.

---

## §7 Multi-GPU extension seams

Single-GPU is v1. The design admits multi-GPU without breaking change:

1. **Per-device stream pool**: today `HipDispatcher` owns a single
   `Vec<HipStream>`. Multi-GPU adds `HipDispatcher::new_per_device(
   device_ids: &[i32])` that owns a `Vec<Vec<HipStream>>`. The Stage
   trait sees one stream per call; the dispatcher routes.

2. **Worker→device assignment**: `PipelineConfig::gpu_assignment(
   strategy: GpuAssignmentStrategy)`. Variants:
   `RoundRobin` (default), `Static(Vec<i32>)`, `MemoryLoadAware`.
   v1 implements only `RoundRobin`; the enum is stable.

3. **Aggregation step**: already device-agnostic — the reducer sums
   per-worker counters, not per-device. No change needed.

v1 hard-codes single-device. Validation of these seams happens through
type-check: every signature above compiles in v1 with `device_ids =
&[0]`, and the multi-GPU extension is purely additive.

---

## §8 Failure-mode policy

| Failure | Detection | Response |
|---|---|---|
| `hipMalloc` OOM | GPU stage returns `RecoverableError::OutOfMemory{ device_id, bytes_requested }` | Executor catches the signal; substitutes the GPU stage's `cpu_fallback()` (Stage trait method) for the offending batch; emits `tracing::warn!{batch_id, snr_idx, device_id, bytes_requested}`; continues. |
| Kernel launch error | GPU stage returns `FatalError::KernelLaunch{ hip_code, kernel, args }` | Executor hard-fails: logs the diagnostic dump and exits with non-zero code. No cleanup of in-flight batches required (process exit). |
| Driver / device unavailable | `hipDeviceGetCount() == 0` at pipeline construction | `Pipeline::build()` returns `FatalError::DeviceUnavailable`. User invokes `--cpu-only` to skip GPU stages. |
| CPU fallback also fails | Substituted CPU stage returns any error | `FatalError::CpuFallbackAlsoFailed{ original }`. No infinite loop. |

**OOM/hard-fail seam (resolves H5):**

- **GPU stages (Phase B `ed575f15` scope)**: only signal an
  `OutOfMemory` error from their `process()`. They do NOT implement
  the fallback dispatch.
- **Executor (Phase C `42eac5cc` scope)**: catches the OOM signal,
  invokes the GPU stage's `cpu_fallback()` accessor to retrieve a
  paired CPU stage, runs that on the same input, continues.
- **Pipeline build**: the typestate builder presets register a CPU
  fallback for every GPU stage automatically. The graph API requires
  explicit `Chain::register_fallback(gpu_stage_id, cpu_stage)` —
  forgetting this is a `BuildError::NoFallback`.

**CLI override**: `--strict-gpu` flag turns OOM into
`FatalError::OutOfMemory` (no fallback). Used by
reproducibility-critical experiments where any CPU dispatch would
break the byte-identity contract that compares CPU-only vs CPU+GPU.

---

## §9 Layered builder vs graph API surfaces

### Typestate builder (presets)

```rust
let pipeline = gf2_sim::presets::dvb_t2::Pipeline::builder()
    .modcod(Modcod::Normal { rate: Rate::R1_2, modulation: Mod::Qam16 })
    .decoder(DecoderConfig::sum_product())
    .demap(DemapMethod::ExactLogMap)
    .channel(Channel::awgn(es_n0_db))
    .parallelism(NonZeroUsize::new(24).unwrap())
    .seed(0xC0DEF00D)
    .checkpoint_dir(Some("/tmp/dvb_run/checkpoints".into()))
    .build()?;
```

Compile-time guarantees:

- Calling `.decoder()` before `.modcod()` fails to type-check
  ("method `decoder` not found"). Typestate generic parameter
  enforces order.
- Invalid (rate, modulation) combinations rejected at `.build()`
  call site via a runtime `Modcod::validate()` check — fully
  compile-time check is possible but adds 50× type-parameter
  complexity and is judged not worth it.

### Graph API (novel chains)

```rust
let mut chain = gf2_sim::graph::Chain::new();
let bch_enc = chain.add(BchEncoder::dvb_t2(rate));
let ldpc_enc = chain.add(LdpcEncoder::dvb_t2(rate));
let interleave = chain.add(BitInterleaver::dvb_t2(rate, modulation));
let modulate = chain.add(GrayQam::new(modulation));
let channel = chain.add(channels::Awgn::new(es_n0_db));
let demap = chain.add(GrayQamDemapper::new(DemapMethod::ExactLogMap));
let deinterleave = chain.add(BitInterleaver::dvb_t2_inverse(rate, modulation));
let ldpc_dec = chain.add(LdpcDecoder::sum_product());
let bch_dec = chain.add(BchDecoder::dvb_t2(rate));

chain.connect(bch_enc, ldpc_enc)?;
chain.connect(ldpc_enc, interleave)?;
chain.connect(interleave, modulate)?;
chain.connect(modulate, channel)?;
chain.connect(channel, demap)?;
chain.connect(demap, deinterleave)?;
chain.connect(deinterleave, ldpc_dec)?;
chain.connect(ldpc_dec, bch_dec)?;

let pipeline = chain.build()?;
```

Runtime errors at `connect()` and `build()`:

- `BuildError::TypeMismatch { from: TypeId, to: TypeId }` —
  connector element types don't match.
- `BuildError::Cyclic` — back-edge detected.
- `BuildError::Disconnected` — graph has more than one root or sink.
- `BuildError::NoFallback { gpu_stage }` — GPU stage in the graph
  has no registered CPU fallback (when `--strict-gpu` is off).

The DVB-T2 typestate preset is implemented as a thin wrapper over
the graph API. Both APIs share the same `Stage` impls and the same
`build()` machinery. The example `examples/dvb_t2_two_apis.rs`
(landed by `8c8302c8`) shows the two forms side-by-side producing
identical output for the same input batch.

---

## §10 Numerical precision strategy

| Quantity | Default | Configurable |
|---|---|---|
| LLR storage | f32 | `--features llr-f64` (mirrors existing `gf2-coding`) |
| AWGN noise samples | f32 | not configurable |
| BP messages | f32 | not configurable in v1 |
| BER reductions | f32, non-associative | excluded from byte-identity (status quo) |
| Frame/error counters | u64 | always integer-exact |

**Mixed precision** (f32 storage + f64 BP check-node accumulation)
is **deferred**. The seam: BP message-update routines take a
`Numeric` trait parameter bound to `f32` in v1; adding `f64` or
`f16` is a single trait impl + feature flag. No Phase 0 follow-up
needed unless empirical results in Phase E show BP accuracy as a
bottleneck.

---

## §11 Determinism contract summary

The four columns `fer`, `frames`, `errors`, `mean_iters` are
**byte-identical** across:

- 1-thread CPU vs N-thread CPU (any N ∈ {1, 2, 4, 8, 24}) at fixed
  seed.
- CPU-only vs CPU+GPU at fixed seed (subject to GPU kernel design
  guarantees: deterministic launch grids, no atomic reductions on
  fp32, in-order stream synchronisation).
- Resume from heartbeat checkpoint vs uninterrupted run at fixed
  seed.

BER is **excluded** from byte-identity. Wall-clock is excluded.

This restates the epic locked-architecture table line; the
operationalisation is the per-worker ChaCha20 seek (§3) plus the
deterministic worker-index aggregation order (§3 closing). The epic
body is the SSOT for the contract statement; this doc is the SSOT
for how the contract is met.

---

## §12 Migration plan from `simulation.rs`

`crates/gf2-coding/src/simulation.rs` is ~5,800 lines. The migration
is **selective**: only the parts that compose with the new
`Stage`/`Pipeline` model move. The rest stays in `gf2-coding`.

### Public APIs that **move** to `gf2-sim`

| `simulation.rs` (current) | `gf2-sim` (new) | Owner task |
|---|---|---|
| `SimulationRunner::run_with_decoder` (L3828) | `Pipeline::run_with_decoder` | `bbf6b6ee` (campaign-binary site) |
| `SimulationRunner::run_coded_iterative_parallel` (L3416) | `Pipeline::run_parallel` | `3fcb7025` + `75c22fa8` |
| `SimulationRunner::run_coded` (L3268) | `Pipeline::run` | `bbf6b6ee` |
| `SimulationRunner::run_coded_iterative` (L3326) | `Pipeline::run_iterative` | `bbf6b6ee` |
| `SimulationConfig` | `gf2_sim::PipelineConfig` | `118a0091` |
| Per-SNR checkpointing helpers (`crates/gf2-coding/src/bin/sim_checkpoint_helper.rs`) | `gf2_sim::checkpoint::{Reader, Writer}` | `5f12e7ff` |
| `tracing` setup blob | `gf2_sim::observability::install_campaign_subscriber` | `118a0091` |

### Public APIs that **stay** in `gf2-coding`

| API | Reason |
|---|---|
| `SimulationRunner::run_uncoded_ber*` (L1517–L1721) | Pre-coded baseline runs; not pipeline-shaped. |
| `BpskAwgnChannel` | Composes inside `gf2_sim::channels::Awgn` as a building block, not a Stage. |
| `BicmAwgnChannel` (campaign-binary's internal channel) | Same — wrapped by `gf2_sim::channels`. |
| All BCH / LDPC / QAM codec types | Codes are not Stages. They are inputs to Stage implementations. |
| `DvbT2Concat` codec | Wrapped by `gf2_sim::presets::dvb_t2`; `81d05bab` reuses it. |
| All ETSI table data (`crates/gf2-coding/data/...`) | Pure data; not pipeline scope. |

### Migration sequence

1. `118a0091` lands `gf2-sim` scaffolding (Cargo.toml + module skeleton + `PipelineConfig` derived from `SimulationConfig`).
2. `3fcb7025` lands `Pipeline::run_parallel` skeleton + per-worker dispatch + reducer.
3. Phase B in parallel lands HIP host infra and GPU stages.
4. `75c22fa8` wires the hybrid scheduler.
5. `bbf6b6ee` migrates `crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs` to call the new pipeline. Existing call sites of `SimulationRunner` in `gf2-coding` are left alone — the migration is opt-in per binary.
6. A future epic (out of `gf2-sim` scope) deprecates `simulation.rs` paths after all callers migrate.

This keeps `gf2-coding` stable during the migration and bounds the
blast radius to the campaign binary.

---

## §13 Phase 0 closure checklist

- [x] §1 Stage/Connector trait shapes + module skeleton
- [x] §2 SoA↔AoS conversions
- [x] §3 ChaCha20 per-worker seek scheme
- [x] §4 Heartbeat-checkpoint schema (v2 backwards-compatible)
- [x] §5 Crate-boundary diagram
- [x] §6 Multi-arch HIP dispatch
- [x] §7 Multi-GPU extension seams
- [x] §8 Failure-mode policy (with OOM seam decision)
- [x] §9 Layered builder vs graph API
- [x] §10 Numerical precision
- [x] §11 Determinism contract summary
- [x] §12 Migration plan from `simulation.rs`

All 13 sections present with implementable detail. No "TBD" leaves.
Decisions deferred to a future task are explicitly named (mixed
precision in §10; multi-GPU concretisation in §7; full
`simulation.rs` deprecation in §12).

Audit findings folded in: M2 (Wave A.2 module skeleton, §1), C5 (per
deps), H5 (OOM seam, §8), H7 (§12 lists all public APIs that move
vs stay), M4 (determinism contract SSOT split, §11 cites the epic
body).
