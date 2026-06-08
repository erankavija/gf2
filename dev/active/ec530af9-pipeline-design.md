# gf2-sim pipeline — Phase 0 design doc

**Issue:** `ec530af9` — Pipeline design doc — gf2-sim
**Parent epic:** `f9717e7e` — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim)
**Author:** `agent:project-lead`
**Created:** 2026-06-07; **Revised:** 2026-06-07 (post-adversarial-review)
**Status:** Draft (awaiting Phase 0 milestone approval)

This is the contract for Phase A and Phase B parallel implementation
work. The 15-row locked-architecture table in the epic body is the
input contract; this doc operationalises every cell with implementable
detail.

Revision history: the initial draft (commit `808b79f6`) was reviewed
by an adversarial Plan agent on 2026-06-07; 24 findings (5 BLOCKING /
6 CRITICAL / 6 HIGH / 4 MEDIUM / 3 LOW) were surfaced. Seven open
questions were escalated to the user and answered the same day; the
remaining structural fixes are baked into this revision. Specific
revision points are tagged inline as `[fixed: <finding-id>]`.

Cross-references: epic `f9717e7e`, run-book
`dev/active/f9717e7e-project-plan.md`.

---

## §1 Stage / Connector trait shapes

The `gf2-sim` crate exposes the following abstractions.

### `Stage<I, O>`

`[fixed: B4, B5, C1, C2]` Adds associated `CpuFallback` type per Q6
(compile-safe over runtime erasure).

```rust
pub trait Stage<I, O>: Send + Sync {
    /// Per-stage scratch storage (acquired from a pool by the executor).
    type Scratch: Default + Send + Sync;

    /// Compile-time-bound CPU fallback for OOM substitution (§8).
    /// Defaults to `Self` so a pure-CPU stage is its own fallback.
    type CpuFallback: Stage<I, O> = Self;

    fn process(
        &self,
        input: &I,
        scratch: &mut Self::Scratch,
    ) -> Result<O, StageError>;

    fn prefers_soa(&self) -> bool { true }
    fn execution_class(&self) -> ExecutionClass;

    /// Returns a paired CPU stage. Default `None` for CPU-only stages.
    /// GPU stages MUST override and return `Some(&fallback)` so the
    /// executor (`42eac5cc`) can substitute on OOM.
    fn cpu_fallback(&self) -> Option<&Self::CpuFallback> { None }
}

pub enum ExecutionClass { CpuOnly, GpuOnly, Hybrid }
```

### Type erasure layer (`AnyStage`, `TypedBatch`)

`[fixed: B4]` The `Pipeline` owns a heterogeneous stage list; each
stage erases its concrete `Stage<I, O>` types via `AnyStage`. The
executor downcasts at the connector boundary using the
`TypedBatch` registry.

```rust
/// Type-erased Stage handle. The `process_any` method downcasts the
/// input batch via `TypedBatch::downcast_ref` and re-erases the output.
pub trait AnyStage: Send + Sync {
    fn input_type(&self) -> TypeId;
    fn output_type(&self) -> TypeId;
    fn execution_class(&self) -> ExecutionClass;
    fn fallback_kind(&self) -> FallbackKind;

    fn process_any(
        &self,
        input: &dyn TypedBatch,
        scratch: &mut dyn AnyScratch,
    ) -> Result<Box<dyn TypedBatch>, StageError>;
}

pub enum FallbackKind {
    /// Stage IS its own fallback (CPU stages).
    SelfFallback,
    /// Stage has a separate CPU fallback registered.
    Registered,
    /// No fallback (will fail on OOM unless `--strict-gpu` is off
    /// AND the executor's preset registered a fallback externally).
    None,
}

/// Marker trait implemented by all batch types crossing stage
/// boundaries. Concrete impls are auto-derived for `LlrBatch`,
/// `SymbolBatch`, `BitPackedBatch`, `HardDecisionBatch`, etc.
pub trait TypedBatch: std::any::Any + Send + Sync {
    fn batch_size(&self) -> usize;
}

/// Type-erased scratch holder. Concrete `Stage::Scratch` types
/// implement this via blanket impl.
pub trait AnyScratch: Send {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
```

`AnyStage` is implemented for every `Stage<I, O>` via a blanket impl
in `crates/gf2-sim/src/stage.rs`; the blanket impl performs the
downcast and re-erasure.

### `Connector<T>` and `Edge`

`[fixed: B4]`

```rust
pub struct Connector<T: TypedBatch> {
    pub batch_size: usize,
    pub frame_len_bits: usize,
    _t: PhantomData<T>,
}

pub struct Edge {
    pub from: StageId,
    pub to: StageId,
    pub element_type: TypeId,
    pub batch_size: usize,
}

pub struct StageId(pub u32);
```

### `Pipeline` and `BatchHandle`

`[fixed: B4]`

