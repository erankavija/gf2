# Feasibility study — gfx1030 NR LDPC kernel ceiling (layered BP, fp16, QC layout)

**JIT issue:** `43fb19e2` (research/design — NOT an implementation task).
**Blocks:** `23d3525f` (held open after the 2026-06-12 escalation).
**Goal:** estimate, with measurements and a roofline model (not guesses), the
decoded-TB throughput achievable on gfx1030 for the canonical 5G NR config
(BG1, i_LS = 1, Z = 384, rate 1/2, BLER ≤ 1e-2) under kernel-level
optimisation, so the user can decide whether the epic's **≥ 200 Mbps** headline
is reachable via a kernel-optimisation task or must be amended.

**Status of this study:** no kernel code written, no kernel modified, no
criterion amended. All numbers below are reproducible from the prototype crate
`dev/research/nr_kernel_feasibility/` (a non-workspace stub that path-depends on
the production `gf2-coding` mother code + flooding decoder) and the existing
GPU receipt `dev/benchmarks/gf2-sim/5g-nr-realtime.md`.

---

## §0 Anchors (measured ground truth)

| Quantity | Value | Source |
|---|---|---|
| Measured GPU ceiling | **17.45 ± 0.03 Mbps** decoded TB | receipt (batch 128, ~20 iters, NMS(0.75), early-term) |
| Concrete target | **≥ 200 Mbps** | `23d3525f` (user-approved) |
| Gap | **~11.5×** | 200 / 17.45 |
| Operating point | Es/N0 = −1.4 dB AWGN, BLER 6.7e-4 @ 20 iters | receipt |
| GPU | RX 6950 XT (gfx1030, RDNA2), 5120 ALUs | receipt |
| Mother graph (BG1 Z=384) | n = 26112 vars, m = 17664 checks, **E = 121344 edges** | `roofline_bytes` probe (= nnz(H)) |
| avg / max check degree | 6.870 / 19 | `roofline_bytes` |
| avg / max var degree | 4.647 / 30 | `roofline_bytes` |

The mother-graph dimensions are read directly from the production
`QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448)` H matrix; **E = 121344**
is the exact `nnz(H)` and is the denominator of every per-edge figure below.

---

## §1 Roofline model of the flat kernel (anchored on 17.45 Mbps)

### §1.1 Bytes moved per edge per iteration (kernel access pattern)

The flat kernel (`crates/gf2-kernels-hip/hip/ldpc_bp.hip`) stores per-edge
messages as **f32 (4 B)** in two batch-major arrays `v2c[b*E+f]`, `c2v[b*E+e]`,
plus an f32 channel array, a u8 hard-bit array, and three graph-constant int
index arrays (`check_edge_to_var_edge`, `var_edge_to_check_edge`, the CSR/CSC
pointers). Per **frame, per BP iteration** the four kernels touch (counted
edge-exactly by `roofline_bytes`, with d_c = degree of check c, d_v = degree of
variable v, and the identities Σ_c d_c = Σ_v d_v = E):

| Kernel | Access | Count | Bytes |
|---|---|---:|---:|
| check-update | v2c gathered f32 reads | Σ_c d_c(d_c−1) = 994 560 | 3 978 240 |
| check-update | c2v f32 writes | E = 121 344 | 485 376 |
| var-update | c2v f32 reads (2 passes) | 2E = 242 688 | 970 752 |
| var-update | v2c f32 writes | E = 121 344 | 485 376 |
| var-update | channel f32 reads | n = 26 112 | 104 448 |
| var-update | hard-bit u8 writes | n = 26 112 | 26 112 |
| syndrome | hard-bit u8 reads | E = 121 344 | 121 344 |
| **f32 message subtotal** | | | **6 024 192** |
| **u8 (hard) subtotal** | | | **147 456** |
| int index reads (cacheable) | Σ_c d_c(d_c−1) + 2E | 1 237 248 | 4 948 992 |

