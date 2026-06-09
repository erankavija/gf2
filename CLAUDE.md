# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Vision

A **research-grade** toolkit for high-performance finite field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Commands

```bash
# Build workspace
cargo build --workspace --all-features

# Run all tests (fast tier — default, matches CI) — ALWAYS use --release
cargo nextest run --workspace --all-features --release --profile ci

# Run tests for a single crate
cargo nextest run -p gf2-core --release --profile ci
cargo nextest run -p gf2-coding --release --profile ci
cargo nextest run -p gf2-algebra --release --profile ci

# Run a single test by name
cargo nextest run -p gf2-core --release -E 'test(test_name)'

# Check formatting (CI enforces this)
cargo fmt --all -- --check

# Fix formatting
cargo fmt --all

# Lint (CI treats warnings as errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build documentation
cargo doc --no-deps --open

# Benchmarks
cargo bench -p gf2-core
cargo bench -p gf2-coding
cargo bench -p gf2-algebra

# Run examples
cargo run -p gf2-coding --example hamming_7_4
cargo run -p gf2-coding --example dvb_t2_ldpc_basic
cargo run -p gf2-coding --example ldpc_awgn --release
cargo run -p gf2-algebra --example permanent_demo --release

# DVB-T2 BICM AWGN campaign runner (one invocation per rate×modulation pair)
# Binary:  crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs
# Plotter: dev/benchmarks/dvb_t2_awgn/plot.py
# Reference TOML: crates/gf2-coding/data/dvb_t2_tr102831_reference.toml
cargo run --release --bin dvb_t2_awgn_campaign -- \
    --rate 1/2 --modulation 16qam \
    --esn0-range 4.0:7.0:0.5 --target-errors 100 \
    --output-dir /tmp/dvb_r12_16qam --seed 42
# After a campaign, produce a PNG overlay vs ETSI TR 102 831 reference:
# python3 dev/benchmarks/dvb_t2_awgn/plot.py \
#     --curve-csv /tmp/dvb_r12_16qam/curve_1_2_16qam.csv \
#     --reference-toml crates/gf2-coding/data/dvb_t2_tr102831_reference.toml \
#     --output /tmp/dvb_r12_16qam/curve_1_2_16qam.png

# Lean4 verification pipeline (requires charon + aeneas + elan)
./scripts/verify-lean.sh

# Just build the committed Lean files (requires elan only)
cd proofs && lake build
```

## Test tiers

Two tiers. Use the fast tier by default. Never run the slow tier as an agent.

| Tier | Command | Per-test limit | Who runs it |
|------|---------|---------------|-------------|
| Fast | `cargo nextest run --workspace --all-features --release --profile ci` | 5 s (hard kill) | CI + agents |
| Slow | `cargo nextest run --workspace --all-features --release --profile slow --run-ignored ignored-only` | 120 s | Nightly CI only |

**Rules — read carefully:**
- **NEVER** pass `--run-ignored all`, `--run-ignored ignored-only`, `-- --ignored`, or `-- --include-ignored` in normal work. Those unlock the slow tier and will stall the agent for minutes.
- Any test calling `SimulationRunner`, `run_curve`, `run_coded`, or `run_coded_iterative` with `max_frames > 50` or `max_queries > 500` **MUST** carry `#[ignore = "sim: <description>"]`.
- Any test expected to exceed 5 s **MUST** carry `#[ignore = "slow: <description>"]` or `#[ignore = "sim: <description>"]`.
- Tests requiring external ETSI test vector files use `#[ignore = "external: <description>"]`.

## Performance rules for test and build commands

1. **ALWAYS use `--release`**. Debug-mode tests take 10–100x longer due to unoptimized SIMD, crypto, and simulation code.
2. **Never run multiple `cargo nextest` or `cargo build` commands in parallel.** They compete for the same build cache and cause lock contention. Run one at a time.
3. **For targeted testing during development**, use `-p gf2-coding` instead of the full workspace.
4. **Test suite wall-clock limit: 60 seconds.** Nextest enforces 5 s per test; if the full suite exceeds 60 s, a test is missing its `#[ignore]`.
5. **Examples and benchmarks also need `--release`** — simulation examples can be 100x slower without optimization.

## Architecture

This is a Cargo workspace with five production crates:

