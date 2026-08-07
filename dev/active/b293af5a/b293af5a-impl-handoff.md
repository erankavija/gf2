# b293af5a GPU-resample — implementation handoff

Status as of 2026-05-18: **all feasible measurement is COMPLETE** (18 cells
genuine PASS, including the long ≈3.4 h q=5 n=24; only the two
hardware-infeasible cells q5n28/q7n24 are out of scope). This file records
which (q,n) cells are DONE and the two documented-infeasible cells. It
originally existed so a long sweep exceeding the per-session wall-clock
budget could be resumed without re-running completed cells (the harness
writes the CSV incrementally after every cell); all completed cells are
persisted in `dev/benchmarks/perm_uniformity/results-2026-05-17-gpu.csv`.

## Harness

- Stub: `dev/research/perm_uniformity_gpu/` (committed, build/fmt/clippy/smoke
  green, GPU-vs-CPU correctness PASS on F_3/F_5/F_7 probes).
- Reuses `perm_uniformity::harness` + `perm_uniformity::png` +
  `gf2_core::field::inverse::det` + `gf2_algebra::gpu::permanent_batch_*`.
- Determinism: seed `0x00c0ffee00000001`; statistical CSV columns
  bit-identical across runs for the same seed.

## Resume command

```bash
# Resume only the REMAINING cells (comma-separated q{Q}n{N} tags):
CELLS=<tag1>,<tag2>,... OUTPUT_DIR=dev/benchmarks/perm_uniformity \
  cargo run --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
  --release --features hip
```

The harness appends/overwrites `results-2026-05-17-gpu.csv` with every cell
in the (optionally `CELLS`-filtered) grid; to preserve already-completed
rows, run with the `CELLS` filter set to ONLY the remaining tags and then
merge the produced rows into the persisted CSV (or re-run the full grid if
the budget allows — completed cheap cells re-run in seconds).

## Cell status (rework — GPU watchdog DEFEATED via §2.5 chunked kernels)

The §2.5 bounded-sub-batch + cooldown mitigation **fully defeated** the
gfx1030 GPU watchdog: zero hangs across q=5 n=20 (which hung twice
before), q=7 n=8/12/16/20, and the q=5 n=24 launches. The
`validate_chunked_equals_unchunked` assertion (sub-batched stream ≡
un-chunked single-launch, byte-identical) PASSED on every run.

**DONE — 18 cells in `results-2026-05-17-gpu.csv`, all genuine PASS**
(an earlier draft of this handoff said "16" — that was an off-by-one; the
pre-q5n24 CSV held 17 cells, now 18 with q5n24):

| q | n | N | TVD_perm | diff_q95 | verdict |
|---|---|---|----------|----------|---------|
| 3 | 6  | 500,000 | 0.02245067 | −0.082992 | PASS |
| 3 | 8  | 500,000 | 0.00260867 | −0.102730 | PASS |
| 3 | 10 | 500,000 | 0.00068267 | −0.105824 | PASS |
| 3 | 12 | 200,000 | 0.00160167 | −0.103108 | PASS |
| 3 | 16 | 200,000 | 0.00051833 | −0.102363 | PASS |
| 3 | 20 | 100,000 | 0.00174333 | −0.101070 | PASS |
| 3 | 24 | 40,000  | 0.00298333 | −0.097283 | **PASS (headline; was 8e4e19a0-noise-excluded)** |
| 3 | 28 | 8,000   | 0.00770833 | −0.086583 | **PASS (headline; was 8e4e19a0-noise-excluded)** |
| 3 | 32 | 2,000   | 0.00983333 | −0.061833 | **PASS (headline; was 8e4e19a0-noise-excluded)** |
| 5 | 8  | 200,000 | 0.00280000 | −0.031890 | PASS |
| 5 | 12 | 200,000 | 0.00215000 | −0.036925 | PASS |
| 5 | 16 | 40,000  | 0.00395000 | −0.025000 | **PASS (new: n>14, absent in 8e4e19a0)** |
| 5 | 20 | 20,000  | 0.00320000 | −0.020700 | **PASS (new: n>14; previously hung GPU ×2, now defeated)** |
| 5 | 24 | 8,000   | 0.00962500 | −0.005875 | **PASS (new: n>14; the 8e4e19a0 q=5-large-n N=8000 standard; 203.4 min, 104 chunked launches, zero hangs)** |
| 7 | 8  | 300,000 | 0.00200810 | −0.013955 | **PASS (new: F_7 n>14 extension; 8e4e19a0 had none)** |
| 7 | 12 | 300,000 | 0.00184190 | −0.013820 | **PASS (new)** |
| 7 | 16 | 40,000  | 0.00454643 | −0.010771 | **PASS (new: n>14, absent in 8e4e19a0)** |
| 7 | 20 | 40,000  | 0.00582857 | −0.012146 | **PASS (new: n>14, absent in 8e4e19a0)** |