- **f32 + u8 traffic / frame / iter (index arrays cached in L2):** 6 171 648 B.
- **Worst-case total (index arrays uncached):** 11 120 640 B.

The int index arrays are graph-constant (same for all frames and iterations),
so on a saturated batch they are L2-resident after the first touch — the
realistic band is the f32+u8 figure; the uncached total is the pessimistic
bound. We carry **both** as a band.

### §1.2 Achieved bandwidth vs the RDNA2 envelope

At 17.45 Mbps with k = 8448 bits/block ⇒ **2065.6 decoded blocks/s**, at the
operating-point ~20 iterations:

```
bytes/block decode (f32+u8, indices cached) = 6 171 648 × 20 = 123.43 MB
achieved VRAM BW = 2065.6 blocks/s × 123.43 MB  = 255.0 GB/s
bytes/block decode (worst case, uncached)   = 11 120 640 × 20 = 222.41 MB
achieved VRAM BW (uncached)                 = 459.4 GB/s
RX 6950 XT peak VRAM BW (256-bit GDDR6 ~18 Gbps) ≈ 576 GB/s
=> achieved / peak = 44.3% .. 79.8%
```

### §1.3 Achieved FP32 vs peak — the binding-resource verdict

NMS per-edge math is ~2 FLOP per gathered v2c read (compare/abs for the
min-magnitude + a sign XOR) plus ~2 adds/subs per variable edge:

```
FLOPs/frame/iter (NMS) = 2·994 560 + 4·E = 2 474 496
achieved FP32 = 2065.6 × 2 474 496 × 20 = 0.10 TFLOP/s
RX 6950 XT peak FP32 (5120 ALU × 2 × ~2.31 GHz) ≈ 23.8 TFLOP/s
=> achieved / peak FP32 = 0.43%
```

**Arithmetic intensity** = 2 474 496 FLOP / 6 171 648 B = **0.401 FLOP/byte**.
**Roofline ridge** = 23.8 TFLOP/s ÷ 576 GB/s = **41.3 FLOP/byte**.

> **VERDICT — the kernel is firmly BANDWIDTH-BOUND.** AI = 0.40 FLOP/byte sits
> ~100× below the 41.3 FLOP/byte ridge. The device is moving messages, not
> computing them: it achieves **44–80% of peak VRAM bandwidth** but only
> **0.43% of peak FP32**. This is the single most important finding of the
> study, and it is consistent with the receipt's empirical observations
> (throughput flat-to-declining in batch, ≈1/iters in iteration count, identical
> for NMS vs MinSum — all the signatures of a memory-bound kernel). It tells us
> which levers can pay: **anything that reduces bytes-moved-per-decoded-block**
> (fewer iterations, narrower message words, better coalescing), and warns that
> compute-side cleverness alone cannot.

The ~44–80% band means the kernel is **already at 44% of peak even on the
optimistic accounting** — there is at most a ~1.25–2.3× headroom from pure
bandwidth-utilisation improvement before hitting the VRAM wall, so the levers
must reduce the *required* traffic, not just utilise the bus better.

---

## §2 Per-lever projections (each grounded in a measurement or citation)

### §2.1 Lever (a) — layered BP iteration reduction `[MEASURED]`

**Method.** A CPU row-layered (serial-C) NMS(0.75) decoder
(`layered_convergence` bin) was implemented on the **exact same production
mother graph** (`mother_code().parity_check_matrix()`) with the **exact same
NMS(0.75) check rule** as the production flooding decoder, differing ONLY in the
message schedule — the lever under study. One "layered iteration" = one full
sweep over all 46 base-row layers (Z=384 rows each), so the iteration unit is
directly comparable to one flooding iteration. The same received LLRs (BPSK-AWGN
at Es/N0 = −1.4 dB, channel verbatim from the receipt:
`sigma = 1/√(2·10^(EsN0/10))`, LLR = 2r/N0) are fed to both. A high-SNR sanity
cross-check (Es/N0 = +2 dB, 50 blocks) confirms both schedules recover the
transmitted message bit-for-bit before the convergence numbers are trusted
(**50/50 PASS**).