```rust
pub struct Pipeline {
    stages: Vec<Box<dyn AnyStage>>,
    edges: Vec<Edge>,
    fallbacks: HashMap<StageId, Box<dyn AnyStage>>,
    config: PipelineConfig,
    builder_lineage: BuilderLineage,
}

/// Opaque handle returned by `Pipeline::submit`; consumed by
/// `Pipeline::collect`. Carries the SoA buffer indices the pipeline
/// allocated for this batch's lifetime.
pub struct BatchHandle {
    batch_id: u64,
    snr_idx: u32,
    buffers: BufferRefs,
}

impl Pipeline {
    pub fn submit(&self, batch: &[Frame]) -> Result<BatchHandle, StageError>;
    pub fn collect(&self, handle: BatchHandle) -> Result<Vec<Frame>, StageError>;
    pub fn run_with_decoder(&self, runner_cfg: &RunnerCfg) -> SimulationResults;
    pub fn run_parallel(&self, runner_cfg: &RunnerCfg, parallelism: NonZeroUsize)
        -> SimulationResults;
}
```

### Error type hierarchy

`[fixed: B5]` `FatalError::OutOfMemory` variant added per Q7.

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
    /// Promoted from RecoverableError::OutOfMemory when `--strict-gpu`
    /// is set; or unconditionally when a CPU fallback is also OOM.
    OutOfMemory { device_id: i32, bytes_requested: usize },
    KernelLaunch { hip_code: i32, kernel: &'static str, args: String },
    DeviceUnavailable,
    BuildError(BuildError),
    CpuFallbackAlsoFailed { original: Box<RecoverableError> },
}

pub enum BuildError {
    Cyclic { involved: Vec<StageId> },
    TypeMismatch { from_stage: StageId, from_type: TypeId, to_stage: StageId, to_type: TypeId },
    Disconnected { stages: Vec<StageId> },
    NoFallback { gpu_stage: StageId },
    InvalidModcod { rate: NrRate, modulation: Modulation },
}
```

### `PipelineConfig`

`[fixed: B4]` Mirrors `gf2_coding::simulation::SimulationConfig`
fields verbatim and adds Phase 0 pipeline knobs.

```rust
pub struct PipelineConfig {
    pub seed: u64,
    pub esn0_db_points: Vec<f64>,
    pub target_errors: u64,
    pub max_frames: u64,
    pub heartbeat_every_frames: u64,
    pub checkpoint_dir: Option<PathBuf>,
    pub tracing_log_path: Option<PathBuf>,
    pub parallelism: NonZeroUsize,
    pub strict_gpu: bool,
}

impl From<&gf2_coding::simulation::SimulationConfig> for PipelineConfig { ... }
```

### Module skeleton (eliminates Wave A.2 merge conflicts)

`118a0091` (scaffolding) creates this layout up-front so the four
Wave A.2 workers can fan out without touching each other's files:

```
crates/gf2-sim/
├── Cargo.toml
├── src/
│   ├── lib.rs                    -- re-exports + crate-level docs
│   ├── pipeline.rs               -- Pipeline, build()
│   ├── stage.rs                  -- Stage, AnyStage, TypedBatch, AnyScratch
│   ├── connector.rs              -- Connector, Edge, StageId
│   ├── error.rs                  -- StageError, RecoverableError, FatalError, BuildError
│   ├── config.rs                 -- PipelineConfig (From<SimulationConfig>)
│   ├── observability.rs          -- tracing setup, install_campaign_subscriber (stub)
│   ├── parallel/                 -- 3fcb7025 owns this dir
│   │   └── mod.rs                  per-worker dispatch + ChaCha20 seek (§3)
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
│   │   └── mod.rs                  v2 schema (§4)
│   ├── executor/                 -- Phase C owns this dir
│   │   └── mod.rs                  (Phase A stub)
│   └── gpu/                      -- Phase B + `feature = "hip"` owns this
│       └── mod.rs                  (Phase A stub)
├── examples/                     -- populated in D + E
└── tests/
    └── determinism.rs            -- 48a0db6c owns this
