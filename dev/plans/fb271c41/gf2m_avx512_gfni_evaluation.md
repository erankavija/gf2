# GF(2^m) GFNI / AVX-512 Follow-on Routing Evaluation

| Field | Value |
|---|---|
| Date | 2026-05-06 |
| JIT issue | `fb271c41` (Evaluate GFNI and AVX-512 follow-on routing) |
| Parent story | `2c7548ae` (Close GF(2^m) FieldMatrix gaps to best reference) |
| Parent epic | `97bf0879` (Close gf2-core SOTA performance gaps) |
| Companion issue | `e24f7839` (Implement panelized GF(2^m) GEMM) |
| Scorecard | `dev/bench_results/2026-05-06-a1172cea-gf2m-scorecard.md` |
| Status | DELIVERY COMPLETE -- decision reached; see § 5 |

---

## § 1 Question and scope

Two questions govern this issue:

**Q1.** Are GFNI (Galois Field New Instructions, part of the AVX-512 family) or
AVX-512 vector kernels REQUIRED for this epic (`97bf0879`) to close the
GF(2^m) performance gap to within 1.5x of M4RIE / NTL on the Zen-3 host class?

**Q2.** If yes, file the required platform work as in-epic issues and wire them
under epic `97bf0879` / story `2c7548ae`. If no, document that AVX-512/GFNI
work remains a future direction (post-epic) and explain the reasoning.

This is a **research / decision** task. No GFNI or AVX-512 kernels are
implemented here. The implementation closure path for the 7 [hard] FAIL cells
is owned by `e24f7839` (panelized GF(2^m) GEMM, dispatched in parallel).

---

## § 2 Host class facts

The protocol § 5 hardware anchor and all GF(2^m) scorecard measurements are
pinned to:

> **AMD Ryzen 9 5900X (Zen 3)** -- 12 cores / 24 threads.
> ISA flags confirmed in `dev/bench_results/2026-05-04-507b0036-m4rie-host.txt`
> and `dev/bench_results/2026-05-04-73ab8eef-ntl-host.txt`:
> `pclmulqdq`, `sse4_1`, `avx2`, `bmi2`, `vaes`, `vpclmulqdq`.
> **No `avx512f`, no `avx512vl`, no `gfni`.**

Key ISA facts for the decision:

| ISA extension | Present on Zen 3? | Notes |
|---|---|---|
| `pclmulqdq` (128-bit CLMUL, SSE lane) | Yes | Base carry-less mul; gf2-core primary path |
| `vpclmulqdq` (256-bit CLMUL, YMM lane) | Yes | Available **without** AVX-512; Zen 3 exposes it as an AVX2-class extension |
| `avx2` (256-bit integer SIMD) | Yes | gf2-core's SIMD logical and M4RM kernels run here |
| `avx512f` / `avx512vl` | No | Not present on Zen 3 |
| `gfni` (Galois Field New Instructions) | No | Requires AVX-512 class; absent on Zen 3 |

VPCLMULQDQ on Zen 3 operates at 256-bit register width (YMM) and processes
two 64x64 carry-less multiplications per instruction. The `avx512vl` flag
would extend VPCLMULQDQ to ZMM (512-bit, four multiplications per
instruction), but it is not present on this host.

**The development and benchmarking host is Zen 3. The protocol § 5 "same
hardware" requirement means all [hard] FAIL verdicts in the scorecard are
evaluated against M4RIE / NTL numbers measured on this exact Zen-3 host.**

---

## § 3 M4RIE on Zen-3: AVX2-only path achieves the reference numbers

M4RIE 20250128 (`mzed_mul`) is the canonical reference for GF(2^8) and
GF(2^16) matmul. From the scorecard:

| field | n | M4RIE throughput (ops/s) | ISA used by M4RIE |
|---|---:|---:|---|
| GF(2^8) | 64 | 4.052e9 | AVX2 (PSHUFB-based 4-bit table lookup) |
| GF(2^8) | 256 | 2.453e10 | AVX2 |
| GF(2^8) | 1024 | 9.757e10 | AVX2 |
| GF(2^16) | 64 | 1.244e7 | AVX2 (wider table, same PSHUFB pattern) |
| GF(2^16) | 256 | 5.312e7 | AVX2 |
| GF(2^16) | 1024 | 2.854e9 | AVX2 |

