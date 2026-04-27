# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Vision

A **research-grade** toolkit for high-performance finite field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Commands

```bash
# Build workspace
cargo build --workspace --all-features

# Run all tests (fast tier — default, matches CI) — ALWAYS use --release
cargo nextest run --workspace --all-features --release --profile ci

# Run tests for a single crate
cargo nextest run -p gf2-core --release --profile ci
cargo nextest run -p gf2-coding --release --profile ci

# Run a single test by name
cargo nextest run -p gf2-core --release -E 'test(test_name)'

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

## Test tiers

Two tiers. Use the fast tier by default. Never run the slow tier as an agent.

| Tier | Command | Per-test limit | Who runs it |
|------|---------|---------------|-------------|
| Fast | `cargo nextest run --workspace --all-features --release --profile ci` | 5 s (hard kill) | CI + agents |
| Slow | `cargo nextest run --workspace --all-features --release --profile slow --run-ignored ignored-only` | 120 s | Nightly CI only |

**Rules — read carefully:**
- **NEVER** pass `--run-ignored all`, `--run-ignored ignored-only`, `-- --ignored`, or `-- --include-ignored` in normal work. Those unlock the slow tier and will stall the agent for minutes.
- Any test calling `SimulationRunner`, `run_curve`, `run_coded`, or `run_coded_iterative` with `max_frames > 50` or `max_queries > 500` **MUST** carry `#[ignore = "sim: <description>"]`.
- Any test expected to exceed 5 s **MUST** carry `#[ignore = "slow: <description>"]` or `#[ignore = "sim: <description>"]`.
- Tests requiring external ETSI test vector files use `#[ignore = "external: <description>"]`.

## Performance rules for test and build commands

1. **ALWAYS use `--release`**. Debug-mode tests take 10–100x longer due to unoptimized SIMD, crypto, and simulation code.
2. **Never run multiple `cargo nextest` or `cargo build` commands in parallel.** They compete for the same build cache and cause lock contention. Run one at a time.
3. **For targeted testing during development**, use `-p gf2-coding` instead of the full workspace.
4. **Test suite wall-clock limit: 60 seconds.** Nextest enforces 5 s per test; if the full suite exceeds 60 s, a test is missing its `#[ignore]`.
5. **Examples and benchmarks also need `--release`** — simulation examples can be 100x slower without optimization.

## Architecture

This is a Cargo workspace with three crates:

- **`gf2-core`** (`crates/gf2-core/`) — Low-level primitives. No dependencies on the other workspace crates. All purely mathematical operations, data structures, and algorithms go here.
- **`gf2-coding`** (`crates/gf2-coding/`) — Error-correcting codes; depends on `gf2-core`.
- **`gf2-kernels-simd`** (`crates/gf2-kernels-simd/`) — Isolated unsafe SIMD kernels (AVX2/AVX512/AARCH64).
- **`gf2-kernels-hip`** (`crates/gf2-kernels-hip/`) — Isolated unsafe HIP/ROCm GPU kernels (device FFI, gfx1030; currently BCJR batch decode + Gray-QAM soft demap prototype). Excluded from the default workspace so non-ROCm hosts still build cleanly; opt in via `--features hip` on `gf2-coding` or by building the crate with its own manifest.

Unsafe code lives exclusively in these two kernel crates; everything else uses `#![deny(unsafe_code)]`.
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

3. **Unsafe isolation** — All `unsafe` code lives exclusively in the two accelerator kernel crates: `gf2-kernels-simd` (CPU SIMD) and `gf2-kernels-hip` (HIP/ROCm GPU FFI). SIMD is detected at runtime via `OnceLock` in `gf2-core/src/lib.rs`; call path is `simd::maybe_simd()` → optional `LogicalFns`. The HIP crate is opt-in via Cargo feature and excluded from the default workspace build.

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

Rust 1.95 (set in `gf2-core`, `gf2-coding`, `gf2-kernels-hip`, and `dev/research/rns_prototype` `Cargo.toml`). Bumped from 1.80 on 2026-04-27.

## Success-criterion maturity markers

Individual success-criterion bullets in JIT issues may carry an inline marker at the start of the line, teaching the code-review gate which criteria are amendable against empirical data and which are hard contracts. This project defines two markers (see `scripts/code-review-prompt.md` for the exact reviewer semantics):