```

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

Cost: O(batch_size · frame_len_bits) per submit/collect. For
DVB-T2 n=64800 at batch=256, that's 64800 × 256 ≈ 16.6 M bits ≈ 2.08
MB transpose per call — negligible vs decoder iterations.

Memory ownership: pipeline owns SoA buffers (Arc-backed per attribute
× stage); user owns AoS. Pipeline acquires/releases SoA buffers from
an internal pool to amortise allocation across batches.

---

## §3 ChaCha20 per-worker seek scheme

`[fixed: B3, C3, H4]` Stride values raised per Q4; seed-derivation
note added per Q2; aggregation-order hedge removed.

> **Amendment 2026-06-07 (user-approved).** `FRAME_STRIDE` is **`2^20`**
> (raised from the original `2^16`), and the stride **unit is ChaCha20
> 32-bit words** (the original text said "u64 words" / "512 KB", which is
> wrong for `rand_chacha 0.9`, whose `set_word_pos`/`get_word_pos` are in
> 32-bit words — `BLOCK_WORDS = 16`, "the offset from the start of the
> stream, in 32-bit words", `rand_chacha-0.9.0/src/chacha.rs:207`). The
> original arithmetic undercounted twice: (a) it assumed f32 noise
> (~1 word/sample), but the implementation draws **two `f64` uniforms per
> Gaussian sample** via `box_muller_cos` = **4 ChaCha20 32-bit words per
> noise sample**; (b) it reasoned from 64-QAM, but the binding (largest)
> case is the **lowest-order modulation** — fewer bits/symbol → more
> symbols → more noise samples. **`QPSK` (2 bits/symbol) is the worst
> case, NOT 16-QAM**: r1/2 QPSK Normal draws a **measured 260 208 32-bit
> words/frame** (n=64800 → 32400 symbols × 2 axes = 64800 noise samples ×
> 4 words = 259 200, plus ~1008 BBFRAME words), versus 16-QAM Normal
> 130 608 and 64-QAM Normal 87 408. The original `2^16 = 65 536` budget
> was ~4× *under* the QPSK draw, so consecutive frames' RNG regions
> overlapped, sharing noise samples (deterministic, so
> byte-identity-across-workers still held, but the inter-frame noise was
> correlated — unacceptable for a research-grade FER simulator). All three
> measured draws are asserted by
> `gf2-sim parallel::tests::test_worst_case_frame_draw_under_stride`, which
> enumerates every supported modulation and checks QPSK binds. The
> `worker_offset` formula *shape* is unchanged; only the `FRAME_STRIDE`
> constant moved. (An intermediate value of `2^19` sized for 16-QAM only,
> giving just ~2× headroom over QPSK; `2^20` restores the ~4× design goal.)

Each per-(SNR, worker) tuple owns an independent `ChaCha20Rng` from
`rand_chacha 0.9` (see §5). All strides are in ChaCha20 **32-bit word**
units. Seek offset:

```
worker_offset(seed, snr_idx, worker_idx, frame_idx_in_worker) =
    snr_idx * SNR_STRIDE                                  // 2^56 32-bit words per SNR
  + (worker_idx as u128) * WORKER_STRIDE                  // 2^40 32-bit words per worker
  + (frame_idx_in_worker as u128) * FRAME_STRIDE          // 2^20 32-bit words per frame
