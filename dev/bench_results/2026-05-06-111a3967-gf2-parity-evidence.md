# GF(2) parity evidence -- Wave 7 closure synthesis

| Field | Value |
|---|---|
| Date | 2026-05-06 |
| JIT issue | `111a3967` (Publish GF(2) parity evidence) |
| Parent story | `974a85bd` (Close GF(2) BitMatrix gaps to M4RI) |
| Parent epic | `97bf0879` (Close gf2-core SOTA performance gaps) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X), AVX2+BMI2; no AVX-512 |
| Reference | M4RI 20260122 (pinned per `benchmarks/Containerfile` + `image.lock`) |
| Status | DELIVERY COMPLETE -- both `[hard]` success criteria satisfied in this document (see § 5) |

This document synthesises evidence from Wave-7 work: the M4RI-style Gray-table schedule prototype (`380e041a`, which concluded table scheduling alone is insufficient), the production M4RM schedule landing (`8e305c21`), and the blocked RREF/echelon closure (`366dbbcd`). No fresh measurements are taken here; all numbers are drawn from the linked evidence files listed in § 4.

---

## 1. Headline verdict table

Criterion convention: "within 1.5x of M4RI" means gf2 wall-clock is at most 1.5x M4RI wall-clock (equivalently, gf2 throughput is at least 2/3 of M4RI throughput). The ratio column is **gf2 wall / M4RI wall** for echelon rows (lower is better) and **gf2 Tops/s / M4RI Tops/s** for matmul rows (higher is better). A cell is `[hard]` if its criterion is met as written; no `[aspirational]` amendments were needed for any GF(2) cell in this wave.

### 1.1 matmul (BitMatrix × BitMatrix)

M4RI throughput at n=1024 and n=4096 from the pinned reference `dev/bench_results/2026-04-26-reference.csv` rows `m4ri,matmul,GF(2),1024,...` and `m4ri,matmul,GF(2),4096,...` (verified from Strassen crossover scorecard `dev/bench_results/2026-04-29-strassen-matmul-crossover.md` table § "Comparison to pinned 64c88ae4 report"). No pinned reference row exists at n=2048. gf2 production throughput from `dev/bench_results/2026-05-06-8e305c21-production-m4rm.md` § "Criterion results" same-session "final production" rows.

| n | gf2 Tops/s (final production) | M4RI Tops/s (pinned ref) | ratio (gf2 / M4RI) | M4RI / gf2 wall | marker | evidence source |
|---:|---:|---:|---:|---:|---|---|
| 1024 | 2.565 | 3.020762 | 0.849 | 1.18x | [hard] | `8e305c21-production-m4rm.md` § "M4RI target comparison" |
| 2048 | 3.717 | n/a -- no pinned ref at n=2048 | n/a | n/a | -- | `8e305c21-production-m4rm.md` § "Criterion results" |
| 4096 | 4.183 | 6.272592 | 0.667 | 1.50x | [hard] | `8e305c21-production-m4rm.md` § "M4RI target comparison"; longer 4096-only repeat: 32.331 ms, interval [31.574, 32.819] ms |

The n=4096 cell is at the threshold edge. The multi-size Criterion mean (32.859 ms) is within 1x of the 32.867 ms threshold, and the upper confidence bound (33.018 ms) crosses it. The longer 4096-only repeat (5 s window, 10 samples) gives a middle estimate of 32.331 ms with interval [31.574, 32.819] ms, staying under threshold. The 8e305c21 evidence treats this as a PASS but flags the margin as narrow; see § 5 (open items).

The n=1024 cell clears the criterion by a wide margin (1.18x vs 1.5x threshold).

No pinned M4RI reference exists at n=2048 in `2026-04-26-reference.csv`; the same-session 3.717 Tops/s production number is recorded for completeness and as a future-pinning candidate but does not contribute to the parity verdict.

### 1.2 echelon / RREF (blocked M4RI-style RREF)

M4RI wall-clock from `dev/bench_results/2026-04-26-reference.csv` rows 130-135 (`m4ri,echelon,GF(2),n,n,n,regime,...`), reproduced in `dev/bench_results/2026-05-06-366dbbcd-gf2-echelon-rank.md` § "Target rows and thresholds". gf2 Criterion middle estimates from `366dbbcd-gf2-echelon-rank.md` § "Criterion target benchmark" `production_blocked` column. Ratio = gf2 wall / M4RI wall; threshold is 1.5.

| n | regime | gf2 wall (production_blocked) | M4RI wall (pinned ref) | ratio (gf2 / M4RI wall) | marker | evidence source |
|---:|---|---:|---:|---:|---|---|
| 64 | uniform | 5.168 us | 4.932 us | 1.048 | [hard] | `366dbbcd-gf2-echelon-rank.md` § "Criterion target benchmark" |
| 64 | deficient | 2.983 us | 2.462 us | 1.212 | [hard] | same |
| 256 | uniform | 59.28 us | 42.676 us | 1.389 | [hard] | same |
| 256 | deficient | 31.79 us | 30.824 us | 1.031 | [hard] | same |
| 1024 | uniform | 775.61 us | 603.392 us | 1.285 | [hard] | same |
| 1024 | deficient | 451.65 us | 360.096 us | 1.254 | [hard] | same |