- `[hard]` — Default. Failure to meet the criterion is a review FAIL; modifying the criterion requires explicit user approval via the escalation path in `.claude/skills/project-lead/references/escalation-policy.md`.
- `[aspirational]` — A target written optimistically before empirical evidence existed. May be amended in-loop if the aggregate contract still holds and `cargo-ci` + `code-review` verify the amended criterion. The amendment must be recorded as a visible note in the issue's description with the observed number and reason (e.g., "crossover threshold updated from k≥16 to k≥4096 based on `dev/benchmarks/run-2026-04-21.csv`").

Criteria without a marker default to `[hard]`. **Correctness requirements are always `[hard]`** regardless of marker — no test-vector equality, field axiom, invariant, or API contract is ever aspirational.

Issue-extraction agents should use `[aspirational]` sparingly, only for targets that are explicitly provisional (expected throughput, speedup factors, crossover thresholds unsupported by prior measurement). When in doubt, use `[hard]`.

This is a project-local convention — JIT itself does not read or enforce the markers; enforcement is entirely in the reviewer prompt at `scripts/code-review-prompt.md`. Do not put the marker definitions in `.jit/config.toml`; that file is for JIT's own schema, not for project conventions consumed by prompt-layer agents.

## Breakdown-time feasibility check

When an issue description mentions specific CPU intrinsics, SIMD lanes, unstable library features, or toolchain-version-dependent behaviour, verify MSRV compatibility **before** accepting the breakdown. Run:

```bash
rustup run 1.95.0 cargo check --workspace --all-features
```

against a minimal stub that uses the intended intrinsic. If the intrinsic is unstable on MSRV 1.95 (or only stabilised in a newer rustc), the implementation must either: (a) restrict to stable intrinsics on the current MSRV, (b) compile-gate behind `#[cfg(all(target_arch = ..., target_feature = ...))]` with a scalar fallback on the default build, or (c) escalate to the user for MSRV bump approval before dispatch.

Previous incident: `afac2262` (AVX-512 ZMM lane) cost a rework cycle and a scope reduction because the intrinsic-feasibility check was not run during breakdown; the ZMM lane was requested on a host that has no AVX-512 hardware AND on an MSRV (then 1.80) that did not stabilise the required intrinsics. MSRV was bumped to 1.95 on 2026-04-27 so those particular intrinsics are now stable; the procedural lesson stands.

## Verification work

Any issue whose core deliverable is a formal proof (Lean4, Coq) or a model-checking harness (Kani, CBMC) is classified as **verification work** and has stricter dispatch rules than implementation work. These rules exist because verification failures look different — a worker cannot know in advance how hard a proof is or whether their approach will be accepted, so each attempt without a pre-approved design is an all-or-nothing shot.

**Before implementation is dispatched on a verification issue, a proof-sketch artefact must exist and be approved.** The proof sketch is a short markdown document (stored alongside the issue's design docs) listing:

1. **Lemmas to be proved**, in statement form only (not with full proofs). One bullet per lemma.
2. **Intended proof strategy per lemma** — the tactic or proof shape, in one line each. Examples:
   - "by induction on the loop iteration count, using `Nat.rec`"
   - "by `scalar_tac` from the bounds in `ValidPrime`"
   - "by `bv_omega` after unfolding `UScalar.val`"
   - "by unwinding the Newton iteration invariant `P * inv ≡ 1 (mod 2^(2^k))`"
3. **Exact production code path** each verification harness must exercise. For Lean4 via Charon/Aeneas, state the module path and the function name that the generated Lean definition will be proved against. For Kani, state the exact production entrypoint signature the harness must call — not a test-copied helper, not a semantically equivalent reimplementation.
4. **For Kani specifically:** the expected unwind bounds and whether the production path uses `OnceLock`-dispatched runtime tables. `OnceLock` paths typically require non-standard unwind strategies and must be flagged in the sketch.

The lead (or the user, if the work has significant architectural impact) reviews and **approves the sketch before any proof code is written**. The implementation issue is then dispatched as "implement this approved proof sketch" — a much more tightly scoped task than "prove X."

Previous incidents:
- `467d835e` needed 10 review cycles because the proof approach (axiom vs derived, placeholder vs full) was re-negotiated each cycle. A pre-approved sketch would have cut this to 2–3 cycles.
- `8889e712` needed 9 cycles — 8 of them all citing the same finding (Kani harness attached to a test-copied table helper instead of the production `OnceLock`-dispatched path). A sketch that named the production code path explicitly would have caught this on attempt 1.

Verification issues that do not have an approved sketch at dispatch time are a process bug. If you find yourself about to dispatch one, stop — create the sketch task first, wire the implementation as a dependent, and return to wave planning.