```

Constants (FRAME_STRIDE revised 2026-06-07; all units = ChaCha20 32-bit words):

| Constant | Value | Notes |
|---|---|---|
| `SNR_STRIDE` | `1 << 56` | 2^56 32-bit words per SNR; far above any practical run |
| `WORKER_STRIDE` | `1 << 40` | 2^40 32-bit words per worker = 4 TiB; 2^20 = 1M frames/worker at FRAME_STRIDE=2^20 |
| `FRAME_STRIDE` | `1 << 20` | 1048576 32-bit words = 4 MiB per frame; ~4× headroom over the measured 260 208-word worst case (r1/2 QPSK Normal) |

Arithmetic check (real f64-Box-Muller cost; **measured**, not estimated):
the per-frame noise sampler draws two `f64` uniforms per Gaussian sample
(`box_muller_cos(u1, u2)`), i.e. **4 ChaCha20 32-bit words per noise
sample**. The binding case is the *lowest-order* modulation (most symbols).
DVB-T2 n=64800 with **QPSK** (2 bits/symbol): 64800/2 = 32400 complex
symbols → 2 axes × 32400 = 64800 noise samples × 4 words = **259 200
words** for AWGN, plus the random BBFRAME fill (~1008 words for the
rate-1/2 k_bch drawn as u64s) ≈ **260 208 words/frame** (measured exactly).
For comparison 16-QAM Normal measures 130 608 and 64-QAM Normal 87 408 —
fewer bits/symbol ⇒ more symbols ⇒ more draws, confirming QPSK binds. BP
decode draws nothing from the noise stream (Phase 0 §10: BP messages are
computed, not sampled). `FRAME_STRIDE = 2^20 = 1 048 576` gives **4.03×
headroom** over QPSK; the debug assert is
`noise_words_drawn ≤ FRAME_STRIDE - 1024`.

Total `worker_offset` fits in 128 bits when frame_idx ≤ 2^20 and
worker_idx ≤ 2^16 (and far beyond). ChaCha20Rng's `set_word_pos` takes
`u128` (verified at `rand_chacha-0.9.0/src/chacha.rs`).

**Seed derivation: NEW scheme; NOT compatible with legacy
`simulation.rs`.** Per Q2 (user-approved 2026-06-07), the new
pipeline DOES NOT preserve the legacy `seed = base ^
rotate_left(snr_index, 13)` derivation. Bytestream byte-identity vs
the legacy `SimulationRunner::run_with_decoder` path is therefore
NOT a requirement of `bbf6b6ee`; the migrated criterion is
"byte-identical between two new-pipeline runs at the same seed."

On GPU (`f6004add`), each kernel thread computes the same offset:
`worker_offset(seed, snr_idx, worker_idx, frame_idx)` and seeds its
local ChaCha20 state. The same algorithm runs CPU-side and
device-side so the byte stream matches (subject to the GPU softmath
constraint in §11).

**Aggregation order at SNR boundary**: per-worker counter sums
(`frames`, `errors`, `total_iterations`, `total_bits`,
`total_bit_errors`) MUST iterate workers in `worker_idx` order. This
is the SSOT order — even though u64 sums are order-invariant for
correctness, the order matters for debug logging consistency and
for any future migration to non-u64 accumulators. Implementations
that re-order are non-compliant.

---

## §4 Heartbeat-checkpoint schema (v2-only)

`[fixed: B2, H3, C4]` v2 is a fresh schema; no v1 back-compat in the
new pipeline.

> **Amendment 2026-06-08 (user-approved): clean-cut, no migration.**
> Q5 originally added a one-shot migration script
> (`checkpoint_migrate.rs`) to convert legacy v1 checkpoints to v2.
> That is **removed**: there are no previous runs and no users, so
> there is **no v1 backward compatibility and no migration tool** at
> all. The "Migration tool" subsection below is retained struck-through
> for revision history only. The v2 schema itself is unchanged.

### v2 schema

```json
{
  "schema_version": 2,
  "snr_index": 5,
  "esn0_db": 6.25,
  "config_hash": "blake3:ef56f88523777b04bf303f18c64de099a06ec322bb3f0124671cd39fad73f420",
  "frames_target": 100000,
  "errors_target": 100,
  "max_frames": 10000000,
  "frames_completed": 37555,
  "errors_accumulated": 0,
  "total_iterations": 65082,
  "total_queries": 37555,
  "total_bits": 1185735384,
  "total_bit_errors": 0,
  "completed": false,
  "worker_states": [
    {"worker_idx": 0, "frames_in_worker": 1564, "rng_word_pos": "6406144"},
    {"worker_idx": 1, "frames_in_worker": 1563, "rng_word_pos": "6402048"}
  ],
  "drain_committed_at_us_since_epoch": 1717891200000000
}
```

Notes:

- `rng_word_pos` is a JSON **string** (decimal `u128`); same convention
  as legacy `simulation.rs:165` (`u128` does not fit in a JSON number
  above 2^53).
- `config_hash` is the blake3 hash of the serialised `PipelineConfig`
  (without `checkpoint_dir` / `tracing_log_path` which are
  path-dependent). Loaded checkpoints whose `config_hash` differs
  from the live config abort with `FatalError::ConfigHashMismatch`
  (variant added to `BuildError` in §1).
- `drain_committed_at_us_since_epoch` is the timestamp at which the
  GPU drain completed and the counters were latched. Used for
  diagnostics.
- `worker_states[]` is **required** in v2 (it's not optional like the
  initial-draft suggestion); per-SNR-boundary checkpoints set
  `frames_in_worker` from the executor's authoritative counter
  (see "Drain commit contract" below).

### ~~Migration tool~~ — REMOVED 2026-06-08 (clean-cut, no migration)

~~Binary at `crates/gf2-sim/src/bin/checkpoint_migrate.rs` converting
legacy v1 checkpoint dirs to v2.~~ **Removed per the user clean-cut
decision (no previous runs, no users ⇒ no v1 back-compat and no
migration tool).** There is no `checkpoint_migrate` binary and no v1
handling anywhere in the pipeline; the reader is v2-only and rejects
any non-v2 file as a hard load error.

### Drain commit contract

`[fixed: C4]` At heartbeat-flush time:

1. Executor calls `Scheduler::drain_for_checkpoint()`.
2. Drain iterates each worker's owned HIP stream and calls
   `hipStreamSynchronize()` (not `hipDeviceSynchronize()` — that
   blocks unrelated contexts).
3. After all streams report idle, each worker's
   `frames_in_worker` counter is the SSOT for that worker's next
   `worker_offset(seed, snr_idx, worker_idx, frames_in_worker)`.
4. The executor latches `worker_states[]` from the SSOT, writes
   the JSON to `<dir>/snr_NNNN.json.tmp`, fsyncs, renames.
5. Workers resume from the next batch using the recorded
   `frames_in_worker` value.

In-flight batches at drain time MUST complete and increment their
worker's `frames_in_worker` before the JSON is written. No partial
batches are recorded mid-flight.

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

`gf2-sim` `Cargo.toml` (`[fixed: B1, H1]`):

```toml
[dependencies]
gf2-core    = { path = "../gf2-core" }
gf2-coding  = { path = "../gf2-coding" }
gf2-kernels-hip = { path = "../gf2-kernels-hip", optional = true }
rayon            = "1"
rand_chacha      = "0.9"      # Q1 decision: future-aligned
rand             = "0.9"
tracing          = "0.1"
serde            = { version = "1", features = ["derive"] }
ctrlc            = "3"        # for SIGINT handler in 5f12e7ff
blake3           = "1"        # for v2 checkpoint config_hash