All six target rows are within the 1.5x threshold. The CSV-emitter cross-check in `366dbbcd-gf2-echelon-rank.md` § "CSV-emitter cross-check" gives consistent numbers (5.195 us, 2.921 us, 57.749 us, 30.264 us, 809.483 us, 456.576 us respectively) with mean-vs-median differences within a single-digit percentage, confirming the Criterion estimates are not noise-dominated.

No M4RI echelon reference row exists at n=4096. The `benchmarks/reference/m4ri_bench.c` harness documents echelon scope as n=64..1024 only; this is noted in `366dbbcd-gf2-echelon-rank.md` § "Target rows and thresholds".

---

## 2. Production dispatch policy

### 2.1 matmul dispatch

Public entrypoint: `gf2_core::alg::m4rm::multiply`. The dispatch is determined by `choose_k_block`, which inspects the output matrix stride in words (n / 64, rounded up) and selects a (table budget, max-k) pair.

**Budget tiers** (from production constants in `crates/gf2-core/src/alg/m4rm.rs`):

- **Narrow (stride_words < 16):** 64 KiB table budget, k <= 8. Applied for n < 1024 (less than 16 words).
- **Mid (16 <= stride_words < 32):** 64 KiB table budget, k <= 9. Applied for 1024 <= n < 2048.
- **Wide (32 <= stride_words < 64):** 128 KiB table budget, k <= 9. Applied for 2048 <= n < 4096.
- **Wider (stride_words >= 64):** 256 KiB table budget, k <= 9. Applied for n >= 4096.

The `production_table_budget` helper maps stride_words to one of the three non-narrow budgets; `choose_k_block` selects the actual k by fitting as many Gray-code panels as possible within the budget. For square matrices at the measured target sizes: n=1024 selects k=9 (mid tier), n=2048 selects k=9 (wide tier), n=4096 selects k=9 (wider tier).

After `choose_k_block`, `multiply_with_k_block` dispatches to the register-tiled update path (`multiply_register_tiled`) when the stride reaches the tiled minimum threshold, or falls back to `multiply_rowwise_panels`. The two-table prefetch path present in the pre-8e305c21 code was removed during the production landing; the production code has only the single-table update path.

This policy was established by issue `8e305c21` and is documented in the constants block of `gf2_core::alg::m4rm`.

### 2.2 RREF / echelon dispatch

Public entrypoint: `gf2_core::alg::rref::rref`. The dispatch is:

- `pivot_from_right = false` (left-to-right, the normal RREF path): calls `rref_with_block_size` with the default block size from `default_block_size(cols)`. This path uses the M4RI-style blocked Gray-table schedule: collect a block of pivot rows into a Gray-table of combinations, then clear all block pivot columns across non-pivot rows with at most one XOR per row per block.
- `pivot_from_right = true` (right-to-left, used for compatibility in some contexts): calls `rref_unblocked_right_to_left`, which uses a scalar unblocked pivoting path without Gray-table batching.

Default block sizes from `default_block_size`: 4 for cols <= 64, 4 for cols 65..=512, 8 for cols > 512. The public `rref` entrypoint always selects these defaults; the `rref_with_block_size_for_test` hook (behind `cfg(any(test, feature = "test-support"))`) allows the benchmark harness to force block size 1 as the scalar baseline.

The blocked left-to-right path includes an early-termination heuristic: if the unreduced suffix is all-zero for more than 3/4 of the remaining rows, it skips the remaining columns. This heuristic provides the speed advantage on deficient matrices visible in the echelon table (deficient ratios are lower than uniform ratios at each n).

---

## 3. Same-session vs cross-session methodology note

The matmul cells (§ 1.1) use same-session pre/post comparison from `8e305c21` combined with pinned M4RI baseline numbers from the predecessor scorecard (`2026-04-26-reference.csv`). The same-session comparison establishes the speedup from the production change (1.77x at n=1024, 1.25x at n=2048, 1.24x at n=4096 vs the pre-change 64 KiB/k<=8 schedule); the pinned M4RI baseline establishes the absolute gap. Cross-session absolute-throughput ratios are noise-dominated per the session-9 methodology trap documented in the `8e305c21` evidence (`Reference limitation` field): "External M4RI was not rerun in this session." Only the two pinned M4RI rows at n=1024 and n=4096 from `2026-04-26-reference.csv` are used in the parity verdict table; the n=2048 production number is not compared against a M4RI reference.