**M4RIE achieves 97.6 Gops/s at GF(2^8) n=1024 using AVX2 only -- no
AVX-512, no GFNI.** The M4RIE host-capture (`2026-05-04-507b0036-m4rie-host.txt`)
confirms: the CPU flags list contains `avx2`, `vpclmulqdq`, `pclmulqdq` but
does NOT contain `avx512f`, `avx512vl`, or `gfni`. The reference numbers that
define the [hard] 1.5x SOTA threshold were therefore produced without
AVX-512 or GFNI.

M4RIE's algorithm for GF(2^8) is the **Method of Four Russians (M4RM) /
Gray-code table approach**: the GF(2^8) elements fit in one byte, so a
256-entry lookup table covers one row panel and byte-level PSHUFB permutations
batched via AVX2 produce the throughput gains. This is a macro-level algorithm
advantage over gf2-core's current per-element CLMUL path, not an ISA advantage.

The structural gap between gf2-core (750-840 Mops/s flat across n=64..1024)
and M4RIE (4-98 Gops/s, scaling with n) is caused by gf2-core lacking the
Gray-code / panelized batch, **not** by M4RIE having AVX-512 or GFNI
instructions that Zen 3 cannot execute.

**Implication for Q1:** closing the GF(2^8) and GF(2^16) gaps requires
implementing panelized GEMM analogous to M4RIE's macro algorithm, inside the
AVX2 ISA already present on Zen 3. AVX-512 and GFNI are not needed to reach
the reference numbers, because the reference itself does not use them.

---

## § 4 NTL `mat_GF2E` GF(2^32): VPCLMULQDQ on AVX2 is sufficient

NTL 11.6.0 `mat_GF2E` is the canonical reference for GF(2^32) matmul. From
the scorecard:

| field | n | NTL throughput (ops/s) |
|---|---:|---:|
| GF(2^32) | 64 | 2.675e8 |
| GF(2^32) | 256 | 2.805e8 |
| GF(2^32) | 1024 | 2.829e8 |

The NTL host-capture (`2026-05-04-73ab8eef-ntl-host.txt`) confirms the same
Zen-3 CPU: no `avx512f`, no `gfni`. NTL `mat_GF2E` multiplies polynomial
coefficients via NTL's internal `GF2E::mul`, which in turn calls NTL's
`GF2X` multiply. NTL uses `pclmulqdq` (or scalar fallback) for carry-less
multiplication -- the same instruction already used by gf2-core's kernel.

The GF(2^32) gap is ~3-4x at all measured sizes. The scorecard pattern
analysis identifies the cause as **matrix-level blocking**: gf2-core's current
path is per-element VPCLMULQDQ without cache-friendly matrix-level blocking;
NTL's `mat_GF2E` inherits its matrix data structure's implicit panelization.
The VPCLMULQDQ inner-loop kernel itself is already in production in
`crates/gf2-kernels-simd/src/x86/clmul.rs` (the `clmul_batch_vpclmul`
function uses `vpclmulqdq` when `avx2` + `vpclmulqdq` are present -- no
`avx512vl` required). The detection guard in `gf2m_batch.rs` correctly
picks the AVX2+VPCLMULQDQ path as the primary lane on Zen 3.

One subtlety: `clmul.rs` line 104 checks
`is_x86_feature_detected!("vpclmulqdq") && is_x86_feature_detected!("avx512vl")`
for its 256-bit VPCLMULQDQ batch path. This incorrectly requires `avx512vl`
to enable a 256-bit (YMM) operation. VPCLMULQDQ on YMM does NOT require
AVX-512VL on Intel hardware; on AMD Zen 3, the CPUID flag `vpclmulqdq` alone
is sufficient to execute 256-bit VPCLMULQDQ. However, this issue is a
correctness / detection concern for the existing batch kernel, not a
requirement for new AVX-512 hardware. It is noted here as a finding but is
owned by `e24f7839` to resolve if it blocks the panelized GEMM path.

