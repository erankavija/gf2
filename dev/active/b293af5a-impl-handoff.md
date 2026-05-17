# b293af5a GPU-resample — implementation handoff

Status as of 2026-05-17. This file records exactly which (q,n) cells of the
GPU high-N uniformity resample are DONE versus REMAINING, with the chosen N
and the precise resume command. It exists so a long sweep that exceeds the
per-session wall-clock budget can be resumed without re-running completed
cells (the harness writes the CSV incrementally after every cell, so all
completed cells are already persisted in
`dev/benchmarks/perm_uniformity/results-2026-05-17-gpu.csv`).

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

**DONE — 16 cells in `results-2026-05-17-gpu.csv`, all genuine PASS:**

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
| 7 | 8  | 300,000 | 0.00200810 | −0.013955 | **PASS (new: F_7 n>14 extension; 8e4e19a0 had none)** |
| 7 | 12 | 300,000 | 0.00184190 | −0.013820 | **PASS (new)** |
| 7 | 16 | 40,000  | 0.00454643 | −0.010771 | **PASS (new: n>14, absent in 8e4e19a0)** |
| 7 | 20 | 40,000  | 0.00582857 | −0.012146 | **PASS (new: n>14, absent in 8e4e19a0)** |

Criterion 4 (F_5 AND F_7 extended past n≤14, perm ≤ det at 95%,
perm≪det/decreasing trend): **SATISFIED** — F_5 to n=20, F_7 to n=20,
all genuine PASS.

**REMAINING — 1 feasible (session-limited), 2 hardware-infeasible:**

| q | n | N | tag | status |
|---|---|---|-----|--------|
| 5 | 24 | 8,000 | q5n24 | FEASIBLE — watchdog defeated (ran 28 clean ≈117 s launches, zero hangs, GPU 99 % throughout, 0 hang signatures in any log); the ≈3.4 h cell was cut **three** times at ≈58–60 min by an *external session resource limit*, NOT a GPU hang and NOT a harness/kernel fault. Needs one uninterrupted ≈3.4 h run (the §2.5 mitigation is already in the committed binary — no code change needed). floor 0.008921 ≪ TVD_det/2≈0.02 (the exact 8e4e19a0 q=5-large-n N=8000 standard) ⇒ the resume run will resolve a genuine PASS. |
| 5 | 28 | — | q5n28 | HARDWARE-INFEASIBLE at the noise-floor-required N: ≈53 h at N=8000 on gfx1030 (≈1.51 s/matrix × 16 for +4 in n). NOT under-sampled to fake a PASS. |
| 7 | 24 | — | q7n24 | HARDWARE-INFEASIBLE: F_7 LUT ≈1.30 s/matrix; required N≥20000 (floor ≪ 0.01) ⇒ ≈7.3 h; N=8000 gives floor 0.01092 > 0.01 (fails requirement). NOT under-sampled. |

**Resume command for q5n24** (the only feasible-pending cell — run in one
uninterrupted session; the §2.5 mitigation is already in the committed
binary, no code change needed):

```bash
cargo build --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
    --release --features hip
OUTPUT_DIR=/tmp/pug_q5n24 CELLS=q5n24 \
    cargo run --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
    --release --features hip
# ≈3.4 h: 104 bounded sub-batch launches @ ≈117 s each, zero hangs expected
# (q5n24 sub-batch=77 from the q=5 work budget 1.3e9). Then merge the one
# produced row into dev/benchmarks/perm_uniformity/results-2026-05-17-gpu.csv
# in grid order (after the 5,20 row, before the 7,8 row).
```

q5n28 and q7n24 are documented as hardware-infeasible at the required N
(see writeup §2.4 / §9 limitation 3) — they are NOT to be forced with an
under-floor N.

## Chosen N per (q,n) and noise-floor justification

See `dev/research/perm_uniformity_gpu/src/main.rs::sweep_grid` (the
authoritative grid) and `dev/plans/r4_gpu_uniformity_resample.md` §2.4. N is
chosen so the Monte-Carlo TVD noise floor `sqrt((q-1)/(2*pi*N))` is
comfortably below TVD_det/2 and TVD_perm is resolved above its own floor.

## Remaining work for the next session

1. Run the resume command for the REMAINING cells above.
2. Cross-check every produced row against the CSV (no guessed numbers).
3. Fill the `dev/plans/r4_gpu_uniformity_resample.md` placeholders from the
   completed CSV (results tables, 8e4e19a0 comparison, HKS fit, the
   now-PASS list, determinism sha256, wall-clock, limitations).
4. Regenerate `tvd_vs_n_gpu.png`.
5. Verify statistical-column bit-stability across two same-seed short runs.
6. Commit results + writeup (NOT `.jit/`); lead handles JIT state + the
   `jit doc add` attach + the user sign-off escalation.