[features]
default = []
hip     = ["dep:gf2-kernels-hip"]
llr-f64 = []
```

Workspace member pattern (matches the existing `gf2-coding ↔
gf2-kernels-hip` setup verified in root `Cargo.toml` and
`crates/gf2-coding/Cargo.toml` line 54): the path-dep on the
optional `gf2-kernels-hip` is gated by the cargo feature. Cargo
resolves but does not build `gf2-kernels-hip` when `--features hip`
is absent (confirmed by the existing `gf2-coding` build on non-ROCm
hosts in CI).

Public surfaces of `gf2-sim`:

- `gf2_sim::Pipeline`, `gf2_sim::Stage`, `gf2_sim::AnyStage`,
  `gf2_sim::Connector`, `gf2_sim::Edge`, `gf2_sim::StageId`.
- `gf2_sim::PipelineConfig`, `gf2_sim::RunnerCfg`.
- `gf2_sim::StageError`, `gf2_sim::RecoverableError`,
  `gf2_sim::FatalError`, `gf2_sim::BuildError`.
- `gf2_sim::presets::dvb_t2::Pipeline` (typestate builder).
- `gf2_sim::graph::Chain`.
- `gf2_sim::channels::{Awgn, Rayleigh, Rician}`.
- `gf2_sim::gpu::HipDispatcher` (only with `--features hip`).

CI: `cargo test --workspace` works on non-ROCm hosts (HIP feature
off by default).

---

## §6 Multi-arch HIP dispatch

(Unchanged from initial draft; reviewer flagged no issues.)

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
matches, the dispatcher emits a `tracing::warn!` event with the
device ID and falls back to the CPU equivalent stage (consistent
with the OOM policy in §8).

Kernel blob format: one `.co` file per (kernel, gfx-target) compiled
ahead of time by the `gf2-kernels-hip` build script (`build.rs`)
invoking `hipcc --offload-arch=<target>`. The build script
gracefully skips any arch whose toolchain is missing — gfx1030 must
always be compiled successfully; the others are best-effort.

---

## §7 Multi-GPU extension seams

`[fixed: H5]` Present tense → future tense per reviewer note.

Single-GPU is v1. Phase B `36075e4c` introduces the host-side types
(`HipDispatcher`, `HipStream`, `HipStreamPool`) that v1 uses; the
multi-GPU extension is additive on top of them. The seams the v1
design admits without breaking change:

1. **Per-device stream pool**: v1 `HipDispatcher` will own a single
   `HipStreamPool`. Multi-GPU adds `HipDispatcher::new_per_device(
   device_ids: &[i32])` that owns `HashMap<DeviceId, HipStreamPool>`.
   The `Stage` trait sees one stream per call; the dispatcher routes.

2. **Worker→device assignment**: `PipelineConfig::gpu_assignment(
   strategy: GpuAssignmentStrategy)` (future). Variants:
   `RoundRobin` (default), `Static(Vec<DeviceId>)`, `MemoryLoadAware`.
   v1 implements only single-device; the enum lands when multi-GPU
   does.

3. **Aggregation step**: already device-agnostic — the reducer sums
   per-worker counters, not per-device.

v1 hard-codes single-device. The above seams are documented now so
the multi-GPU extension is purely additive; no v1 signature
forecloses them.

---

## §8 Failure-mode policy

`[fixed: B5, C2]` Fallback registration via associated type per Q6;
`FatalError::OutOfMemory` cross-link confirmed.

| Failure | Detection | Response |
|---|---|---|
| `hipMalloc` OOM | GPU stage returns `RecoverableError::OutOfMemory{ device_id, bytes_requested }` | Executor catches; substitutes `stage.cpu_fallback()` (via the `Stage::CpuFallback` associated type) on the offending batch; emits `tracing::warn!{batch_id, snr_idx, device_id, bytes_requested}`; continues. |
| Kernel launch error | GPU stage returns `FatalError::KernelLaunch{ hip_code, kernel, args }` | Executor hard-fails: logs JSON diagnostic dump to `<diagnostic_dir>/<timestamp>.json` and exits with non-zero code. |
| Driver / device unavailable | `hipDeviceGetCount() == 0` at pipeline construction | `Pipeline::build()` returns `FatalError::DeviceUnavailable`. User invokes `--cpu-only` to skip GPU stages. |
| CPU fallback also fails | Substituted CPU stage returns any error | `FatalError::CpuFallbackAlsoFailed{ original }`. No infinite loop. |
| `--strict-gpu` set, OOM occurs | Same `RecoverableError::OutOfMemory` from the GPU stage | Executor promotes to `FatalError::OutOfMemory { device_id, bytes_requested }` (the variant added to §1 per Q7) and hard-fails. |

**OOM/hard-fail seam (resolves H5, C2):**

- **GPU stages (Phase B `ed575f15` scope)**: only signal an
  `OutOfMemory` error from their `process()`. They do NOT
  implement the fallback dispatch.
- **Executor (Phase C `42eac5cc` scope)**: catches the OOM signal,
  invokes the GPU stage's associated `Stage::CpuFallback` type via
  `stage.cpu_fallback().expect("preset registered fallback")` on
  the same input, continues.
- **Pipeline build registers fallbacks**: presets use the
  associated-type machinery (each GPU stage's `CpuFallback`
  associated type names the CPU stage to use). The graph API uses
  `Chain::register_fallback(gpu_stage_id, cpu_stage_id)` which
  installs a runtime entry in `Pipeline.fallbacks`. The build check
  rejects any GPU stage without a registered fallback via
  `BuildError::NoFallback { gpu_stage }`.

**CLI override**: `--strict-gpu` promotes OOM to
`FatalError::OutOfMemory` (no fallback). Used by
reproducibility-critical experiments.

---

## §9 Layered builder vs graph API surfaces

`[fixed: C5]` Graph example rewritten to use **stage adapter
constructors that Phase A workers introduce** rather than nonexistent
constructors on `gf2-coding` types.

### Typestate builder (presets)

```rust
use gf2_sim::presets::dvb_t2::{Pipeline, Modcod, Channel};
use gf2_sim::{DecoderConfig, DemapMethod};

