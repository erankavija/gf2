# Development Roadmap

This document outlines the phased development plan for the `gf2` workspace. The early phases focus on the `gf2-core` primitives, while later phases inform the `gf2-coding` applications layer.

## Overview

The detailed phase plans now live alongside each crate:

- Core primitives & performance engineering: `crates/gf2-core/ROADMAP.md`
- Coding theory & compression research: `crates/gf2-coding/ROADMAP.md`

This document provides a high-level index of strategic goals and cross-cutting concerns that span both crates.

## Current Focus (2024-11-15)

**gf2-core** polynomial arithmetic optimization complete:
- ✅ **Complete**: GF(2^m) field arithmetic with table-based multiplication
- ✅ **Complete**: Polynomial operations (add, multiply, divide, GCD, eval)
- ✅ **Complete**: Minimal polynomial computation for BCH codes
- ✅ **Complete**: Batch polynomial evaluation for syndrome computation
- ✅ **Complete**: Comprehensive benchmarks establishing baseline performance
- 🎯 **Next**: Karatsuba multiplication for 2-3x speedup (optional optimization)

**gf2-coding** BCH implementation complete, ready for DVB-T2 LDPC:
- ✅ **Complete**: BCH encoder/decoder with algebraic decoding
- ✅ **Complete**: Berlekamp-Massey algorithm and Chien search
- ✅ **Complete**: DVB-T2 BCH parameters (all code rates, both frame sizes)
- ✅ **Complete**: 45 tests with comprehensive validation
- 🎯 **In Progress**: DVB-T2 LDPC quasi-cyclic codes (Phase C10.1)
- 🔮 **Next**: QAM modulation and FEC chain integration

See `crates/gf2-core/docs/performance_session_notes.md` for optimization roadmap.
See `crates/gf2-coding/ROADMAP.md` for DVB-T2 implementation timeline.

---

## Strategic Pillars

1. Performance & Correctness (gf2-core)
2. Algorithmic Breadth & Education (gf2-coding)
3. Composability & Clean APIs (workspace-wide)
4. Documentation & Demonstrations (workspace-wide)
5. Research Experimentation (gf2-coding)

## Cross-Cutting Themes

- Tail masking invariants remain foundational for all bit-level operations.
- Runtime dispatch and potential SIMD acceleration in core unlock faster high-level algorithms.
- Property-based tests validate both primitive and composite behaviors.
- Polynomial arithmetic (core) is a dependency enabler for advanced decoding (coding).
- Rank/select primitives (core) feed sparse graph and syndrome operations (coding).

## Milestone Aggregation (Current State)

| Milestone | Core Focus | Coding Focus | Status |
|-----------|------------|--------------|--------|
| M1        | Scalar baseline complete | Hamming codes + basic decoder | ✅ Complete |
| M2        | SIMD backends (x86_64 AVX2) | Convolutional encoder | ✅ Core complete |
| M3        | Extension field GF(2^m) arithmetic | BCH algebraic decoding | ✅ Complete |
| M4        | Sparse matrix primitives | LDPC sparse matrices | ✅ Core complete |
| M5        | Polynomial optimization (Karatsuba) | DVB-T2 LDPC construction | 🎯 Coding in progress |
| M6        | Rank/select & scanning | DVB-T2 QAM modulation | Planned |
| M7        | Polar transforms | DVB-T2 FEC simulation | Planned |
| M8        | Kernel safety & audits | Production readiness | Ongoing |

## Open Research Questions (Workspace View)

- Balancing general abstractions with specialized fast paths: how much auto-dispatch at coding layer?
- Interplay between transform-based compression and error correction ordering for pipeline design.
- Feasibility of GPU or heterogeneous acceleration without fragmenting API ergonomics.
- Standardizing polynomial representations across code families for reuse.

## Contribution Guide (Summary)

See crate-specific roadmaps for granular tasks. High-impact areas:

- Benchmark coverage across diverse CPU microarchitectures.
- Expanding educational examples (annotated decoding traces, syndrome walkthroughs).
- Safety audits of future SIMD and `unsafe` blocks.
- Research implementations documented with reproducible benchmarks.

## Future Vision

A cohesive toolkit where low-level bit operations, linear algebra, polynomial arithmetic, and rich code families interoperate seamlessly—serving production systems and research labs alike.

*For detailed tasks and timelines, consult the per-crate roadmap files. This overview remains intentionally concise and strategic.*
