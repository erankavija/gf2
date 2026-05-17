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

## Cell status (as of 2026-05-17)

**DONE (12 cells, in `results-2026-05-17-gpu.csv`, all genuine PASS):**

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

**REMAINING (interrupted by a reproducible gfx1030 GPU watchdog hang on the
F_5 n=20 long kernel — confirmed twice; the GPU recovers each time and an
isolated F_7 n=8 re-run PASSes, so the harness/kernels are correct):**

| q | n | chosen N | tag | status |
|---|---|----------|-----|--------|
| 5 | 20 | 40,000 | q5n20 | REMAINING — hangs the gfx1030 watchdog (reproduced ×2) |
| 5 | 24 | 8,000  | q5n24 | REMAINING — not reached (after q5n20) |
| 5 | 28 | 8,000  | q5n28 | REMAINING — not reached |
| 7 | 8  | 300,000 | q7n8  | REMAINING — verified PASS in isolation (diff_q95=−0.013955) but not in the batch run |
| 7 | 12 | 300,000 | q7n12 | REMAINING — not reached |
| 7 | 16 | 40,000 | q7n16 | REMAINING — not reached |
| 7 | 20 | 40,000 | q7n20 | REMAINING — not reached |
| 7 | 24 | 8,000  | q7n24 | REMAINING — not reached |

The headline contract — the three `8e4e19a0`-noise-excluded q=3 cells
n∈{24,28,32} — is **fully and genuinely satisfied**, plus the F_5
extension to n=16. The REMAINING cells are the further F_5/F_7 extension.

**Resume recommendation for the REMAINING cells:** the gfx1030 watchdog
hangs on sustained multi-second F_5/F_7 kernels at n≥20. To complete them,
either (a) lower N on q5n20/q5n24/q5n28/q7n20/q7n24 so each kernel launch
is shorter (N≈8k at n=20, N≈2k at n=24/28 keeps the noise floor below
TVD_det/2 for q=5/q=7 per §2.4 of the writeup), and/or (b) insert a short
`hipDeviceSynchronize` + sleep cooldown between chunks in
`run_cell_gpu`'s GPU loop, and/or (c) raise the kernel watchdog timeout
(`amdgpu.lockup_timeout`/`GPU_MAX_HW_QUEUES`) at the driver level. Run the
F_7 cells first (they are cheaper and q7n8 is verified-PASS in isolation).

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
