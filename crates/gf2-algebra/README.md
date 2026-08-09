# gf2-algebra

Packed finite-field abstractions and high-performance matrix permanent algorithms over small prime fields (F_3, F_5, F_7), built on [`gf2-core`](../gf2-core/README.md).

`gf2-algebra` is the workspace home for the **bipedal encoding** of F_3 / F_5 / F_7 elements into parallel `u64` bit-planes, and the `permanent_bipedal*` algorithm family that evaluates the matrix permanent via Ryser's inclusion-exclusion formula in Gray-code order. It sits on top of `gf2-core` (for `FiniteField`, `Fp<P>`, `BitVec`) and is `#![deny(unsafe_code)]` — every SIMD or GPU path it dispatches through lives in the dedicated `gf2-kernels-simd` and `gf2-kernels-hip` crates, in keeping with the project's unsafe-isolation invariant.

Epic design doc: [`dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent.md`](../../dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent.md).

## Motivation

The reference paper (Scheinerman 2024, arXiv 2407.20205v2) introduces a "bipedal" map encoding F_3^n as a pair of F_2^n vectors, enabling add/sub/mul in 6/6/2 bitwise ops respectively. Combined with Ryser's formula in Gray-code order this gives O(n * 2^n) field operations where each step is a constant number of 64-bit word ops. This crate extends that idea with scalar single-matrix evaluation, direct and batched AVX2 kernels, Rayon parallelism, and HIP/ROCm GPU dispatch.

## Headline performance numbers

The historical receipt `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv` predates the current scalar dispatcher selection. On its AMD Ryzen 9 5900X host, the public entry point still selected the single-matrix AVX2 path; these figures record that earlier routing rather than the current dispatcher:

| n  | `permanent_mod3_reference` | Historical AVX2 dispatcher | Speedup |
|----|----------------------------|-----------------------------|---------|
| 24 | 1 473 800 µs               | 213 970 µs                  | 6.9x    |
| 28 | 27 360 000 µs              | 3 414 600 µs                | 8.0x    |
| 36 | 9 030 741 000 µs           | 848 484 000 µs              | 10.6x   |

GPU batch processing (from `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv`, AMD Radeon RX 6950 XT / gfx1030):

| n  | Batch (M) | CPU-SIMD perm/s | GPU perm/s  | GPU / CPU |
|----|-----------|-----------------|-------------|-----------|
| 24 | 256       | 4.839           | 137.680     | 28.5x     |
| 28 | 256       | 0.302           | 8.753       | 29.0x     |

GPU dominates for n <= 28 at large batch sizes (M >= 256); CPU-SIMD is faster for n = 32 at M = 4 (kernel-launch overhead dominates at small batch).

## Install

```toml
[dependencies]
gf2-algebra = { path = "crates/gf2-algebra" }  # simd + parallel + f5 + f7 on by default

# Minimal (scalar only, F_3 only)
gf2-algebra = { path = "...", default-features = false }

# With GPU (requires ROCm / hipcc)
gf2-algebra = { path = "...", features = ["hip"] }
```

## Getting started

### Compute the permanent of a random F_3 matrix

```rust
use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_core::gfp::Fp;

// Build a 5x5 all-ones matrix over F_3 and compute its permanent.
let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 25];
let mat = Bipedal3Matrix::from_row_major(&ones, 5, 5);
// 5! = 120 = 0 mod 3
assert_eq!(permanent_bipedal3(&mat), Fp::<3>::new(0));
```

### Field-generic Ryser permanent (correctness oracle)

```rust
use gf2_algebra::permanent::permanent_ryser;
use gf2_core::gfp::Fp;

// 3x3 identity over F_7: permanent = 1
let mut id = vec![Fp::<7>::new(0); 9];
for i in 0..3 { id[i * 3 + i] = Fp::<7>::new(1); }
assert_eq!(permanent_ryser::<Fp<7>>(&id, 3), Fp::<7>::new(1));
```

### Packed F_5 and F_7

```rust
use gf2_algebra::packed::{Packed5, Packed7, PackedField};
use gf2_core::gfp::Fp;

// F_5: 64 lanes per u64-triple
let a = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(3));
let b = <Packed5 as PackedField<Fp<5>>>::splat(Fp::<5>::new(4));
assert_eq!(a.add(b).lane(0), Fp::<5>::new(2)); // 3 + 4 == 2 mod 5

// F_7: 16 lanes per u64-pair
let c = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(5));
let d = <Packed7 as PackedField<Fp<7>>>::splat(Fp::<7>::new(4));
assert_eq!(c.mul(d).lane(0), Fp::<7>::new(6)); // 5 * 4 == 6 mod 7
```

### Parallel permanent (Rayon, requires `--features parallel`)

```rust
use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3_parallel;
use gf2_core::gfp::Fp;

let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 16];
let mat = Bipedal3Matrix::from_row_major(&ones, 4, 4);
// 4! = 24 = 0 mod 3
assert_eq!(permanent_bipedal3_parallel(&mat), Fp::<3>::new(0));
```

## Module map

| Module       | Purpose                                                                              |
|--------------|--------------------------------------------------------------------------------------|
| `packed`     | `PackedField` / `PackedFieldVec` traits and per-prime impls: `Bipedal3` (F_3, 64 lanes), `Packed5` (F_5, 64 lanes), `Packed7` (F_7, 16 lanes). Also `*Matrix` types for each. |
| `permanent`  | `permanent_ryser` (field-generic oracle), `permanent_mod3_reference` (paper baseline), `permanent_bipedal{3,5,7}` fast paths, parallel and multi-word variants. |
| `gray`       | Gray-code subset enumerator used by Ryser's formula and all bipedal kernels.         |
| `parallel`   | Rayon-based work-stealing dispatch (feature = "parallel", default on).               |
| `gpu`        | HIP/ROCm host-side batch dispatcher (feature = "hip", default off).                 |
| `testutil`   | Deterministic random matrix generators (feature = "test-support" or `cfg(test)`).    |

