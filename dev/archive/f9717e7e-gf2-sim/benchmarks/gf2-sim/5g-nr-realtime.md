# 5G NR LDPC GPU real-time decode-rate receipt (issue `23d3525f`)

Tuning sweep + measured decode-rate ceiling for the **flat GPU LDPC BP kernel**
(reused unchanged from Phase B `a930be7f`) parameterised for 5G NR by the
host-side `GpuNr5gDecoder`
(`crates/gf2-sim/src/gpu/nr_5g_ldpc.rs`). Design SSOT:
`dev/active/ec530af9-pipeline-design.md` (§6 shared kernel binary).

> **VERDICT: BELOW TARGET — escalation required (gate NOT weakened).**
> The measured decode-rate ceiling at the BLER ≤ 1e-2 operating point is
> **17.45 ± 0.03 Mbps**, ~11.5× below the **≥ 200 Mbps** concrete target. This
> receipt records the full sweep, the operating point, and the analysis of why
> the target is unreachable with the existing kernel **unchanged**, per the
> task's STOP instruction ("report your measured ceiling … the lead escalates;
> the gate is never weakened and the receipt is never fudged").

> **OUTCOME (2026-06-12b, user-approved option B):** the criterion was
> amended to the attested measurement above. Study `43fb19e2`
> (`dev/active/43fb19e2-nr-kernel-feasibility.md`) showed the kernel
> is bandwidth-bound at 44% of peak VRAM; the full optimisation stack
> (layered BP 1.756× measured, fp16 ≈1.95×, QC layout 1.16×,
> reduced-graph 1.07×) projects 50–83 Mbps — kernel work deferred to a
> future epic. The ≥ 200 Mbps references below are the receipt's
> historical record of the original target.

## Hardware / software

| Item | Value |
|------|-------|
| GPU | AMD Radeon RX 6950 XT (`gfx1030`, RDNA2) |
| GPU cores | 5120 stream processors |
| CPU host | AMD Ryzen 9 5900X (12-core / 24-thread) |
| GPU driver | `amdgpu` (in-tree kernel driver, Linux 7.0.10-arch1-1) |
| ROCm | 7.2.4 |
| HIP | 7.2.53211-9999 (AMD clang 22.0.0git) |
| Kernel block size | `BLOCK_THREADS = 256`, grid = ⌈work / 256⌉ (fixed, design §6) |
| Host load during measurement | `/proc/loadavg` 0.63 (no foreign `cargo`/`rustc`/`bg3`; only an idle `jit-server` web process) |

The **lead re-measures on a verified-quiet host before attesting**
`parallelism-pays`; the numbers below are reproducible from one documented
command line (next section).

## Configuration (the headline / concrete target)

| Parameter | Value |
|-----------|-------|
| Base graph | BG1 (46×68, K_b = 22) |
| Lifting set | `i_LS` = 1 (`lifting_set_index(384) == 1`; 384 = 3·2⁷, the a = 3 set) |
| Lifting size Z | 384 |
| Code rate | 1/2 |
| Modulation | QPSK (Q_m = 2) |
| Message length k (transport block) | 8448 bits (= 22 · 384) |
| Transmitted codeword n (E) | 16896 bits (= 2k) |
| Mother code decoded by BP | full_n = 26112 cols (= 68·384), m = 17664 rows (= 46·384) |
| Decoder algorithm | NormalizedMinSum(α = 0.75) |
| Early termination | syndrome-check per iteration, on |
| Layers | 1 |
| Channel | per-bit BPSK-AWGN (decoder-throughput only; RF impairments out of scope) |

**Why BP runs on the larger mother code.** 5G NR rate matching (TS 38.212
§5.3.2) is realised by LLR initialisation on the FULL mother code (punctured =
0, filler = strong prior), not by removing columns from H. So the BP graph the
GPU decodes has **full_n = 26112** variable nodes and **17664** check rows —
≈1.55× the transmitted length and substantially larger than the n = 16896
transport surface. The decode cost is set by the mother-graph size, not the
transmitted n.