Criterion 4 (F_5 AND F_7 extended past n≤14, perm ≤ det at 95%,
perm≪det/decreasing trend): **SATISFIED** — F_5 to n=24, F_7 to n=20,
all genuine PASS.

**REMAINING — 0 feasible, 2 hardware-infeasible:**

q5n24 is **DONE** (above): it completed in one uninterrupted 203.4 min run
(104 bounded sub-batch launches ≈117 s each, zero GPU hangs) after the
prior three ≈58–60 min cuts by an *external session resource limit* (never
a GPU hang or harness/kernel fault). TVD_perm=0.00962500 (CI lo 0.00563 >
0, resolved above floor 0.008921), diff_q95=−0.005875 < 0 ⇒ genuine PASS.
Its row is merged into `results-2026-05-17-gpu.csv` in grid order (after
`5,20`, before `7,8`); the CSV header carries a `# provenance:` note
recording the two-commit measurement (16+1 bulk cells at runtime
HEAD=4fb9db1e with the S2.5 source uncommitted, q5n24 at HEAD=57f12685,
byte-identical binary verified via empty `git diff c0a24b4a..57f12685`
over the harness + deps).

| q | n | N | tag | status |
|---|---|---|-----|--------|
| 5 | 28 | — | q5n28 | HARDWARE-INFEASIBLE at the noise-floor-required N: ≈53 h at N=8000 on gfx1030 (≈1.51 s/matrix × 16 for +4 in n). NOT under-sampled to fake a PASS. |
| 7 | 24 | — | q7n24 | HARDWARE-INFEASIBLE: F_7 LUT ≈1.30 s/matrix; required N≥20000 (floor ≪ 0.01) ⇒ ≈7.3 h; N=8000 gives floor 0.01092 > 0.01 (fails requirement). NOT under-sampled. |

q5n28 and q7n24 are documented as hardware-infeasible at the required N
(see writeup §2.4 / §9 limitation 3) — they are NOT to be forced with an
under-floor N.

## Chosen N per (q,n) and noise-floor justification

See `dev/research/perm_uniformity_gpu/src/main.rs::sweep_grid` (the
authoritative grid) and `dev/plans/r4_gpu_uniformity_resample.md` §2.4. N is
chosen so the Monte-Carlo TVD noise floor `sqrt((q-1)/(2*pi*N))` is
comfortably below TVD_det/2 and TVD_perm is resolved above its own floor.

## Status — all measurement work COMPLETE (2026-05-18)

1. ✅ q5n24 resume run completed (one uninterrupted 203.4 min run, exit 0,
   genuine PASS). The two hardware-infeasible cells (q5n28, q7n24) remain
   documented as such — NOT to be forced with an under-floor N.
2. ✅ q5n24 row cross-checked against the run log and merged into
   `results-2026-05-17-gpu.csv` in grid order (18 data rows, single exact
   `5,24,8000,...` row, verified no double-insert).
