# Receipt tables for jit:5cbb6545

Written by `dev/active/5cbb6545/survey/summarize.py` from the committed receipts, their snapshotted addenda, the acceptance summaries beside them, the clock observation `dev/active/5cbb6545/survey/clock-observation.json` and the conformance record `dev/active/5cbb6545/validation.json`. Running it from the repository root reproduces this file byte for byte. Source: the receipts under `dev/bench_results/5cbb6545/`; finding counts are in § Campaigns, column *Findings*.

## Method

Every stage measures one executable and selects its route with `GF2_COUNT_ARM`, so a ratio attributes to the route rather than to two builds (each stage's arm descriptions are in its `receipt.json`). The protocol, contract and addendum-schema pins, the producing-input snapshot and the frozen ledger prefix travel inside each receipt's `inputs/`. The acceptance tool owns every decision: this table copies its estimates, intervals, decisions and outcomes and recomputes none of them. Exploratory cells decide nothing and spend no comparison; their rows carry outcome `pilot`.

Protocol version 4, pinned at `dev/active/f547c394/protocol.md` digest `047b8f395637`, contract `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md` digest `9f3c7563580b`. Shared settings: {"bootstrap_resamples": 10000, "child_timeout_seconds": 120, "confirmatory_pairs": 24, "family_alpha": 0.05, "flagged_window_factor": 2.0, "max_confirmatory_attempts_per_candidate": 1, "max_flagged_fraction": 0.1, "max_pilot_trials_per_cell": 8, "pilot_max_pairs": 24, "pilot_min_pairs": 6, "quality_confidence": 0.95, "window_target_ms": 100, "windows_per_execution": 5}.

## Campaigns

One row per committed receipt, in the order `run-count-campaign.sh` runs its stages. *Attempt* is the family attempt the receipt's own frozen ledger prefix makes it, counting only entries that reserved a comparison, as `trial_ledger::attempt_alpha` does; a stage that reserves none decides nothing and shows none. *m* is the comparisons the prefix reserves, *alpha_c* the corrected per-comparison alpha the tool applied, and *Draws per tail* the expected draws in each interval tail at that alpha, the P-20 tail-support quantity. *Arms* digests the arm executable and the survey's arm source in the receipt's own producing snapshot, so two stages that measured the same arms are recognisable. *Verdict* is the receipt's acceptance verdict; *Qualifies* is the separate production-qualification flag and remains explicit even when the receipt itself is accepted.

| Stage | Label | Cells | Attempt | m | alpha_c | Draws per tail | Resolution | Margins (worthwhile / equivalence / gap) | Verdict | Qualifies | Findings | Arms (executable / arms.rs) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `popcount-sweep` | pilot | 23 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `53bce8f572a3` / `2ea45ec999e5` |
| `popcount-sweep2` | pilot | 6 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `53bce8f572a3` / `2ea45ec999e5` |
| `popcount-pilot` | pilot | 6 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `2a999f6f60ec` / `2ea45ec999e5` |
| `popcount-pilot-r2` | pilot | 6 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `2a999f6f60ec` / `2ea45ec999e5` |
| `popcount-confirmation` | confirmation | 6 | 1 | 6 | 0.004167 | 20.83 | 0.020 | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `f951eab0aa3a` / `ff167fc2a896` |
| `fused-sweep` | pilot | 9 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `53bce8f572a3` / `2ea45ec999e5` |
| `fused-sweep2` | pilot | 6 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `53bce8f572a3` / `2ea45ec999e5` |
| `fused-pilot` | pilot | 6 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `2a999f6f60ec` / `2ea45ec999e5` |
| `fused-pilot-r2` | pilot | 6 | — | 1 | 0.025000 | 125.00 | — | 1.10 / 1.03 / 1.20 | accepted | no | 0 | `2a999f6f60ec` / `2ea45ec999e5` |
| `fused-confirmation` | confirmation | 6 | 1 | 6 | 0.004167 | 20.83 | 0.030 | 1.10 / 1.04 / 1.20 | accepted | no | 0 | `f951eab0aa3a` / `ff167fc2a896` |

## Numerical resolution and tail support

Each confirmation's declared measurement resolution against the widest relative half-width of the pilot receipt it pins, and the P-20 tail support at its corrected alpha. *Pilot widest* and *Declared* are the committed derivation record's, which the canonical freezer computed from the pinned pilot's raw pairs; this table checks that the record pins the same pilot receipt and digest as the addendum and copies the two numbers. *Arms match* states whether the pinned pilot measured the same arm executable the confirmation measured.

| Confirmation | Pinned pilot | Derivation record | Pilot widest relative half-width | Declared resolution | alpha_c | Draws per tail | Arms match | Cells recorded not-confirmatory |
|---|---|---|---|---|---|---|---|---|
| `popcount-confirmation` | `popcount-pilot-r2` | `dev/active/5cbb6545/pilot-resolution-popcount.txt` | 0.012148 | 0.020 | 0.004167 | 20.83 | no | none |
| `fused-confirmation` | `fused-pilot-r2` | `dev/active/5cbb6545/pilot-resolution-fused.txt` | 0.028926 | 0.030 | 0.004167 | 20.83 | no | none |

## Rates (REQ-09)

Per-call latency, useful bytes per second and cycles per byte for every arm of every confirmatory cell. *ns/call* is the median over the cell's pairs and is descriptive; *Bytes/call* is the operand bytes one call feeds to the route, as `families.bytes_per_call` derives them from the cell's own case; *GB/s* divides the two. *Cycles/byte* multiplies the same median by the clock § Clock observation records for that cell and arm and divides by the bytes: an estimate at an observed clock, not a counted cycle total.

| Cell | Arm | Bytes/call | ns/call | GB/s | Clock (cycles/ns) | Cycles/byte |
|---|---|---|---|---|---|---|
| `popcount-w4-dispatch` | `legacy-dispatch` | 32 | 3.27 | 9.800 | 4.737 | 0.4834 |
| `popcount-w4-dispatch` | `resolved-dispatch` | 32 | 3.61 | 8.873 | 4.726 | 0.5326 |
| `popcount-w8-dispatch` | `legacy-dispatch` | 64 | 3.87 | 16.535 | 4.698 | 0.2841 |
| `popcount-w8-dispatch` | `resolved-dispatch` | 64 | 3.49 | 18.330 | 4.680 | 0.2553 |
| `popcount-w128-dispatch` | `legacy-dispatch` | 1024 | 19.03 | 53.800 | 4.753 | 0.0883 |
| `popcount-w128-dispatch` | `resolved-dispatch` | 1024 | 18.24 | 56.127 | 4.745 | 0.0845 |
| `popcount-w1024-dispatch` | `legacy-dispatch` | 8192 | 130.62 | 62.718 | 4.757 | 0.0758 |
| `popcount-w1024-dispatch` | `resolved-dispatch` | 8192 | 97.12 | 84.350 | 4.786 | 0.0567 |
| `popcount-w16384-dispatch` | `legacy-dispatch` | 131072 | 1986.67 | 65.976 | 4.738 | 0.0718 |
| `popcount-w16384-dispatch` | `resolved-dispatch` | 131072 | 1478.41 | 88.657 | 4.745 | 0.0535 |
| `popcount-w16384-vs-libpopcnt` | `resolved-dispatch` | 131072 | 1480.44 | 88.536 | 4.743 | 0.0536 |
| `popcount-w16384-vs-libpopcnt` | `libpopcnt` | 131072 | 1438.47 | 91.119 | 4.756 | 0.0522 |
| `and-w128-fused` | `and-legacy-fused` | 2048 | 20.34 | 100.706 | 4.727 | 0.0469 |
| `and-w128-fused` | `and-resolved-fused` | 2048 | 21.08 | 97.156 | 4.733 | 0.0487 |
| `and-w512-fused` | `and-legacy-fused` | 8192 | 72.46 | 113.049 | 4.705 | 0.0416 |
| `and-w512-fused` | `and-resolved-fused` | 8192 | 55.13 | 148.601 | 4.750 | 0.0320 |
| `and-w4096-fused` | `and-legacy-fused` | 65536 | 572.24 | 114.526 | 4.724 | 0.0412 |
| `and-w4096-fused` | `and-resolved-fused` | 65536 | 448.83 | 146.016 | 4.721 | 0.0323 |
| `and-w4096-vs-two-pass` | `and-two-pass` | 65536 | 1342.51 | 48.816 | 4.733 | 0.0970 |
| `and-w4096-vs-two-pass` | `and-resolved-fused` | 65536 | 449.88 | 145.673 | 4.719 | 0.0324 |
| `matvec-1024x16384` | `matvec-legacy` | 4194304 | 47406.75 | 88.475 | 4.707 | 0.0532 |
| `matvec-1024x16384` | `matvec-resolved` | 4194304 | 40246.31 | 104.216 | 4.738 | 0.0455 |
| `matvec-1024x4096` | `matvec-legacy` | 1048576 | 16793.20 | 62.440 | 4.713 | 0.0755 |
| `matvec-1024x4096` | `matvec-resolved` | 1048576 | 17055.61 | 61.480 | 4.725 | 0.0768 |

## Conversion costs (REQ-09)

Every probe of every cell that declares its conversion costs included: the whole-consumer cells, one row per arm per stage that measured them. Each figure is one probe of the cell's first pair, in nanoseconds, as the arm child measured it beside the timed windows: `setup_ns` builds that arm's inputs, which for a matrix-vector cell is the whole matrix, and `dispatch_ns` is the backend selection one call performs. A cell that declares no conversion costs records none. These are the dispatch and setup costs the timed calls of those cells already contain.

| Stage | Cell | Metric kind | Arm | setup_ns | pack_ns | unpack_ns | batch_fill_ns | dispatch_ns |
|---|---|---|---|---|---|---|---|---|
| `fused-sweep` | `sweep-and-w4096-vs-two-pass` | whole-consumer | `and-two-pass` | 240 | 443 | 0 | 0 | 1 |
| `fused-sweep` | `sweep-and-w4096-vs-two-pass` | whole-consumer | `and-csa-fused` | 1020 | 0 | 0 | 0 | 0 |
| `fused-pilot` | `and-w4096-vs-two-pass` | whole-consumer | `and-two-pass` | 300 | 470 | 0 | 0 | 1 |
| `fused-pilot` | `and-w4096-vs-two-pass` | whole-consumer | `and-resolved-fused` | 160 | 0 | 0 | 0 | 0 |
| `fused-pilot` | `matvec-1024x16384` | whole-consumer | `matvec-legacy` | 412902 | 0 | 728 | 0 | 1 |
| `fused-pilot` | `matvec-1024x16384` | whole-consumer | `matvec-resolved` | 467383 | 0 | 732 | 0 | 1 |
| `fused-pilot` | `matvec-1024x4096` | whole-consumer | `matvec-legacy` | 288972 | 0 | 725 | 0 | 1 |
| `fused-pilot` | `matvec-1024x4096` | whole-consumer | `matvec-resolved` | 257721 | 0 | 726 | 0 | 1 |
| `fused-pilot-r2` | `and-w4096-vs-two-pass` | whole-consumer | `and-two-pass` | 210 | 457 | 0 | 0 | 1 |
| `fused-pilot-r2` | `and-w4096-vs-two-pass` | whole-consumer | `and-resolved-fused` | 140 | 0 | 0 | 0 | 0 |
| `fused-pilot-r2` | `matvec-1024x16384` | whole-consumer | `matvec-legacy` | 411792 | 0 | 733 | 0 | 1 |
| `fused-pilot-r2` | `matvec-1024x16384` | whole-consumer | `matvec-resolved` | 409222 | 0 | 726 | 0 | 1 |
| `fused-pilot-r2` | `matvec-1024x4096` | whole-consumer | `matvec-legacy` | 259342 | 0 | 725 | 0 | 1 |
| `fused-pilot-r2` | `matvec-1024x4096` | whole-consumer | `matvec-resolved` | 295961 | 0 | 1313 | 0 | 1 |
| `fused-confirmation` | `and-w4096-vs-two-pass` | whole-consumer | `and-two-pass` | 240 | 456 | 0 | 0 | 1 |
| `fused-confirmation` | `and-w4096-vs-two-pass` | whole-consumer | `and-resolved-fused` | 170 | 0 | 0 | 0 | 0 |
| `fused-confirmation` | `matvec-1024x16384` | whole-consumer | `matvec-legacy` | 1102606 | 0 | 728 | 0 | 1 |
| `fused-confirmation` | `matvec-1024x16384` | whole-consumer | `matvec-resolved` | 453343 | 0 | 733 | 0 | 1 |
| `fused-confirmation` | `matvec-1024x4096` | whole-consumer | `matvec-legacy` | 327871 | 0 | 726 | 0 | 1 |
| `fused-confirmation` | `matvec-1024x4096` | whole-consumer | `matvec-resolved` | 252081 | 0 | 725 | 0 | 1 |

## Clock observation

The clock the cycles-per-byte column converts at, from `dev/active/5cbb6545/survey/clock-observation.json`: one child per arm per confirmatory cell, on that cell's own case, under `perf stat` for user-space cycles and task-clock. *Timed share* is the fraction of the child's task-clock its five timed windows occupy, from the child's own window record. The record establishes a clock and no comparison; *Route* is the route the observation's candidate-producing executable took. The final production route is stated in the findings and protected by the conformance record. The observation runs an arm executable of its own, whose digest the line below this table gives beside the revision; the receipts' own arm digests are in § Campaigns.

| Cell | Arm | Route | Cycles | Task clock (ms) | Cycles/ns | Timed share |
|---|---|---|---|---|---|---|
| `and-w128-fused` | `and-legacy-fused` | `gf2-kernels-simd:avx2-and-popcnt` | 2568016467 | 543.32 | 4.727 | 0.921 |
| `and-w128-fused` | `and-resolved-fused` | `gf2-ops-and-popcount:simd-nibble-lut` | 2590717325 | 547.40 | 4.733 | 0.920 |
| `and-w4096-fused` | `and-legacy-fused` | `gf2-kernels-simd:avx2-and-popcnt` | 2711664850 | 574.03 | 4.724 | 0.868 |
| `and-w4096-fused` | `and-resolved-fused` | `gf2-ops-and-popcount:simd-carry-save` | 2642689709 | 559.78 | 4.721 | 0.894 |
| `and-w4096-vs-two-pass` | `and-resolved-fused` | `gf2-ops-and-popcount:simd-carry-save` | 2642721076 | 560.00 | 4.719 | 0.893 |
| `and-w4096-vs-two-pass` | `and-two-pass` | `gf2-core-copy-and-inplace-popcount` | 2842117210 | 600.52 | 4.733 | 0.882 |
| `and-w512-fused` | `and-legacy-fused` | `gf2-kernels-simd:avx2-and-popcnt` | 2745718576 | 583.62 | 4.705 | 0.868 |
| `and-w512-fused` | `and-resolved-fused` | `gf2-ops-and-popcount:simd-carry-save` | 2649692389 | 557.86 | 4.750 | 0.894 |
| `matvec-1024x16384` | `matvec-legacy` | `gf2-matvec-legacy:avx2-and-popcnt` | 2744881069 | 583.17 | 4.707 | 0.880 |
| `matvec-1024x16384` | `matvec-resolved` | `gf2-matvec:simd-carry-save` | 2834892852 | 598.39 | 4.738 | 0.830 |
| `matvec-1024x4096` | `matvec-legacy` | `gf2-matvec-legacy:avx2-and-popcnt` | 2759965345 | 585.61 | 4.713 | 0.848 |
| `matvec-1024x4096` | `matvec-resolved` | `gf2-matvec:simd-nibble-lut` | 2893726634 | 612.47 | 4.725 | 0.854 |
| `popcount-w1024-dispatch` | `legacy-dispatch` | `gf2-legacy-popcount:simd-nibble-lut` | 2702771455 | 568.21 | 4.757 | 0.878 |
| `popcount-w1024-dispatch` | `resolved-dispatch` | `gf2-ops-popcount:simd-carry-save` | 2616156336 | 546.63 | 4.786 | 0.904 |
| `popcount-w128-dispatch` | `legacy-dispatch` | `gf2-legacy-popcount:simd-nibble-lut` | 2738089178 | 576.07 | 4.753 | 0.863 |
| `popcount-w128-dispatch` | `resolved-dispatch` | `gf2-ops-popcount:simd-nibble-lut` | 2730919529 | 575.56 | 4.745 | 0.868 |
| `popcount-w16384-dispatch` | `legacy-dispatch` | `gf2-legacy-popcount:simd-nibble-lut` | 2673993247 | 564.43 | 4.738 | 0.882 |
| `popcount-w16384-dispatch` | `resolved-dispatch` | `gf2-ops-popcount:simd-carry-save` | 2604132057 | 548.87 | 4.745 | 0.910 |
| `popcount-w16384-vs-libpopcnt` | `libpopcnt` | `libpopcnt-4.2-popcnt:cpuid=0x800020:bytes=131072` | 2601256189 | 546.91 | 4.756 | 0.912 |
| `popcount-w16384-vs-libpopcnt` | `resolved-dispatch` | `gf2-ops-popcount:simd-carry-save` | 2608130000 | 549.88 | 4.743 | 0.909 |
| `popcount-w4-dispatch` | `legacy-dispatch` | `gf2-legacy-popcount:scalar` | 2647666592 | 558.92 | 4.737 | 0.897 |
| `popcount-w4-dispatch` | `resolved-dispatch` | `gf2-ops-popcount:scalar` | 2619624860 | 554.34 | 4.726 | 0.903 |
| `popcount-w8-dispatch` | `legacy-dispatch` | `gf2-legacy-popcount:simd-nibble-lut` | 2545803834 | 541.92 | 4.698 | 0.874 |
| `popcount-w8-dispatch` | `resolved-dispatch` | `gf2-ops-popcount:simd-nibble-lut` | 2539165115 | 542.52 | 4.680 | 0.881 |

Observation revision `006e456df8b8`, arm executable `ea061cc3cff3`, command `dev/scripts/ccx1-bench-flock.sh --full-host taskset -c <cpus> perf stat -x, -e cycles:u,task-clock:u -- <arm executable>`.

## Correctness coverage (REQ-08)

The conformance record beside this survey, whose raw outputs carry the case counts of each group and the route each width resolves to. The campaign launcher refuses to take the benchmark mutex unless this record passes, so no timed window precedes it.

| Group | Source | Raw output | Exit status |
|---|---|---|---|
| shared suite | `crates/gf2-core/tests/popcount_routes.rs` | `dev/active/5cbb6545/validation-raw.txt` | 0 |
| arm verifier | `dev/active/5cbb6545/survey/gf2-side/src/bin/count-verify.rs` | `dev/active/5cbb6545/validation-arms.txt` | 0 |
| production selection audit | `rg for unsupported CSA selector and automatic-dispatch references` | `dev/active/5cbb6545/validation-production.txt` | 0 |
| source-evidence reproduction | `dev/active/5cbb6545/survey/source-evidence.json` | `dev/active/5cbb6545/validation-source-evidence.txt` | 0 |

All groups pass: `passed` is true in `dev/active/5cbb6545/validation.json`.

## Cells: `popcount-sweep`

Campaign `popcount-sweep-5cbb6545-20260913t123255z`, receipt [`2026-09-13-5cbb6545-popcount-sweep`](./2026-09-13-5cbb6545-popcount-sweep/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [2.15, 1.61, 2.28]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `sweep-popcount-w1-scalar-popcnt` | popcount-1-words-random-aligned | improvement | 6 | `legacy-dispatch` 2.60 | `scalar-popcnt` 2.12 | 1.2271 [1.2212, 1.2308] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:scalar → gf2-kernels-simd:popcnt-words |
| `sweep-popcount-w2-scalar-popcnt` | popcount-2-words-random-aligned | improvement | 6 | `legacy-dispatch` 3.46 | `scalar-popcnt` 2.33 | 1.4831 [1.4811, 1.4882] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:scalar → gf2-kernels-simd:popcnt-words |
| `sweep-popcount-w4-scalar-popcnt` | popcount-4-words-random-aligned | improvement | 6 | `legacy-dispatch` 3.41 | `scalar-popcnt` 2.75 | 1.2370 [1.2273, 1.2435] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:scalar → gf2-kernels-simd:popcnt-words |
| `sweep-popcount-w7-scalar-popcnt` | popcount-7-words-random-aligned | improvement | 6 | `legacy-dispatch` 6.15 | `scalar-popcnt` 3.73 | 1.6489 [1.6415, 1.6553] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:scalar → gf2-kernels-simd:popcnt-words |
| `sweep-popcount-w8-nibble-vs-scalar` | popcount-8-words-random-aligned | improvement | 6 | `scalar-popcnt` 2.95 | `nibble-lut` 2.81 | 1.0470 [1.0385, 1.0585] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:popcnt-words → gf2-kernels-simd:avx2-popcnt |
| `sweep-popcount-w12-nibble-vs-scalar` | popcount-12-words-random-aligned | improvement | 6 | `scalar-popcnt` 4.08 | `nibble-lut` 3.31 | 1.2320 [1.2205, 1.2450] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:popcnt-words → gf2-kernels-simd:avx2-popcnt |
| `sweep-popcount-w16-nibble-vs-scalar` | popcount-16-words-random-aligned | improvement | 6 | `scalar-popcnt` 4.58 | `nibble-lut` 3.71 | 1.2334 [1.2114, 1.2391] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:popcnt-words → gf2-kernels-simd:avx2-popcnt |
| `sweep-popcount-w16-csa` | popcount-16-words-random-aligned | improvement | 6 | `nibble-lut` 3.71 | `csa` 7.53 | 0.4933 [0.4908, 0.4950] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w32-csa` | popcount-32-words-random-aligned | improvement | 6 | `nibble-lut` 6.55 | `csa` 9.02 | 0.7266 [0.7190, 0.7339] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w64-csa` | popcount-64-words-random-aligned | improvement | 6 | `nibble-lut` 10.57 | `csa` 13.17 | 0.8030 [0.7843, 0.8264] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w96-csa` | popcount-96-words-random-aligned | improvement | 6 | `nibble-lut` 14.22 | `csa` 17.23 | 0.8257 [0.8230, 0.8289] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w128-csa` | popcount-128-words-random-aligned | improvement | 6 | `nibble-lut` 18.00 | `csa` 19.03 | 0.9461 [0.9092, 0.9503] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w256-csa` | popcount-256-words-random-aligned | improvement | 6 | `nibble-lut` 33.34 | `csa` 30.11 | 1.1074 [1.1036, 1.1113] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w1024-csa` | popcount-1024-words-random-aligned | improvement | 6 | `nibble-lut` 129.66 | `csa` 98.59 | 1.3151 [1.3113, 1.3160] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w16384-csa` | popcount-16384-words-random-aligned | improvement | 6 | `nibble-lut` 1982.84 | `csa` 1491.10 | 1.3298 [1.3236, 1.3366] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w1024-off3-csa` | popcount-1024-words-random-offset-3-words | improvement | 6 | `nibble-lut` 130.43 | `csa` 99.68 | 1.3085 [1.2896, 1.3110] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w1024-ones-csa` | popcount-1024-words-all_one-aligned | improvement | 6 | `nibble-lut` 129.62 | `csa` 98.91 | 1.3105 [1.3060, 1.3136] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w1024-zeros-csa` | popcount-1024-words-all_zero-aligned | improvement | 6 | `nibble-lut` 129.57 | `csa` 97.79 | 1.3250 [1.3218, 1.3310] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep-popcount-w1024-vs-libpopcnt` | popcount-1024-words-random-aligned | comparator-gap | 6 | `csa` 98.67 | `libpopcnt` 92.98 | 1.0613 [1.0576, 1.0684] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt-csa → libpopcnt-4.2-popcnt:cpuid=0x800020:bytes=8192 |
| `sweep-popcount-w1024-vs-mula-avx2-harley-seal` | popcount-1024-words-random-aligned | comparator-gap | 6 | `csa` 99.18 | `mula-avx2-harley-seal` 102.53 | 0.9673 [0.9548, 0.9892] at 0.9750 | inconclusive | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt-csa → mula-avx2-harley-seal |
| `sweep-popcount-w16384-vs-libpopcnt` | popcount-16384-words-random-aligned | comparator-gap | 6 | `csa` 1490.58 | `libpopcnt` 1438.06 | 1.0365 [1.0313, 1.0420] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt-csa → libpopcnt-4.2-popcnt:cpuid=0x800020:bytes=131072 |
| `sweep-popcount-w16384-vs-mula-avx2-harley-seal` | popcount-16384-words-random-aligned | comparator-gap | 6 | `csa` 1493.26 | `mula-avx2-harley-seal` 1537.88 | 0.9710 [0.9679, 0.9774] at 0.9750 | inconclusive | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt-csa → mula-avx2-harley-seal |
| `sweep-popcount-w1m-streaming-csa` | popcount-1048576-words-random-aligned-streaming | improvement | 6 | `nibble-lut` 258959.96 | `csa` 242031.43 | 1.0699 [1.0660, 1.0790] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |

## Cells: `popcount-sweep2`

Campaign `popcount-sweep2-5cbb6545-20260913t124628z`, receipt [`2026-09-13-5cbb6545-popcount-sweep2`](./2026-09-13-5cbb6545-popcount-sweep2/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [2.6, 1.93, 2.1]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `sweep2-popcount-w160-csa` | popcount-160-words-random-aligned | improvement | 6 | `nibble-lut` 22.00 | `csa` 21.98 | 1.0008 [0.9720, 1.0141] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep2-popcount-w192-csa` | popcount-192-words-random-aligned | improvement | 6 | `nibble-lut` 25.92 | `csa` 24.66 | 1.0510 [1.0439, 1.0612] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep2-popcount-w224-csa` | popcount-224-words-random-aligned | improvement | 6 | `nibble-lut` 29.56 | `csa` 27.66 | 1.0687 [1.0525, 1.0758] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep2-popcount-w256-csa` | popcount-256-words-random-aligned | improvement | 6 | `nibble-lut` 33.48 | `csa` 30.15 | 1.1105 [1.1070, 1.1170] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep2-popcount-w320-csa` | popcount-320-words-random-aligned | improvement | 6 | `nibble-lut` 41.09 | `csa` 35.85 | 1.1463 [1.1407, 1.1503] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |
| `sweep2-popcount-w384-csa` | popcount-384-words-random-aligned | improvement | 6 | `nibble-lut` 48.43 | `csa` 41.66 | 1.1625 [1.1536, 1.1704] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-popcnt → gf2-kernels-simd:avx2-popcnt-csa |

## Cells: `popcount-pilot`

Campaign `popcount-pilot-5cbb6545-20260913t125443z`, receipt [`2026-09-13-5cbb6545-popcount-pilot`](./2026-09-13-5cbb6545-popcount-pilot/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [3.09, 5.52, 4.11]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `popcount-w4-dispatch` | popcount-4-words-random-aligned | improvement | 6 | `legacy-dispatch` 3.26 | `resolved-dispatch` 3.20 | 1.0181 [1.0169, 1.0571] at 0.9750 | not-worse | pilot | 0/60 | gf2-legacy-popcount:scalar → gf2-ops-popcount:scalar-popcnt |
| `popcount-w8-dispatch` | popcount-8-words-random-aligned | non-regression | 6 | `legacy-dispatch` 4.04 | `resolved-dispatch` 3.48 | 1.1624 [1.1552, 1.2190] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-nibble-lut |
| `popcount-w128-dispatch` | popcount-128-words-random-aligned | non-regression | 6 | `legacy-dispatch` 19.03 | `resolved-dispatch` 18.21 | 1.0448 [1.0414, 1.0481] at 0.9750 | not-worse | pilot | 0/60 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-nibble-lut |
| `popcount-w1024-dispatch` | popcount-1024-words-random-aligned | improvement | 6 | `legacy-dispatch` 129.84 | `resolved-dispatch` 97.61 | 1.3302 [1.3072, 1.3371] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-carry-save |
| `popcount-w16384-dispatch` | popcount-16384-words-random-aligned | improvement | 6 | `legacy-dispatch` 1962.78 | `resolved-dispatch` 1487.40 | 1.3196 [1.3146, 1.3248] at 0.9750 | improved | pilot | 0/60 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-carry-save |
| `popcount-w16384-vs-libpopcnt` | popcount-16384-words-random-aligned | comparator-gap | 6 | `resolved-dispatch` 1486.65 | `libpopcnt` 1440.14 | 1.0323 [1.0297, 1.0345] at 0.9750 | not-worse | pilot | 0/60 | gf2-ops-popcount:simd-carry-save → libpopcnt-4.2-popcnt:cpuid=0x800020:bytes=131072 |

## Cells: `popcount-pilot-r2`

Campaign `popcount-pilot-r2-5cbb6545-20260913t130101z`, receipt [`2026-09-13-5cbb6545-popcount-pilot-r2`](./2026-09-13-5cbb6545-popcount-pilot-r2/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [1.68, 3.04, 3.41]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `popcount-w4-dispatch` | popcount-4-words-random-aligned | improvement | 24 | `legacy-dispatch` 3.26 | `resolved-dispatch` 3.19 | 1.0238 [1.0198, 1.0362] at 0.9750 | not-worse | pilot | 0/240 | gf2-legacy-popcount:scalar → gf2-ops-popcount:scalar-popcnt |
| `popcount-w8-dispatch` | popcount-8-words-random-aligned | non-regression | 24 | `legacy-dispatch` 4.03 | `resolved-dispatch` 3.49 | 1.1555 [1.1512, 1.1583] at 0.9750 | improved | pilot | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-nibble-lut |
| `popcount-w128-dispatch` | popcount-128-words-random-aligned | non-regression | 24 | `legacy-dispatch` 19.01 | `resolved-dispatch` 18.19 | 1.0453 [1.0421, 1.0463] at 0.9750 | not-worse | pilot | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-nibble-lut |
| `popcount-w1024-dispatch` | popcount-1024-words-random-aligned | improvement | 24 | `legacy-dispatch` 129.78 | `resolved-dispatch` 97.48 | 1.3314 [1.3289, 1.3333] at 0.9750 | improved | pilot | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-carry-save |
| `popcount-w16384-dispatch` | popcount-16384-words-random-aligned | improvement | 24 | `legacy-dispatch` 1960.69 | `resolved-dispatch` 1485.11 | 1.3202 [1.3145, 1.3226] at 0.9750 | improved | pilot | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-carry-save |
| `popcount-w16384-vs-libpopcnt` | popcount-16384-words-random-aligned | comparator-gap | 24 | `resolved-dispatch` 1488.84 | `libpopcnt` 1439.78 | 1.0341 [1.0311, 1.0365] at 0.9750 | not-worse | pilot | 0/240 | gf2-ops-popcount:simd-carry-save → libpopcnt-4.2-popcnt:cpuid=0x800020:bytes=131072 |

## Cells: `popcount-confirmation`

Campaign `popcount-confirmation-5cbb6545-20260913t132314z`, receipt [`2026-09-13-5cbb6545-popcount-confirmation`](./2026-09-13-5cbb6545-popcount-confirmation/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [2.96, 5.11, 4.43]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `popcount-w4-dispatch` | popcount-4-words-random-aligned | improvement | 24 | `legacy-dispatch` 3.27 | `resolved-dispatch` 3.61 | 0.9054 [0.9030, 0.9187] at 0.9958 | regressed | fail | 0/240 | gf2-legacy-popcount:scalar → gf2-ops-popcount:scalar-popcnt |
| `popcount-w8-dispatch` | popcount-8-words-random-aligned | non-regression | 24 | `legacy-dispatch` 3.87 | `resolved-dispatch` 3.49 | 1.1085 [1.1032, 1.1252] at 0.9958 | improved | pass | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-nibble-lut |
| `popcount-w128-dispatch` | popcount-128-words-random-aligned | non-regression | 24 | `legacy-dispatch` 19.03 | `resolved-dispatch` 18.24 | 1.0432 [1.0392, 1.0463] at 0.9958 | not-worse | pass | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-nibble-lut |
| `popcount-w1024-dispatch` | popcount-1024-words-random-aligned | improvement | 24 | `legacy-dispatch` 130.62 | `resolved-dispatch` 97.12 | 1.3449 [1.3437, 1.3480] at 0.9958 | improved | pass | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-carry-save |
| `popcount-w16384-dispatch` | popcount-16384-words-random-aligned | improvement | 24 | `legacy-dispatch` 1986.67 | `resolved-dispatch` 1478.41 | 1.3438 [1.3399, 1.3461] at 0.9958 | improved | pass | 0/240 | gf2-legacy-popcount:simd-nibble-lut → gf2-ops-popcount:simd-carry-save |
| `popcount-w16384-vs-libpopcnt` | popcount-16384-words-random-aligned | comparator-gap | 24 | `resolved-dispatch` 1480.44 | `libpopcnt` 1438.47 | 1.0292 [1.0263, 1.0327] at 0.9958 | not-worse | not-material | 0/240 | gf2-ops-popcount:simd-carry-save → libpopcnt-4.2-popcnt:cpuid=0x800020:bytes=131072 |

## Cells: `fused-sweep`

Campaign `fused-sweep-5cbb6545-20260913t124244z`, receipt [`2026-09-13-5cbb6545-fused-sweep`](./2026-09-13-5cbb6545-fused-sweep/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [1.85, 1.76, 2.1]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `sweep-and-w8-csa` | and-popcnt-8-words-random-aligned | improvement | 6 | `and-legacy-fused` 3.54 | `and-csa-fused` 6.72 | 0.5262 [0.5105, 0.5377] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w16-csa` | and-popcnt-16-words-random-aligned | improvement | 6 | `and-legacy-fused` 4.61 | `and-csa-fused` 7.42 | 0.6209 [0.6149, 0.6260] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w32-csa` | and-popcnt-32-words-random-aligned | improvement | 6 | `and-legacy-fused` 6.65 | `and-csa-fused` 9.38 | 0.7092 [0.7037, 0.7168] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w64-csa` | and-popcnt-64-words-random-aligned | improvement | 6 | `and-legacy-fused` 11.72 | `and-csa-fused` 13.11 | 0.8940 [0.8764, 0.9011] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w96-csa` | and-popcnt-96-words-random-aligned | improvement | 6 | `and-legacy-fused` 16.27 | `and-csa-fused` 17.35 | 0.9373 [0.9188, 0.9419] at 0.9750 | regressed | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w128-csa` | and-popcnt-128-words-random-aligned | improvement | 6 | `and-legacy-fused` 20.44 | `and-csa-fused` 19.05 | 1.0730 [1.0621, 1.0963] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w512-csa` | and-popcnt-512-words-random-aligned | improvement | 6 | `and-legacy-fused` 73.35 | `and-csa-fused` 55.17 | 1.3294 [1.3242, 1.3365] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w4096-csa` | and-popcnt-4096-words-random-aligned | improvement | 6 | `and-legacy-fused` 581.27 | `and-csa-fused` 444.02 | 1.3091 [1.2990, 1.3175] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep-and-w4096-vs-two-pass` | and-popcnt-4096-words-random-aligned-whole-consumer | improvement | 6 | `and-two-pass` 1441.44 | `and-csa-fused` 443.41 | 3.2508 [3.2420, 3.3112] at 0.9750 | improved | pilot | 0/60 | gf2-core-copy-and-inplace-popcount → gf2-kernels-simd:avx2-and-popcnt-csa |

## Cells: `fused-sweep2`

Campaign `fused-sweep2-5cbb6545-20260913t124728z`, receipt [`2026-09-13-5cbb6545-fused-sweep2`](./2026-09-13-5cbb6545-fused-sweep2/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [1.91, 1.84, 2.06]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `sweep2-and-w160-csa` | and-popcnt-160-words-random-aligned | improvement | 6 | `and-legacy-fused` 24.89 | `and-csa-fused` 22.73 | 1.0953 [1.0900, 1.1001] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep2-and-w192-csa` | and-popcnt-192-words-random-aligned | improvement | 6 | `and-legacy-fused` 29.24 | `and-csa-fused` 25.03 | 1.1682 [1.1652, 1.1730] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep2-and-w224-csa` | and-popcnt-224-words-random-aligned | improvement | 6 | `and-legacy-fused` 33.91 | `and-csa-fused` 29.29 | 1.1577 [1.0839, 1.2147] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep2-and-w256-csa` | and-popcnt-256-words-random-aligned | improvement | 6 | `and-legacy-fused` 39.42 | `and-csa-fused` 32.33 | 1.2192 [1.2105, 1.2282] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep2-and-w320-csa` | and-popcnt-320-words-random-aligned | improvement | 6 | `and-legacy-fused` 48.58 | `and-csa-fused` 38.64 | 1.2574 [1.2226, 1.2596] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |
| `sweep2-and-w384-csa` | and-popcnt-384-words-random-aligned | improvement | 6 | `and-legacy-fused` 56.05 | `and-csa-fused` 43.46 | 1.2895 [1.2837, 1.2953] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-kernels-simd:avx2-and-popcnt-csa |

## Cells: `fused-pilot`

Campaign `fused-pilot-5cbb6545-20260913t125637z`, receipt [`2026-09-13-5cbb6545-fused-pilot`](./2026-09-13-5cbb6545-fused-pilot/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [2.55, 4.53, 3.89]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `and-w128-fused` | and-popcnt-128-words-random-aligned | non-regression | 6 | `and-legacy-fused` 20.25 | `and-resolved-fused` 20.71 | 0.9778 [0.9744, 0.9800] at 0.9750 | not-worse | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-nibble-lut |
| `and-w512-fused` | and-popcnt-512-words-random-aligned | improvement | 6 | `and-legacy-fused` 72.44 | `and-resolved-fused` 55.53 | 1.3046 [1.2365, 1.3108] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-carry-save |
| `and-w4096-fused` | and-popcnt-4096-words-random-aligned | improvement | 6 | `and-legacy-fused` 572.09 | `and-resolved-fused` 445.80 | 1.2833 [1.2812, 1.2863] at 0.9750 | improved | pilot | 0/60 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-carry-save |
| `and-w4096-vs-two-pass` | and-popcnt-4096-words-random-aligned-whole-consumer | improvement | 6 | `and-two-pass` 1336.46 | `and-resolved-fused` 445.65 | 2.9989 [2.9876, 3.0310] at 0.9750 | improved | pilot | 0/60 | gf2-core-copy-and-inplace-popcount → gf2-ops-and-popcount:simd-carry-save |
| `matvec-1024x16384` | matvec-1024-rows-16384-cols-random | improvement | 6 | `matvec-legacy` 46998.50 | `matvec-resolved` 39482.44 | 1.1904 [1.1579, 1.2149] at 0.9750 | improved | pilot | 0/60 | gf2-matvec-legacy:avx2-and-popcnt → gf2-matvec:simd-carry-save |
| `matvec-1024x4096` | matvec-1024-rows-4096-cols-random | non-regression | 6 | `matvec-legacy` 15288.28 | `matvec-resolved` 16587.70 | 0.9217 [0.8866, 0.9772] at 0.9750 | inconclusive | pilot | 0/60 | gf2-matvec-legacy:avx2-and-popcnt → gf2-matvec:simd-nibble-lut |

## Cells: `fused-pilot-r2`

Campaign `fused-pilot-r2-5cbb6545-20260913t130716z`, receipt [`2026-09-13-5cbb6545-fused-pilot-r2`](./2026-09-13-5cbb6545-fused-pilot-r2/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [1.43, 2.09, 2.85]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `and-w128-fused` | and-popcnt-128-words-random-aligned | non-regression | 24 | `and-legacy-fused` 20.24 | `and-resolved-fused` 20.73 | 0.9761 [0.9719, 0.9782] at 0.9750 | not-worse | pilot | 0/240 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-nibble-lut |
| `and-w512-fused` | and-popcnt-512-words-random-aligned | improvement | 24 | `and-legacy-fused` 72.34 | `and-resolved-fused` 55.34 | 1.3072 [1.3049, 1.3090] at 0.9750 | improved | pilot | 0/240 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-carry-save |
| `and-w4096-fused` | and-popcnt-4096-words-random-aligned | improvement | 24 | `and-legacy-fused` 571.59 | `and-resolved-fused` 445.47 | 1.2831 [1.2818, 1.2861] at 0.9750 | improved | pilot | 0/240 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-carry-save |
| `and-w4096-vs-two-pass` | and-popcnt-4096-words-random-aligned-whole-consumer | improvement | 24 | `and-two-pass` 1337.81 | `and-resolved-fused` 445.17 | 3.0051 [3.0011, 3.0253] at 0.9750 | improved | pilot | 0/240 | gf2-core-copy-and-inplace-popcount → gf2-ops-and-popcount:simd-carry-save |
| `matvec-1024x16384` | matvec-1024-rows-16384-cols-random | improvement | 24 | `matvec-legacy` 45934.55 | `matvec-resolved` 39139.20 | 1.1736 [1.1443, 1.2076] at 0.9750 | improved | pilot | 0/240 | gf2-matvec-legacy:avx2-and-popcnt → gf2-matvec:simd-carry-save |
| `matvec-1024x4096` | matvec-1024-rows-4096-cols-random | non-regression | 24 | `matvec-legacy` 14981.04 | `matvec-resolved` 15923.91 | 0.9408 [0.9205, 0.9644] at 0.9750 | regressed | pilot | 0/240 | gf2-matvec-legacy:avx2-and-popcnt → gf2-matvec:simd-nibble-lut |

## Cells: `fused-confirmation`

Campaign `fused-confirmation-5cbb6545-20260913t165142z`, receipt [`2026-09-13-5cbb6545-fused-confirmation`](./2026-09-13-5cbb6545-fused-confirmation/), toolchain rustc 1.95.0 (59807616e 2026-04-14), host AMD Ryzen 9 5900X 12-Core Processor, kernel Linux 7.2.2-arch1-1, load average at the host observation [2.28, 1.49, 0.91]. *Speedup* and *Interval* are the acceptance tool's at the corrected confidence; per-call times are medians over the cell's pairs and are descriptive. *Route* is the route each arm reported taking for the cell's own width, from the pairs' `selected_path`.

| Cell | Workload | Objective | Pairs | Baseline ns/call | Candidate ns/call | Speedup [interval] | Decision | Outcome | Flagged | Route baseline → candidate |
|---|---|---|---|---|---|---|---|---|---|---|
| `and-w128-fused` | and-popcnt-128-words-random-aligned | non-regression | 24 | `and-legacy-fused` 20.34 | `and-resolved-fused` 21.08 | 0.9647 [0.9609, 0.9696] at 0.9958 | inconclusive | inconclusive | 0/240 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-nibble-lut |
| `and-w512-fused` | and-popcnt-512-words-random-aligned | improvement | 24 | `and-legacy-fused` 72.46 | `and-resolved-fused` 55.13 | 1.3145 [1.3086, 1.3203] at 0.9958 | improved | pass | 0/240 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-carry-save |
| `and-w4096-fused` | and-popcnt-4096-words-random-aligned | improvement | 24 | `and-legacy-fused` 572.24 | `and-resolved-fused` 448.83 | 1.2750 [1.2671, 1.2811] at 0.9958 | improved | pass | 0/240 | gf2-kernels-simd:avx2-and-popcnt → gf2-ops-and-popcount:simd-carry-save |
| `and-w4096-vs-two-pass` | and-popcnt-4096-words-random-aligned-whole-consumer | improvement | 24 | `and-two-pass` 1342.51 | `and-resolved-fused` 449.88 | 2.9841 [2.9702, 3.0088] at 0.9958 | improved | pass | 0/240 | gf2-core-copy-and-inplace-popcount → gf2-ops-and-popcount:simd-carry-save |
| `matvec-1024x16384` | matvec-1024-rows-16384-cols-random | improvement | 24 | `matvec-legacy` 47406.75 | `matvec-resolved` 40246.31 | 1.1779 [1.1479, 1.2107] at 0.9958 | improved | pass | 0/240 | gf2-matvec-legacy:avx2-and-popcnt → gf2-matvec:simd-carry-save |
| `matvec-1024x4096` | matvec-1024-rows-4096-cols-random | non-regression | 24 | `matvec-legacy` 16793.20 | `matvec-resolved` 17055.61 | 0.9846 [0.9532, 0.9957] at 0.9958 | inconclusive | inconclusive | 0/240 | gf2-matvec-legacy:avx2-and-popcnt → gf2-matvec:simd-nibble-lut |