- **`gf2-core`** (`crates/gf2-core/`) — Low-level primitives. No dependencies on the other workspace crates. All purely mathematical operations, data structures, and algorithms go here.
- **`gf2-coding`** (`crates/gf2-coding/`) — Error-correcting codes; depends on `gf2-core`.
- **`gf2-algebra`** (`crates/gf2-algebra/`) — Packed F_3 / F_5 / F_7 element types and fast matrix permanents (bipedal F_3, packed F_5 / F_7) on CPU (scalar, AVX2, Rayon) and HIP/ROCm GPU. Depends on `gf2-core`. Delivers the `gf2-algebra-permanent` epic: ~10.6x single-thread AVX2 speedup over the in-tree Rust reference at n=36; GPU batch ~28-30x CPU-SIMD at n=24/28 (M=256); F_5/F_7 packed kernels; Lean V1 bipedal F_3 correctness proof complete, Lean V2 (Ryser bounded n<=63) in progress.
- **`gf2-sim`** (`crates/gf2-sim/`) — CPU+GPU pipeline: research-grade FEC simulation harness composing `gf2-coding` codes into a parallel, optionally GPU-accelerated, deterministic pipeline via `Pipeline` / `Stage` / `Connector` primitives. Depends on `gf2-core` and `gf2-coding`; optional HIP/ROCm acceleration behind feature `hip`. The v2 successor to `gf2_coding::simulation`. `#![deny(unsafe_code)]`. Design SSOT: `dev/active/ec530af9-pipeline-design.md`.

  **Within-SNR parallelism + determinism contract (`parallel/` module, design doc §3/§11).** The within-SNR frame parallelism lives in `gf2-sim/src/parallel/mod.rs`: `worker_offset(seed, snr_idx, worker_idx, frame_idx_in_worker)` is the verbatim §3 ChaCha20 word-position seek in **32-bit-word units** (`SNR_STRIDE = 1<<56`, `WORKER_STRIDE = 1<<40`, `FRAME_STRIDE = 1<<20` — raised from `1<<16` in the 2026-06-07 §3 amendment after the measured worst-case per-frame draw of 260 208 32-bit words for **r1/2 QPSK Normal** (the lowest-order modulation has the most symbols, hence the most noise draws — QPSK binds, not 16-QAM) exceeded the old budget; `seed` selects the *stream*, the offset selects the *position*). A fast-tier regression guard `parallel::tests::test_worst_case_frame_draw_under_stride` enumerates every supported modulation, measures each per-frame draw, and asserts QPSK is the max with ~4x headroom. `run_snr_point(...)` fans out global frames `0..max_frames` across `parallelism` rayon workers; each worker owns its own seeked `ChaCha20Rng` inside a `WorkerCtx`. **The byte-identity guarantee** (`fer`/`frames`/`errors`/`mean_iters` identical across worker counts {1,2,4,8,24} at fixed seed) rests on two rules: (1) **each frame's RNG is keyed on the global frame index** — `run_snr_point` reseeks to `worker_offset(seed, snr_idx, 0, g)` before every frame, so the per-frame outcome is a pure function of `g` regardless of which physical worker ran it (the `worker_idx` term is reserved for the Phase C executor / GPU paths that own fixed partitions); (2) **per-worker counters are reduced in `worker_idx` order** via `WorkerCounters::reduce_in_worker_order` (the SSOT order, mandatory even though `u64` sums are order-invariant). The reusable single-frame DVB-T2 BICM-AWGN kernel is `gf2-sim/src/frame_sim.rs::DvbT2BicmFrameSim` (draws all randomness — random BBFRAME + AWGN — from the per-worker `rand_chacha 0.9` stream, the §5 pin). The byte-identity regression is guarded by two complementary slow-tier suites: `tests/parallel_determinism.rs` (direct `frame_sim` dispatch, 3 configs × {1,2,4,8,24} workers, issue `3fcb7025`) and `tests/determinism.rs` (typestate preset path via `Pipeline::dvb_t2()` + heartbeat-resume parity, issue `48a0db6c`); a fast-tier smoke guard for the seek/aggregation logic lives in `parallel/mod.rs` unit tests. Throughput receipts: `dev/benchmarks/gf2-sim/parallelism-receipts.md`; benchmark bin `cargo run -p gf2-sim --release --bin parallel_throughput`.
- **`gf2-kernels-simd`** (`crates/gf2-kernels-simd/`) — Isolated unsafe SIMD kernels (AVX2/AVX512/AARCH64).
- **`gf2-kernels-hip`** (`crates/gf2-kernels-hip/`) — Isolated unsafe HIP/ROCm GPU kernels (device FFI, gfx1030; currently BCJR batch decode + Gray-QAM soft demap prototype, and gf2-algebra batch permanents). Excluded from the default workspace so non-ROCm hosts still build cleanly; opt in via `--features hip` on `gf2-coding` or `gf2-algebra`, or by building the crate with its own manifest. The `host/` module provides the `gf2-sim` GPU pipeline host plumbing (design doc §6) — see the HIP host dispatcher model below.

#### Multi-arch HIP targets (design doc §6)

The compile-time gfx target list (quoted from design-doc §6, `dev/active/ec530af9-pipeline-design.md`). gfx1030 is the CI target compiled and exercised today; the rest are documented seams whose kernel blobs are best-effort-compiled by `build.rs` and whose runtime detection is wired but unexercised until matching hardware is available.