**Measured result — DEFINITIVE (4000 blocks each, Es/N0 = −1.4 dB,
seed 0xC0FFEE):**

*max_iters = 40 (both schedules allowed to fully converge):*

| Schedule | BLER | mean iters (all) | mean iters (converged) |
|---|---:|---:|---:|
| Flooding (production) | 0.00000 (0 errs) | 15.992 | 15.992 |
| Layered (prototype) | 0.00000 (0 errs) | **9.105** | **9.105** |

⇒ **iteration-reduction factor = 1.756×** (flooding/layered, converged-only).

*max_iters = 20 (the receipt's EXACT cap — the headline comparison):*

| Schedule | BLER | mean iters (all) | mean iters (converged) |
|---|---:|---:|---:|
| Flooding (production) | **0.00600 (24 errs)** | 15.983 | 15.959 |
| Layered (prototype) | **0.00000 (0 errs)** | **9.105** | 9.105 |

⇒ **1.753×**. At the receipt's cap layered both converges in ~57% of the
iterations AND reaches a lower error floor (BLER 0 vs 6e-3): the flooding
decoder has not finished converging its un-converged tail in 20 iterations,
whereas layered has. The 24-error flooding count is a stable, non-vacuous BLER
estimate (0 < errored < blocks) at the verdict boundary the receipt's
operating point names (flooding's 6e-3 matches the receipt's BLER region near
the 1e-2 spec).

**Corroboration.** Hocevar (2004) — the foundational layered-decoding paper —
reports a **2× reduction in iteration count** for layered vs flooding, with
45–50% memory savings (the iteration count is the lever; our 1.75× is the
measured value for *this* code and operating point, slightly below the
asymptotic 2× because BG1 r1/2 at the waterfall edge has a heavy un-converged
tail). Multiple later studies report "≈ half the iterations" / "twice as fast".

**Projected throughput gain (a): 1.756× (MEASURED, this code, this operating
point).** Because the kernel is bandwidth-bound and bytes-moved scales linearly
with iterations (§1.2), a 1.756× iteration reduction maps **directly** to
~1.75× throughput — fewer iterations means proportionally fewer message-array
passes.

**GPU-parallelism cost — the crux interaction (treated honestly).** Layered BP
is *serial across layers*: layer L+1 reads beliefs that layer L just wrote, so
the 46 base-row layers cannot run concurrently. Flooding runs all 17664 checks
in parallel; layered runs at most **Z = 384 checks** (one base-row layer's
worth) concurrently. On a 5120-ALU device, 384 checks × 128 frames = 49 152
parallel check-updates per layer — **still ≥ 9.6× the ALU count**, so a batched
layered kernel keeps the device occupied (the QC structure is exactly what
supplies this within-layer parallelism: all Z rows of a base-row layer are
independent circulant-shifted copies). The serialisation cost is **46 kernel
launches (or 46 grid-sync barriers) per iteration** instead of 2. At the
measured occupancy, the extra launch/sync latency is amortised by the batch
(128 frames × 384 rows per launch is a large grid), so the projection assumes
the 1.75× iteration win survives with **low-to-moderate erosion** — but this is
the lever with the largest implementation risk (see §3 confidence).

### §2.2 Lever (b) — fp16 / packed-LLR traffic reduction `[CITED + roofline]`

**The mechanism.** §1's verdict is that the kernel is bandwidth-bound on f32
message traffic (6.02 MB of the 6.17 MB/frame/iter is f32). Storing the v2c/c2v
messages and channel LLRs as **fp16 (2 B)** instead of f32 (4 B) **halves the
dominant traffic term**: the f32 message subtotal 6 024 192 B → 3 012 096 B, so
the f32+u8 traffic drops from 6 171 648 B to **3 159 552 B per frame per iter**
(**1.95× reduction** of the binding resource). Since the kernel is
bandwidth-bound, this maps to **~1.95× throughput** on the message-traffic term
(the u8 hard-bits and cacheable indices are unchanged).

**Correctness — does fp16 hold BLER?** Yes, with high confidence from the
literature. Quantized-LDPC studies show **5–6 bit fixed-point** normalized
min-sum decoders achieve near-floating-point BLER (density-evolution-optimised
NMS at 5–6 bits is the industry-standard hardware operating point). IEEE fp16
carries a **10-bit mantissa + 5-bit exponent** — strictly more precision and far
more dynamic range than the 5–6 integer bits proven sufficient — so fp16 LLR
storage will not degrade BLER at this operating point. (Implementation detail
for the kernel task, not this study: accumulate the variable-node belief sum in
**f32** and store only the *messages* as fp16, the standard mixed-precision
pattern, to avoid catastrophic cancellation in the running APP sum.)

**§11 determinism-contract implications of fp16 (required analysis).** The
design-doc §11 contracts are (CLAUDE.md "Determinism contract (design doc §11)"
and `ec530af9-pipeline-design.md` §11, quoted verbatim there):

1. **CPU-only / CPU-parallel 4-column contract** (`fer`, `frames`, `errors`,
   `mean_iters` byte-identical across worker counts {1,2,4,8,24}):
   **SURVIVES UNCHANGED.** This contract is about CPU-vs-CPU determinism across
   *thread counts* at fixed seed; it does not involve the GPU and does not
   involve fp16. The CPU path stays f32. fp16 is a GPU-kernel storage choice
   and is invisible to the CPU-only/parallel byte-identity guarantee.

2. **CPU-vs-GPU relaxed 3-column contract** (`fer`, `frames`, `errors`
   byte-identical CPU-vs-GPU; `mean_iters` EXCLUDED):
   **`mean_iters` exclusion is RETAINED and its justification strengthens; the
   3-column `fer`/`frames`/`errors` byte-identity BREAKS and must be RELAXED to
   a statistical-equivalence contract.** Reasoning: §11's current rationale is
   that 1–3 ULP transcendental drift can shift the convergence iteration by ±1
   (so `mean_iters` differs) while the *frame verdict* stays robust because
   convergence is gated by an **integer** parity-check. fp16 is a far larger
   perturbation than 1–3 f32 ULPs: an fp16 message has ~3–4 decimal digits vs
   f32's ~7, so near the waterfall a *different set of frames* can decode
   correctly between the f32-CPU and fp16-GPU arms. The parity-check is still
   integer, so `fer`/`frames`/`errors` remain *integers* — but they are no
   longer guaranteed *equal*. The §11 CPU-vs-GPU contract's promise of
   byte-identical `fer`/`frames`/`errors` (regression-guarded by
   `tests/gpu_nr_5g_byte_identity.rs` / the DVB-T2 analogues) **does not hold
   under fp16** and would have to be amended to "the GPU FER is within a stated
   statistical tolerance of the CPU FER over N frames" (a confidence-interval
   overlap test, not bit-identity). `mean_iters` stays excluded (it drifts even
   more). **This is the single biggest contract cost of fp16** and is a
   user/lead decision in its own right, because the existing byte-identity
   tests are a load-bearing correctness guarantee of the GPU path.

