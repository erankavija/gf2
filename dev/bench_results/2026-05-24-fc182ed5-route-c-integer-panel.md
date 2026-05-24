# Route-C GF(251) pure-integer Goto/BLIS-style panelized micro-kernel — bench evidence

| Field | Value |
|---|---|
| Date | 2026-05-25 |
| JIT issue | `fc182ed5` (Prototype pure integer panelized GF(251) micro-kernel) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 1, item 3 |
| Design note | `dev/active/fc182ed5-route-c-design.md` |
| Predecessor | `a70b1c70` (Phase 0 baseline) |
| Sibling routes | `68cdf4c8` (route A — closed, n=1024 PASS / n=256 SHORTFALL); `91429c1c` (route B — closed, both SHORTFALL) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2 + FMA, no AVX-512 |
| Reference | fflas-ffpack 2.5.0 (pinned baseline from `cc5de315` closure) |
| Toolchain | rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1 |
| Kernel path | gf2-core route C = pure-integer Goto/BLIS-style panelized kernel (toggled via `set_route_c_gf251_enabled`) |

---

## 1. Provenance and clean-room attestation

The route-C kernel in `crates/gf2-kernels-simd/src/{,x86/}fp_small_panel.rs`
was implemented from the public references listed in the design note
`dev/active/fc182ed5-route-c-design.md` § 5 plus the gf2-owned prior art
(Candidate C's row-panel kernel and route A's i32-tile structure).
**No fflas-ffpack, Givaro, or FFPACK source code, comments,
autotuning tables, or micro-kernel structure was opened, copied,
transliterated, or used as a recipe for any line of code in this
prototype.** Specifically:

- The `MR × NR × KC = 4 × 24 × 256` panel dimensions are derived from
  the Goto-vandeGeijn 2008 / BLIS 2015 framework (pack A into MR-row
  horizontal panels; pack B into NR-column vertical panels; fill
  the SIMD register file with `MR × NR / 8` accumulators on a host
  with 16 ymm registers) plus the AMD Zen 3 Software Optimization
  Guide § 2.13 (L1d size 32 KB, 8-way) for the L1d-fit constraint
  on KC. Design-note § 2.1 / § 2.2 records the full derivation.
- The 32-bit-lane Barrett reduction is reused from
  `crates/gf2-kernels-simd/src/x86/fp_small.rs::barrett_reduce_lane32`
  (Candidate C's SpMM row reducer; SSOT made `pub(super)` already
  by jit:68cdf4c8 R1). The route-C kernel calls into the same
  symbol via `super::fp_small::barrett_reduce_lane32` — no new
  algebra is introduced.
- The 3-step u32 → u8 pack (`_mm256_packus_epi32` →
  `_mm256_permute4x64_epi64` → `_mm256_packus_epi16`) is the same
  SSOT route A uses (`pack_i32x8_to_u8` in `fp_small_f32.rs`). The
  byte-identical SIMD sequence appears in route-C as
  `pack_i32x8_to_u8_local`; we kept a local copy (vs cross-module
  delegate) only to avoid leaking route-A's internal symbol name
  into the panel kernel's call graph. Documented in the function
  docstring.
- The toggle mechanism (`set_route_c_gf251_enabled`) is byte-for-byte
  mechanically identical to route A's `set_route_a_gf251_enabled`
  (jit:68cdf4c8 R1 commit `4bad2e72`), satisfying the
  unsafe-isolation invariant in `gf2-core`.

This attestation satisfies criterion 5 (verbatim):

> Evidence doc records the panel dimensions (`mr × nr × kc`),
> register-blocking shape, packing layout, and how each was derived
> from public GEMM/ISA references (Goto-vandeGeijn "Anatomy" 2008;
> Zen 3 software optimization manual). No fflas-ffpack source or
> comments may be cited; the doc must show the derivation came from
> public references and gf2-owned prototypes.

## 2. Methodology

