# permanent-gpu-crossover

S5 (JIT issue `a9e461de`): GPU-vs-CPU-SIMD throughput crossover measurement for
`permanent_bipedal3` (F_3) on gfx1030.

The harness sweeps n ∈ {24, 28, 32, 36, 40, 44} with M = 256 (n ≤ 36) or M = 64
(n ≥ 40), comparing sequential CPU SIMD (`permanent_bipedal3`) against GPU batch
(`gf2_algebra::gpu::permanent_batch_bipedal3`) on the same matrices.

The substantive writeup and results table live at `dev/plans/s5_gpu_crossover.md`.
The CSV output lands at `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-YYYY-MM-DD.csv`.

## Build and run

```bash
# Requires ROCm + gfx1030 device.
cargo build --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
    --release --features hip

cargo run --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
    --release --features hip
```

Without `--features hip` the binary prints a message and exits cleanly (no GPU required for build).

## Smoke tests

```bash
cargo test --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
    --release --features hip -- --ignored
```
