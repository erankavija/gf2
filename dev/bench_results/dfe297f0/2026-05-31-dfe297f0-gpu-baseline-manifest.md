# GPU prototype baseline manifest (dfe297f0)

> **Issue:** `jit:dfe297f0` (Pin GPU prototype baselines) — story `gpu-evidence-infra`, epic `806eb14e` (HIP/ROCm GPU prototype wave).
> **Spec:** `dev/plans/hip_gpu_prototype_wave.md` (§ Evidence protocol, § S0).
> **Purpose:** Freeze the CPU comparators, design workloads, and existing-HIP submission behavior **before** S1/S2/S3 implementation issues claim speedups. Downstream speedup claims (LDPC ≥3×, BCH ≥5×, scheduler throughput) are measured against the comparators and workloads pinned here.

This manifest is the authoritative baseline reference for the GPU prototype wave. It is a *pinning* artifact, not a measurement harness — building the reusable batch-sweep / phase-timing harness is the separate task `698fc999` (Add GPU benchmark evidence harness), and syndrome-isolated + CPU-NMS refinements are flagged below as downstream work for that task and for S2/S3.

## 1. Environment metadata

| Field | Value |
|---|---|
| Measurement commit | `7344a879` (working tree clean of any gf2-coding / gf2-kernels-hip edits by this task) |
| Date | 2026-05-31 |
| CPU | AMD Ryzen 9 5900X (12c / 24t, Zen 3, AVX2+FMA, no AVX-512) |
| `nproc` | 24 |
| GPU | AMD Radeon RX 6950 XT (RDNA2, `gfx1030`) |
| ROCm | 7.2 |
| `hipcc` | `/opt/rocm/bin/hipcc`, HIP version 7.2.53211-9999 (AMD clang 22.0.0git) |
| Offload arch | `gfx1030` (hardcoded `crates/gf2-kernels-hip/build.rs:32`) |
| rustc | 1.95.0 (59807616e 2026-04-14) — MSRV |
| CPU comparator parallelism | rayon, enabled via `--features parallel` (24 threads) |

CPU criterion runs used `--sample-size 10 --warm-up-time 1 --measurement-time 3` to pin a stable median quickly; downstream harness (`698fc999`) may re-measure with fuller statistics. Reported numbers are criterion's `[lower median upper]`.

## 2. LDPC BP — CPU comparator (S2 baseline)