| Target | Arch family | CI today |
|---|---|---|
| gfx1030 | RDNA2 (RX 6800/6900/6950 XT) | yes |
| gfx1100 | RDNA3 (RX 7900 XT/XTX) | seam only |
| gfx1200 | RDNA4 | seam only |
| gfx90a | CDNA2 (MI200) | seam only |
| gfx940 | CDNA3 (MI300 gfx940 stepping) | seam only |
| gfx942 | CDNA3 (MI300 gfx942 stepping) | seam only |

Runtime detection (`GfxTarget::detect` / `detect_device` in `crates/gf2-kernels-hip/src/host/arch.rs`): the dispatcher reads the device's **GCN arch name string** via `hipGetDeviceProperties`' `gcnArchName` (`query_arch_name`), strips any feature suffix (e.g. `"gfx942:sramecc+:xnack-"` → `"gfx942"` by splitting on the first `':'`), and maps the canonical `gfxNNNN` head to a `GfxTarget` (`from_arch_name`). Detection is **name-based, not compute-capability-based** — gfx940 and gfx942 share the *same* compute capability (9.4) but load *different* kernel blobs, so only the name string distinguishes them; compute-capability matching could not. The matched target selects a precompiled kernel blob under `crates/gf2-kernels-hip/kernels/<gfx-target>/`. If the arch name matches no blob this build compiled, `detect` emits a `tracing::warn!` event carrying the raw `gcnArchName` and returns `HipError::UnsupportedArch`, which the `gf2-sim` boundary maps to a recoverable transient fault so the executor falls back to the CPU equivalent stage (consistent with the OOM policy in §8).

#### HIP host dispatcher model (design doc §5, §6, §8)

The crate-boundary layering the dispatcher sits in (quoted from design-doc §5, `dev/active/ec530af9-pipeline-design.md`): `gf2-sim` is the new crate atop `gf2-coding`/`gf2-core`, with the `gf2-kernels-hip` GPU FFI an *optional* (`feature = "hip"`) leaf dependency.

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

The `gf2-sim` GPU pipeline (`feature = "hip"`) is built on host plumbing in `gf2-kernels-hip::host`:

- **`HipStreamPool`** (`host/streams.rs`) — fixed-size pool of RAII `HipStream`s bound to one device; hands out streams round-robin (`acquire`, deterministic) or oldest-idle (`acquire_idle`, which surfaces genuine `hipStreamQuery` faults as `Err` while skipping merely-busy `hipErrorNotReady` streams). The pool is `Send + Sync` so it can be shared by reference (`&HipStreamPool`) across rayon workers (Phase C scheduler `75c22fa8`): each worker calls `acquire`/`acquire_idle` to obtain a *distinct* stream via the shared atomic cursor. `Sync` is sound because `HipStream` is itself `Sync` (its `&self` methods are read-only or thread-safe HIP-runtime calls; no `&self` host mutation). A compile-time `_assert_sync` test in `streams.rs` enforces the bound.
- **`DeviceBuffer<T>` / `PinnedHostBuffer<T>`** (`host/alloc.rs`) — the single canonical RAII `hipMalloc`/`hipFree` (and pinned `hipHostMalloc`) primitives. The in-crate decoder/demapper/permanent kernels use `DecoderDeviceBuffer`, a byte-oriented adapter over `host::DeviceBuffer<u8>` (no second hipMalloc/hipFree implementation). Both buffer types are `Send`-only and deliberately **not** `Sync`: `DeviceBuffer::copy_from_host` / `copy_from_pinned_async` mutate device memory through a shared `&self`, so sharing one buffer by `&` across threads would be a data race. The concurrency model is per-worker-owned buffers (moved in via `Send`), never shared by `&`. Consequently `HipDispatcher` (which embeds a pinned `StageScratch`) is `Send` but not `Sync`; workers share the *pool* via `dispatcher.streams()`, not the dispatcher itself.
- **`GfxTarget`** (`host/arch.rs`) — name-based multi-arch detection (above).
- **`HipDispatcher`** (`gf2-sim/src/gpu/mod.rs`) — owns the `HipStreamPool` plus per-stage scratch; the `gf2-sim`-side consumer the Phase B kernel stages and Phase C executor build on.

Error mapping at the `gf2-sim` boundary (`map_hip_error` in `gf2-sim/src/gpu/mod.rs`, design doc §8): `HipError::OutOfMemory` → `RecoverableError::OutOfMemory` (executor substitutes the CPU fallback; `--strict-gpu` promotes to fatal in the executor); `HipError::UnsupportedArch` → `RecoverableError::Transient` after a `tracing::warn!` (CPU fallback per §6, **not** fatal); `HipError::NoDevice` → `FatalError::DeviceUnavailable`; `HipError::BlobLoad` (a typed file-I/O failure when a kernel `*.co` for the active arch is missing/unreadable; carries the path and reports `hipErrorFileNotFound` = 301, never a fabricated `hipSuccess` code) → `FatalError::KernelLaunch` (configuration fault, fatal); any other `HipError::Hip` → `FatalError::KernelLaunch`.