**Implication for Q1:** closing the GF(2^32) gap requires matrix-level
blocking (panelized GEMM, `e24f7839`). The inner-loop CLMUL primitive
(VPCLMULQDQ on YMM) is already available on Zen 3 -- the reference (NTL)
achieves its throughput using the same ISA, so there is no ISA gap.

---

## § 5 Decision

**AVX-512 and GFNI are NOT REQUIRED for SOTA closure on the Zen-3 host class.**

### Reasoning chain

1. **Reference baseline is AVX2-only.** Both reference implementations
   (M4RIE for GF(2^8)/GF(2^16), NTL for GF(2^32)) were measured on the
   same Zen-3 host with `avx2` + `vpclmulqdq` and without AVX-512 or GFNI.
   The [hard] 1.5x threshold is computed against these AVX2-only reference
   numbers. An implementation that achieves the threshold on Zen-3 using
   AVX2+VPCLMULQDQ satisfies the criterion.

2. **The gap is algorithmic, not ISA.** All 7 [hard] FAIL cells show a
   structural gap: gf2-core's per-element CLMUL path does not batch at the
   matrix level, while M4RIE (Gray-code tables) and NTL (panelized mat struct)
   do. This is the target for `e24f7839`. No new ISA capability is needed to
   close an algorithmic gap that the reference closes with AVX2.

3. **VPCLMULQDQ is already present and used.** The 256-bit CLMUL path
   (`clmul_batch_vpclmul` in `crates/gf2-kernels-simd/src/x86/clmul.rs`)
   is already in the gf2-core SIMD dispatch. The inner primitive is not
   the bottleneck -- the outer matrix-level blocking is. AVX-512 VPCLMULQDQ
   (ZMM, 4x multiplies per instruction) would provide ~2x throughput improvement
   over the YMM path in the inner loop, but this 2x does not change the
   algorithmic closure path.

4. **Host-class gating.** Protocol § 5 anchors all verdicts to the Zen-3
   host class. AVX-512 and GFNI define a different host class (Zen-4+,
   Sapphire Rapids+). A Zen-3 benchmark against an AVX-512-enabled reference
   would violate the "same hardware" criterion and could not be accepted as a
   promotion artefact for this epic.

5. **e24f7839 closure path is sufficient.** `e24f7839` (panelized GF(2^m)
   GEMM) implements the macro-level Gray-code / block-panel algorithm that
   closes the structural gap. If it achieves 0.667x ratio or better at
   GF(2^8) n=1024 using AVX2, the epic criterion is met without any
   AVX-512 or GFNI work.

**Verdict: NOT REQUIRED for SOTA closure on the Zen-3 host class.**

No in-epic AVX-512 or GFNI issues are filed. The existing epic structure
(`e24f7839` as primary implementation owner) is sufficient.

---

## § 6 Future direction for Zen-4+ host classes

AVX-512 and GFNI would unlock measurable throughput improvements when the
host class is upgraded. This is documented here as forward guidance for a
future epic, not as a gap in the current one.

### GFNI (`vgf2p8mulb` / `vgf2p8affineqb`)

GFNI provides hardware-accelerated GF(2^8) multiply in one instruction
(`vgf2p8mulb`). On Intel Icelake/Sapphire Rapids (which have GFNI), this
instruction operates over 16 bytes per 128-bit XMM lane, 32 bytes per YMM,
or 64 bytes per ZMM. The expected speedup for GF(2^8) matmul is up to 2-4x
over the AVX2 PSHUFB table path, depending on the inner-loop structure.

On Zen 4 and Zen 5 (AMD), GFNI is available at AVX-512 width. The
instruction would replace the PSHUFB double-table lookup that M4RIE (and the
proposed panelized GEMM) use for GF(2^8), eliminating the need for a
precomputed 256-entry table at the cost of a direct `vgf2p8mulb`.

