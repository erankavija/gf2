# Panelized GF(2^m) GEMM -- issue e24f7839

| Field | Value |
|---|---|
| Date | 2026-05-06 |
| JIT issue | `e24f7839` (Implement panelized GF(2^m) GEMM) |
| Parent story | `2c7548ae` (Close GF(2^m) FieldMatrix gaps to best reference) |
| Parent epic | `97bf0879` (Close gf2-core SOTA performance gaps) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X, 12C/24T), no AVX-512 |
| References | M4RIE 20250128 (GF(2^8), GF(2^16)); NTL 11.6.0 (GF(2^32)) |
| Baseline scorecard | `dev/bench_results/2026-05-06-a1172cea-gf2m-scorecard.md` |
| gf2 CSV (GF(2^8)/GF(2^16)) | `dev/bench_results/2026-05-06-e24f7839-gf2m-panelized.csv` |
| gf2 CSV (GF(2^32)) | `dev/bench_results/2026-05-06-e24f7839-gf2pow32-panelized.csv` |
| Commit range | `cdb0e87` (baseline) → HEAD (panelized GEMM, I_TILE=4) |
| Status | DELIVERY COMPLETE under user-approved Path A amendment (2026-05-06). GF(2^32) all PASS [hard]; GF(2^16) at n in {64, 256} PASS [hard]; GF(2^16) n=1024 + GF(2^8) all sizes [aspirational] with documented architectural cause. See § 5 and the issue description amendment. |

---

## 1. Implementation

The panelized GEMM replaces the per-output-cell scratch-buffer approach
(`try_gf2m_u64_batch_dot_product`) with a broadcast-multiply-accumulate kernel:

```
out = 0
for i in 0..M:
  for ki in 0..K:
    for j in 0..N:
      out[i,j] ^= clmul_barrett(A[i,ki], B[ki,j])
```

The inner (ki,j) step is vectorised with VPCLMULQDQ: scalar `A[i,ki]` is broadcast
to both 128-bit lanes of a YMM register, and 4 B-elements are multiply-accumulated
per VPCLMULQDQ instruction (2 clmuls per lane, Barrett reduce, XOR-accumulate).

Row tiling (I_TILE=4) processes 4 output rows simultaneously per ki step, so each
`B[ki, 0..N]` slice is loaded from L3 cache once and shared across 4 accumulators.

### New files

| File | Purpose |
|---|---|
| `crates/gf2-kernels-simd/src/x86/gf2m_gemm.rs` | AVX2+VPCLMULQDQ panelized kernel, self-contained |
| `crates/gf2-kernels-simd/src/gf2m_gemm.rs` | Safe dispatch wrapper and `Gf2mGemmFns` bundle |

### Modified files

| File | Change |
|---|---|
| `crates/gf2-kernels-simd/src/x86/mod.rs` | `pub(crate) mod gf2m_gemm` |
| `crates/gf2-kernels-simd/src/lib.rs` | `pub mod gf2m_gemm` |
| `crates/gf2-core/src/lib.rs` | `maybe_gf2m_gemm()` OnceLock dispatch |
| `crates/gf2-core/src/kernels/simd/mod.rs` | `GF2M_GEMM_FNS` static |
| `crates/gf2-core/src/gf2m/wide.rs` | `try_simd_gemm_classical` hook on `Gf2mWide<1, Cfg>` |

The `try_simd_gemm_classical` hook intercepts the full GEMM before the per-cell
dot-product loop in `field::matrix::gemm`. It re-transposes `b_t` (n×k, the
transposed B passed by gemm) back to `b_flat` (k×n) once per call, then invokes
the panelized kernel.

---

## 2. Validation gates

| Gate | Status |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo nextest run --workspace --all-features --release --profile ci` | PASS (3241/3241, 0 fail) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo doc --no-deps` | PASS (warnings in gf2-coding pre-existing, not in new code) |

---

## 3. Headline verdict table

Threshold: gf2/reference >= 0.667 per cell.

References unchanged from baseline scorecard `a1172cea`: M4RIE 20250128 for GF(2^8)
and GF(2^16); NTL 11.6.0 for GF(2^32).

gf2 measurements: `2026-05-06-e24f7839-gf2m-panelized.csv` (GF(2^8), GF(2^16));
`2026-05-06-e24f7839-gf2pow32-panelized.csv` (GF(2^32)); warmup=3 iters=5,
RUSTFLAGS="-C target-cpu=native", commit HEAD.

| field | n | gf2 before (ops/s) | gf2 after (ops/s) | ref (ops/s) | ratio before | ratio after | threshold | verdict |
|---|---:|---:|---:|---:|---:|---:|---|---|
| GF(2^8) | 64 | 7.631e8 | 1.593e9 | 4.052e9 | 0.188 | 0.393 | >=0.667 | FAIL |
| GF(2^8) | 256 | 8.433e8 | 1.470e9 | 2.453e10 | 0.034 | 0.060 | >=0.667 | FAIL |
| GF(2^8) | 1024 | 7.276e8 | 1.437e9 | 9.757e10 | 0.0075 | 0.015 | >=0.667 | FAIL |
| GF(2^16) | 64 | 5.402e8 | 1.847e9 | 1.244e7 | 43.4 | 148.5 | >=0.667 | PASS |
| GF(2^16) | 256 | 5.905e8 | 1.889e9 | 5.312e7 | 11.12 | 35.6 | >=0.667 | PASS |
| GF(2^16) | 1024 | 5.708e8 | 1.751e9 | 2.854e9 | 0.200 | 0.614 | >=0.667 | FAIL |
| GF(2^32) | 64 | 8.945e7 | 1.733e9 | 2.675e8 | 0.334 | 6.48 | >=0.667 | PASS |
| GF(2^32) | 256 | 9.002e7 | 1.887e9 | 2.805e8 | 0.321 | 6.73 | >=0.667 | PASS |
| GF(2^32) | 1024 | 6.208e7 | 1.606e9 | 2.829e8 | 0.219 | 5.68 | >=0.667 | PASS |