Unsafe code lives exclusively in these two kernel crates; everything else uses `#![deny(unsafe_code)]`. Standalone `dev/research/<crate>/` stubs (non-workspace prototypes) are exempt: they may contain `unsafe` if necessary to exercise the surface they prototype, provided each `pub unsafe fn` carries a top-of-function `// SAFETY:` comment explaining the preconditions the caller must uphold. Production crates remain bound by the kernel-crates-only rule.
- **`proofs/`** — Lean4 formal verification of `gfp/` and `gfpn/` field arithmetic and `gf2-algebra::packed::bipedal3` correctness, auto-generated via Charon/Aeneas. See `proofs/README.md`. Covers `Fp<P>` (Montgomery arithmetic), `QuadraticExt`, `CubicExt` (tower extensions), and bipedal F_3 add/sub/mul/neg.

### gf2-core module map

| Module | Purpose |
|--------|---------|
| `bitvec` / `bitslice` | Dense bit storage in `Vec<u64>`, little-endian bit order |
| `matrix` | `BitMatrix` — row-major bit-packed matrix |
| `sparse` | CSR/CSC sparse matrices |
| `alg/` | M4RM multiplication, Gauss-Jordan inversion, RREF |
| `field/` | `FiniteField` / `ConstField` trait hierarchy and axiom test harness |
| `gf2m/` | GF(2^m) arithmetic, generic over storage width via sealed `UintExt` trait |
| `gfp/` | GF(p) prime field `Fp<P>` with Montgomery multiplication internals |
| `gfpn/` | Tower extensions: `QuadraticExt<C>`, `CubicExt<C>` over `ExtConfig` trait |
| `primitive_polys` | Static database of primitive polynomials for m=2..16 |
| `kernels/` | Runtime dispatch to scalar or SIMD backends |
| `compute/` | Parallel batch operations (rayon backend) |
| `io/` | Serde-based serialization (feature-gated) |

### gf2-algebra module map

| Module | Purpose |
|--------|---------|
| `packed/` | `PackedField` / `PackedFieldVec` traits and per-prime impls: `Bipedal3` (F_3, 64 lanes), `Packed5` (F_5, 64 lanes), `Packed7` (F_7, 16 lanes), plus `*Matrix` types for each |
| `permanent/` | `permanent_ryser` (field-generic oracle), `permanent_mod3_reference` (paper baseline), `permanent_bipedal{3,5,7}` fast paths, parallel and multi-word variants |
| `gray` | Gray-code subset enumerator used by Ryser's formula and all bipedal kernels |
| `parallel` | Rayon-based work-stealing dispatch (feature = "parallel", default on) |
| `gpu` | HIP/ROCm host-side batch dispatcher (feature = "hip", default off) |
| `testutil` | Deterministic random matrix generators (feature = "test-support" or `cfg(test)`) |

### gf2-coding module map

| Module | Purpose |
|--------|---------|
| `linear` | `LinearBlockCode`, `SyndromeTableDecoder` — Hamming codes |
| `bch/` | BCH codes with Berlekamp-Massey + Chien search; `dvb_t2/` sub-module contains all 12 DVB-T2 configurations |
| `ldpc/` | Belief-propagation decoder; `dvb_t2/` has tables from ETSI EN 302 755; `encoding/` uses Richardson-Urbanke with cache; `dvb_t2/concat.rs` = `DvbT2Concat` (BCH+LDPC concatenated codec); `dvb_t2/bit_interleaver.rs` = `DvbT2BitInterleaver` (column-row bit interleaver) |
| `modem/` | Gray-QAM mapper (`GrayQamMapper`), fast demapper (`FastGrayQamDemapper`), `ModemSpec` preset workflow; see `examples/dvb_t2_bicm_chain.rs` for the canonical BICM chain composition |
| `convolutional` | Viterbi decoder skeleton |
| `traits` | `BlockEncoder`, `HardDecisionDecoder`, `GeneratorMatrixAccess` — unified interfaces |
| `llr` | `Llr` type (f32 by default, f64 with `llr-f64` feature) for soft-decision decoding |
| `channel` | AWGN channel simulation with BPSK modulation |
| `simulation` | BER/FER simulation harness; with `sim-observability` feature: per-SNR JSON checkpoints (`checkpoint_dir`), JSON-lines tracing (`tracing_log_path`), within-SNR heartbeats (`heartbeat_every_frames`), SIGINT/SIGTERM flush via `ctrlc`, deterministic `ChaCha20Rng` seek for byte-identical resume. Checkpoint/resume support: `run_coded` / `run_coded_iterative` / `run_with_decoder`: full per-SNR + within-SNR (heartbeat) checkpointing via `ChaCha20Rng::set_word_pos`; `run_uncoded_ber_with_channel`: per-SNR-boundary checkpointing only (heartbeat resume not implemented; uncoded paths are fast enough that per-SNR granularity is sufficient); `run_coded_iterative_parallel`: per-SNR-boundary checkpointing only (within-SNR heartbeat resume is architecturally unavailable with rayon-parallel SNR-point dispatch). |