## Reproduction command

```bash
# From the repo root, on a gfx1030 host with ROCm. Defaults reproduce the
# canonical sweep (Es/N0 = -1.4 dB, 3000 BLER blocks/cell, 5 throughput reps).
cargo bench -p gf2-sim --features hip --bench nr_5g_realtime
```

Optional env overrides (all default to the headline sweep): `NR5G_ESN0_DB`
(per-bit Es/N0, default `-1.4`), `NR5G_BLER_BLOCKS` (default `3000`),
`NR5G_THRPUT_REPS` (default `5`), `NR5G_ALGO` (`nms`|`minsum`), `NR5G_EARLY`
(`1`|`0`). Without `--features hip` or a usable GPU the bench prints a skip line
and exits 0.

## Channel / operating-point calibration

Per-bit BPSK-AWGN: `sigma = 1/sqrt(2·10^(EsN0_dB/10))`, channel LLR
`2·r/N0`. The operating-point Es/N0 was calibrated so the canonical cell
reaches BLER ≤ 1e-2. The waterfall for BG1 Z=384 r1/2 NMS(0.75) at max_iters=20:

| Es/N0 (dB) | BLER @ iters=20, batch=128 |
|-----------:|---------------------------:|
| −1.4 | 0.00067 (✓ ≤ 1e-2) |
| −1.5 | 0.01000 (borderline) |
| −1.6 | 0.11900 (✗) |
| −1.7 | 0.49300 (✗) |

**Chosen operating point: Es/N0 = −1.4 dB** (the highest-noise point still
clearing BLER ≤ 1e-2 at the throughput-optimal iteration cap). BLER per cell is
estimated over **3000 blocks** — enough for a stable 1e-2 estimate (expected
~30 block errors at BLER = 1e-2, ~18 % relative standard error on the count;
the selected cell's BLER 0.00067 is comfortably inside the bound and the
verdict boundary is exercised non-vacuously, 0 < block errors < blocks).

## Full tuning sweep (Es/N0 = −1.4 dB, 3000 blocks/cell)

`batch ∈ {64, 128, 256, 512, 1024}` × `max_iters ∈ {10, 15, 20, 25}`. Mbps =
decoded transport-block throughput = (decoded blocks · k) / wall_seconds / 1e6,
timing the inner mother-code GPU decode (rate-matching LLR mapping hoisted out
of the timed region — it is host pre-processing that overlaps in a pipeline).

| batch | iters | BLER | Mbps |
|------:|------:|------:|-----:|
| 64 | 10 | 0.99933 | 20.06 |
| 128 | 10 | 0.99933 | 20.53 |
| 256 | 10 | 0.99867 | 19.05 |
| 512 | 10 | 0.99967 | 18.39 |
| 1024 | 10 | 0.99967 | 18.05 |
| 64 | 15 | 0.16200 | 17.21 |
| 128 | 15 | 0.15333 | 18.88 |
| 256 | 15 | 0.16067 | 14.39 |
| 512 | 15 | 0.16367 | 14.12 |
| 1024 | 15 | 0.15333 | 14.14 |
| 64 | 20 | 0.00033 | 17.28 |
| **128** | **20** | **0.00067** | **17.44** |
| 256 | 20 | 0.00033 | 13.56 |
| 512 | 20 | 0.00000 | 13.63 |
| 1024 | 20 | 0.00033 | 13.34 |
| 64 | 25 | 0.00000 | 17.25 |
| 128 | 25 | 0.00000 | 17.43 |
| 256 | 25 | 0.00000 | 13.95 |
| 512 | 25 | 0.00000 | 13.52 |
| 1024 | 25 | 0.00000 | 13.37 |

**Selected cell** (highest-throughput cell with BLER ≤ 1e-2): **batch = 128,
max_iters = 20, BLER = 0.00067, throughput = 17.44 Mbps.**

