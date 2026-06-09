# Phase B — HIP/ROCm GPU stages: receipts

Phase B of the `gf2-sim` CPU+GPU FEC simulation pipeline epic (`f9717e7e`).
Design contract SSOT: `dev/active/ec530af9-pipeline-design.md` (§5 crate
boundaries, §6 multi-arch HIP, §8 host dispatcher / error mapping, §11
determinism contract).

The per-kernel throughput receipts (the `parallelism-pays` gate evidence for
each GPU stage) live in the shared parallelism receipts file; this file records
the **Phase B closer**: the end-to-end CPU-vs-GPU byte-identity attestation that
ties the per-kernel stages into the full DVB-T2 BICM chain verdict.

## Per-kernel GPU-stage receipts (links)

Each GPU stage's throughput receipt (speedup vs the `c0b1702d` single-thread and
`3fcb7025` 24-thread CPU baselines) is recorded in
[`./parallelism-receipts.md`](./parallelism-receipts.md):

| Stage | Issue | Speedup (headline) | Receipt section |
|-------|-------|--------------------|-----------------|
| GPU ChaCha20 + Box-Muller AWGN | `f6004add` | 14.91× GPU / CPU-1T (AWGN-step) | [`#f6004add`](./parallelism-receipts.md#f6004add--gpu-chacha20--box-muller-awgn-kernel) |
| GPU LDPC belief-propagation batch decode | `a930be7f` | 253.51× GPU / CPU-1T; 28.98× GPU / CPU-24T | [`#a930be7f`](./parallelism-receipts.md#a930be7f--gpu-ldpc-belief-propagation-batch-decode-kernel) |
| GPU Gray-QAM max-log soft-demap | `d3f1616a` | 16-QAM 12.59×, 64-QAM 16.09× GPU / CPU-1T | [`#d3f1616a`](./parallelism-receipts.md#d3f1616a--gpu-gray-qam-max-log-soft-demap-stage) |

Per-kernel correctness (byte-identity at the kernel boundary) is regression-
guarded by the two `#[cfg(feature = "hip")]` suites
`crates/gf2-sim/tests/gpu_ldpc_byte_identity.rs` (`a930be7f`, BP hard-decision
codeword bit-for-bit) and `crates/gf2-sim/tests/gpu_demap_byte_identity.rs`
(`d3f1616a`, max-log demap within the measured ULP-or-absolute tolerance). The
GPU AWGN noise is bit-identical to the CPU noise (`f6004add`), which is why the
chain harness below can share ONE noise realisation between paths.

## 14f59c2d — CPU-vs-GPU byte-identity, full DVB-T2 BICM chain (Phase B close)

- **Status:** ATTESTED. The three-column §11 CPU-vs-GPU relaxed contract
  (`fer`/`frames`/`errors` byte-identical; `mean_iters` excluded; `ber`
  excluded) holds **end-to-end** across the full DVB-T2 BICM chain for all three
  named (rate, modulation) configurations, 200 frames each, on gfx1030.
- **Date:** 2026-06-09.
- **Hardware:** CPU = AMD Ryzen 9 5900X (12C/24T), GPU = AMD Radeon RX 6950 XT
  (gfx1030, RDNA2). Single GPU — the suite assumes no concurrent GPU work.
- **Test:** `crates/gf2-sim/tests/gpu_byte_identity.rs`, THREE per-config
  functions `gpu_chain_verdict_byte_identical_{r12_16qam,r23_64qam,r34_16qam}`
  (each `#[ignore = "sim: ..."]`, each gated on
  `gf2_kernels_hip::host::device_mem_info().is_ok()`). Split one-per-config so
  each stays under the 120 s slow-tier cap (`.config/nextest.toml`
  `[profile.slow] slow-timeout = 120s`).
- **Run command:**
  ```bash
  cargo test -p gf2-sim --features hip --release \
      --test gpu_byte_identity -- --ignored --nocapture --test-threads=1
  ```
  (`--test-threads=1`: single gfx1030 — the three GPU tests must not run
  concurrently.)
- **The three §11 columns are FRAME-level (per `WorkerCounters`):** the
  determinism SSOT `gf2_sim::parallel::WorkerCounters` defines
  `errors += u64::from(errored)` — one count per **errored frame**. The three
  byte-identical columns are `frames`, `errors` (= errored-frame count), and
  `fer` (= errors / frames). The per-frame **bit**-error count (the BER
  numerator) is NOT one of the three: `ber` is **excluded** from byte-identity
  (non-associative f32 reduction; `152388f4`; design §11 "Always-excluded:
  ber"). This suite asserts the FRAME-error columns and only LOGS the bit-error
  sum (like `mean_iters`).
- **What is compared (the §11 CPU-vs-GPU relaxed contract):** a CPU-only path and
  a CPU+GPU path are hand-composed (the Phase C hybrid executor
  `75c22fa8`/`de160fc5` does not exist yet), differing **only** in which device
  runs the GPU-eligible stages:
  - **Shared per frame** (computed once, fed identically to both): random k_bch
    BBFRAME → BCH+LDPC encode → bit-interleave → Gray-QAM map → **ONE** AWGN
    noise realisation → noisy rx I/Q symbols. GPU AWGN is NOT re-exercised here
    (its bit-identity to CPU noise is proven by `f6004add`); sharing one noise
    realisation isolates the comparison to the demap→decode verdict.
  - **CPU-only path** (run across the rayon pool, per-frame outcome
    thread-independent): CPU `FastGrayQamDemapper` max-log → deinterleave → CPU
    LDPC BP + BCH via `DvbT2Concat::decode_soft_counted`.
  - **CPU+GPU path** (single batched GPU launches): GPU `GpuGrayQamDemapper`
    max-log over the whole sweep → deinterleave → ONE batched `GpuLdpcBp`
    `decode_batch` → extract k_ldpc systematic bits → CPU `BchDecoder` outer
    decode across rayon (the same `BchCode::dvb_t2` SSOT `DvbT2Concat::new`
    builds; BCH has no GPU kernel, so it stays on CPU on both arms).
  - **MAX-LOG on both sides:** `GpuGrayQamDemapper` is max-log only, so BOTH
    paths use `DemapMethod::MaxLog` (apples-to-apples; never GPU max-log vs CPU
    exact-log-MAP).
- **Decoder config:** `NormalizedMinSum(0.75)` with early termination, BP cap 50
  (the DVB-T2 default).
- **Operating point — the §11 waterfall regime (non-vacuous):** each Es/N0 sits
  in the **waterfall** (steep part of the FER curve), calibrated empirically at
  the per-config seed so the 200-frame sweep yields `0 < errored_frames < frames`
  — a non-trivial mix of clean decodes and errored frames. The test **asserts**
  non-vacuity (`errored_frames > 0 && < frames`). This is the regime §11 names
  verbatim — "For LDPC BP **near** the convergence threshold ... the frame's
  final verdict ... is robust to that drift; `fer`/`frames`/`errors` remain
  byte-identical." It is at this verdict boundary (not above threshold, where
  every frame trivially decodes and the asserts would be 0 == 0) that
  GPU-demap+GPU-BP drift could flip a borderline frame's verdict; the §11 claim
  is that it does NOT. The waterfall is sharp for NMS(0.75) max-log: e.g. r1/2
  16-QAM goes 6.2 dB → 200/200 errored, 6.4 → 105/200, 6.6 → 3/200.

### Result — three FRAME columns byte-identical (CPU == GPU), non-vacuous

| Config | Es/N0 (dB) | frames | errored_frames (= `errors`) | fer | CPU == GPU |
|--------|-----------:|-------:|----------------------------:|----:|:----------:|
| r1/2 16-QAM | 6.4 | 200 | 105 | 0.525000 | yes |
| r2/3 64-QAM | 14.3 | 200 | 33 | 0.165000 | yes |
| r3/4 16-QAM | 10.2 | 200 | 70 | 0.350000 | yes |

All three configs: `frames`, `errors` (errored-frame count), `fer` byte-identical
between the CPU-only and CPU+GPU paths at the fixed per-config seed. The sweeps
are non-vacuous (105 / 33 / 70 errored frames of 200), so the verdict boundary is
genuinely exercised — not one of those frames flipped verdict between CPU and
GPU. Per-test wall time ≈ 31–40 s (r1/2 measured 30.76 s standalone; all three
together 104 s), each well under the 120 s slow-tier cap.

### mean_iters (BOTH paths + diff) + bit-error sum (LOGGED, NOT asserted)

`mean_iters` is excluded from CPU-vs-GPU byte-identity (§11: RDNA2 transcendental
ULP drift can shift the BP early-termination iteration by ±1 without changing the
integer-state parity-check verdict). It is logged for **both** paths and the
diff: the GPU per-frame iteration counts come from
`GpuLdpcBp::decode_batch_with_iters` (the additive observability API added by
`14f59c2d`; the existing `decode_batch` delegates to it and is byte-for-byte
unchanged). The count convention is **aligned to the CPU `decode_to_codeword`**
(the 1-indexed pass at which the syndrome first passes, or `max_iterations` if it
never converges), so the CPU-vs-GPU diff below is the genuine near-threshold
drift §11 describes. The bit-error sum (the BER numerator) is also logged only
(`ber` excluded):

| Config | CPU mean_iters | GPU mean_iters | diff | bit-error sum CPU | bit-error sum GPU |
|--------|---------------:|---------------:|-----:|------------------:|------------------:|
| r1/2 16-QAM | 50.0000 | 50.0000 | +0.0000 | 2085 | 2085 |
| r2/3 64-QAM | 49.4750 | 49.4750 | +0.0000 | 1163 | 1163 |
| r3/4 16-QAM | 46.4050 | 46.4050 | +0.0000 | 11600 | 11600 |

(CPU `mean_iters` is high here because at the waterfall most errored frames run
to the 50-iteration cap — exactly the near-threshold regime §11 describes. The
CPU-vs-GPU `mean_iters` diff is `+0.0000` at these seeds — the convergence passes
align frame-for-frame, validating the CPU-aligned count convention; §11 permits a
non-zero ±1-scale diff here, which is logged not asserted. The bit-error sums
likewise matched CPU == GPU at these seeds, a bonus; they are NOT asserted and
may differ on an errored frame without violating the contract.)

### Escalation contract (how a real §11 violation surfaces)

The §11 contract is on the FRAME verdict. The test compares each frame's
`errored` flag CPU-vs-GPU; if a frame errors on one path but not the other (the
`errors`/`fer` columns would diverge), the test **PANICS** with the exact
(config, frame index, CPU vs GPU verdict and bit-error counts) and does NOT relax
the criterion or move the operating point. That escalation is a real result to
report to the lead — a §11-scope user decision, not a test edit. On this run no
config diverged: all three frame-verdict columns are byte-identical at the
waterfall.

(History: an earlier draft incorrectly asserted the **bit**-error sum and was
forced above threshold to make it zero — a vacuous regime. That was corrected to
assert the FRAME columns at the waterfall, per the lead review.)