Integration tests of note:
- `tests/dvb_t2_bicm_chain.rs` — end-to-end BICM roundtrip for Normal × {1/2, 2/3, 3/4} × {16-QAM, 64-QAM} (6 configs); slow tier (requires LDPC encoder, ~2-10 s per config).

Examples of note:
- `examples/dvb_t2_bicm_chain.rs` — canonical DVB-T2 BICM chain demonstration (Normal × 1/2 × 16-QAM forward + inverse composition); slow to run (same LDPC encoder constraint).

### gf2-sim module map

(The prose `gf2-sim` architecture paragraph and the within-SNR parallelism contract above are the SSOT for *how* the parallel executor and HIP host dispatcher work; this table indexes the *landed Phase A modules* — design SSOT `dev/active/ec530af9-pipeline-design.md`.)

| Module | Purpose |
|--------|---------|
| `pipeline` | `Pipeline` (built, runnable type-erased stage graph) + `BatchHandle`; obtained only through a validating builder |
| `graph` | `Chain` — the add/connect/`build()` graph API that topo-sorts, type-re-validates, and emits a `Pipeline` |
| `presets/dvb_t2` | `Pipeline::dvb_t2()` typestate fluent builder (`Modcod`/`Channel`, `NeedsModcod→…→Ready`); the production DVB-T2 BICM preset over the graph API |
| `stage` | `Stage<I,O>` / `AnyStage` type-erasure (`erase`), `BatchSize`, `ExecutionClass`, `TypedBatch`, `FallbackKind` |
| `stages` | DVB-T2 BICM stage wrappers (`DvbT2Encode`, `BitInterleave`/`BitDeinterleave`, `GrayQamMap`/`GrayQamDemap`, decode) + `dvb_t2_bicm_stages` factory |
| `connector` | `Connector`, `Edge`, `StageId` — typed edge endpoints between stages |
| `batch` | SoA batch buffer types (`BitPackedBatch`, `HardDecisionBatch`, `LlrBatch`, `SymbolBatch`) |
| `channels` | Channel stages: `awgn` (`Awgn`, `es_n0_db_to_sigma`), `rayleigh`, `rician` — per-frame noise injection from the per-worker ChaCha20 stream |
| `parallel` | Within-SNR frame parallelism: `worker_offset` (§3 ChaCha20 seek), `WorkerCtx`, `WorkerCounters`, `FrameOutcome`, `run_snr_point` / `run_snr_point_range` / `run_snr_point_stateless` |
| `frame_sim` | `DvbT2BicmFrameSim` — the reusable deterministic single-frame BICM-AWGN kernel the within-SNR dispatch drives |
| `checkpoint` | v2 heartbeat-checkpoint schema (`CheckpointV2`/`WorkerState`), `config_hash`, atomic `CheckpointWriter`, v2-only `CheckpointReader`, `run_snr_point_checkpointed` / `run_sweep_checkpointed`, SIGINT flush (`clear_interrupt`/`is_interrupted`/`request_interrupt`); `bin/checkpoint_sweep` drives the kill-during-fsync proof |
| `config` | `PipelineConfig` — the run configuration (seed, Es/N0 points, frame/error budgets, heartbeat cadence, checkpoint/tracing paths, parallelism, `strict_gpu`) |
| `error` | `BuildError`, `StageError`, `RecoverableError`, `FatalError` — the typed error hierarchy and the `gf2-sim`↔HIP error mapping boundary |
| `executor` | Hybrid CPU/GPU executor: `Scheduler` (rayon-worker ∥ HIP-stream double-buffered CPU-prep/GPU-decode overlap, `75c22fa8`), `SimulationResults`/`SnrPointResult` (the `WorkerCounters` projection `Pipeline::run` returns), `OverlapTimeline` (overlap attestation), `RunPlan`. Later Phase C tasks add OOM-catch / CPU-fallback dispatch (`42eac5cc`), DAG topology (`de160fc5`), and GPU drain-for-checkpoint (`571c11c4`) |
| `gpu` | HIP/ROCm host plumbing (`feature = "hip"`): `HipDispatcher` (owns the `HipStreamPool` + per-stage scratch), `map_hip_error`, `awgn` GPU stage |
| `observability` | Tracing-subscriber install + JSON-lines campaign log plumbing |

### Determinism contract (design doc §11)

The byte-identity guarantees the parallel executor and checkpoint/resume rest on are pinned in design-doc §11 (`dev/active/ec530af9-pipeline-design.md`), quoted verbatim:

> ### CPU-only / CPU-parallel contract (unchanged)
>
> The four columns `fer`, `frames`, `errors`, `mean_iters` are
> **byte-identical** across worker counts {1, 2, 4, 8, 24} at fixed
> seed. Resume-from-checkpoint produces byte-identical results vs
> uninterrupted run at fixed seed.
>
> ### CPU-vs-GPU contract (relaxed)
>
> The **three** columns `fer`, `frames`, `errors` are byte-identical
> across CPU-only vs CPU+GPU at fixed seed. **`mean_iters` is
> EXCLUDED** from CPU-vs-GPU byte-identity.
>
> **Rationale**: RDNA2 hardware transcendentals (`v_sin_f32`,
> `v_cos_f32`, hardware `tanh` via `v_exp_f32`) differ from CPU
> `f32::sin`/`f32::cos`/`f32::tanh` polynomial reductions by 1–3
> ULPs in some ranges. For LDPC BP near the convergence threshold,
> ULP differences in LLR messages can change the iteration at which
> the parity-check passes by ±1 — so `mean_iters` is not bit-exact
> across paths. The frame's final verdict (does the codeword decode
> correctly?) is robust to that drift; `fer`/`frames`/`errors`
> remain byte-identical because BP convergence is determined by a
> parity-check at each iteration boundary that has integer (not
> floating-point) state.
>
> ### Always-excluded
>
> `ber` (non-associative f32 horizontal reduction; status-quo
> amendment from `152388f4`).
>
> `wall_seconds` (run-duration-dependent).

The CPU-only/parallel contract is regression-guarded by two complementary slow-tier integration suites: `tests/parallel_determinism.rs` (the direct `frame_sim` dispatch, issue `3fcb7025`) and `tests/determinism.rs` (the typestate preset production path via `Pipeline::dvb_t2()` + heartbeat-resume parity, issue `48a0db6c`); both share the four-column / BER-excluded comparison in `tests/common/mod.rs`. The CPU-vs-GPU relaxed contract (the three columns `fer`/`frames`/`errors` byte-identical, where `errors` is the **frame**-error count per `WorkerCounters`, and `mean_iters` excluded) is regression-guarded by three `#[cfg(feature = "hip")]` slow-tier suites: the two per-kernel suites `tests/gpu_ldpc_byte_identity.rs` (BP hard-decision codeword bit-for-bit, issue `a930be7f`) and `tests/gpu_demap_byte_identity.rs` (max-log demap within the measured ULP-or-absolute tolerance, issue `d3f1616a`), plus the chain-level closer `tests/gpu_byte_identity.rs` (full DVB-T2 BICM frame verdict end-to-end over r1/2 16-QAM, r2/3 64-QAM, r3/4 16-QAM, issue `14f59c2d`). The chain suite shares ONE AWGN noise realisation between paths (GPU AWGN is separately proven by `f6004add`), runs both demap + LDPC BP on the GPU vs CPU (the CPU path across rayon, the GPU demap + BP as single batched launches), and holds BCH outer decode on CPU on both arms (no GPU BCH kernel). It runs each config at a **waterfall** Es/N0 (the steep part of the FER curve) calibrated so the 200-frame sweep is **non-vacuous** — `0 < errored_frames < frames`, asserted — so the verdict boundary §11 is about is genuinely exercised; this is the regime §11 names verbatim ("near the convergence threshold ... the frame's final verdict ... is robust to that drift"). It asserts the three columns `frames`/`errors`(frame errors)/`fer` byte-identical there; one `#[ignore]` test per config keeps each under the 120 s slow-tier cap. All three suites log `mean_iters` and never assert it (the §11 CPU-vs-GPU exclusion); the per-frame **bit**-error count / `ber` is excluded entirely (non-associative f32 reduction, `152388f4`) — it legitimately drifts CPU-vs-GPU on an errored frame, so only the frame verdict is contractual. Each GPU suite is gated on GPU presence (skips when `gf2_kernels_hip::host::device_mem_info().is_err()`) and carries `#[ignore]` per the test-tier rules; the chain-level three-column byte-identity attestation, per-config waterfall Es/N0 + errored-frame mix, and per-config `mean_iters` are recorded in `dev/benchmarks/gf2-sim/gpu-stages-receipts.md`.

### `parallelism-pays` throughput gate (gf2-sim epic `f9717e7e`)

Beyond the standard `cargo-ci` / `code-review` / `tests` gates, the `gf2-sim` epic carries an extra **`parallelism-pays`** quality gate on its perf-bearing tasks (the within-SNR parallel executor `3fcb7025`, the GPU stages, and the hybrid executor). It is an **attested throughput receipt**, not an automatic check: each gated task must record a speedup receipt in `dev/benchmarks/gf2-sim/parallelism-receipts.md` (schema in `dev/active/f9717e7e-project-plan.md` §5) reporting both the single-thread CPU baseline (from `c0b1702d`, ~1.6216 fps) **and** the CPU-24-thread baseline (from `3fcb7025`), and meeting the task's `[hard]` threshold (e.g. ≥12× single-thread for `3fcb7025`; GPU LDPC BP additionally ≥3× the 24-thread CPU baseline). **Throughput must be measured on a verified-quiet machine** (`cat /proc/loadavg` ≈ 0, no `bg3`/foreign `cargo`/`rustc`) — external CPU load understates throughput and invalidates the receipt (and would flake the 5 s-per-test fast tier). The gate is never removed to make a task pass; a failure that does not yield to standard optimisation escalates per `feedback_quality_gates`.