let pipeline = Pipeline::builder()
    .modcod(Modcod::Normal { rate: Rate::R1_2, modulation: Mod::Qam16 })
    .decoder(DecoderConfig::sum_product())
    .demap(DemapMethod::ExactLogMap)
    .channel(Channel::awgn(es_n0_db))
    .parallelism(NonZeroUsize::new(24).unwrap())
    .seed(0xC0DEF00D)
    .checkpoint_dir(Some("/tmp/dvb_run/checkpoints".into()))
    .build()?;
```

Compile-time guarantees: typestate generic parameter enforces stage
order; invalid `(rate, modulation)` combinations rejected at
`.build()` via `Modcod::validate()` returning
`BuildError::InvalidModcod` (declared in §1). Legal combinations:
rate ∈ {1/2, 2/3, 3/4} × modulation ∈ {16qam, 64qam}; the six
DVB-T2 in-scope MODCODs.

### Graph API (novel chains)

`[fixed: C5]` The example uses **`gf2-sim` stage adapter
constructors** that `81d05bab` and `c09d3e95` introduce. These
adapters wrap the existing `gf2-coding` types (`DvbT2Concat`,
`DvbT2BitInterleaver`, `BatchMapper`, `BatchSoftDemapper`).
Adapter constructor names are stable from Phase A onward.

```rust
use gf2_sim::graph::Chain;
use gf2_sim::stages::dvb_t2 as adapters;       // 81d05bab introduces
use gf2_sim::stages::channels::AwgnStage;       // db9836e4 introduces
use gf2_sim::stages::modem::{QamMapStage, QamDemapStage};
use gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat;
use gf2_coding::ldpc::FrameSize;
use gf2_coding::CodeRate;

let codec = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2);

let mut chain = Chain::new();
let bch_enc      = chain.add(adapters::BchEncoderStage::from(&codec));
let ldpc_enc     = chain.add(adapters::LdpcEncoderStage::from(&codec));
let interleave   = chain.add(adapters::BitInterleaverStage::dvb_t2(rate, mod_));
let modulate     = chain.add(QamMapStage::new(mod_));
let channel      = chain.add(AwgnStage::new(es_n0_db, mod_));
let demap        = chain.add(QamDemapStage::new(mod_, DemapMethod::ExactLogMap));
let deinterleave = chain.add(adapters::BitInterleaverStage::dvb_t2_inverse(rate, mod_));
let ldpc_dec     = chain.add(adapters::LdpcDecoderStage::from(&codec, DecoderConfig::sum_product()));
let bch_dec      = chain.add(adapters::BchDecoderStage::from(&codec));

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

- `BuildError::TypeMismatch { from_stage, from_type, to_stage, to_type }`
- `BuildError::Cyclic { involved }`
- `BuildError::Disconnected { stages }`
- `BuildError::NoFallback { gpu_stage }` (only when a GPU stage is
  used without a registered CPU fallback and `--strict-gpu` is off)
- `BuildError::InvalidModcod { rate, modulation }`

The DVB-T2 typestate preset is implemented as a thin wrapper over
the graph API; the two share the underlying machinery. The example
`examples/dvb_t2_two_apis.rs` (landed by `8c8302c8`) shows the two
forms side-by-side producing byte-identical output for the same
input batch.

**Adapter constructor responsibility**: `81d05bab` (DVB-T2 preset)
introduces every `adapters::*` constructor used above. `c09d3e95`
(graph API) introduces the `gf2_sim::stages::modem::*` and
`gf2_sim::stages::channels::*` constructors. The adapter constructors
are part of `gf2-sim`'s public API; the wrapped `gf2-coding` types
stay internal.

---

## §10 Numerical precision strategy

`[fixed: M3]` `Numeric` trait stub added.

