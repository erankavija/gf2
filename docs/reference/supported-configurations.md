# Supported configurations

Each crate's `Cargo.toml` is authoritative for the facts below; this page
states how they combine.

## Toolchain

| Component | Requirement | Source |
|---|---|---|
| Rust | 1.95 or newer, edition 2021 | `rust-version` in the crate manifests |
| CI compilers | stable Rust for the build, test, lint and doc battery; Rust 1.95.0 for the `gf2-sim` compile-fail guard | [`ci.yml`](../../.github/workflows/ci.yml) |
| Lean proofs | the toolchain pinned in [`lean-toolchain`](../../proofs/lean-toolchain), installed through elan | [formal verification how-to](../how-to/formal-verification.md) |
| Lean regeneration | patched Charon and Aeneas with a pinned Rust nightly | [formal verification how-to](../how-to/formal-verification.md) |
| GPU build | ROCm `hipcc`, located through `ROCM_PATH` (default `/opt/rocm`) | [`build.rs`](../../crates/gf2-kernels-hip/build.rs) |

## Operating systems

CI builds and tests on GitHub's `ubuntu-latest` runner, so Linux on x86_64 is
the tested platform. `gf2-core`, `gf2-coding`, `gf2-algebra`, `gf2-stats` and
`gf2-kernels-simd` contain no operating-system-specific code. `gf2-sim` uses
`std::os::fd` and builds only on Unix targets. Off Linux, its campaign
filesystem layer opens paths with `O_NOFOLLOW` in place of `openat2`
no-symlink resolution, and permanent-campaign provenance emission refuses with
`EmissionRefusal::UnknownBinaryDigest` because it hashes the running binary
through `/proc/self/exe`. The HIP build script assumes a Linux ROCm layout and
links `libamdhip64`.

## CPU

Every SIMD kernel bundle has a scalar fallback. With the `simd` feature, the
first call probes the CPU and binds a kernel bundle for the life of the
process:

- x86 and x86_64 hosts with AVX2 select the `gf2-kernels-simd` kernels; individual
  bundles additionally require FMA, BMI2, PCLMULQDQ, VPCLMULQDQ, SSE4.1 or
  POPCNT and fall back to scalar code when a required extension is absent.
- AVX-512 hosts run the AVX2 kernels; no AVX-512 kernel is dispatched.
- AArch64 and other architectures run the scalar path.

## GPU

GPU acceleration targets AMD GPUs through HIP. The statically linked kernels
are compiled for gfx1030 only. `build.rs` also compiles per-architecture code
objects for the targets listed in its `GFX_TARGETS`; gfx1030 is mandatory and
the others are best-effort. `gf2_kernels_hip::host::GfxTarget::detect` returns
`HipError::UnsupportedArch` for a device without a compiled object, and the
`gf2-sim` dispatcher falls back to the CPU stage; a host without a visible
device yields `HipError::NoDevice`, which `gf2-sim` reports as
`FatalError::DeviceUnavailable`. The kernel set is listed in the
[`gf2-kernels-hip` crate documentation](../../crates/gf2-kernels-hip/src/lib.rs).

## Cargo features

Defaults are marked with ✓. `test-support` exposes shared test helpers and
conformance harnesses to integration tests and downstream crates; it is
omitted from the table.

| Crate | Feature | Default | Effect |
|---|---|---|---|
| `gf2-core` | `rand` | ✓ | Random vector, matrix and field-element generators |
| `gf2-core` | `io` | ✓ | Serde serialization and checksummed `FieldMatrix` files |
| `gf2-core` | `simd` | | Runtime SIMD detection and dispatch |
| `gf2-core` | `parallel` | | Rayon batch operations |
| `gf2-core` | `visualization` | | PNG export of matrices |
| `gf2-core` | `tuning-profile` | | Versioned tuning-profile parsing and serialization |
| `gf2-coding` | `simd` | ✓ | Enables `gf2-core/simd` |
| `gf2-coding` | `sim-observability` | ✓ | Per-SNR JSON checkpoints, SIGINT/SIGTERM flush, JSON-lines tracing, seekable ChaCha20 RNG |
| `gf2-coding` | `parallel` | | Rayon batch encode and decode |
| `gf2-coding` | `llr-f64` | | `f64` LLRs in place of `f32` |
| `gf2-coding` | `visualization` | | Enables `gf2-core/visualization` |
| `gf2-coding` | `hip` | | GPU batch BCJR for product codes, Gray-QAM demapping and BCH syndrome evaluation |
| `gf2-coding` | `tuning-profile` | | JSON codec for coding-owned tuning selectors |
| `gf2-coding` | `bench-csv` | | Sparse benchmark CSV emitter support |
| `gf2-algebra` | `simd` | ✓ | SIMD dispatch for packed F_3 arithmetic and permanents |
| `gf2-algebra` | `parallel` | ✓ | Rayon batch permanents |
| `gf2-algebra` | `f5` | ✓ | Packed F_5 types and permanent |
| `gf2-algebra` | `f7` | ✓ | Packed F_7 types and permanent |
| `gf2-algebra` | `hip` | | `gf2_algebra::gpu` batch permanents over F_3, F_5 and F_7 |
| `gf2-algebra` | `serde` | | Serde derives on packed types |
| `gf2-algebra` | `tuning-profile` | | JSON codec for algebra-owned tuning selectors |
| `gf2-sim` | `hip` | | `gf2_sim::gpu` dispatch and the hybrid CPU/GPU executor; enables `hip` on `gf2-coding` and `gf2-algebra` |
| `gf2-sim` | `llr-f64` | | Enables `gf2-coding/llr-f64` |
| `gf2-kernels-simd` | `avx2`, `avx512` | | Gate no code; kernel selection is runtime detection |
| `gf2-kernels-hip` | `hip` | | Adds the BCH syndrome and permanent kernels to the linked library |

`gf2-stats` has no features. List the current set with
`./scripts/cargo-budget.sh cargo metadata --format-version 1 --no-deps`.

## Installation

The crates are not on crates.io. Depend on them through Git or a local path;
Cargo resolves the workspace members from either source:

```toml
[dependencies]
gf2-core = { git = "https://github.com/erankavija/gf2" }
gf2-coding = { git = "https://github.com/erankavija/gf2", features = ["parallel"] }
# or, from a checkout:
# gf2-core = { path = "../gf2/crates/gf2-core" }
```

Pin a `rev` for reproducible builds.

## Workspace exclusion

`gf2-kernels-hip` is excluded from the default workspace because its build
script requires `hipcc`. Build it directly with
`./scripts/cargo-budget.sh cargo build --manifest-path crates/gf2-kernels-hip/Cargo.toml`.
The `hip` features of `gf2-coding`, `gf2-algebra` and `gf2-sim` depend on it
by path, so `./scripts/cargo-budget.sh cargo build --workspace --all-features`
also requires ROCm; without ROCm, select features explicitly, as
[`scripts/cargo-ci.sh`](../../scripts/cargo-ci.sh) does.

## Limitations

- CI exercises Linux on x86_64 only, and Windows cannot build `gf2-sim`.
- SIMD kernels exist for x86 and x86_64 only.
- GPU support is limited to AMD HIP, and gfx1030 is the only exercised
  device architecture.
- `gf2_algebra::gpu` exists only with the `hip` feature and panics on a HIP
  runtime failure; callers without a device use the CPU permanents directly.
- Lean proofs cover the functions `scripts/verify-lean.sh` extracts; see
  [Run and extend the Lean 4 proofs](../how-to/formal-verification.md).