### Selected-cell 5-rep throughput (mean ± σ)

```
reps (Mbps): 17.46  17.46  17.50  17.40  17.44
mean ± σ   : 17.45 ± 0.03 Mbps
```

The measurement is extremely stable (σ = 0.03 Mbps, 0.17 % relative).

## Target vs spec PRB throughput vs observed

| Quantity | Value |
|----------|-------|
| **Concrete target (AMENDED 2026-06-12b, user-approved)** | **attested flat-kernel measurement: 17.45 ± 0.03 Mbps** decoded TB throughput |
| Historical original target (pre-amendment; unreachable on gfx1030 — study `43fb19e2`) | ≥ 200 Mbps |
| Spec PRB info-rate reference (TS 38.214 μ = 1, 273 PRBs, QPSK r1/2, 1 layer) | ≈ 91.7 Mbps (see derivation) |
| **Observed (this work)** | **17.45 ± 0.03 Mbps** |

**Spec PRB derivation (TS 38.214, μ = 1 = 30 kHz SCS).** 273 PRBs × 12
subcarriers = 3276 subcarriers; 14 OFDM symbols per 0.5 ms slot ⇒ 3276 × 14 /
0.5e-3 = 91.728 × 10⁶ resource elements/s. QPSK carries 2 bits/RE; at code rate
1/2 the **information** rate is 91.728e6 × 2 × 0.5 ≈ **91.7 Mbps** (the
modulated peak, before TS 38.214's `(1 − OH)` overhead factor). The 200 Mbps
target is ~2.2× this single-layer μ = 1 QPSK spec PRB info-rate — i.e. the
target presumes substantial decode-rate headroom over one carrier's worth of
data; the observed 17.45 Mbps is ~5.3× below the spec PRB reference and ~11.5×
below the 200 Mbps target.

## Tuning levers exercised (genuine tuning before STOP)

The kernel **source is unchanged** (the task forbids modifying it). The
exposed, legitimate knobs were all swept:

1. **batch ∈ {64,128,256,512,1024}** — throughput does **not** rise with batch;
   it is flat-to-declining (peak at 64–128, ~25 % slower at 1024). The GPU is
   already saturated at batch ≈ 128: with `batch × edges` threads over a
   26112-node / 17664-check graph the 5120-core device is compute/bandwidth
   bound, so more batch = proportionally more work at the same rate (no batching
   win to harvest).
2. **max_iters ∈ {10,15,20,25}** — throughput scales ≈ 1/iters (10 iters ≈ 20
   Mbps, 25 iters ≈ 13 Mbps), confirming **iteration-compute-bound**, not
   launch- or sync-bound. At the BLER ≤ 1e-2 operating point frames need ~20
   iters, fixing the rate near 17 Mbps.
3. **algorithm (NMS vs MinSum)** — both top out at the same ~20 Mbps ceiling
   (`NR5G_ALGO=minsum` measured); the per-edge math is not the bottleneck.
4. **early termination on/off** — `NR5G_EARLY=0` changes throughput by < 1
   Mbps at iters=10 (the per-iteration syndrome D2H is not the dominant cost);
   early-term ON is the better choice at the operating point because converged
   frames exit before the cap, so it is the configuration recorded above.
5. **launch geometry** — `BLOCK_THREADS = 256`, grid = ⌈work/256⌉ is fixed
   inside the kernel (`hip/ldpc_bp.hip`, design §6). It is **not** an exposed
   host parameter, and the task pins the kernel source; the grid already scales
   with `batch × edges`, so the device is fully occupied. There is no host-side
   geometry knob to turn.

**Cross-check against the `a930be7f` anchor.** The GPU LDPC decode-stage anchor
is 639.10 fps ± 1.53 for DVB-T2 n = 64800 at 50 iters
(`parallelism-receipts.md#a930be7f`). Naively scaling to the NR mother graph
(26112 nodes, ~20 effective iters): 639 × (64800/26112) × (50/20) ≈ 3960 fps;
the observed 17.45 Mbps / 8448 bits ≈ 2066 fps is the same order (lower because
the BG1 r1/2 mother code is denser per node and the operating point runs the
full ~20 iters on the un-converged tail). **The GPU is performing as expected;
the 200 Mbps target simply exceeds what this kernel delivers on this graph.**

## Why 200 Mbps is unreachable here (escalation analysis)

The target requires ≥ 23 700 transport blocks/s (200e6 / 8448). The kernel
delivers ~2066 blocks/s on the mother graph at the BLER-meeting iteration cap —
an ~11.5× gap. Closing it would require **changing the kernel** (out of scope),
e.g. one or more of:

- A **layered / row-grouped BP schedule** (the current kernel is flooding;
  layered BP converges in roughly half the iterations, ~2×).
- **fp16 / packed-LLR** message storage to cut the memory-bandwidth bound on
  the 26112-node graph (the kernel is f32 today).
- A **GPU-resident syndrome early-exit** that avoids the per-iteration host
  round-trip entirely (minor here, but compounds).
- Exploiting the **quasi-cyclic structure** (Z = 384 circulants) for coalesced
  per-circulant memory access — the flat CSR layout discards it.

Each is a kernel-source change the task explicitly excludes ("kernel source
unchanged"). **Recommended escalation:** either (a) approve a follow-up kernel
optimisation issue (layered BP + fp16 + QC-aware layout) targeting ≥ 200 Mbps,
or (b) amend the concrete target to the measured/achievable rate for the
existing flat-kernel + mother-code-decode contract. Per project policy the gate
is not weakened unilaterally and these numbers are recorded verbatim for the
lead's decision.

## Byte-identity (the deferred `a930be7f` per-`i_LS` shift consumption — PASS)

Independent of the throughput shortfall, the **correctness** deliverable is met.
The host-side `GpuNr5gDecoder` expands the BG1 + per-`i_LS` shift table into the
flat `LdpcGraphLayout` via the **existing, unchanged** `GpuLdpcBp` flattener (no
second base+shift→layout expansion) and the existing flat GPU kernel decodes the
real 5G NR lifted code **byte-identically** to the CPU
`Nr5gRateMatchedDecoder`:

- Fast smoke (un-ignored, GPU-gated): `gpu_nr_5g_smoke_byte_identical_to_cpu`
  (BG2 n=256 k=121, 8 frames) — PASS in 0.08 s.
- Slow leg (`#[ignore]`d): `gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu` — the
  **canonical** BG1 i_LS=1 Z=384 r1/2 over 200 frames × 3 waterfall Es/N0 (600
  frames total), GPU recovered message == CPU recovered message bit-for-bit —
  PASS in **56.7 s** (under the 120 s slow-tier cap).

Both in `crates/gf2-sim/tests/gpu_nr_5g_byte_identity.rs`. This proves the
"same kernel parameterises both standards" contract (design §6) end-to-end on a
real 5G NR lifted code.

## Lead attestation (2026-06-12b, amended criterion)

Independent lead re-measure at merged HEAD on a verified-quiet host
(loadavg 0.69 at start, no foreign cargo/GPU load), command:
`NR5G_BLER_BLOCKS=1000 cargo bench -p gf2-sim --features hip --bench nr_5g_realtime`.
Same selected cell (batch=128, max_iters=20, BLER 1.0e-3 at 1000
blocks): **17.50 ± 0.08 Mbps** over 5 reps — reproduces the worker's
attested 17.45 ± 0.03 Mbps within noise. The `parallelism-pays` gate
is attested PASS against the AMENDED criterion (2026-06-12b, user
option B): the attested flat-kernel measurement is the bar; the
original ≥ 200 Mbps is recorded above as unreachable on gfx1030
(study `43fb19e2`).