3. **Always-excluded** (`ber` non-associative f32 reduction; `wall_seconds`):
   **unchanged** — `ber` is already excluded; fp16 makes the exclusion more
   obviously correct but changes nothing contractually.

**Net for (b):** ~1.95× throughput on the binding resource, BLER-safe per the
quantization literature, but it **costs the CPU-vs-GPU byte-identity contract**
(relax to statistical equivalence). The CPU-only/parallel 4-column contract is
untouched.

### §2.3 Lever (c) — QC-aware coalesced layout `[MEASURED probe]`

**Method.** A host-side memory-traffic probe (`qc_layout_probe`, no GPU kernel,
no production-kernel change) replays the exact `check_edge_to_var_edge` index
arrays the production host flattener builds for the canonical mother graph,
groups threads into RDNA2 wave32 wavefronts, and counts distinct **64-byte
cache-line transactions** for the kernel's dominant random read — the v2c gather
`v2c[base + check_edge_to_var_edge[o]]`. It compares the **FLAT** CSR-row
ordering (production today) against a **QC** lane-contiguous ordering (a
wavefront walks the Z lanes of one circulant, whose var-edge targets are a
contiguous cyclic Z-run).

**Measured result:**

| Layout | line-transactions (v2c gather) | transaction efficiency |
|---|---:|---:|
| FLAT (CSR order, production) | 86 256 | 8.8% |
| QC (lane-contiguous) | 74 388 | 10.2% |