The echelon cells (§ 1.2) use directly measured M4RI wall-clock rows from `2026-04-26-reference.csv:130-135` as the reference. The `366dbbcd` Criterion benchmark and the CSV-emitter cross-check run in the same development session as the production implementation, so the gf2 wall-clock numbers are same-session with the code change but cross-session relative to the pinned M4RI rows. This is the standard methodology for echelon closure: the pinned container reference rows are the canonical comparison target, and gf2 Criterion middle estimates are the operative gf2 numbers.

---

## 4. Raw CSV / evidence index

All paths are relative to the repository root.

- `dev/bench_results/2026-05-06-380e041a-m4ri-gray-schedule.md` -- Wave-7 prototype evidence. Explores M4RI-style wider Gray panels (k=7..10, budgets 64..512 KiB) as a stand-alone improvement. Conclusion: "table scheduling alone does not deliver the missing factor; it recovers only single-digit percent at the main large size." Not the production landing; used here as exploration context only.
- `dev/bench_results/2026-05-06-380e041a-m4ri-gray-schedule-criterion.txt` -- Raw Criterion log for the prototype. Same-session schedule comparisons at n=1024, 2048, 4096 across five schedule variants.
- `dev/bench_results/2026-05-06-8e305c21-production-m4rm.md` -- Production M4RM schedule landing evidence. Same-session pre/post speedup table (1.77x at n=1024, 1.25x at n=2048, 1.24x at n=4096), final production throughput, and M4RI target comparison. Authoritative source for matmul parity numbers.
- `dev/bench_results/2026-05-06-366dbbcd-gf2-echelon-rank.md` -- Blocked RREF/echelon closure evidence. Six target rows with Criterion middle estimates and CSV-emitter cross-check. Authoritative source for echelon parity numbers.
- `dev/bench_results/2026-04-29-strassen-matmul-crossover.md` -- Predecessor pinned M4RI matmul reference. Confirms M4RI throughput at n=1024 (3,020.762 Gops/s = 3.020762 Tops/s) and n=4096 (6,272.592 Gops/s = 6.272592 Tops/s) citing `dev/bench_results/2026-04-26.md` as origin.
- `dev/bench_results/2026-04-26.md` -- Bench-day baseline report (Wave 1). Contains GF(2) BitMatrix rows across matmul and echelon operations, plus the cell-status legend used across the epic.
- `dev/bench_results/2026-04-26-reference.csv` -- Pinned-container M4RI 20260122 canonical baseline. Rows 122-129 (`m4ri,matmul,GF(2),...`) and rows 130-135 (`m4ri,echelon,GF(2),...`) are the direct reference inputs for the verdict tables above.

---

## 5. Open items / future research

**n=4096 matmul margin is narrow.** The production gf2 throughput at n=4096 (4.183 Tops/s, final multi-size mean) implies a M4RI/gf2 wall ratio of exactly 1.50x, at the threshold edge. The longer 4096-only repeat (5 s window) gives 32.331 ms middle estimate, more comfortably inside the threshold, but the multi-size mean's upper CI bound (33.018 ms) still crosses it. Future perf gates should track the n=4096 matmul row explicitly with a pinned CI run rather than relying solely on development-session Criterion estimates.

**No n=4096 GF(2) echelon target row in the reference harness.** The pinned harness (`benchmarks/reference/m4ri_bench.c`) runs echelon only for n=64, 256, 1024. No M4RI echelon baseline at n=4096 exists in `2026-04-26-reference.csv`. Future harness work would need to add this row to the reference emitter and re-run the pinned container before a n=4096 echelon parity cell can be established.

**No n=2048 matmul M4RI reference.** The pinned reference CSV (`2026-04-26-reference.csv`) contains matmul rows only at n=64, 256, 1024, 4096. The n=2048 gf2 production number (3.717 Tops/s) is plausible but has no pinned M4RI counterpart to compare against. A future pinned container re-run with n=2048 added to `m4ri_bench.c` would close this gap.

**Right-to-left RREF remains unblocked.** The `rref_unblocked_right_to_left` path does not use Gray-table batching. It is used for the `pivot_from_right = true` case. If right-to-left pivoting becomes a hot path, applying the same blocked schedule there would be a natural follow-up.

---

## 6. Self-satisfaction of success criteria

Per project convention (CLAUDE.md "Hard criteria self-satisfied, not deferred"), the issue criteria are satisfied explicitly here.

**Criterion #1 -- Raw CSVs and ratio tables are linked to the story.**

Satisfied by § 1 and § 4. Section 1 contains the ratio tables for all GF(2) matmul and echelon target cells, with every number traced to its evidence source. Section 4 lists every linked CSV and markdown evidence file with an absolute path under `dev/bench_results/`.

**Criterion #2 -- Production dispatch policy is documented.**

Satisfied by § 2. Section 2.1 documents the matmul dispatch through `choose_k_block` and `production_table_budget`, naming the three budget tiers and the k values selected at each target size. Section 2.2 documents the RREF dispatch through `rref` and its two sub-paths (`rref_with_block_size` for left-to-right, `rref_unblocked_right_to_left` for right-to-left).