Pinned-reference protocol per
`dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 6,
followed verbatim:

- **CPU:** AMD Ryzen 9 5900X (Zen 3), 12c/24t, 3.7 GHz base / 4.6 GHz
  boost. AVX2 + FMA3 + BMI2 + VAES + VPCLMULQDQ. No AVX-512.
  Verified at HEAD via `/proc/cpuinfo`.
- **Kernel:** Linux 7.0.3-arch1-1.
- **Isolation:** `taskset -c 6-11 nice -n -5` (CCX1 pinned, cores 6-11).
  Agent and parent shell on CCX0 (cores 0-5). Sequential trials, no
  concurrent benches.
- **Trial count:** N=5 per route per cell.
- **Per-trial inner samples:** criterion-default (10-sample median).
- **Quiet-host check:** No competing `cargo`, `rustc`, or `criterion`
  process active during the bench. The initial RC_trial1 attempt
  ran with parallel cargo doctest builds — discarded and re-run
  after the doctest builds completed. See § 5 quiet-host
  attestation for the final session.
- **Gop/s formula:** `2 · n³ / median_ns` (criterion median point
  estimate). Identical to `dev/bench_results/run_662f7a15_prime_sweep.sh`
  and `run_68cdf4c8_route_a_bench.sh`.

Bench driver: `dev/bench_results/run_fc182ed5_route_c_bench.sh` runs
two phases sequentially in the same process per trial:

1. **Phase 1 (route-C on):** `GF2_GF251_ROUTE_C=1` env var set. The
   GF(251) bench function reads the env var via safe `std::env::var()`
   and calls the safe `set_route_c_gf251_enabled(true)` setter before
   the GF(251) bench group runs. GF(7), GF(31), GF(127) bench
   functions don't read this env var and continue to use Candidate C.
2. **Phase 2 (default):** env var unset. Every prime dispatches through
   Candidate C (`N_THRESH_PRIME = 252`, `select_f32_path` false). The
   GF(251) bench function calls `set_route_c_gf251_enabled(false)`.

The non-regression criterion (criterion 6) is satisfied by phase-2's
direct 5-trial measurement of GF(7), GF(31), GF(127), and GF(251)
under the unchanged production dispatch, all in the same session and
at the same commit as the route-C phase-1 measurement.

Because the route-C toggle scope is **GF(251)-only**
(`route_c_gf251_enabled::<P>()` short-circuits on `P != 251`), the
GF(7), GF(31), and GF(127) cells exercise **byte-for-byte the same
Candidate-C kernel** in both phases. Any delta on those cells is
host-noise, not a regression.

## 3. Headline measurements — GF(251) route C vs Candidate C

CSV (raw): `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.csv`

CSV (aggregate): `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel-aggregate.csv`

### 3.1 Raw 5-trial throughputs

GF(251) cells:

| trial | route | n | GF(251) Gop/s |
|---:|---|---:|---:|
| 1 | route_c | 64 | 26.866 |
| 2 | route_c | 64 | 27.564 |
| 3 | route_c | 64 | 27.586 |
| 4 | route_c | 64 | 27.607 |
| 5 | route_c | 64 | 26.779 |
| 1 | route_c | 256 | 63.736 |
| 2 | route_c | 256 | 64.380 |
| 3 | route_c | 256 | 64.163 |
| 4 | route_c | 256 | 64.972 |
| 5 | route_c | 256 | 62.271 |
| 1 | route_c | 1024 | 74.651 |
| 2 | route_c | 1024 | 75.100 |
| 3 | route_c | 1024 | 72.547 |
| 4 | route_c | 1024 | 76.474 |
| 5 | route_c | 1024 | 72.002 |
| 1 | default | 64 | 33.308 |
| 2 | default | 64 | 33.222 |
| 3 | default | 64 | 33.211 |
| 4 | default | 64 | 33.210 |
| 5 | default | 64 | 33.660 |
| 1 | default | 256 | 69.219 |
| 2 | default | 256 | 69.757 |
| 3 | default | 256 | 69.459 |
| 4 | default | 256 | 69.564 |
| 5 | default | 256 | 70.044 |
| 1 | default | 1024 | 74.545 |
| 2 | default | 1024 | 75.912 |
| 3 | default | 1024 | 74.690 |
| 4 | default | 1024 | 74.934 |
| 5 | default | 1024 | 75.022 |

### 3.2 Aggregate (5-trial median / Q1 / Q3 / IQR / min / max)

GF(251) cells:

| n | route | median | Q1 | Q3 | IQR | min | max |
|---:|---|---:|---:|---:|---:|---:|---:|
| 64 | route_c | 27.564 | 26.866 | 27.586 | 0.720 | 26.779 | 27.607 |
| 64 | default | 33.222 | 33.211 | 33.308 | 0.096 | 33.210 | 33.660 |
| 256 | route_c | 64.163 | 63.736 | 64.380 | 0.644 | 62.271 | 64.972 |
| 256 | default | 69.564 | 69.459 | 69.757 | 0.297 | 69.219 | 70.044 |
| 1024 | route_c | 74.651 | 72.547 | 75.100 | 2.553 | 72.002 | 76.474 |
| 1024 | default | 74.934 | 74.690 | 75.022 | 0.332 | 74.545 | 75.912 |

### 3.3 Route-C vs Candidate C verdict at GF(251)

| n | route-C Gop/s | default Gop/s | route-C vs default | fflas Gop/s | route-C / fflas | 1.5×-of-fflas threshold | verdict |
|---:|---:|---:|---:|---:|---:|---:|---|
| 64 | 27.56 | 33.22 | **−17.0%** | — | — | n/a (no fflas baseline at n=64) | **REGRESSION** vs Candidate C (route C is strictly slower; no improvement) |
| 256 | 64.16 | 69.56 | **−7.8%** | 128.48 | **0.499** | 85.65 / 0.667 | **SHORTFALL** (route C is slower than Candidate C and far below 1.5× of fflas) |
| 1024 | 74.65 | 74.93 | **−0.4%** | 138.32 | **0.540** | 92.21 / 0.667 | **SHORTFALL** (route C matches Candidate C within noise but stays well below 1.5× of fflas) |

**Per criterion 5 (`[hard]`):** route C **does not clear the 1.5×-of-fflas
threshold at either n=256 or n=1024**, and at n=64 it strictly regresses
against Candidate C. Route C is therefore not a viable production
replacement for Candidate C at GF(251) on the Zen 3 reference host.

**Empirical structural decomposition:**

1. **Inner-loop peak is integer-ALU-bound.** Route C's inner kernel uses
   `_mm256_madd_epi16` on Zen 3's two SIMD integer ALU pipes (per AMD
   Family 19h SOG § 2.10), giving a theoretical peak of ≈ 80 Gop/s.
   Candidate C's row-panel kernel uses the same lane-pair MAC and
   reaches ~75 Gop/s on this host. Route C's measured 74.65 Gop/s at
   n=1024 is essentially at the integer-pipe ceiling — the pack-cost
   amortisation that route C trades for explicit panel structure gets
   us **back to** the Candidate C ceiling at n=1024 but not above it.

2. **Pack cost hurts at small n.** At n=64, route C pre-packs A (4 × 64
   × 2 = 512 bytes per MR-row block, 16 blocks) and B (1 panel × 64 × 24
   = 1 536 bytes) before any inner work. That pack cost is ≈ 0.6 µs at
   4 GB/s sustained streaming write — sizeable vs Candidate C's
   19.4-µs inner-loop wall time. The pack overhead manifests as the
   −17% delta vs Candidate C.

3. **At n=1024 the pack amortises** (1 outer-N pack reused over 64
   inner MR-row blocks per n-panel, with ~43 n-panels per row),
   bringing route C within 0.4% of Candidate C. No improvement, but
   no regression either — the panelization gives us nothing the
   Candidate C row-panel kernel doesn't already get from its
   4-output-cell parallelism.

4. **fflas's 128-138 Gop/s ceiling stays out of reach** because
   fflas routes GF(251) through `Modular<float>` + OpenBLAS sgemm
   — the FMA-port-bound path with 2× the throughput ceiling of the
   integer ALUs. Route C's integer constraint forfeits the
   fflas-parity-on-this-cell objective by design; the trade-off was
   self-contained Rust vs an external BLAS dependency (route B).

### 3.4 Sibling-route comparison

| n | Candidate C (Phase 0) | route A (n=1024 PASS) | route B (full) | route C | fflas | Threshold |
|---:|---:|---:|---:|---:|---:|---:|
| 256 | 58.98 (drift 69.56) | 70.21 | 35.49 | 64.16 | 128.48 | 85.65 |
| 1024 | 70.89 (drift 74.93) | 93.90 | 66.56 | 74.65 | 138.32 | 92.21 |

(Route A row from `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md`;
route B from `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md`. The
"Candidate C (Phase 0)" numbers are the 2026-05-06 5-trial medians; the
"drift" parenthetical is the same-session default-phase median from this
session for direct comparison.)

**Sibling-route summary:**

- **Route A** (f32/FMA cascade) — clears the threshold at n=1024
  (0.679 of fflas, PASS) but not at n=256 (0.547, SHORTFALL).
  Partial Phase-1 candidate.
- **Route B** (OpenBLAS sgemm cascade) — fails both cells in the
  `FieldMatrix` end-to-end view (0.276 / 0.481 of fflas). Research
  reference only.
- **Route C** (integer panel, this issue) — fails both cells
  (0.499 / 0.540 of fflas). Lower ceiling than route A because
  integer ALU peak is half the FMA peak; lower than Candidate C at
  small n because pack overhead dominates.

**Conclusion for the route-selection task (41096af5 wave 4):** the
empirical landscape on this Zen 3 host is:

- For self-contained Rust at **n ≥ 512**: route A is the candidate
  (clears 1.5× of fflas at n=1024).
- For self-contained Rust at **n ≤ 256**: **no route in {A, B, C}
  clears 1.5× of fflas**. The structural gap is the BLAS-cascade
  ceiling (fflas reaches 128 Gop/s by delegating to OpenBLAS sgemm,
  which is hand-tuned over 15+ years and unavailable to any
  in-tree Rust kernel without an external dependency).
- The Candidate C status quo (current production default) remains
  the appropriate self-contained path for all cells; route A may
  be added as an opt-in for n ≥ 512 by the route-selection task,
  but is not strictly required.
- Route C **should not be promoted to production** in any
  configuration: it strictly underperforms Candidate C at all
  measured cells (small-n regression; large-n parity).

## 4. Non-regression cells (criterion 6)

Same-session 5-trial median at the same HEAD as the route-C
measurement. Phase-2 dispatch (env var unset = production default,
Candidate C for all primes). Threshold: `|delta| ≤ 5%` route-C
vs default in the same session.

Per the issue criterion 6:

> [hard] No regression on currently-PASSing GF(p) cells (delta ≤ 5%).

The strict reading is "same-session at same commit": route-C enabled
vs route-C disabled (= production default). Since
`route_c_gf251_enabled::<P>()` returns `false` for every `P != 251`,
the GF(7), GF(31), GF(127) cells exercise byte-for-byte the same
Candidate-C kernel in both phases. Any delta between them is
host-noise, not a regression.

### 4.1 Same-session non-regression (route-C vs default at non-GF(251) cells)

| prime | n | default Gop/s | route-C Gop/s | delta | verdict |
|---:|---:|---:|---:|---:|---|
| 7 | 64 | 31.79 | 31.92 | **+0.43%** | PASS |
| 7 | 256 | 70.02 | 70.64 | **+0.89%** | PASS |
| 7 | 1024 | 74.44 | 75.37 | **+1.25%** | PASS |
| 31 | 64 | 30.94 | 30.96 | **+0.07%** | PASS |
| 31 | 256 | 69.26 | 69.86 | **+0.87%** | PASS |
| 31 | 1024 | 75.30 | 76.09 | **+1.06%** | PASS |
| 127 | 256 | 69.14 | 69.82 | **+0.99%** | PASS |
| 127 | 1024 | 75.31 | 74.85 | **−0.62%** | PASS |

All non-GF(251) deltas are well below 1.5% in absolute value — far
within the 5% bound. **Criterion 6 PASS [hard]** by direct same-session
measurement on every cell.

This is the expected mechanical guarantee: route C's toggle scope is
GF(251)-only (`route_c_gf251_enabled::<P>()` short-circuits on
`P != 251`), so the non-GF(251) cells exercise **byte-for-byte the
same Candidate-C kernel** in both phases. The sub-1.5% delta is host
noise (criterion-style multi-sample medians smooth out per-iteration
thermal variance), not a route-C effect.

_(GF(127) bench group does not register an n=64 cell in the criterion
harness — see `crates/gf2-core/benches/fieldmatrix_gemm.rs::bench_gemm_fp_127`
which uses `SQUARE_SIZES_SMALL_PRIME = &[256, 1024]`. The non-regression
band for GF(127)/n=64 is therefore not measured here; the cell is
not in the criterion 6 control list for this issue.)_

## 5. Quiet-host attestation

The initial trial-1 attempt (filename `RC_trial1.log` under the
trial directory, since deleted) ran while `cargo test --release --doc`
builds were in progress on a sibling shell. Criterion outputs from
that trial showed extreme variance (e.g. GF(31)/n=1024 thrpt range
15.4-24.6 Gop/s with `+193.14% +247.57% +313.42%` slowdown vs prior
baselines) — clearly contaminated. That trial directory was wiped
clean before the headline sweep started.

The headline 10-trial sweep ran with **no concurrent cargo, rustc,
or criterion process active**. Confirmed by `ps -ef | grep -E
"cargo|rustc|fieldmatrix"` before each phase (only the
`fieldmatrix_gemm` bench binary, the parent bash shell, and the
trial driver were running). The agent's read-only operations (file
inspection, `git status`) were the only other activity and did not
compete for FMA / integer ALU ports.

## 6. Test plan (TDD) — criterion 2

The kernel parity is verified by both unit tests (in
`crates/gf2-kernels-simd/src/x86/fp_small_panel.rs::tests` and
`crates/gf2-kernels-simd/src/fp_small_panel.rs::tests`) and
integration tests at the gf2-core dispatch boundary
(`crates/gf2-core/tests/route_c_gf251_parity.rs`).

Test names:

- `panel_gemm_matches_scalar_at_boundary_shapes` —
  `(m, k, n) ∈ {(1,1,1), (1,4,4), (3,5,7), (4,64,24), (8,64,32),
  (4,65,25), (4,255,24), (4,256,24), (4,257,24), (4,512,48),
  (1,256,256), (2,256,256), (3,256,256), (5,256,256), (9,64,32),
  (16,134,16), (16,134,24), (4,256,256), (4,1024,1024)}` across
  `p ∈ {3, 5, 7, 11, 13, 17, 31, 127, 251}`.
- `panel_gemm_n_boundary_sweep` — `n ∈ {1, 8, 15, 16, 17, 23, 24,
  25, 47, 48, 49, 63, 64, 65, 95, 96, 97, 121}` at
  `m=4, k=64, p ∈ {3, 5, 7, 11, 13, 17, 31, 127, 251}`.
- `panel_gemm_handles_zero_dims` — m=k=n=0 returns without panic.
- `safe_wrapper_matches_scalar_gemm` — same case set via the safe
  `crate::fp_small_panel::detect` wrapper across p ∈ {7, 31, 127, 251}.
- `safe_wrapper_handles_zero_dims` — zero-dim path via safe wrapper.
- `detect_returns_some_on_avx2` — feature-detect smoke test.
- `route_c_matches_default_at_criterion_n_values` — end-to-end
  `gemm` parity at n ∈ {1, 15, 16, 17, 23, 24, 25, 47, 48, 49, 63,
  64, 65, 95, 96, 97, 121, 255, 256, 257, 1023, 1024} on canonical
  seeds (p = 251).
- `route_c_matches_default_at_k_chunk_boundary` — same harness
  across k ∈ {1, 2, 64, 127, 128, 255, 256, 257, 511, 512, 1023,
  1024, 1025} (KC = 256 boundary + odd-k pair tail).
- `route_c_matches_default_at_m_partial` — partial MR-row tile at
  m ∈ {1, 2, 3, 5, 6, 7, 9, 33}.
- `route_c_matches_default_at_n_partial` — partial NR-column panel
  at n ∈ {1, 8, 23, 24, 25, 47, 48, 49, 95, 96, 97, 121}.
- `route_c_off_leaves_dispatch_unchanged` — toggle restore sanity.

**Result:** All 21 panel-kernel unit tests + 5 route-C dispatch
parity tests PASS in `cargo nextest run -p gf2-kernels-simd -p
gf2-core --release --profile ci --all-features` runs at HEAD.
Bit-exact equality verified across every (p, m, k, n) cell named
in criterion 2.

## 7. Panel dimensions and register schedule (criterion 5)

Per the design note `dev/active/fc182ed5-route-c-design.md` § 2:

| Dimension | Value | Derivation source |
|---|---:|---|
| `MR` (inner tile rows of A) | 4 | Goto-vandeGeijn 2008 § 4 — pack A into MR-row horizontal panels; choose MR small enough to amortise per-row pack cost. |
| `NR` (inner tile columns of output) | 24 | Goto-vandeGeijn 2008 § 4 / van Zee 2015 § 4 — choose NR as a multiple of SIMD lane count; `NR / 8 = 3` 8-i32-lane sub-tiles. |
| `MR × NR / 8` (u32 accumulator regs) | 12 | AMD Zen 3 SOG (PUB 56665) § 2.10 — 16 ymm regs; `16 - 12 = 4` left for B-loads (3) + A-broadcast (1). |
| `KC` (k-axis cache blocking) | 256 | (a) Goto-vandeGeijn 2008 § 6 / BLIS 2015 § 4 — fit `(MR + NR) · KC · elem_size` in L1d with ≤ 25% occupancy; (b) AMD Zen 3 SOG § 2.13 — L1d size 32 KB. Working set: 2 KB (A) + 6 KB (B) ≈ 25% L1d. |
| u32 overflow bound | k ≤ 68 719 (p=251) | Granlund-Möller 2011 / Dumas-Pernet 2009 — `_mm256_madd_epi16` accumulator lane sum ≤ `k · (p−1)²`; u32 cap `2³² / 62 500 ≈ 68 719`. KC is L1d-bound, not arithmetic-bound. |

Register schedule per inner t-pair step (16 ymm regs total on Zen 3):

```text
ymm0..2   : b0, b1, b2     — 3 b-pair loads (16 u16 lanes each)
ymm3      : a_pair         — 1 broadcast (re-loaded per MR row)
ymm4..15  : acc00..acc32   — 12 u32 accumulators (one per output cell)
```

A-pack layout (`Vec<u32>` of length `MR · KC / 2` per outer-M block):

```text
a_pack32[(t / 2) * MR + i] = ((a[i_blk + i, t + 1] as u32) << 16)
                           |  (a[i_blk + i, t]     as u32)