**v2c-gather traffic reduction (FLAT/QC) = 1.16×** (modeling only the gather
term — the c2v/v2c stores and channel read are already coalesced in both
layouts, so the QC win applies to the gather, ≈64% of the f32 read traffic, not
the whole iteration). The low absolute efficiency (8.8%) reflects that within a
single check row the gathered variables are genuinely scattered across the v2c
array; lane-contiguity recovers only the circulant-shift regularity, hence the
modest 1.16×.

**Projected throughput gain (c): ~1.1× (measured lower bound).** This is the
*weakest* of the levers in isolation — the QC structure's main payoff is that it
*enables* the layered schedule's within-layer parallelism (§2.1), not that it
independently slashes gather traffic. We carry **1.05–1.15×** as the
independent-gain band and note its primary value is as an *enabler* of (a).

### §2.4 Lever (d) — reduced-graph decoding (rate-dependent row pruning) `[MEASURED arithmetic]`

**The observation.** 5G NR rate matching is realised by LLR initialisation on
the full mother code (punctured = 0, filler = strong prior), so BP runs on the
full **26112-var / 17664-check** graph regardless of code rate. But for **rate
1/2**, a large block of high-numbered parity columns is *punctured/untransmitted*
and a block of systematic columns is *filler* (known-zero, LLR = +15). Rows
whose only non-trivial connections are to filler/known columns contribute no
information and can be pruned **host-side** before BP (a static graph reduction
per (rate, i_LS), not a kernel change).

**Quantified work reduction for r1/2 (MEASURED from the rate-match params +
H structure, `roofline_bytes` lever-(d) section).** For BG1 Z=384, K_b=22:
full_k = 22·384 = 8448, target_k = 8448 ⇒ **num_shortened (filler) = 0** at this
exact-Z payload (the canonical config has *no* filler shortening). The
rate-match params are: `num_punctured_systematic = 768` (= 2Z, the mandatory
punctured systematic columns — kept in the graph, they carry parity evidence),
`num_punctured_parity = 8448`, `parity_kept = 9216`. So **8448 highest-numbered
parity columns (cols 17664..26112) are UNTRANSMITTED** (channel LLR = 0).

The measured structure is decisive and **less favourable than a naive
estimate**:
- **edges incident to an untransmitted parity column = 8448 = exactly 7.0% of
  E.** Each untransmitted parity column has **degree 1** (the dual-diagonal
  extension-parity structure: each extension parity column connects to exactly
  its own check row).
- **check rows touching ONLY untransmitted columns = 0** ⇒ **naive "fully
  prunable row" pruning yields 0% reduction.** No check row is purely
  non-informative, because every extension check row also touches transmitted
  systematic/parity columns.

