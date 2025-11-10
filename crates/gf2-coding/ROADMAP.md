# gf2-coding Roadmap

This roadmap captures the higher-level coding theory and compression research layers built atop `gf2-core` primitives. It intentionally separates exploratory, algorithmic work from low-level performance engineering.

## Phase C1: Foundational Block Codes (Complete)
- ✅ Linear block code abstraction (`LinearBlockCode`) with generator (G) & parity-check (H) matrices
- ✅ Systematic encoding path; syndrome computation; simple Hamming construction helper
- ✅ Deterministic tests for encode/decode roundtrips
- ✅ Property-based tests with `proptest` (roundtrip, linearity, error correction, orthogonality)
- ✅ Integration tests covering full workflows and edge cases
- ✅ Benchmarks: encoding throughput, syndrome computation, decoding with/without errors, batch operations

## Phase C2: Convolutional Code Framework (Planned)
- Shift-register based encoder/decoder scaffolding (current skeleton) → full implementation
- Polynomial representation for generator taps (re-uses future `GF2Poly`)
- Streaming tests: bit error insertion, recovery latency, state reset correctness

## Phase C3: Advanced Decoding Algorithms (Planned)
- Syndrome table optimization (compressed mapping)
- Berlekamp–Massey for BCH-like codes (depends on polynomial arithmetic)
- Chien search integration for root finding in GF(2^m)
- Soft-decision hooks: placeholder traits for LLR-based decoding

## Phase C4: Sparse & Graph-Based Codes (Research)
- LDPC sparse matrix format (CSR / bit-packed hybrid)
- Belief propagation decoding; early stopping criteria; damping strategies
- Profiling memory bandwidth vs. iteration count

## Phase C5: Polar & Modern Codes (Research)
- Successive cancellation (SC) and SC-List decoder prototypes
- Bit-channel reliability sorting; fast Hadamard-like transforms leveraging `BitMatrix`
- Evaluate integration points with rank/select primitives

## Phase C6: Compression Experiments (Exploratory)
- Bit-level transforms (run-length, delta, XOR chaining) using `BitVec` APIs
- Entropy modeling playground (simple adaptive frequency coder over GF(2) residuals)
- Comparative benchmarks: raw vs. transformed bitstreams

## Phase C7: Performance & Ergonomics Polish (Ongoing)
- Unified error handling and panic messages → shift towards `Result` where appropriate
- Trait refinements: streaming vs. batch encode/decode unification
- Doc examples with visual syndrome / decoding traces

## Research Placeholders / Open Questions
- Optimal data structures for extremely sparse parity-check matrices?
- When to switch from table-based to algebraic decoding for medium block lengths?
- Feasibility of GPU offload (via future crates) for LDPC iterations?
- Interplay between compression transforms and error correction ordering

## Principles
- Keep experimental algorithms isolated—avoid regressing core performance
- Favor clarity & correctness first; optimize after baseline metrics exist
- Rich documentation & visual aids for educational usability

Refer back to `crates/gf2-core/ROADMAP.md` for low-level performance phases.
