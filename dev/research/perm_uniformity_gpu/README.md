# perm-uniformity-gpu

JIT issue `b293af5a`: GPU-accelerated high-N resample of the perm(A)-vs-det(A)
uniformity experiment over GF(q) for q in {3, 5, 7}, on gfx1030.

This stub **supersedes the noise-limited cells of `8e4e19a0`**: q=3 at
n ∈ {24, 28, 32} (which `8e4e19a0` had to noise-exclude at N = 500/200/50)
and the F_5 / F_7 sweeps (which `8e4e19a0` capped at n ≤ 14 for the
single-word CPU permanent). The closed, signed-off `8e4e19a0` is *not*
reopened; this is a follow-up that replaces its noise-excluded cells with
high-N GPU data.

## What is reused (no re-implementation)

- `perm_uniformity::harness::{tvd_from_counts, bootstrap_tvd_ci,
  bootstrap_diff_ci, CellResult}` — the `8e4e19a0` SSOT statistics, via a
  path dependency on the existing `perm_uniformity` crate.
- `perm_uniformity::png::write_png_file` — the deterministic PNG encoder.
- `gf2_core::field::inverse::det` — the project's canonical determinant.
- `gf2_algebra::gpu::permanent_batch_bipedal{3,5,7}` (`--features hip`) — the
  epic's GPU batch permanent.
- `gf2_algebra::testutil::random_matrix_with_rng` — the seed-pinned LCG draw.

The **only new logic** is the GPU-batched sampling loop in
`src/main.rs::run_cell_gpu`: it draws N independent seeded matrices in the
exact same seed→matrix order the CPU `8e4e19a0` harness uses, buffers them,
pushes them through the GPU batch permanent in fixed chunks, computes det on
the CPU from an independent seeded stream, and feeds both `u8` sample streams
into the reused bootstrap functions with the identical per-cell seeds. GPU
batch chunking cannot perturb the seed→matrix map (all N matrices are
generated in strict seed order *before* any chunking).

## Build and run

```bash
# Requires ROCm + a gfx1030 device at runtime, hipcc at build time.
cargo build --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
    --release --features hip

cargo run --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
    --release --features hip
```

Without `--features hip` the binary builds and prints a message then exits
non-zero (no GPU required for the build), matching the
`permanent_gpu_crossover` / `gf2-kernels-hip` precedents.

Resume a partial sweep with the `CELLS` env (comma-separated `q{Q}n{N}` tags):

```bash
CELLS=q3n28,q3n32 cargo run --manifest-path \
    dev/research/perm_uniformity_gpu/Cargo.toml --release --features hip
```

The CSV is written incrementally after every cell, so a long sweep never
loses completed cells.

## Outputs

- CSV: `dev/benchmarks/perm_uniformity/results-2026-05-17-gpu.csv` (exact
  `8e4e19a0` schema; does **not** overwrite the committed CPU
  `results-2026-05-15.csv`).
- Plot: `dev/benchmarks/perm_uniformity/tvd_vs_n_gpu.png`.
- Writeup: `dev/plans/r4_gpu_uniformity_resample.md`.
- Repro: `scripts/perm-uniformity-gpu-repro.sh`.

## Smoke tests

```bash
cargo test --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml --release
```

The substantive GPU-vs-CPU permanent correctness assertion runs inside the
binary itself (`validate_gpu_matches_cpu`, before any measurement).
