# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Vision

A **research-grade** toolkit for high-performance finite field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Commands

```bash
# Build workspace
cargo build --workspace --all-features

# Run all tests (fast tier — ALWAYS use --release)
cargo nextest run --workspace --all-features --release --profile ci

# Single crate / single test
cargo nextest run -p gf2-core --release --profile ci
cargo nextest run -p gf2-coding --release --profile ci
cargo nextest run -p gf2-algebra --release --profile ci
cargo nextest run -p gf2-core --release -E 'test(test_name)'

cargo fmt --all -- --check      # check formatting
cargo fmt --all                 # fix formatting
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --no-deps --open

# Benchmarks
cargo bench -p gf2-core
cargo bench -p gf2-coding
cargo bench -p gf2-algebra

# Examples
cargo run -p gf2-coding --example hamming_7_4
cargo run -p gf2-coding --example dvb_t2_ldpc_basic
cargo run -p gf2-coding --example ldpc_awgn --release
cargo run -p gf2-algebra --example permanent_demo --release

# DVB-T2 BICM AWGN campaign (binary: crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs)
# v2 benchmark receipts: dev/benchmarks/gf2-sim/
cargo run --release --bin dvb_t2_awgn_campaign -- \
    --rate 1/2 --modulation 16qam \
    --esn0-range 4.0:7.0:0.5 --target-errors 100 \
    --output-dir /tmp/dvb_r12_16qam --seed 42

# Lean4 verification (full: requires charon + aeneas + elan; committed files only: requires elan)
./scripts/verify-lean.sh
cd proofs && lake build
```

## Test tiers

Two tiers. Use the fast tier by default. Never run the slow tier as an agent.

| Tier | Command | Per-test limit | Who runs it |
|------|---------|---------------|-------------|
| Fast | `cargo nextest run --workspace --all-features --release --profile ci` | 5 s (hard kill) | CI + agents |
| Slow | `cargo nextest run --workspace --all-features --release --profile slow --run-ignored ignored-only` | 120 s | Nightly CI only |

- **NEVER** pass `--run-ignored all`, `--run-ignored ignored-only`, `-- --ignored`, or `-- --include-ignored` in normal work.
- Tests calling `SimulationRunner`, `run_curve`, `run_coded`, or `run_coded_iterative` with `max_frames > 50` or `max_queries > 500` **MUST** carry `#[ignore = "sim: ..."]`.
- Tests expected to exceed 5 s **MUST** carry `#[ignore = "slow: ..."]` or `#[ignore = "sim: ..."]`.

## Performance rules for test and build commands

1. **ALWAYS use `--release`**. Debug-mode tests take 10–100x longer.
2. **Never run multiple `cargo nextest` or `cargo build` in parallel.** Lock contention on the build cache.
3. **For targeted testing**, use `-p gf2-coding` instead of the full workspace.
4. **Test suite wall-clock limit: 60 seconds.** If exceeded, a test is missing its `#[ignore]`.
5. **Examples and benchmarks also need `--release`**.

## Architecture

Five production crates, two isolated kernel crates, one proofs package:

- **`gf2-core`** (`crates/gf2-core/`) — Low-level primitives. No dependencies on other workspace crates.
- **`gf2-coding`** (`crates/gf2-coding/`) — Error-correcting codes; depends on `gf2-core`.
- **`gf2-algebra`** (`crates/gf2-algebra/`) — Packed F_3/F_5/F_7 types and fast matrix permanents on CPU (scalar, AVX2, Rayon) and HIP/ROCm GPU.
- **`gf2-sim`** (`crates/gf2-sim/`) — CPU+GPU FEC simulation pipeline via `Pipeline`/`Stage`/`Connector`. Optional HIP/ROCm via feature `hip`. Design SSOT: `dev/active/ec530af9-pipeline-design.md`. `#![deny(unsafe_code)]`.
- **`gf2-kernels-simd`** (`crates/gf2-kernels-simd/`) — Isolated unsafe SIMD kernels (AVX2/AVX512/AARCH64).
- **`gf2-kernels-hip`** (`crates/gf2-kernels-hip/`) — Isolated unsafe HIP/ROCm GPU kernels; excluded from default workspace. All `unsafe` production code lives in these two kernel crates only.
- **`proofs/`** — Lean4 formal verification of `gfp/`/`gfpn/` arithmetic and `bipedal3`. See `proofs/README.md`.

