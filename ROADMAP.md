# Development Roadmap

This document outlines the phased development plan for the `gf2` workspace. The early phases focus on the `gf2-core` primitives, while later phases inform the `gf2-coding` applications layer.

## Overview

The detailed phase plans now live alongside each crate:

- Core primitives & performance engineering: `crates/gf2-core/ROADMAP.md`
- Coding theory & compression research: `crates/gf2-coding/ROADMAP.md`

This document provides a high-level index of strategic goals and cross-cutting concerns that span both crates.

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

## Milestone Aggregation (Snapshot)

| Milestone | Core Focus | Coding Focus | Dependency |
|-----------|------------|--------------|------------|
| M1        | Scalar baseline complete | Hamming codes + basic decoder | ✅ |
| M2        | Wide buffer optimizations | Convolutional encoder skeleton | Core M2 aids throughput |
| M3        | SIMD backends & dispatch | Advanced decoding prototypes | Core SIMD for large parity checks |
| M4        | Rank/select & scanning | LDPC sparse matrices | Rank/select required |
| M5        | Polynomial arithmetic | BCH / Reed-Solomon prep | Polynomial ops required |
| M6        | Kernel safety & audits | Soft-decision decoding hooks | Shared correctness |
| M7        | N/A (support) | Compression transform research | Reuses BitVec APIs |

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