## Features

| Feature        | Default | Effect |
|----------------|---------|--------|
| `simd`         | yes     | Enable the direct and four-matrix AVX2 APIs via `gf2-kernels-simd`; single-matrix `permanent_bipedal3` remains scalar. |
| `parallel`     | yes     | Rayon-backed `permanent_bipedal3_parallel` with work-stealing Gray-block schedule. |
| `f5`           | yes     | Enable `Packed5`, `Packed5Vec`, `Packed5Matrix`, and `permanent_bipedal5`. |
| `f7`           | yes     | Enable `Packed7`, `Packed7Vec`, `Packed7Matrix`, and `permanent_bipedal7`. |
| `hip`          | no      | Enable `gf2_algebra::gpu` (requires ROCm / hipcc; AMD gfx1030+). |
| `serde`        | no      | Serde `Serialize` / `Deserialize` on packed types. |
| `test-support` | no      | Expose `testutil::random_matrix` / `random_matrix_with_rng` to downstream crates and benchmarks. |

## Acceleration

- **SIMD** (default on): On x86_64 hosts with AVX2, `permanent_bipedal3_batch` evaluates up to four matrices together through `gf2-kernels-simd`; `permanent_bipedal3_singleword_simd` exposes the single-matrix kernel directly for conformance work. The public single-matrix `permanent_bipedal3` entry point selects its faster scalar kernel on every host. Runtime detection is cached via `OnceLock` (no `build.rs` magic), and the batched API falls back safely when AVX2 is unavailable.
- **Parallel** (default on): `permanent_bipedal3_parallel` partitions the 2^n Gray-code walk into independent chunks, each dispatched via Rayon work-stealing. Chunk size tunable via the optional `chunk_log2` parameter; default is auto-selected from `dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent.md`.
- **Multi-word** (built-in): For `n > 63` the column-sum vector spans `ceil(n / 64)` Bipedal3 words, using the R3 cache-blocking design from `dev/plans/60c30e2d/r3_multi_word_streaming.md`. Supported up to `n = 255`.
- **GPU** (opt-in, `--features hip`): `gf2_algebra::gpu::permanent_batch_bipedal{3,5,7}` sends a whole batch to the device in one kernel launch (one GPU block per matrix). Requires `hipcc` and a ROCm 6.x+ environment; the crate is excluded from the default workspace build.

## Examples

`permanent_demo` is self-contained (inline deterministic LCG) and runs with the
default feature set:

```bash
cargo run --release -p gf2-algebra --example permanent_demo
```

The other examples below use the `test-support`-gated `testutil` generators, so
run them with `cargo run --release -p gf2-algebra --features test-support --example <name>`:

| Example                  | What it shows                                                                |
|--------------------------|------------------------------------------------------------------------------|
| `permanent_demo`         | Headline benchmark: times `permanent_bipedal3` at n=24 vs `permanent_mod3_reference` at n=20; prints throughput and ±5% check against S1 CSV. Self-contained — no `test-support` feature needed. |
| `paper_repro_slope`      | Reproduces the paper's Table 2 `O(n * 2^n)` scaling slope at n = 8..24.     |
| `parallel_chunk_sweep`   | Sweep chunk-size parameter for `permanent_bipedal3_parallel`.                |
| `parallel_scaling_sweep` | Measure parallel scaling over 1..N_CPUS threads at fixed n.                 |
| `s3_scalar_vs_avx2_sanity` | Compare the direct scalar and single-matrix AVX2 kernels at n in {16, 20, 24}; confirms bit-identical results. |

## Testing

```bash
# Fast tier (CI, agents)
cargo nextest run -p gf2-algebra --release --profile ci --all-features

# Doc tests (all # Examples blocks)
cargo test --doc -p gf2-algebra --all-features

# Benchmarks
cargo bench -p gf2-algebra

# Lint
cargo clippy -p gf2-algebra --all-targets --all-features -- -D warnings
```

Always use `--release`: debug mode is 10-100x slower on the packed arithmetic and the benchmark suite has a 5 s per-test wall-clock limit.

To repeat the live crate-prose audit for superseded single-matrix AVX2 dispatch
framing (with archived development history excluded), run:

```bash
rg -n -i \
  'dispatch wiring and kernel correctne\x73s|batched multi-matrix path.*performance-oriented u\x73er|CPU ana\x6cogue|in later wave\x73|public (entry point|dispatcher).*(selects|prefers).*AVX2' \
  crates/gf2-algebra \
  --glob '!dev/archive/**'
```

## Documentation

- Epic design: [`dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent.md`](../../dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent.md)
- Crate boundary: [`dev/archive/ae82bd73-gf2-algebra-permanent/plans/6e20133d/d1a_gf2_algebra_boundary.md`](../../dev/archive/ae82bd73-gf2-algebra-permanent/plans/6e20133d/d1a_gf2_algebra_boundary.md)
- Packed API surface: [`dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md`](../../dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md)
- Feature-gate matrix: [`dev/archive/ae82bd73-gf2-algebra-permanent/plans/4fced99b/d1c_feature_matrix.md`](../../dev/archive/ae82bd73-gf2-algebra-permanent/plans/4fced99b/d1c_feature_matrix.md)
- S1 speedup data: [`dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`](../../dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv)
- GPU crossover data: [`dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv`](../../dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv)
- Workspace overview: [`../../README.md`](../../README.md)

## License

MIT — see [`../../LICENSE-MIT`](../../LICENSE-MIT).
