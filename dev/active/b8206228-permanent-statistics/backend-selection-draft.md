# Draft backend selection receipt

Status: draft. This file is the single multi-cell selection receipt bound by the draft manifest. It carries one addressable cell decision per row; unresolved rows carry a placeholder backend and a freeze-blocking status. A final freeze replaces this draft with a committed receipt whose bytes are hashed into `CellSpec.backend_receipt`.

## Selection contract

The controlling contract is `dev/simulation_results/permanent-zero-fraction/protocol.md`, section `Backend freeze`. Rule 1 requires shared per-matrix behavioral equivalence and validation-anchor passage. Rule 2 requires host/build safety, capability, launch-duration, and resource conditions. Rule 3 requires a cell-applicable composite draw-pack-evaluate-count receipt; ranking is within one matched cohort only. The exact 296a41c9 cohort uses four configurations, twelve fresh processes per configuration, one initial 90-second machine warm-up, at least 3 seconds of per-configuration warm-up, at least five timed repetitions and five timed seconds, and a fixed 120-second per-process cap.

## Raw receipts considered

The committed paths and SHA-256 values are listed in [`receipt-inventory.md`](receipt-inventory.md). The 296a41c9 canonical raw receipt is `dev/benchmarks/permanent_campaign/backend-ordering.csv`; the narrative is `backend-ordering.md`. The determinant companion is `dev/benchmarks/permanent_campaign/determinant-cost.csv`, and the b488f02c and GPU-study artifacts provide older, differently timed, or supporting evidence.

## Candidate provenance for the dischargeable cohort

| Candidate | Host / accelerator | Source and compiler | Build, binary, workers | Workload, warm-up, repetitions | Rule-1 / Rule-2 evidence |
|---|---|---|---|---|---|
| `accelerator` at $(q,n)=(3,28)$ | AMD Ryzen 9 5900X; AMD Radeon RX 6950 XT `gfx1030`; ROCm 7.2.4; driver/kernel `7.1.6-arch1-1` | harness `414d31f8`; Rust 1.95.0; HIP build | release + `hip`; binary `6e24533cfbac987a0cec20af02f9dfb0a7bd9ce12c9e80cbdc00cad72150ccad`; one GPU worker; powersave | batch $M=1024$; initial locked 90 s machine warm-up, then per-process configuration warm-up; 3 timed repetitions; 12 fresh processes | q3 shared equivalence rows include n=28 with zero mismatches; backend-ordering records the capability and resource state |
| `intra_matrix_parallel` at $(q,n)=(3,28)$ | AMD Ryzen 9 5900X; 24 logical CPUs; AVX2; no AVX-512F; powersave | harness `414d31f8`; Rust 1.95.0 | release + `hip`; same binary hash; 24 Rayon workers; same lock | cell $M=96$; same locked warm-up discipline; 5 timed repetitions; 12 fresh processes | q3 shared equivalence rows include n=28 with zero mismatches; full-host Rayon and lock conditions are recorded |
| `batch_parallel` at $(q,n)=(5,24)$ | AMD Ryzen 9 5900X; 24 logical CPUs; AVX2; no AVX-512F; powersave | harness `414d31f8`; Rust 1.95.0 | release + `hip`; same binary hash; 24 Rayon workers; same lock | batch $M=96$; same locked warm-up discipline; 5 timed repetitions; 12 fresh processes | q5 shared equivalence evidence and backend-ordering capability record |
| `accelerator` at $(q,n)=(7,20)$ | AMD Ryzen 9 5900X; AMD Radeon RX 6950 XT `gfx1030`; ROCm 7.2.4; driver/kernel `7.1.6-arch1-1` | harness `414d31f8`; Rust 1.95.0; HIP build | release + `hip`; same binary hash; one GPU worker; powersave | batch $M=1024$; same locked warm-up discipline; 5 timed repetitions; 12 fresh processes | q7 n=20 shared equivalence/resource evidence; packed CPU paths above n=16 are excluded by capability |

## Study evidence excluded from freeze selection

