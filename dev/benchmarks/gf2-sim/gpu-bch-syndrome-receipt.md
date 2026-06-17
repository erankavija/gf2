# GPU batch BCH syndrome evaluation — performance receipt

JIT issue: `9012f8a0` (HIP/ROCm GPU-accelerated batch BCH syndrome evaluation)
Design doc: `dev/active/gpu-batch-bch-syndrome-plan.md` (§11)
Date: 2026-06-17. Attested by: agent:claude on the gfx1030 host.

All figures below come from an actual run on the hardware described; none are
estimated. The `[hard]` 5x gate is decode-sub-step vs decode-sub-step (GPU
syndrome eval vs CPU `compute_syndromes` measured in isolation), per the
`a930be7f` precedent.

## Hardware / software metadata

| Item | Value |
|------|-------|
| GPU | AMD Radeon RX 6950 XT (gfx1030, RDNA2) |
| CPU | AMD Ryzen 9 5900X (12C/24T) |
| rayon threads | 24 |
| ROCm | 7.2.4 (`/opt/rocm/.info/version`) |
| Kernel | Linux 7.0.10-arch1-1 |
| Build | `--release`, hipcc `--offload-arch=gfx1030 -O3` |

## Design workload

DVB-T2 Normal Rate 1/2 BCH (read from `DvbBchParams::for_code`):
`n = 32400`, `k = 32208`, `t = 12`, `2t = 24` syndromes/frame, field GF(2^16).
Frame population is a fixed-seed mix of valid codewords, `<= t` correctable, and
`> t` uncorrectable errors.

## Exact commands

```bash
# Correctness ladder rungs 1-3 (field-level, gf2-kernels-hip):
cargo test --manifest-path crates/gf2-kernels-hip/Cargo.toml --features hip \
    --release --test gpu_bch_syndrome_field -- --ignored

# Correctness ladder rungs 4-5 (chain, gf2-sim):
cargo test -p gf2-sim --features hip --release \
    --test gpu_bch_syndrome_byte_identity -- --ignored

# Throughput + sweep + phase split:
cargo run -p gf2-sim --release --features hip \
    --bin gpu_bch_syndrome_throughput -- \
    --frames 1024 --repeats 5 --sweep 64,256,1024,4096
```

Raw bench stdout captured at `/tmp/bch_tp.out` during attestation; reproduced
verbatim below.

## Correctness ladder (all PASS, zero tolerance)

| Rung | What | Result |
|------|------|--------|
| 1 | Exhaustive GF(2^4) device `gf_mul` vs CPU `Gf2mField` (all 256 pairs) | PASS |
| 2 | Uploaded `exp`/`log` table equality, GF(2^14) + GF(2^16) | PASS |
| 3 | Small BCH(15)/GF(2^4) Horner fixture: GPU == hand value == CPU | PASS |
| 4 | DVB-T2 Short (GF2^14) + Normal (GF2^16), 200 frames mixed errors, all 2t u16 syndromes byte-identical to CPU | PASS |
| 5 | GPU syndromes → CPU Berlekamp-Massey + Chien decode == CPU-only decode | PASS |

GF arithmetic is exact integer (uploaded CPU tables), so byte-identity holds
with **zero tolerance** — no ULP drift (unlike the LDPC f32 path).

## Throughput (operating point: 1024 frames, 5 repeats, best-of)

```
GPU  syndrome fps :         7267.9
CPU  1T  fps      :           82.7   (measured on 64 frames, context only)
CPU 24T  fps      :           11.6
speedup vs 1T     :          87.88x
speedup vs 24T    :         625.30x   <-- [hard] gate (>= 5x)
GATE (>= 5x vs 24T): PASS
```