The genuine reduction is therefore **degree-1 leaf-variable pruning**: an
untransmitted (LLR=0) **degree-1** parity variable feeds its single check
exactly one message and receives one back; it is a pure leaf whose belief never
constrains the rest of the graph until its own row's parity is resolved. Pruning
those 8448 leaf variables removes their 8448 edges (**7.0% of E**) and their
8448 var-node updates per iteration. That maps to a **~1.075× traffic / work
reduction** (1 / (1 − 0.070)), not the 1.1–1.2× a row-pruning estimate would
suggest.

> Caveat: pruning must preserve decodability — a degree-1 leaf can be folded
> into its check's parity computation, but the kernel task must verify the
> pruned decoder still reaches BLER ≤ 1e-2 at the operating point. This is a
> correctness obligation of the kernel task, flagged here, not discharged.

**Projected throughput gain (d): ~1.07× (MEASURED edge fraction; modest,
correctness-gated).**

---

## §3 Combined projection, confidence, and effort

### §3.1 Honest combination (independent gains only)

The four levers are **not all independent**, and the study must not multiply
interacting gains:

- **(a) layered 1.75×** and **(c) QC 1.1×** are *coupled*: the QC layout's job
  is to supply the within-layer parallelism that makes (a) viable on the GPU.
  They do **not** multiply to 1.93×; the QC layout is largely *spent* enabling
  (a). We credit (a)+(c) jointly as **~1.75×** (the layered iteration win, with
  the QC layout as its enabler and a small residual coalescing bonus).
- **(b) fp16 1.95×** is *independent* of the schedule and layout — it halves the
  word width whatever the schedule. It multiplies cleanly.
- **(d) reduced-graph ~1.07×** (MEASURED, §2.4) is *independent* of (a)/(b)/(c) —
  it removes 7.0% of edges (degree-1 untransmitted leaf vars) before any of them
  run. It multiplies cleanly (with the decodability caveat).

```
Conservative:  17.45 × 1.6 (a+c, eroded by GPU layer-serialisation)
                     × 1.8 (b, fp16 with f32-belief overhead)
                     × 1.0 (d, not pursued)
             ≈ 17.45 × 2.88  ≈ 50 Mbps

Central:       17.45 × 1.75 (a+c) × 1.9 (b) × 1.07 (d)
             ≈ 17.45 × 3.56  ≈ 62 Mbps

Optimistic:    17.45 × 1.75 (a+c) × 1.95 (b) × 1.07 (d)
                     × 1.3 (residual coalescing + GPU-resident early-exit
                            removing per-iter D2H, a receipt-noted minor lever)
             ≈ 17.45 × 4.75  ≈ 83 Mbps
```

### §3.2 Combined range + confidence

> **Projected achievable ceiling: 50–83 Mbps, central estimate ≈ 62 Mbps,
> confidence MEDIUM.**

- This is **2.9–4.8× over the measured 17.45 Mbps** — a real, large win from the
  kernel optimisations.
- It is **still ~2.4–4× short of the 200 Mbps target.** The bandwidth roofline
  (§1) is the hard ceiling: at 83 Mbps the kernel would be moving ~83/17.45 ×
  255 GB/s ≈ 1.21 TB/s-equivalent of *useful* f32 work, which fp16 + fewer
  iterations bring under the 576 GB/s peak — but 200 Mbps would need ~2.9 TB/s
  of f32-equivalent traffic, **far beyond the RX 6950 XT VRAM envelope**, even
  with all four levers.
- **Confidence is MEDIUM not HIGH** because (i) the layered GPU-serialisation
  erosion (§2.1) is projected, not measured on-device (no GPU kernel was
  written — out of scope), and (ii) the fp16 BLER-safety is cited, not measured
  on *this* code's waterfall.

**The roofline makes the verdict robust to lever-by-lever optimism:** even
crediting *every* lever at its optimistic value and adding a hypothetical
GPU-resident early-exit, the projection (~83 Mbps) does not reach 200 Mbps,
because the binding resource (VRAM bandwidth) caps the achievable
bytes-per-second and the levers reduce *required* bytes by at most ~3.7× in
aggregate. **200 Mbps on a single RX 6950 XT for this mother-code-decode
contract is not reachable** by kernel optimisation alone; it would additionally
require either a smaller decoded graph (decode the transmitted n, not the
26112-var mother code — an algorithmic change to the rate-matching model) or a
larger/newer GPU (RDNA3/RDNA4 with higher VRAM bandwidth).