The q3, q5, and q7 GPU-study grids remain committed factual evidence. Their candidate rows use source revision `292320d5e7d1fefd6b94262d0960e40c8ea35ba1`, Rust 1.95.0 (`rustc 1.95.0 (59807616e 2026-04-14)`), HIP `7.2.53211-9999` / AMD clang `22.0.0git`, release + `hip`, and the AMD Ryzen 9 5900X / Radeon RX 6950 XT `gfx1030` host under `powersave` with ROCm 7.2.4 and kernel `7.1.6-arch1-1`. The grid receipts identify each candidate's $(q,n)$ workload, batch size, worker count, warm-up, timed repetitions, and 120-second process cap. The q3 finding is no-go under `dev/studies/0dffa759/findings.md` §3; its n=16,20,24 interior window is preserved under §3.2 and does not retain an accelerator configuration. The owner decision dated 2026-08-17 rules the retained `f5-three-plane` and `f7-three-plane-permanent` study prototypes out of freeze selection; their factual rows remain in `receipt-inventory.md`.

| Study rows retained as evidence | Cohort and workload | Factual study result | Freeze status |
|---|---|---|---|
| q3 n=16,20,24 | q3 grid; end-to-end composite draw-pack-evaluate-count; CPU and device rows | The interior-window rates are preserved as the confounded `fold-gf3` finding | `dev/studies/0dffa759/findings.md` §3 is no-go and §3.2 preserves the finding; no freeze selection |
| q5 n=12,16,20,24 | q5 grid; end-to-end composite rows including `f5-three-plane` | `f5-three-plane` is a retained go prototype in the study receipt | Owner decision dated 2026-08-17 excludes the study-cohort prototype from freeze selection; inventory evidence remains |
| q7 n=12,16,20,24,28 | q7 grid; end-to-end composite rows including `f7-three-plane-permanent` | `f7-three-plane-permanent` is a retained go prototype in the study receipt | Owner decision dated 2026-08-17 excludes the study-cohort prototype from freeze selection; inventory evidence remains |

These study rows are nomination evidence only. The protocol does not permit ranking them against the 296a41c9 measurements because source revision, binary identity, workload schedule, and timing protocol differ. The only dischargeable decisions in this draft are the exact 296a41c9 rows at $(3,28)$, $(5,24)$, and $(7,20)$.

## Cell decisions

| q | n | Selected backend in this draft | Rule 3 outcome | Receipt status |
|---:|---:|---|---|---|
| 3 | 4 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 5 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 6 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 7 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 8 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 9 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 10 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 11 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 12 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 13 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 14 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 15 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 16 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 17 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 18 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 19 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 20 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 21 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 22 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 23 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 24 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 25 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 26 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 27 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 3 | 28 | `accelerator` | ranked: accelerator mean 19.359758 matrices/s exceeds intra-matrix Rayon mean 17.994302 | dischargeable now |
| 5 | 4 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 5 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 6 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 7 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 8 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 9 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 10 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 11 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 12 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 13 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 14 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 15 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 16 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 17 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 18 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 19 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 20 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 21 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 22 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 23 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 5 | 24 | `batch_parallel` | only-one-eligible: the 296a cohort contains one eligible configuration | dischargeable now |
| 7 | 4 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 5 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 6 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 7 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 8 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 9 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 10 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 11 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 12 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 13 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 14 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 15 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 16 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 17 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 18 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 19 | `generic_ryser (placeholder)` | not applicable: same-cohort timing is absent or unresolved | gap: placeholder only |
| 7 | 20 | `accelerator` | only-one-eligible: the 296a cohort contains one eligible configuration | dischargeable now |

The $(3,28)$ ranking uses the 296a41c9 pooled composite means: 19.359758 matrices/s for the accelerator and 17.994302 matrices/s for intra-matrix Rayon; the receipt reports the independent 12-process comparison and its conservative interval. The study rows are not selections. The $(5,24)$ and $(7,20)$ rows use the protocol's only-one-eligible path, with exclusions recorded in `backend-exclusions-draft.md`; they do not claim a cross-backend ranking.

The draft does not use permanent or determinant zero counts, intervals, or test outcomes as selection inputs. The determinant companion is evaluated on every cell and its marginal timing remains separate from backend selection, as required by the protocol.