The GPU full call is `compute_syndromes_batch_gpu` measured end-to-end. That
function does NOT amortise device setup across calls: on EVERY invocation it
rebuilds `BchFieldTables` from the CPU exp/log tables, allocates a fresh
`GpuBchSyndrome` evaluator, uploads the exp/log tables AND the `α^1..α^(2t)`
points to the device, repacks every frame's coefficient stream, runs the H2D +
Horner kernel + D2H, rehydrates the u16 syndromes into `Gf2mElement`s, and drops
the evaluator. The throughput bin times that entire function, so the 7267.9 fps
/ 87.9x figure is **conservative and inclusive** — it pays per-call table/point
upload and allocation, not the amortised-setup best case. CPU numbers are
`compute_syndromes` in isolation (no BM/Chien). CPU-1T is measured on a 64-frame
subset (single-thread n=32400 GF(2^16) syndrome eval is ~12 ms/frame; fps is
rate-invariant).

### `[hard]` 5x gate — MET, by a very large margin

GPU syndrome throughput is **625x** the rayon-24T CPU `compute_syndromes` and
**87.9x** the single-thread path. The gate (`>= 5x` vs the best production CPU
path) passes against *either* CPU baseline.

### Note on the CPU-24T anomaly (24T slower than 1T)

The rayon-24T number (11.6 fps) is **slower** than single-thread (82.7 fps).
This is a contention artefact in the CPU `compute_syndromes`, not a GPU
advantage inflation: each call clones `Arc<FieldParams>` O(2t·n) times (the
`Gf2mElement` `Mul`/`Add` operators `Arc::clone` the shared field params on every
operation), so 24 threads hammering the same atomic refcount cache-line
ping-pong far outweighs the parallelism. The honest "best production CPU path"
here is therefore single-thread at 82.7 fps; the GPU still clears the 5x gate by
**87.9x** against it. (Optimising the CPU `compute_syndromes` Arc traffic is out
of scope for this GPU-offload issue; recorded here so the divisor choice is
transparent.)

## Batch-size sweep (GPU full call vs CPU-24T compute_syndromes)

```
   batch         GPU fps      CPU24T fps       speedup
      64          4123.6            11.7       353.34x
     256          6810.0            11.6       588.15x
    1024          7186.6            11.6       619.63x
    4096          6835.8            11.6       590.10x
```

GPU throughput rises with batch (more frames hide the per-(frame,point) Horner
latency) and plateaus around batch 1024 on this device. The CPU-24T divisor is
flat (the Arc contention above dominates regardless of batch).

## Coarse phase split (1024 frames)

```
host coeff repack       :    114.450 ms  (81.2% of full call)
device setup + xfer + kernel :     26.444 ms  (18.8% of full call)
```

The second line is the whole non-repack remainder of the call, NOT device
transfer alone: it includes the per-call `BchFieldTables` rebuild, the
`GpuBchSyndrome` allocation, the one-shot exp/log + points upload, the input
H2D, the Horner kernel, the output D2H, and the `Gf2mElement` rehydrate. (A
finer device-internal H2D / kernel / D2H split would need wrapper-level event
instrumentation; this coarse two-way split is the "where practical" §11 ask.)

The GPU call is currently dominated by the **host-side coefficient repack** (per
frame: parity-reversed ++ message-reversed bit packing into ceil(n/64) u64
words), not by the device. Everything device-touching (setup + transfers +
kernel) is only ~18.8% of the wall time. Even with the repack AND the per-call
setup included, the full GPU call clears the gate by 87.9x; a future
optimisation could parallelise or SIMD the repack to widen the margin further
(the repack is pure CPU bit-twiddling and embarrassingly parallel).

## Per-call device setup is included, not amortised

`compute_syndromes_batch_gpu` rebuilds the `BchFieldTables`, allocates a fresh
`GpuBchSyndrome`, and uploads the GF(2^m) `exp`/`log` tables (256 KB total for
m=16) plus the `α^1..α^(2t)` evaluation points ON EVERY CALL — the evaluator is
constructed and dropped inside the function rather than held across batches. The
throughput figures above therefore **include** that per-call upload and
allocation in the timed region; they are not the amortised-setup best case. A
future API change could hoist the evaluator out of the call to amortise the
table/point upload across batches, but those numbers would have to be
re-measured on the GPU before being claimed — the figures here reflect the
current inclusive code path.