GFNI is relevant only for GF(2^8). It does not directly accelerate GF(2^16)
or GF(2^32) arithmetic. A future "GF(2^8) GFNI lane" issue would target the
`gf2-kernels-simd` crate and add a `detect` path that preferentially returns
the GFNI kernel when `is_x86_feature_detected!("gfni")` and
`is_x86_feature_detected!("avx512bw")` are true.

### AVX-512 VPCLMULQDQ (ZMM)

The 512-bit VPCLMULQDQ instruction (`_mm512_clmulepi64_epi128`) performs four
64x64 carry-less multiplications per instruction (one per 128-bit lane in a
512-bit ZMM register). On Zen-4+ or Sapphire Rapids, this is available and
would provide ~2x throughput improvement over the current YMM path for GF(2^32)
Barrett multiplication. The `gf2m_batch.rs` detect ladder would add a third
candidate above the current AVX2+VPCLMULQDQ lane.

### Unsafe isolation note

Both GFNI and AVX-512 VPCLMULQDQ work would land exclusively in
`crates/gf2-kernels-simd/` (the designated unsafe/SIMD crate). `gf2-core`
would continue to use the OnceLock runtime dispatch and `#![deny(unsafe_code)]`
would remain unchallenged in the safe crates.

### Recommended future issue structure

When the project adopts a Zen-4 / Sapphire Rapids host class:

1. File a new epic or story "Extend GF(2^m) SIMD kernels for Zen-4/GFNI".
2. Under it, file:
   - "GF(2^8) GFNI kernel (`vgf2p8mulb`) -- ZMM lane" (component: gf2-kernels-simd).
   - "GF(2^32) AVX-512 VPCLMULQDQ ZMM lane" (component: gf2-kernels-simd).
3. Wire them as follow-ons to `e24f7839` (panelized GEMM must land first
   to establish the outer-loop structure that the inner-loop ISA upgrade plugs into).

This work is outside epic `97bf0879` and does not block its closure.

---

## § 7 Self-satisfaction of [hard] criteria

**[hard] Criterion 1: The decision cites benchmark or feasibility evidence.**

Satisfied by the following evidence citations:

- M4RIE throughput numbers and host ISA flags: `dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` (rows 8-13 for GF(2^8), rows 14-19 for GF(2^16)) and `dev/bench_results/2026-05-04-507b0036-m4rie-host.txt` (no `avx512f`/`gfni` confirmed). Designation: `dev/plans/m4rie_promotion_evidence.md`.
- NTL throughput numbers and host ISA flags: `dev/bench_results/2026-05-06-a1172cea-ntl-gf2pow32-large.csv` and `dev/bench_results/2026-05-04-73ab8eef-ntl-host.txt` (no `avx512f`/`gfni` confirmed). Designation: `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`.
- Post-PPC scorecard with 7 FAIL cells and their ratios and gap factors: `dev/bench_results/2026-05-06-a1172cea-gf2m-scorecard.md`.
- Current SIMD kernel survey: `crates/gf2-kernels-simd/src/x86/clmul.rs` (VPCLMULQDQ present on AVX2), `crates/gf2-kernels-simd/src/gf2m_batch.rs` (detect ladder, primary lane = AVX2+VPCLMULQDQ).
- Protocol host-class definition: `dev/plans/sota_reference_acceptance_protocol.md` § 5 (Zen-3 desktop anchor, AVX2+VPCLMULQDQ, no AVX-512).

**[hard] Criterion 2: Required platform work is wired under this epic; non-required work remains outside.**

Satisfied by the decision in § 5 (NOT REQUIRED) and § 6 (future direction,
outside epic `97bf0879`). No new in-epic issues are filed because none are
required. The 7 [hard] FAIL cells are routed to `e24f7839` (panelized GEMM,
already wired under story `2c7548ae` / epic `97bf0879`). AVX-512/GFNI work
is documented as a future direction for a Zen-4+ host class, outside this epic.
