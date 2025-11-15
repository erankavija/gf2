# gf2-core Documentation

This directory contains design documents, session notes, and performance analysis for the gf2-core crate.

## Quick Links

### Current Session (2024-11-15)
- **[Performance Session Notes](performance_session_notes.md)** - Latest optimization session, polynomial benchmarking
- **[Polynomial Benchmarks](polynomial_benchmarks.md)** - Detailed performance analysis and optimization roadmap

### GF(2^m) Implementation
- **[GF(2^m) Session Notes](GF2M_SESSION_NOTES.md)** - Complete development history of extension field implementation
- **[GF(2^m) Design](GF2M_DESIGN.md)** - Architecture and design decisions

## What to Read Next

**Starting polynomial optimization?** → Read `performance_session_notes.md`  
**Understanding GF(2^m) internals?** → Read `GF2M_DESIGN.md`  
**Want implementation history?** → Read `GF2M_SESSION_NOTES.md`

## Performance Summary

Current bottleneck: **Polynomial multiplication** (352 µs for degree 200 in GF(256))

Optimization pipeline:
1. ✅ Baseline benchmarks established
2. ⏭️ **Next**: Karatsuba multiplication (2-3x speedup expected)
3. 🔮 Future: SIMD field operations (additional 2-4x)

See `performance_session_notes.md` for detailed plan.
