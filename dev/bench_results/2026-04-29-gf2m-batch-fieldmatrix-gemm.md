# GF(2^m) FieldMatrix batch-GEMM evidence — 2026-04-29

Issue: `577b9e7f` — wire GF(2^m) batch kernels into `FieldMatrix` multiply.

## Host / toolchain

| Item | Value |
|---|---|
| CPU | AMD Ryzen 9 5900X 12-Core Processor |
| Cores / threads | 12 / 24 |
| ISA flags relevant here | `pclmulqdq`, `avx2`, `vpclmulqdq`, `sse4_1` |
| Cache | L1d 384 KiB total, L2 6 MiB total, L3 64 MiB total |
| OS | Linux 6.19.11-arch1-1 x86_64 |
| rustc | `rustc 1.95.0 (59807616e 2026-04-14)` |
| cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |

## Commands

Validation:

```bash
CARGO_TARGET_DIR=target-577b9e7f cargo fmt -p gf2-core -- --check
CARGO_TARGET_DIR=target-577b9e7f cargo check -p gf2-core --release --lib --features simd,rand
CARGO_TARGET_DIR=target-577b9e7f cargo test -p gf2-core --release --lib --features simd,rand gf2m_batch_gemm -- --nocapture
CARGO_TARGET_DIR=target-577b9e7f cargo test -p gf2-core --release --lib --features simd,rand gemm_matches_naive_gf2 -- --nocapture
CARGO_TARGET_DIR=target-577b9e7f cargo check -p gf2-core --release --bench fieldmatrix_gf2m_batch_gemm --features simd,rand
CARGO_TARGET_DIR=target-577b9e7f cargo clippy -p gf2-core --release --lib --features simd,rand -- -D warnings
CARGO_TARGET_DIR=target-577b9e7f cargo nextest run -p gf2-core --release --profile ci --features simd,rand
```

Benchmark:

```bash
CARGO_TARGET_DIR=target-577b9e7f RUSTFLAGS="-C target-cpu=native" \
  cargo bench -p gf2-core --bench fieldmatrix_gf2m_batch_gemm \
  --features simd,rand -- --quiet
```

The benchmark compares:

- `scalar_eager`: private benchmark reference, classical triple loop with one
  scalar field multiplication per inner-loop iteration.
- `batch_gemm`: production `field::matrix::gemm`, now routing supported
  single-word GF(2^m) dot products through the batched VPCLMULQDQ-aware
  product-sum hook and reusing scratch buffers across output cells.

## Results

Criterion median throughput:

| field | shape `(m×k×n)` | scalar eager | batch GEMM | speedup |
|---|---:|---:|---:|---:|
| GF(2^8) | `64×64×64` | 23.985 Mops/s | 182.48 Mops/s | 7.61× |
| GF(2^8) | `128×8×128` | 12.540 Mops/s | 89.101 Mops/s | 7.11× |
| GF(2^8) | `128×32×128` | 12.326 Mops/s | 164.72 Mops/s | 13.36× |
| GF(2^16) | `64×64×64` | 10.734 Mops/s | 186.88 Mops/s | 17.41× |
| GF(2^16) | `128×8×128` | 11.412 Mops/s | 124.71 Mops/s | 10.93× |
| GF(2^16) | `128×32×128` | 19.523 Mops/s | 297.05 Mops/s | 15.22× |
| GF(2^32) | `64×64×64` | 17.754 Mops/s | 313.35 Mops/s | 17.65× |
| GF(2^32) | `128×8×128` | 18.181 Mops/s | 133.25 Mops/s | 7.33× |
| GF(2^32) | `128×32×128` | 17.897 Mops/s | 275.80 Mops/s | 15.41× |

Against the `64c88ae4` published gf2-side baselines:

| field / published shape | `64c88ae4` baseline | nearest new measured shape | new throughput | ratio |
|---|---:|---:|---:|---:|
| GF(2^8), `n=64` | 36.455 Mops/s | `64×64×64` | 182.48 Mops/s | 5.01× |
| GF(2^16), `n=64` | 32.548 Mops/s | `64×64×64` | 186.88 Mops/s | 5.74× |
| GF(2^8), `1024×1024×8` | 36.429 Mops/s | `128×8×128` | 89.101 Mops/s | 2.45× |
| GF(2^16), `1024×1024×8` | 32.479 Mops/s | `128×8×128` | 124.71 Mops/s | 3.84× |
| GF(2^8), `1024×1024×32` | 36.269 Mops/s | `128×32×128` | 164.72 Mops/s | 4.54× |
| GF(2^16), `1024×1024×32` | 32.646 Mops/s | `128×32×128` | 297.05 Mops/s | 9.10× |

## Compute-bound vs memory-bound interpretation

These development cells are cache-resident: the largest input/output matrices
are at most tens of KiB each, and even the scratch traffic implied by exporting
`u64` dot-product lanes is on the order of single-digit GiB/s at the measured
times. That is well below this host's cache and DRAM bandwidth. The observed
speedups therefore come from reducing compute and dispatch overhead — replacing
per-element GF(2^m) multiplication in the matrix inner loop with batched
VPCLMULQDQ product slices — rather than from improving DRAM locality.

The remaining bottleneck is not DRAM bandwidth; it is per-output-cell dot
setup: exporting row/column values into scratch buffers, invoking the batch
kernel, and XOR-reducing products. A future matrix-panel kernel could improve
reuse further by batching several output cells at once, but this change already
exposes the Tier-C carry-less multiply gains to `FieldMatrix::gemm` while
preserving the scalar fallback.