| Quantity | Default | Configurable |
|---|---|---|
| LLR storage | f32 | `--features llr-f64` (cfg-gated `type Llr = f64`) |
| AWGN noise samples | f32 | not configurable |
| BP messages | f32 | not configurable in v1 |
| BER reductions | f32, non-associative | excluded from byte-identity (status quo) |
| Frame/error counters | u64 | always integer-exact |

`Numeric` trait stub (§ for future precision work):

```rust
pub trait Numeric: Copy + Send + Sync + 'static {
    fn from_f32(v: f32) -> Self;
    fn to_f32(self) -> f32;
    fn box_muller_pair(u1: Self, u2: Self) -> (Self, Self);
    fn bp_check_node_op(messages: &[Self]) -> Self;
}

impl Numeric for f32 { /* v1 impl */ }
// f64 impl behind `feature = "llr-f64"` or future work.
```

Mixed precision (f32 storage + f64 accumulation for BP check nodes)
is deferred. Adding `Numeric for f64` + a feature flag is a single
trait impl.

---

## §11 Determinism contract summary

`[fixed: H2, Q3]` Per Q3, `mean_iters` is dropped from the CPU-vs-GPU
contract.

### CPU-only / CPU-parallel contract (unchanged)

The four columns `fer`, `frames`, `errors`, `mean_iters` are
**byte-identical** across worker counts {1, 2, 4, 8, 24} at fixed
seed. Resume-from-checkpoint produces byte-identical results vs
uninterrupted run at fixed seed.

### CPU-vs-GPU contract (relaxed)

The **three** columns `fer`, `frames`, `errors` are byte-identical
across CPU-only vs CPU+GPU at fixed seed. **`mean_iters` is
EXCLUDED** from CPU-vs-GPU byte-identity.

**Rationale**: RDNA2 hardware transcendentals (`v_sin_f32`,
`v_cos_f32`, hardware `tanh` via `v_exp_f32`) differ from CPU
`f32::sin`/`f32::cos`/`f32::tanh` polynomial reductions by 1–3
ULPs in some ranges. For LDPC BP near the convergence threshold,
ULP differences in LLR messages can change the iteration at which
the parity-check passes by ±1 — so `mean_iters` is not bit-exact
across paths. The frame's final verdict (does the codeword decode
correctly?) is robust to that drift; `fer`/`frames`/`errors`
remain byte-identical because BP convergence is determined by a
parity-check at each iteration boundary that has integer (not
floating-point) state.

### Always-excluded

`ber` (non-associative f32 horizontal reduction; status-quo
amendment from `152388f4`).

`wall_seconds` (run-duration-dependent).

### Operationalisation

Per-worker ChaCha20 seek (§3) plus the worker-index-ordered
aggregation. The contract is the same across the CPU-only/parallel
paths; the GPU-vs-CPU relaxation lives in `14f59c2d`'s and
`0d9cb8e3`'s test bodies (they assert byte-identity of the three
columns, log `mean_iters` for diagnostics without asserting).

---

## §12 Migration plan from `simulation.rs`

`[fixed: C6, M4]` `install_campaign_subscriber` clarified as **new**
code with the same effect as `simulation.rs:370`'s
`setup_tracing_guard`; legacy-vs-new byte-identity is dropped per
Q2.

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
| `SimulationConfig` | `gf2_sim::PipelineConfig` (with `From` impl) | `118a0091` |
| Per-SNR checkpointing helpers (`crates/gf2-coding/src/bin/sim_checkpoint_helper.rs`) | `gf2_sim::checkpoint::{Reader, Writer}` (v2 schema) | `5f12e7ff` |

### Public APIs that **stay** in `gf2-coding`