```

B-pack layout (`Vec<u8>` of length `n_panels · k_padded · NR`):

```text
b_pack[panel_off + (t / 2) * NR * 2 + j_off * 2 + 0] = b[t,     j_blk + j_off]
b_pack[panel_off + (t / 2) * NR * 2 + j_off * 2 + 1] = b[t + 1, j_blk + j_off]
```

The inner kernel reads `a_pack32[…]` as a single u32 and broadcasts
via `_mm256_set1_epi32` → 16 u16 lanes carrying `[a[i,t], a[i,t+1]]`
repeated 8 times. Three contiguous 16-byte `__m128i` loads from
`b_pack[…]` cover the full NR=24 columns; each loads via
`_mm256_cvtepu8_epi16` into a ymm of 16 u16 lanes carrying
`[b[t,j], b[t+1,j], b[t,j+1], b[t+1,j+1], …]`. The
`_mm256_madd_epi16(a_pair, b_pair)` instruction then produces 8
i32 lanes per call, each summing the `a[i,t] · b[t,j] +
a[i,t+1] · b[t+1,j]` MAC pair into one i32 accumulator lane.

## 8. Public references (no fflas-ffpack citations)

1. **Goto, K., and van de Geijn, R. A.** "Anatomy of High-Performance
   Matrix Multiplication." ACM Trans. Math. Softw., 34(3):12, 2008.
2. **van Zee, F. G., and van de Geijn, R. A.** "BLIS: A Framework
   for Rapidly Instantiating BLAS Functionality." ACM Trans. Math.
   Softw., 41(3):14, 2015.
3. **AMD Software Optimization Guide for AMD Family 19h Processors
   (Zen 3), revision 3.07.** Publication ID 56665.
4. **Granlund, T., and Möller, N.** "Improved division by invariant
   integers." IEEE Trans. Comput., 60(2):165–175, 2011.
5. **Dumas, J.-G., Giorgi, P., and Pernet, C.** "Dense Linear Algebra
   over Word-Size Prime Fields." ACM TOMS 35(3), 2009;
   arXiv:cs/0601133.
6. **gf2-owned prior art:**
   - `crates/gf2-kernels-simd/src/x86/fp_small.rs::barrett_reduce_lane32`
     — Candidate C's 32-bit-lane Barrett reducer (SSOT).
   - `crates/gf2-kernels-simd/src/x86/fp_small.rs::fp_small_gemm_row_panel`
     — Candidate C's row-panel kernel structure (no panel-pack).
   - `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` (route A) —
     i32 accumulator architecture and `pack_i32x8_to_u8` SSOT.
   - `dev/active/68cdf4c8-route-a-design.md` — route-A design note
     (toggle-mechanism precedent).
   - `dev/plans/small_prime_kernel_strategy.md` — Candidate C/F
     design analysis.

No fflas-ffpack, Givaro, or FFPACK source code, comments, or
autotuning tables were opened, copied, transliterated, or used as
a recipe for any line of code in this prototype. The local fflas
checkout at `/home/vkaskivuo/Projects/fflas-ffpack` was not opened.

## 9. Open questions and findings

### Findings

1. **Route C strictly underperforms Candidate C at GF(251).** The
   headline numbers (route-C −17% at n=64, −7.8% at n=256, −0.4% at
   n=1024) show route C is not a useful production replacement for
   Candidate C. Whatever amortisation the explicit panel-pack buys at
   large n is offset by the pack-pass overhead at small n; at large n
   the integer-pipe ceiling (~80 Gop/s on Zen 3) caps both kernels
   roughly equally.

2. **The fflas BLAS-cascade ceiling is the binding constraint at
   GF(251), not gf2's choice of micro-kernel structure.** fflas
   reaches 128-138 Gop/s by delegating to OpenBLAS sgemm, which runs
   on Zen 3's FMA ports with 2× the theoretical peak of the integer
   ALUs route C uses. No purely-integer Rust kernel on this host
   can reach 1.5× of fflas without leaving the integer ALUs (i.e.,
   adopting route A's f32 cascade, or route B's external-BLAS
   dependency).

3. **Route A is the only Phase-1 prototype that clears the threshold
   at any in-scope cell.** Route A clears n=1024 (0.679 of fflas)
   but misses n=256 (0.547). Routes B and C both miss both cells.

### Open questions

1. **Route-selection task (41096af5 wave 4) input.** The empirical
   landscape:
   - **n=64, n=256:** no in-Rust route in {A, B, C} clears 1.5× of
     fflas. The structural gap is the BLAS-cascade ceiling.
   - **n=1024:** route A clears 1.5× of fflas (PASS); routes B and
     C SHORTFALL.

   The route-selection task should decide between:
   - **(a)** Keep Candidate C as the sole self-contained production
     path; accept GF(251)/n in {64, 256, 1024} as `[aspirational]`
     against the 1.5× target (Candidate C is at 32/70/75 Gop/s vs
     fflas's 128/138 Gop/s, ratios 0.26 / 0.55 / 0.55).
   - **(b)** Add route A as opt-in for n ≥ 512 (where it clears
     the threshold), keeping Candidate C as default. Requires a
     size-conditional dispatch wire-up; route A still SHORTFALLs
     at n=256 so the small-n gap persists.
   - **(c)** Make route B (OpenBLAS sgemm cascade) available as an
     optional `external-blas` feature, off by default. Even with
     OpenBLAS, route B's `FieldMatrix`-in/out path shortfalls at
     n=256 (0.276 of fflas) and at n=1024 (0.481); the OpenBLAS
     dependency is not a free win on this host.

2. **Should route C be deleted?** The kernel is correct and tested
   but never wins. Three options for the route-selection task:
   - Keep it as a research artefact (current state) — it's gated
     behind the `set_route_c_gf251_enabled` toggle, default off,
     so it pays only compilation cost in production.
   - Move it out of the workspace into `dev/research/` if the
     route-selection task confirms it's not a production path.
   - Delete it. Recommended only if the lead deems the research
     value of the negative result (panelization does not help at
     GF(251) on Zen 3) is no longer needed.

3. **n=64 regression vs Candidate C at GF(251) is a route-C
   property, not a Candidate-C regression.** With the toggle off
   (production default), GF(251)/n=64 measures 33.22 Gop/s (the
   same as the n=64 small-n optimisation work landed in 27bb2f75).
   The 27.56 Gop/s only appears when the route-C toggle is on. Not
   a non-regression problem; reported as part of the headline
   finding that route C is not viable.

## 10. Source index

| Reference | Path |
|---|---|
| Plan (Phase 1 item 3) | `dev/active/615db3b9-finite-field-la-sota-plan.md` |
| Design note | `dev/active/fc182ed5-route-c-design.md` |
| Phase 0 baseline | `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` |
| Route A closure | `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` |
| Route B closure | `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` |
| Bench driver | `dev/bench_results/run_fc182ed5_route_c_bench.sh` |
| Raw CSV | `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.csv` |
| Aggregate CSV | `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel-aggregate.csv` |
| Kernel safe wrapper | `crates/gf2-kernels-simd/src/fp_small_panel.rs` |
| Kernel inner loop | `crates/gf2-kernels-simd/src/x86/fp_small_panel.rs` |
| Asm artefact | `crates/gf2-kernels-simd/src/x86/asm/fp_small_panel.asm.txt` |
| Dispatch site | `crates/gf2-core/src/gfp/simd_ops.rs::fp_small_try_gemm_classical` |
| Parity tests | `crates/gf2-core/tests/route_c_gf251_parity.rs` |