### gf2-core module map

| Module | Purpose |
|--------|---------|
| `bitvec` / `bitslice` | Dense bit storage in `Vec<u64>`, little-endian bit order |
| `matrix` | `BitMatrix` — row-major bit-packed matrix |
| `sparse` | CSR/CSC sparse matrices |
| `alg/` | M4RM multiplication, Gauss-Jordan inversion, RREF |
| `field/` | `FiniteField` / `ConstField` trait hierarchy and axiom test harness |
| `gf2m/` | GF(2^m) arithmetic, generic over storage width |
| `gfp/` | GF(p) prime field `Fp<P>` with Montgomery multiplication |
| `gfpn/` | Tower extensions: `QuadraticExt<C>`, `CubicExt<C>` |
| `primitive_polys` | Static database of primitive polynomials for m=2..16 |
| `kernels/` | Runtime dispatch to scalar or SIMD backends |
| `compute/` | Parallel batch operations (rayon backend) |
| `io/` | Serde-based serialization (feature-gated) |

### gf2-algebra module map

| Module | Purpose |
|--------|---------|
| `packed/` | `PackedField` traits and impls: `Bipedal3` (F_3), `Packed5` (F_5), `Packed7` (F_7), plus `*Matrix` types |
| `permanent/` | `permanent_ryser` (oracle), `permanent_bipedal{3,5,7}` fast paths, parallel and multi-word variants |
| `gray` | Gray-code subset enumerator for Ryser's formula and bipedal kernels |
| `parallel` | Rayon work-stealing dispatch (feature = "parallel") |
| `gpu` | HIP/ROCm host-side batch dispatcher (feature = "hip") |
| `testutil` | Deterministic random matrix generators |

### gf2-coding module map

| Module | Purpose |
|--------|---------|
| `linear` | `LinearBlockCode`, `SyndromeTableDecoder` — Hamming codes |
| `bch/` | BCH with Berlekamp-Massey + Chien; `dvb_t2/` has all 12 DVB-T2 configurations |
| `ldpc/` | BP decoder; `dvb_t2/` = ETSI EN 302 755 + `DvbT2Concat` + `DvbT2BitInterleaver`; `nr_5g/` = 3GPP TS 38.212 LDPC |
| `modem/` | Gray-QAM mapper/demapper, `ModemSpec`; `examples/dvb_t2_bicm_chain.rs` is the canonical BICM composition |
| `convolutional` | Viterbi decoder skeleton |
| `traits` | `BlockEncoder`, `HardDecisionDecoder`, `GeneratorMatrixAccess` |
| `llr` | `Llr` type (f32 default, f64 with `llr-f64` feature) |
| `channel` | AWGN channel simulation |
| `simulation` | BER/FER harness; checkpoint/resume, SIGINT flush, ChaCha20 RNG seek |

### Key design invariants

1. **Tail masking** — Padding bits beyond `len_bits` in the last `u64` word of a `BitVec` must always be zero. Every mutating operation must call `mask_tail()`.
2. **Bit numbering** — Bit `i` lives in `word = i >> 6`, `mask = 1u64 << (i & 63)`.
3. **Unsafe isolation** — `unsafe` in production code lives exclusively in `gf2-kernels-simd` and `gf2-kernels-hip`. `dev/research/` stubs are exempt; each `pub unsafe fn` must carry a `// SAFETY:` comment.
4. **Functional at API level** — High-level code prefers pure functions and immutability; `kernels/` may use mutation and loops.

## Features