- **Best production CPU comparator:** `LdpcDecoder::decode_batch` → `LdpcDecoder::decode_batch_with_config` (`crates/gf2-coding/src/ldpc/core.rs:1027` → `:1061`).
- **Rayon:** YES — `decode_batch_with_config` dispatches `(0..n).into_par_iter()` under `#[cfg(feature = "parallel")]` (`core.rs:1069`). `parallel` is opt-in (not a default feature); the best production batch path uses it, so the baseline is measured with it enabled.
- **Algorithm at baseline:** `DecoderConfig::default()` = MinSum. (S2's GPU kernel targets **Normalized** Min-Sum; per-iteration cost is essentially identical, but S2 must re-measure the CPU comparator with `DecoderAlgorithm::NormalizedMinSum` at the same shape for an apples-to-apples ≥3× check — see § 6.)
- **Design workload:** DVB-T2 **Normal** frame, `CodeRate::Rate3_5` (n = 64800, k = 38880), all-LLR = 10.0, `max_iterations = 50`, batch sweep {10, 50, 100, 202}. **Selected design point: batch = 202.**
- **Reproduce:** `cargo bench -p gf2-coding --features parallel --bench ldpc_throughput -- ldpc_decode_batch`

| Batch | Wall (median) | Per-frame | Throughput |
|---|---|---|---|
| 10 | 49.20 ms | 4.92 ms | 0.96 MiB/s |
| 50 | 245.52 ms | 4.91 ms | 0.97 MiB/s |
| 100 | 461.03 ms | 4.61 ms | 1.01 MiB/s |
| **202** | **879.84 ms** | **4.36 ms** | **1.06 MiB/s** |

**Observation (baseline behavior, not a defect to fix here):** wall scales near-linearly with batch size despite 24 rayon threads. The per-frame `code.clone()` inside the parallel map (`core.rs:1071`) plus memory-bandwidth contention on the n=64800 working set limit parallel scaling. This is the production behavior the GPU prototype competes against; S2 should compare against this real path, not an idealized one.

## 3. BCH syndrome — CPU comparator (S3 baseline)

- **Syndrome-eval comparator:** `BchDecoder::compute_syndromes` (`crates/gf2-coding/src/bch/core.rs:568`), vectorized Horner via `r_poly.eval_batch(&eval_points)` (`:623`).
- **Best production batch entry point (full hard-decision decode):** `BchDecoder::decode_batch` (`crates/gf2-coding/src/bch/core.rs:548`).
- **Rayon:** YES — `decode_batch` uses `received.par_iter().map(|cw| self.decode(cw))` under `#[cfg(feature = "parallel")]` (`core.rs:549`).
- **Design workload:** DVB-T2 **Short** frame, `CodeRate::Rate1_2`, batch sweep {1, 10, 50, 100}. **Selected design point: batch = 100.**
- **Reproduce:** `cargo bench -p gf2-coding --features parallel --bench bch_parallel -- bch_batch_decode`

| Batch | Wall (median, full decode) | Per-codeword | Throughput |
|---|---|---|---|
| 10 | 161.36 ms | 16.1 ms | 62.0 elem/s |
| 50 | 910.95 ms | 18.2 ms | 54.9 elem/s |
| **100** | **1.8489 s** | **18.5 ms** | **54.1 elem/s** |

**Scope boundary — syndrome-isolated number is downstream:** this bench times the *full* decode (syndrome + Berlekamp-Massey + Chien). The GPU prototype (S3) targets **syndrome evaluation only** (≥5×), so the apples-to-apples CPU comparator is `compute_syndromes` in isolation. No syndrome-only batch bench exists today; adding it is the evidence-harness task `698fc999`. The full-decode numbers above are pinned as an **upper bound** and the comparator function is identified; S3 must measure `compute_syndromes` at batch = 100 against the GPU Horner kernel.

## 4. Existing BCJR HIP baseline — submission behavior (S1 scheduler baseline)

- **Prototype:** `GpuBcjrBatch::decode_batch` (`crates/gf2-kernels-hip/src/lib.rs:307`).
- **Submission model — direct / default-stream, blocking per batch call:**
  - H→D memcpy (`lib.rs:345`)
  - kernel launch on the **default stream** — `ffi::launch_bcjr_batch(..., ptr::null_mut())`, null = `hipStream_t(0)` (`lib.rs:353-362`)
  - full-device sync — `ffi::hip_device_synchronize()` (`lib.rs:370`)
  - D→H memcpy (`lib.rs:380`)
  - No per-thread streams, no events, no persistent graph. The Gray-QAM demapper (`GpuGrayQamDemapper::demap_batch`, `lib.rs:629`) uses the same default-stream + `hip_device_synchronize` model (launch `:727`, sync `:749`).
- **This is the S1 contention baseline:** S1 (`d77519a3`) must quantify this direct/default-stream + full-device-sync submission against a shared scheduler (bounded queue, batch coalescing, stream/event completion).
- **Environment validated live:** `cargo test --manifest-path crates/gf2-kernels-hip/Cargo.toml --release --test gpu_cpu_crosscheck` → **7 passed, 0 failed in 3.66s** on the RX 6950 XT, including `test_gpu_batch64_matches_serial_cpu` and `prop_gpu_batch_matches_serial_cpu`. The HIP toolchain builds, the GPU runs, and the BCJR prototype produces CPU-equivalent results — the baseline GPU environment is confirmed operational.
- **Throughput-number boundary:** there is no batch-BCJR GPU-vs-CPU throughput bench or phase-timing breakdown yet (only a Gray-QAM crossover bench at `crates/gf2-kernels-hip/benches/gpu_vs_cpu_gray_qam.rs` and the correctness crosscheck above). Building the BCJR throughput sweep + H→D / kernel / D→H phase breakdown is the harness task `698fc999`; S1 consumes it.

## 5. Baseline manifest paths for downstream tasks

| Downstream | Consumes from this manifest |
|---|---|
| S2 LDPC BP (`37e0b235`) | § 2 — comparator `decode_batch_with_config` (rayon), DVB-T2 Normal Rate3_5 @ batch 202, 50 iters, **879.84 ms**; re-measure CPU with NormalizedMinSum (§ 6). |
| S3 BCH syndrome (`9012f8a0`) | § 3 — comparator `compute_syndromes`, DVB-T2 Short Rate1/2 @ batch 100; full-decode upper bound **1.849 s**; measure syndrome-isolated CPU time. |
| S1 scheduler (`d77519a3`) | § 4 — direct/default-stream + `hip_device_synchronize` submission model (`lib.rs:353/370`) as the contention baseline. |
| Harness (`698fc999`) | This manifest's metadata schema, reproduce commands, and the gaps in §§ 3–4 (syndrome-isolated bench; BCJR throughput + phase timing). |
| Speed-threshold checks (`100eda77`) | Pinned design points: LDPC ≥3× of 879.84 ms @ batch 202; BCH syndrome ≥5× of the syndrome-isolated CPU number @ batch 100; scheduler throughput improvement vs § 4. |

**This manifest path:** `dev/bench_results/2026-05-31-dfe297f0-gpu-baseline-manifest.md` (attached to `dfe297f0` via `jit doc add`).

## 6. Known refinements explicitly deferred (in scope of downstream tasks, not this pinning task)

- CPU **NormalizedMinSum** LDPC comparator at DVB-T2 Normal Rate3_5 batch 202 (for S2's apples-to-apples ≥3×). MinSum is pinned here; per-iteration cost is near-identical.
- **Syndrome-isolated** CPU baseline (`compute_syndromes` only) at DVB-T2 Short Rate1/2 batch 100 (for S3's ≥5×). Full decode is pinned here as an upper bound.
- BCJR GPU throughput sweep + H→D / kernel / D→H **phase-timing breakdown** (harness `698fc999`; S1 scheduler comparison).

## 7. Success-criteria mapping (dfe297f0)

- [hard] Baseline artifacts record commit, CPU/GPU model, ROCm, hipcc path/version, thread count, benchmark commands → **§ 1** (+ reproduce commands in §§ 2–4).
- [hard] LDPC BP baseline identifies best production CPU comparator incl. rayon → **§ 2** (`decode_batch_with_config`, rayon under `parallel`, measured at the design workload).
- [hard] BCH syndrome baseline identifies best production CPU comparator incl. rayon → **§ 3** (`compute_syndromes` / `decode_batch`, rayon under `parallel`; syndrome-isolated refinement flagged downstream).
- [hard] Existing BCJR HIP baseline captures direct/default-stream submission behavior → **§ 4** (submission model with file:line; env validated live on gfx1030).
- [hard] Baseline manifest paths documented for downstream LDPC, BCH, scheduler tasks → **§ 5**.