| API | Reason |
|---|---|
| `SimulationRunner::run_uncoded_ber*` (L1517–L1721) | Pre-coded baseline runs; not pipeline-shaped. |
| `BpskAwgnChannel` | Composes inside `gf2_sim::channels::Awgn` as a building block, not a Stage. |
| `BicmAwgnChannel` (campaign-binary's internal channel) | Same — wrapped by `gf2_sim::channels`. |
| All BCH / LDPC / QAM codec types | Codes are not Stages. They are inputs to Stage implementations. |
| `DvbT2Concat` codec | Wrapped by `gf2_sim::presets::dvb_t2`; `81d05bab` reuses it. |
| All ETSI table data (`crates/gf2-coding/data/...`) | Pure data; not pipeline scope. |
| `simulation.rs:370` `setup_tracing_guard` (private) | Stays internal to `simulation.rs`; the campaign binary's new pipeline path uses `gf2_sim::observability::install_campaign_subscriber` instead. |

### New code

`gf2_sim::observability::install_campaign_subscriber(config: &PipelineConfig) -> impl Drop`
is **new** code (not a migration of an existing public API). Its
effect mirrors the private `setup_tracing_guard(config)` invoked
inside `simulation.rs:370`; the campaign binary's `main()` calls
the new public function explicitly when `bbf6b6ee` lands.
`118a0091` lands the stub; `bbf6b6ee` wires the binary's `main()`
to call it.

### Byte-identity vs legacy

Per Q2 (user-approved 2026-06-07), `bbf6b6ee`'s success criterion is
"byte-identical between two new-pipeline runs at the same seed",
NOT "byte-identical vs the legacy `simulation.rs` path." The legacy
seed derivation (`seed = base ^ rotate_left(snr_index, 13)`,
`simulation.rs:431`) is intentionally not preserved; the new
pipeline uses the cleaner `worker_offset` from §3.

### Migration sequence

1. `118a0091` lands `gf2-sim` scaffolding (Cargo.toml + module
   skeleton + `PipelineConfig` with `From<SimulationConfig>`).
2. `3fcb7025` lands `Pipeline::run_parallel` skeleton + per-worker
   dispatch + reducer.
3. Phase B in parallel lands HIP host infra and GPU stages.
4. `75c22fa8` wires the hybrid scheduler.
5. `bbf6b6ee` migrates `crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs`
   to call the new pipeline. Existing call sites of `SimulationRunner`
   in `gf2-coding` are left alone — the migration is per-binary, not
   a hard cut.
6. A future epic (out of `gf2-sim` scope) deprecates `simulation.rs`
   paths after all callers migrate.

This keeps `gf2-coding` stable during the migration and bounds the
blast radius to the campaign binary.

---

## §13 Phase 0 closure checklist (non-normative)

`[fixed: M1]` Section renamed to "non-normative" so the 12-section
contract from `ec530af9`'s body is unambiguous (this is a 13th
section serving as a self-check, not a content section).

- [x] §1 Stage/Connector trait shapes + module skeleton + type
      erasure + concrete types for `AnyStage`, `StageScratch`,
      `BatchHandle`, `Edge`, `BuildError`
- [x] §2 SoA↔AoS conversions
- [x] §3 ChaCha20 per-worker seek scheme (FRAME_STRIDE=2^20,
      WORKER_STRIDE=2^40, SNR_STRIDE=2^56; 32-bit-word units; legacy
      compat dropped; FRAME_STRIDE sized for the QPSK-Normal worst case
      per the 2026-06-07 amendment)
- [x] §4 Heartbeat-checkpoint schema v2 (no v1 back-compat; clean-cut,
      NO migration tool — removed 2026-06-08 per user decision)
- [x] §5 Crate-boundary diagram + `rand_chacha = "0.9"`
- [x] §6 Multi-arch HIP dispatch
- [x] §7 Multi-GPU extension seams (future-tense)
- [x] §8 Failure-mode policy (with OOM seam decision; associated
      type fallback)
- [x] §9 Layered builder vs graph API (with adapter-constructor
      ownership)
- [x] §10 Numerical precision (with Numeric trait stub)
- [x] §11 Determinism contract summary (CPU-vs-GPU relaxed to
      three columns)
- [x] §12 Migration plan from `simulation.rs` (with legacy compat
      explicitly dropped)

All 12 mandatory sections present with implementable detail. No
"TBD" leaves. Decisions deferred to future tasks are explicitly
named:

- Mixed precision (§10): future task triggered if BP accuracy is a
  bottleneck (no known trigger today).
- Multi-GPU concretisation (§7): future task triggered when a
  multi-GPU host is on hand.
- Full `simulation.rs` deprecation (§12): future epic.

---

## Revision log

| Revision | Date | Notes |
|---|---|---|
| 1 (initial) | 2026-06-07 | First draft of all 12 sections; committed at `808b79f6` |
| 2 (this) | 2026-06-07 | Adversarial-review-driven revision. 5 BLOCKING + 6 CRITICAL + 6 HIGH + 4 MEDIUM + 3 LOW findings folded in. Seven escalation questions answered by user. Specific revision points tagged inline as `[fixed: <finding-id>]`. |
| 3 (impl note) | 2026-06-07 | Two MSRV/coherence-forced implementation deviations recorded during Phase A scaffolding (`118a0091`), for downstream Phase B/C stage implementers. **(a) `Stage::CpuFallback` has NO `= Self` default** — associated-type defaults are unstable on MSRV 1.95 (E0658). Every `Stage` impl names `type CpuFallback` explicitly (`= Self` for pure-CPU stages). Q6 intent (compile-bound CPU fallback) is preserved. **(b) The §1 "blanket `impl AnyStage for S`" is realised via an `ErasedStage<I,O,S>` adapter + `erase()` helper in `stage.rs`** — a literal `impl<I,O,S: Stage<I,O>> AnyStage for S` is rejected by E0207 (`I`/`O` unconstrained by `Self`). Additionally `TypedBatch` is split: implementers define `BatchSize` (one method) and a blanket impl supplies `TypedBatch` + the `as_any` downcast hook. `Stage::Scratch` gained a `'static` bound (required for `Any` downcasting). Code SSOT for implementers is `crates/gf2-sim/src/stage.rs`. |