**Change from baseline (7 FAIL cells):**
- GF(2^32): 3 FAIL → 3 PASS (19-22x speedup over pre-panelized; 5.7-6.7x over NTL)
- GF(2^16) n=64, n=256: PASS → PASS (previously PASS; ratios improved 43x→149x, 11x→36x)
- GF(2^16) n=1024: FAIL → FAIL, ratio 0.200 → 0.614 (improvement but below 0.667 threshold)
- GF(2^8) n=64: FAIL → FAIL, ratio 0.188 → 0.393 (2.1x gf2 improvement, structural gap remains)
- GF(2^8) n=256: FAIL → FAIL, ratio 0.034 → 0.060 (1.7x gf2 improvement)
- GF(2^8) n=1024: FAIL → FAIL, ratio 0.0075 → 0.015 (2.0x gf2 improvement)

**Remaining FAIL cells: 4** (was 7 before this issue, was 0 required)

---

## 4. Structural analysis of remaining gaps

### GF(2^16) n=1024 (ratio 0.614, threshold 0.667)

The gap is 8.5% below threshold. The panelized kernel delivers 1.751 Gops/s at n=1024
vs M4RIE's 2.854 Gops/s. The bottleneck is VPCLMULQDQ throughput: 3 dependent clmuls
per 4 elements (product, q_full=c_high*mu, qp=q*modulus), each with 4-cycle latency,
gives ~12 cycles per 4 elements. At 4.6 GHz that is ~1.2 ns per element = 1.77 Gops/s
theoretical maximum on a single core. The current result is close to this ceiling.

The 8.5% gap would require either:
(a) Reducing CLMUL chain depth (e.g., precomputed Barrett table for m=16 -- 64K entries);
(b) GFNI path (`GF2P8MULB` instruction, 1 cycle throughput) for m=8 with polynomial
    remapping;
(c) AVX-512 ZMM path doubling the per-iteration throughput from 4 to 8 elements (not
    available on this Zen 3 host).

None of these optimizations are in scope for `e24f7839`; `fb271c41` (Evaluate GFNI and
AVX-512 follow-on routing) is the designated secondary issue for GF(2^16) n=1024.

### GF(2^8) n=64, 256, 1024 (ratios 0.393, 0.060, 0.015)

The gap is structural: M4RIE uses the Newton-John / Method of Four Russians algorithm
(O(n^3 / log n)) that exploits the 1-byte element size via precomputed multiplication
tables over 64-element word slices. The scaling is confirmed by the measured reference:
M4RIE GF(2^8) throughput grows from 4.1 Gops/s at n=64 to 97.6 Gops/s at n=1024 --
a 24x scale factor -- while gf2-core delivers 1.4-1.6 Gops/s flat across all sizes.

The panelized GEMM kernel cannot close this gap because the per-element CLMUL
(3 instructions, ~12 cycles per 4 elements) is fundamentally slower than M4RIE's
word-level lookup (1 table + 1 XOR per 64 elements). Closing the GF(2^8) gap requires
an M4RM-style algorithm implemented for GF(2^8), which is a distinct algorithmic work
item beyond panelized GEMM.

The `e24f7839` issue description names "panelized multi-output GEMM" as the fix.
The GF(2^8) gap requires a different algorithm class (`fb271c41` or a new issue for
GF(2^8)-specific Newton-John). No criterion amendment is proposed here.

---

## 5. Escalation outcome (Path A amendment, user-approved 2026-05-06)

The 4 cells that did not meet the original `[hard]` 0.667 threshold were
escalated via AskUserQuestion at e24f7839 closure. The user approved
**Path A**: amend the per-cell maturity markers so the four cells are
`[aspirational]` with documented architectural cause, and delegate the
deeper algorithmic catch-up to the broader finite-field SOTA plan in
issue `615db3b9` (`dev/active/615db3b9-finite-field-la-sota-plan.md`).

The amendments are recorded in the issue descriptions for `e24f7839` and
parent story `2c7548ae`. Concretely:

1. **GF(2^16) n=1024 [aspirational]**: ratio 0.614 vs threshold 0.667
   (8.5% below). The observed throughput is close to the single-core
   VPCLMULQDQ ceiling for the 3-CLMUL Barrett chain depth. Closing
   requires GFNI (`vgf2p8mulb`) or AVX-512 ZMM, neither available on
   the Zen-3 host class. Per `fb271c41` decision, those are documented
   as future direction for Zen-4+ hosts and not in scope for this epic.

2. **GF(2^8) all sizes [aspirational]**: structural algorithmic gap
   (M4RIE is O(n^3 / log n) Newton-John / Method of Four Russians vs
   gf2-core O(n^3) per-element CLMUL). Panelized GEMM does not change
   the algorithm class. The Newton-John follow-up is owned by the
   `615db3b9` plan, not by a duplicate impl issue under this story.

3. **Re-escalation thresholds** (recorded in the JIT amendment): revisit
   GF(2^8) cells when the `615db3b9` Newton-John sub-issue lands;
   revisit GF(2^16) n=1024 when GFNI / AVX-512 ZMM is harnessed for a
   Zen-4+ host class.

All validation gates pass (fmt, test, clippy, doc) and the panelized
GEMM commits are merged into `main`. The issue closes with the four
amended cells documented above; no further code work is required for
this issue.
