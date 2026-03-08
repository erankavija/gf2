# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Vision

A **research-grade** toolkit for high-performance finite field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Commands

```bash
# Build workspace
cargo build --workspace --all-features

# Run all tests (match CI)
cargo test --workspace --all-features

# Run tests for a single crate
cargo test -p gf2-core
cargo test -p gf2-coding

# Run a single test by name
cargo test -p gf2-core <test_name>

# Check formatting (CI enforces this)
cargo fmt --all -- --check

# Fix formatting
cargo fmt --all

# Lint (CI treats warnings as errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build documentation
cargo doc --no-deps --open

# Benchmarks
cargo bench -p gf2-core
cargo bench -p gf2-coding

# Run examples
cargo run -p gf2-coding --example hamming_7_4
cargo run -p gf2-coding --example dvb_t2_ldpc_basic
cargo run -p gf2-coding --example ldpc_awgn --release

# Lean4 verification pipeline (requires charon + aeneas + elan)
./scripts/verify-lean.sh

# Just build the committed Lean files (requires elan only)
cd proofs && lake build
```

## Architecture

This is a Cargo workspace with three crates:

- **`gf2-core`** (`crates/gf2-core/`) — Low-level primitives. No dependencies on the other workspace crates. All purely mathematical operations, data structures, and algorithms go here.
- **`gf2-coding`** (`crates/gf2-coding/`) — Error-correcting codes; depends on `gf2-core`.
- **`gf2-kernels-simd`** (`crates/gf2-kernels-simd/`) — Isolated unsafe SIMD kernels (AVX2/AVX512/AARCH64). This is the only crate allowed to contain `unsafe` code; everything else uses `#![deny(unsafe_code)]`.
- **`proofs/`** — Lean4 formal verification of `gfp/` and `gfpn/` field arithmetic, auto-generated via Charon/Aeneas. See `proofs/README.md`. Covers `Fp<P>` (Montgomery arithmetic), `QuadraticExt`, and `CubicExt` (tower extensions).

### gf2-core module map

| Module | Purpose |
|--------|---------|
| `bitvec` / `bitslice` | Dense bit storage in `Vec<u64>`, little-endian bit order |
| `matrix` | `BitMatrix` — row-major bit-packed matrix |
| `sparse` | CSR/CSC sparse matrices |
| `alg/` | M4RM multiplication, Gauss-Jordan inversion, RREF |
| `field/` | `FiniteField` / `ConstField` trait hierarchy and axiom test harness |
| `gf2m/` | GF(2^m) arithmetic, generic over storage width via sealed `UintExt` trait |
| `gfp/` | GF(p) prime field `Fp<P>` with Montgomery multiplication internals |
| `gfpn/` | Tower extensions: `QuadraticExt<C>`, `CubicExt<C>` over `ExtConfig` trait |
| `primitive_polys` | Static database of primitive polynomials for m=2..16 |
| `kernels/` | Runtime dispatch to scalar or SIMD backends |
| `compute/` | Parallel batch operations (rayon backend) |
| `io/` | Serde-based serialization (feature-gated) |

### gf2-coding module map

| Module | Purpose |
|--------|---------|
| `linear` | `LinearBlockCode`, `SyndromeTableDecoder` — Hamming codes |
| `bch/` | BCH codes with Berlekamp-Massey + Chien search; `dvb_t2/` sub-module contains all 12 DVB-T2 configurations |
| `ldpc/` | Belief-propagation decoder; `dvb_t2/` has tables from ETSI EN 302 755; `encoding/` uses Richardson-Urbanke with cache |
| `convolutional` | Viterbi decoder skeleton |
| `traits` | `BlockEncoder`, `HardDecisionDecoder`, `GeneratorMatrixAccess` — unified interfaces |
| `llr` | `Llr` type (f32 by default, f64 with `llr-f64` feature) for soft-decision decoding |
| `channel` | AWGN channel simulation with BPSK modulation |
| `simulation` | BER/FER simulation harness |

### Key design invariants

1. **Tail masking** — Padding bits beyond `len_bits` in the last `u64` word of a `BitVec` must always be zero. Every mutating operation must call `mask_tail()`. This is the most critical correctness invariant.

2. **Bit numbering** — Bit `i` lives in `word = i >> 6`, `mask = 1u64 << (i & 63)`.

3. **Unsafe isolation** — All `unsafe` code lives exclusively in `gf2-kernels-simd`. SIMD is detected at runtime via `OnceLock` in `gf2-core/src/lib.rs`; call path is `simd::maybe_simd()` → optional `LogicalFns`.

4. **Functional at API level, imperative allowed in kernels** — High-level code (outside `kernels/`) prefers pure functions, iterator combinators, and immutability. `kernels/` uses mutation and loops for speed.

## Features

| Crate | Feature | Effect |
|-------|---------|--------|
| `gf2-core` | `simd` | Enables AVX2/SIMD kernels via `gf2-kernels-simd` |
| `gf2-core` | `parallel` | Rayon batch operations |
| `gf2-core` | `visualization` | PNG matrix export |
| `gf2-core` | `io` | Serde serialization (default on) |
| `gf2-coding` | `simd` | Propagates to `gf2-core/simd` (default on) |
| `gf2-coding` | `parallel` | Rayon BCH/LDPC batch |
| `gf2-coding` | `llr-f64` | Use f64 instead of f32 for LLRs |

## Testing conventions

TDD is followed strictly: write the test first, implement minimal code to pass, then add property-based tests for mathematical invariants.

- Unit tests live in `#[cfg(test)] mod tests` within the same file as the implementation.
- Property-based tests use `proptest`; integration tests go in `tests/`.
- Test naming: `test_<operation>_<scenario>` (e.g., `test_shift_left_word_boundary`).
- Always cover word-boundary edge cases: 0, 1, 63, 64, 65 bits.
- All public APIs need doc comment examples — these are tested by `cargo test --doc` and must compile and pass.

## Documentation standards

Every public item must have a doc comment with: description, `# Arguments`, `# Examples` (tested), `# Panics` (if applicable), and `# Complexity` for non-trivial operations.

## Git workflow

**Commit messages** follow conventional commits:
```
type(scope): brief description

Longer explanation if needed.
```

* Valid types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`.
* Reference the jit issue short ID in the scope prefixed with jit: (e.g., `feat(jit:8ce6f8aa): ...`)
* First line under 72 chars.

## Adding a new error-correcting code

1. Implement the relevant traits from `gf2_coding::traits`: `BlockEncoder`, `HardDecisionDecoder`, and/or `SoftDecoder`.
2. Add standard-specific factory constructors (e.g., `MyCode::dvb_t2()`, `MyCode::nr_5g()`).
3. Validate against known test vectors from the relevant standard.
4. Add benchmarks for encoding and decoding throughput in `benches/`.
5. Add an example in `examples/` demonstrating usage.

## MSRV

Rust 1.80 (set in both `gf2-core` and `gf2-coding` `Cargo.toml`).