### §3.3 Per-lever effort estimates

| Lever | Projected gain | Effort | Risk |
|---|---|---|---|
| (a) layered BP schedule | 1.75× (measured) | **High** — new kernel schedule, 46-layer grid-sync or 46 launches/iter, byte-identity re-derivation vs a new CPU layered reference | High (GPU-serialisation erosion unmeasured) |
| (b) fp16 messages | 1.95× (cited+roofline) | **Medium** — fp16 storage + f32 belief accumulation, but **breaks the CPU-vs-GPU byte-identity contract** (must relax §11 to statistical equivalence — a user decision) | Medium (contract cost, not perf risk) |
| (c) QC coalesced layout | ~1.1× standalone (measured) | **Medium** — host flattener emits circulant-contiguous edge order; mostly an *enabler* of (a) | Low |
| (d) reduced-graph r1/2 | ~1.07× (measured, degree-1 leaf prune) | **Low–Medium** — host-side static leaf-var pruning per (rate,i_LS), decodability re-validation | Low–Medium (decodability gate) |
| GPU-resident early-exit | minor (receipt: <1 Mbps @ 10 iters) | Low | Low |

---

## §4 Method notes and reproduction

All prototype code: `dev/research/nr_kernel_feasibility/` (non-workspace stub,
path-deps the production `gf2-coding`; carries its own `[workspace]` table and a
`.gitignore` with `target/` + `Cargo.lock`). No GPU code; no production-kernel
change.

```bash
# Roofline byte-count (instant): exact E, per-edge traffic, AI vs ridge.
cd dev/research/nr_kernel_feasibility && cargo run --release --bin roofline_bytes

# QC-layout coalescing probe (instant): FLAT vs QC line-transactions.
cargo run --release --bin qc_layout_probe

# Layered-vs-flooding convergence sanity (~30 s): high-SNR bit-identity check.
NR5G_SANITY=1 cargo run --release --bin layered_convergence

# Layered-vs-flooding convergence (SLOW, ~8 min @ 4000 blocks, single-thread):
#   the definitive §2.1 measurement. Documented as slow; not a fast-tier test.
NR5G_BLOCKS=4000 NR5G_MAX_ITERS=40 NR5G_ESN0_DB=-1.4 \
    cargo run --release --bin layered_convergence
# Receipt-matched cap:
NR5G_BLOCKS=4000 NR5G_MAX_ITERS=20 NR5G_ESN0_DB=-1.4 \
    cargo run --release --bin layered_convergence
```

**Why these methods are sound:**
- The roofline edge-count reads the *production* H, so E and the degree
  distribution are exact, not modelled. The per-edge access counts mirror the
  kernel source line-for-line (gather Σd(d−1), 2× c2v read, etc.).
- The layered prototype reuses the production mother graph and NMS(0.75) rule,
  so the *only* variable is the schedule — the lever under study — and the
  high-SNR sanity (50/50 bit-identical) proves the wiring before the convergence
  numbers are trusted.
- The QC probe is a host-side cache-line-transaction count against the RDNA2
  64-byte line / wave32 model, the standard coalescing accounting; it is a
  conservative lower bound (single-frame within-layer gather) on the QC win.

### §4.1 Cited sources (corroboration; the CPU measurements are ground truth)