### Key design invariants

1. **Tail masking** — Padding bits beyond `len_bits` in the last `u64` word of a `BitVec` must always be zero. Every mutating operation must call `mask_tail()`. This is the most critical correctness invariant.

2. **Bit numbering** — Bit `i` lives in `word = i >> 6`, `mask = 1u64 << (i & 63)`.

3. **Unsafe isolation** — All `unsafe` code in production crates lives exclusively in the two accelerator kernel crates: `gf2-kernels-simd` (CPU SIMD) and `gf2-kernels-hip` (HIP/ROCm GPU FFI). SIMD is detected at runtime via `OnceLock` in `gf2-core/src/lib.rs`; call path is `simd::maybe_simd()` → optional `LogicalFns`. The HIP crate is opt-in via Cargo feature and excluded from the default workspace build. Standalone `dev/research/<crate>/` prototype stubs (not workspace members) are exempt from this rule when their purpose is to exercise an unsafe surface (e.g., intrinsic feasibility checks); each `pub unsafe fn` in a stub must carry a top-of-function `// SAFETY:` comment.

4. **Functional at API level, imperative allowed in kernels** — High-level code (outside `kernels/`) prefers pure functions, iterator combinators, and immutability. `kernels/` uses mutation and loops for speed.

## Features

| Crate | Feature | Effect |
|-------|---------|--------|
| `gf2-core` | `simd` | Enables AVX2/SIMD kernels via `gf2-kernels-simd` |
| `gf2-core` | `parallel` | Rayon batch operations |
| `gf2-core` | `visualization` | PNG matrix export |
| `gf2-core` | `io` | Serde serialization (default on) |
| `gf2-coding` | `simd` | Propagates to `gf2-core/simd` (default on) |
| `gf2-coding` | `parallel` | Rayon BCH/LDPC batch |
| `gf2-coding` | `llr-f64` | Use f64 instead of f32 for LLRs |
| `gf2-coding` | `sim-observability` | Checkpointing, SIGINT flush, JSON-lines tracing, ChaCha20 RNG seek (default on; embedded users can opt out with `default-features = false`) |
| `gf2-algebra` | `simd` | AVX2 dispatch for `permanent_bipedal3` (default on) |
| `gf2-algebra` | `parallel` | Rayon `permanent_bipedal3_parallel` (default on) |
| `gf2-algebra` | `f5` | `Packed5`, `Packed5Matrix`, `permanent_bipedal5` (default on) |
| `gf2-algebra` | `f7` | `Packed7`, `Packed7Matrix`, `permanent_bipedal7` (default on) |
| `gf2-algebra` | `hip` | HIP/ROCm GPU batch permanents (`gpu` module; requires hipcc) |
| `gf2-sim` | `hip` | HIP/ROCm GPU pipeline stages via `gf2-kernels-hip` (`gpu` module; default off) |
| `gf2-sim` | `llr-f64` | Use f64 instead of f32 for LLRs (default off) |

## Testing conventions

TDD is followed strictly: write the test first, implement minimal code to pass, then add property-based tests for mathematical invariants.

- Unit tests live in `#[cfg(test)] mod tests` within the same file as the implementation.
- Property-based tests use `proptest`; integration tests go in `tests/`.
- Test naming: `test_<operation>_<scenario>` (e.g., `test_shift_left_word_boundary`).
- Always cover word-boundary edge cases: 0, 1, 63, 64, 65 bits.
- All public APIs need doc comment examples — these are tested by `cargo test --doc` and must compile and pass.

## Documentation standards

Every public item must have a doc comment with: description, `# Arguments`, `# Examples` (tested), `# Panics` (if applicable), and `# Complexity` for non-trivial operations.

## Git workflow

**Commit messages** follow conventional commits:
```
type(scope): brief description

Longer explanation if needed.
```