3. ✅ Writeup `dev/plans/r4_gpu_uniformity_resample.md` updated from the
   completed CSV (§2.4 table, §3 F_5 table + paragraph, §6 now-PASS list
   renumbered, §7 determinism digest e505a44c… for 18 cells + "16"→17/18
   off-by-one correction, §8 assembly/PLOT_ONLY + wall-clock, §9
   limitation 2 RESOLVED + limitation 6 reach). `## Approval` left
   "Pending … sign-off" (user signs off; the lead does not self-approve).
4. ✅ `tvd_vs_n_gpu.png` regenerated from the 18-cell SSOT CSV via the new
   harness `PLOT_ONLY` mode (reuses `perm_uniformity::png`; no duplicate
   plotting logic; never calls `write_csv`, so the provenance header is
   preserved). Repro script updated to drive PLOT_ONLY.
5. ✅ Determinism: §7 guarantees 1 (two-run seed sha256) + 2
   (`validate_chunked_equals_unchunked` byte-identity, asserted every run)
   carry over; q5n24 statistical columns are seed-deterministic.

## Update 2026-05-18 — code-review FAIL → rework + high-N + criteria amendment

6. ✅ b293af5a code-review FAILed (commit da0abd52). Code findings reworked
   (commit 313ad762): SSOT `finalize_cell` factor-out (both run_cell and
   run_cell_gpu delegate); `ssot_overwrite_guard` resume-safety fail-fast;
   `parse_csv_data_row` extracted + tested; finalize_cell/guard/parser
   unit tests. fmt/clippy/tests green.
7. ✅ Contract findings (criteria 2/3/4 falsified by data) escalated. User
   chose "Long run to raise N"; q=3 re-measured at up to **8,000,000**
   samples (n=10/12 8M, n=16 4M, n=20 2M, n=24 800k). Conclusive: q=3
   TVD_perm collapses to ≈1e-4 by n=10, **at/below the MC noise floor even
   at N=8M** (fundamental resolution limit, not a budget one). The high-N
   run was STOPPED after n=24 per user direction (q3n28 @ N=80k ≈45 h —
   GPU-wall-clock infeasible with the watchdog-bounded sub-batch); n=28/32
   kept at original N (8k/2k). `finalize_cell` proven result-neutral by a
   bit-identical q3n6 cols-1-9 re-run across the refactor.
8. ✅ Merged: 7 high-N q=3 rows replace the old q=3 n≤24 rows; n=28/32 +
   all F_5/F_7 + q5n24 preserved. New 18-cell digest
   `4031d01b…7367616c`. CSV provenance note records the 3 measurement
   epochs. Writeup §2.4/§3/§4/§5/§6/§7/§8/§9 synced to the high-N data +
   the conclusive sub-floor finding (digest verified == CSV).
9. ✅ User approved (escalation 2026-05-18) amending criteria 2/3/4 to the
   empirically-true contract: `[hard]` core (perm ≤ det at 95%,
   `diff_q95<0`, noise-exclusion eliminated) MET at all 18 cells; the
   literal above-floor / strict-monotone / strict-decreasing sub-clauses
   reclassified `[aspirational]` (sub-MC-floor, conclusive). Exact text
   staged at `dev/active/b293af5a-amendment.txt`.

**Remaining:**
- **USER action:** apply the amendment to the JIT issue (the agent is
  blocked from writing `[hard]` criteria to shared state by the standing
  "stop amending on your own" boundary):
  `jit issue update b293af5a --description "$(cat dev/active/b293af5a-amendment.txt)"`
- Lead, after the amendment is applied: run `code-review` gate (cargo-ci
  already run); re-present the writeup for user sign-off; on sign-off
  record the dated approval in §Approval; `jit doc add` the regenerated
  PNG; close b293af5a; then epic completion report + transition
  ae82bd73 → done.
