# Supported configurations

## Toolchain

| Component | Requirement | Source |
|---|---|---|
| Rust | 1.95 or newer, edition 2021 | the workspace toolchain contract in [`AGENTS.md`](../../AGENTS.md); `rust-version` in every crate manifest except `gf2-kernels-simd`, which declares none |
| CI compilers | stable Rust for the build, test, lint and doc battery; Rust 1.95.0 for the `gf2-sim` compile-fail guard | [`ci.yml`](../../.github/workflows/ci.yml) |
| Lean proofs | the toolchain pinned in [`lean-toolchain`](../../proofs/lean-toolchain), installed through elan | [formal verification how-to](../how-to/formal-verification.md) |
| Lean regeneration | patched Charon and Aeneas with a pinned Rust nightly | [formal verification how-to](../how-to/formal-verification.md) |
| GPU build | ROCm `hipcc`, located through `ROCM_PATH` (default `/opt/rocm`); tested with ROCm 7.2 | [`build.rs`](../../crates/gf2-kernels-hip/build.rs), [`gf2-kernels-hip` crate documentation](../../crates/gf2-kernels-hip/src/lib.rs) |

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

Each crate's `[features]` table, linked below, defines and documents its
features. This table records the defaults and the features that enable
features in other crates.

| Crate | Default | Enables in other crates |
|---|---|---|
| [`gf2-core`](../../crates/gf2-core/Cargo.toml) | `rand`, `io` | none |
| [`gf2-coding`](../../crates/gf2-coding/Cargo.toml) | `simd`, `sim-observability` | `simd`, `parallel`, `visualization`, `tuning-profile` enable the same `gf2-core` feature; `bench-csv` enables `gf2-core/test-support`; `hip` enables `gf2-kernels-hip/hip` |
| [`gf2-algebra`](../../crates/gf2-algebra/Cargo.toml) | `simd`, `parallel`, `f5`, `f7` | `simd`, `parallel`, `tuning-profile` enable the same `gf2-core` feature; `hip` enables `gf2-kernels-hip/hip` |
| [`gf2-sim`](../../crates/gf2-sim/Cargo.toml) | none | `hip` enables `gf2-coding/hip` and `gf2-algebra/hip`; `llr-f64` enables `gf2-coding/llr-f64` |
| [`gf2-kernels-simd`](../../crates/gf2-kernels-simd/Cargo.toml) | none | none; `avx2` and `avx512` gate no code |
| [`gf2-kernels-hip`](../../crates/gf2-kernels-hip/Cargo.toml) | none | none |
| [`gf2-stats`](../../crates/gf2-stats/Cargo.toml) | no features | none |

List the resolved set with
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