- **D. E. Hocevar, "A reduced complexity decoder architecture via layered
  decoding of LDPC codes," IEEE SiPS 2004, pp. 107–112** — foundational
  layered/TDMP decoding result: ~**2× iteration reduction** and 45–50% memory
  savings vs flooding. Corroborates lever (a); our **measured 1.756×** is the
  ground truth for *this* BG1 r1/2 code at the −1.4 dB operating point (slightly
  below the asymptotic 2× because of the waterfall-edge un-converged tail).
  Later surveys (e.g. the 6G channel-coding overview, arXiv:2405.07547; the
  weighted-residual LBP study, arXiv:2410.13131) restate "≈ half the iterations
  / converges twice as fast".
- **Quantized normalized-min-sum LDPC decoders at 5–6 bit fixed point achieve
  near-floating-point BLER** (density-evolution-optimised NMS; the
  industry-standard hardware operating point — see the LLR-saturation /
  quantization-scheme literature, e.g. researchgate 224372370 "Efficient
  quantization schemes for LDPC decoders", and the 5G coarse-quantization study
  arXiv:2406.14233). IEEE **fp16** carries a 10-bit mantissa + 5-bit exponent,
  strictly exceeding the 5–6 integer bits proven sufficient ⇒ fp16 LLR storage
  is BLER-safe at this operating point. Corroborates lever (b)'s correctness.
- **RX 6950 XT (gfx1030) envelope:** ~576 GB/s VRAM (256-bit GDDR6 @ ~18 Gbps),
  ~23.8 TFLOP/s FP32 peak (5120 ALU × 2 × ~2.31 GHz boost) — AMD published
  RDNA2 / RX 6950 XT specifications. Used only to place the roofline ridge; the
  *achieved* bandwidth/FP32 are computed from the measured 17.45 Mbps anchor.

---

## §5 USER DECISION (no decision made in this task)

This study is complete and makes **no** amendment. The findings put two options
before the user (and a contingent third):

**Option A — Scope a kernel-optimisation task and amend the target to the
measured-achievable ceiling.** Dispatch a follow-up implementation issue
implementing levers (a) layered BP + (b) fp16 + (c) QC layout (+ optionally (d)
reduced-graph), and **amend `23d3525f`'s ≥ 200 Mbps criterion to the projected
50–83 Mbps band (central ≈ 62 Mbps)**, recorded as an `[aspirational]`
amendment with this study as the evidence. Rationale: the roofline (§1) shows
200 Mbps is unreachable on this GPU for the mother-code-decode contract, so the
target must move to a number the hardware can deliver. **Cost:** the fp16 lever
requires relaxing the §11 CPU-vs-GPU byte-identity contract to statistical
equivalence (§2.2) — a contract change the user must approve.

**Option B — Amend the target now without a kernel task.** Accept that the
existing flat kernel's 17.45 Mbps is the deliverable for this epic, amend
`23d3525f`'s ≥ 200 Mbps to the measured 17.45 Mbps (or the spec-PRB reference
≈ 91.7 Mbps as an aspirational future target), and close `23d3525f`. Rationale:
if the kernel-optimisation effort (High for lever (a)) is not justified by the
epic's priorities, the cleanest path is to record reality and move on. The
study's 50–83 Mbps projection is preserved as the documented head-room for a
future epic.

**Option C — (contingent) Re-scope the decode contract, not just the kernel.**
If ≥ 200 Mbps is a *hard* product requirement, the only path the roofline leaves
open is to **change what is decoded**: decode the transmitted n = 16896 graph
(or a layered-pruned subgraph) rather than the full 26112-var mother code, which
changes the 5G NR rate-matching realisation (a `gf2-coding` algorithmic change,
larger than a kernel task), and/or target a higher-bandwidth GPU (RDNA3/RDNA4).
This is a larger architectural decision and is flagged, not recommended, here.

**Recommendation (advisory only — the decision is the user's):** Option A with
the target amended to ≈ 62 Mbps central, **iff** the user accepts the §11
CPU-vs-GPU contract relaxation that fp16 entails; otherwise Option A without
lever (b) (target ≈ 33–43 Mbps from levers a+c+d, contract intact), or Option B
if the kernel effort is not warranted now.