| Crate | Feature | Effect |
|-------|---------|--------|
| `gf2-core` | `simd` | AVX2/SIMD kernels via `gf2-kernels-simd` |
| `gf2-core` | `parallel` | Rayon batch operations |
| `gf2-core` | `visualization` | PNG matrix export |
| `gf2-core` | `io` | Serde serialization (default on) |
| `gf2-coding` | `simd` | Propagates to `gf2-core/simd` (default on) |
| `gf2-coding` | `parallel` | Rayon BCH/LDPC batch |
| `gf2-coding` | `llr-f64` | f64 LLRs |
| `gf2-coding` | `sim-observability` | Checkpointing, SIGINT flush, ChaCha20 seek (default on) |
| `gf2-algebra` | `simd` | AVX2 dispatch for `permanent_bipedal3` (default on) |
| `gf2-algebra` | `parallel` | Rayon `permanent_bipedal3_parallel` (default on) |
| `gf2-algebra` | `f5` | `Packed5`, `Packed5Matrix`, `permanent_bipedal5` (default on) |
| `gf2-algebra` | `f7` | `Packed7`, `Packed7Matrix`, `permanent_bipedal7` (default on) |
| `gf2-algebra` | `hip` | HIP/ROCm GPU batch permanents |
| `gf2-sim` | `hip` | HIP/ROCm GPU pipeline stages via `gf2-kernels-hip` (default off) |
| `gf2-sim` | `llr-f64` | f64 LLRs (default off) |

## Testing conventions

TDD strictly: write the test first, implement minimal code to pass, then add property-based tests for mathematical invariants.

- Unit tests: `#[cfg(test)] mod tests` in the same file as the implementation.
- Property-based tests: `proptest`; integration tests in `tests/`.
- Test naming: `test_<operation>_<scenario>`.
- Always cover word-boundary edge cases: 0, 1, 63, 64, 65 bits.
- All public APIs need doc examples tested by `cargo test --doc`.

## Documentation standards

Every public item needs a doc comment with: description, `# Arguments`, `# Examples` (tested), `# Panics` (if applicable), `# Complexity` for non-trivial operations.

## Git workflow

Commit messages follow conventional commits: `type(scope): brief description`. Valid types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`. Reference the JIT issue short ID in scope (`feat(jit:8ce6f8aa): ...`). First line under 72 chars.

## MSRV

Rust 1.95 (set in `gf2-core`, `gf2-coding`, `gf2-kernels-hip`, `dev/research/rns_prototype` `Cargo.toml`).

## Success-criterion maturity markers

JIT issue criteria may carry an inline marker:

- `[hard]` — Default. Review FAIL if unmet; amending requires explicit user approval via the escalation path.
- `[aspirational]` — Written before empirical evidence existed. May be amended in-loop if the aggregate contract still holds, `cargo-ci` + `code-review` pass, and the amendment records the observed number and reason in the issue description.

**Correctness requirements are always `[hard]`** regardless of marker. Use `[aspirational]` only for explicitly provisional targets (throughput, speedup factors, crossover thresholds). Enforcement is in `scripts/code-review-prompt.md`; do not put marker definitions in `.jit/config.toml`.

## Breakdown-time feasibility check

When an issue mentions specific CPU intrinsics, SIMD lanes, or toolchain-version-dependent behaviour, verify MSRV compatibility **before** accepting the breakdown: `rustup run 1.95.0 cargo check --workspace --all-features`. If unstable on MSRV 1.95: (a) restrict to stable intrinsics, (b) compile-gate with a scalar fallback, or (c) escalate for MSRV bump approval.

## Verification work

Issues delivering a formal proof (Lean4, Coq) or model-checking harness (Kani, CBMC) require a **pre-approved proof sketch** before dispatch. The sketch lists: (1) lemmas in statement form, (2) proof strategy per lemma, (3) exact production code path each harness must exercise (module path + function name), (4) for Kani: expected unwind bounds and any `OnceLock`-dispatched paths. The lead approves before any proof code is written. Dispatching without an approved sketch is a process bug — create the sketch task first, wire implementation as a dependent.