* Valid types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`.
* Reference the jit issue short ID in the scope prefixed with jit: (e.g., `feat(jit:8ce6f8aa): ...`)
* First line under 72 chars.

## Adding a new error-correcting code

1. Implement the relevant traits from `gf2_coding::traits`: `BlockEncoder`, `HardDecisionDecoder`, and/or `SoftDecoder`.
2. Add standard-specific factory constructors (e.g., `MyCode::dvb_t2()`, `MyCode::nr_5g()`).
3. Validate against known test vectors from the relevant standard.
4. Add benchmarks for encoding and decoding throughput in `benches/`.
5. Add an example in `examples/` demonstrating usage.

## MSRV

Rust 1.95 (set in `gf2-core`, `gf2-coding`, `gf2-kernels-hip`, and `dev/research/rns_prototype` `Cargo.toml`). Bumped from 1.80 on 2026-04-27.

## Success-criterion maturity markers

Individual success-criterion bullets in JIT issues may carry an inline marker at the start of the line, teaching the code-review gate which criteria are amendable against empirical data and which are hard contracts. This project defines two markers (see `scripts/code-review-prompt.md` for the exact reviewer semantics):

- `[hard]` — Default. Failure to meet the criterion is a review FAIL; modifying the criterion requires explicit user approval via the escalation path in `.claude/skills/project-lead/references/escalation-policy.md`.
- `[aspirational]` — A target written optimistically before empirical evidence existed. May be amended in-loop if the aggregate contract still holds and `cargo-ci` + `code-review` verify the amended criterion. The amendment must be recorded as a visible note in the issue's description with the observed number and reason (e.g., "crossover threshold updated from k≥16 to k≥4096 based on `dev/benchmarks/run-2026-04-21.csv`").

Criteria without a marker default to `[hard]`. **Correctness requirements are always `[hard]`** regardless of marker — no test-vector equality, field axiom, invariant, or API contract is ever aspirational.

Issue-extraction agents should use `[aspirational]` sparingly, only for targets that are explicitly provisional (expected throughput, speedup factors, crossover thresholds unsupported by prior measurement). When in doubt, use `[hard]`.

This is a project-local convention — JIT itself does not read or enforce the markers; enforcement is entirely in the reviewer prompt at `scripts/code-review-prompt.md`. Do not put the marker definitions in `.jit/config.toml`; that file is for JIT's own schema, not for project conventions consumed by prompt-layer agents.

## Breakdown-time feasibility check

When an issue description mentions specific CPU intrinsics, SIMD lanes, unstable library features, or toolchain-version-dependent behaviour, verify MSRV compatibility **before** accepting the breakdown. Run:

```bash
rustup run 1.95.0 cargo check --workspace --all-features
```

against a minimal stub that uses the intended intrinsic. If the intrinsic is unstable on MSRV 1.95 (or only stabilised in a newer rustc), the implementation must either: (a) restrict to stable intrinsics on the current MSRV, (b) compile-gate behind `#[cfg(all(target_arch = ..., target_feature = ...))]` with a scalar fallback on the default build, or (c) escalate to the user for MSRV bump approval before dispatch.

Previous incident: `afac2262` (AVX-512 ZMM lane) cost a rework cycle and a scope reduction because the intrinsic-feasibility check was not run during breakdown; the ZMM lane was requested on a host that has no AVX-512 hardware AND on an MSRV (then 1.80) that did not stabilise the required intrinsics. MSRV was bumped to 1.95 on 2026-04-27 so those particular intrinsics are now stable; the procedural lesson stands.

## Verification work

Any issue whose core deliverable is a formal proof (Lean4, Coq) or a model-checking harness (Kani, CBMC) is classified as **verification work** and has stricter dispatch rules than implementation work. These rules exist because verification failures look different — a worker cannot know in advance how hard a proof is or whether their approach will be accepted, so each attempt without a pre-approved design is an all-or-nothing shot.

**Before implementation is dispatched on a verification issue, a proof-sketch artefact must exist and be approved.** The proof sketch is a short markdown document (stored alongside the issue's design docs) listing:

1. **Lemmas to be proved**, in statement form only (not with full proofs). One bullet per lemma.
2. **Intended proof strategy per lemma** — the tactic or proof shape, in one line each. Examples:
   - "by induction on the loop iteration count, using `Nat.rec`"
   - "by `scalar_tac` from the bounds in `ValidPrime`"
   - "by `bv_omega` after unfolding `UScalar.val`"
   - "by unwinding the Newton iteration invariant `P * inv ≡ 1 (mod 2^(2^k))`"
3. **Exact production code path** each verification harness must exercise. For Lean4 via Charon/Aeneas, state the module path and the function name that the generated Lean definition will be proved against. For Kani, state the exact production entrypoint signature the harness must call — not a test-copied helper, not a semantically equivalent reimplementation.
4. **For Kani specifically:** the expected unwind bounds and whether the production path uses `OnceLock`-dispatched runtime tables. `OnceLock` paths typically require non-standard unwind strategies and must be flagged in the sketch.

The lead (or the user, if the work has significant architectural impact) reviews and **approves the sketch before any proof code is written**. The implementation issue is then dispatched as "implement this approved proof sketch" — a much more tightly scoped task than "prove X."

Previous incidents:
- `467d835e` needed 10 review cycles because the proof approach (axiom vs derived, placeholder vs full) was re-negotiated each cycle. A pre-approved sketch would have cut this to 2–3 cycles.
- `8889e712` needed 9 cycles — 8 of them all citing the same finding (Kani harness attached to a test-copied table helper instead of the production `OnceLock`-dispatched path). A sketch that named the production code path explicitly would have caught this on attempt 1.

Verification issues that do not have an approved sketch at dispatch time are a process bug. If you find yourself about to dispatch one, stop — create the sketch task first, wire the implementation as a dependent, and return to wave planning.
