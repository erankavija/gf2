# GF(2^m) Thread Safety Requirements

**Date**: 2025-11-30  
**Status**: Requirements Definition  
**Priority**: MEDIUM (blocks BCH parallel batch operations)  
**Target**: gf2-core v0.2.0

## Problem Statement

`Gf2mField` uses `Rc<FieldParams>` internally, which is not `Send + Sync`. This prevents parallel batch operations on BCH (and future Reed-Solomon, Goppa) codes.

### Current Architecture

```rust
pub struct Gf2mField {
    params: Rc<FieldParams>,  // ❌ Not Send + Sync
}
```

**Impact:**
- ❌ BCH `decode_batch()` cannot use rayon parallelism
- ❌ Reed-Solomon batch operations will have same limitation
- ❌ Any GF(2^m)-based code cannot parallelize efficiently
- ✅ LDPC codes work fine (use GF(2) only, no extension fields)

### Why This Matters

BCH and Reed-Solomon codes are computationally expensive:
- **BCH decoding**: Berlekamp-Massey (O(n²)), Chien search (O(n·t))
- **RS decoding**: Similar complexity
- **Typical workload**: 100-1000 codewords per batch
- **Performance gap**: 1× (sequential) vs 8-12× (parallel on 12-core CPU)

Without parallelism, BCH/RS throughput is 8-12× slower than LDPC.

## Requirements

### REQ-1: Replace `Rc` with `Arc` in `Gf2mField`

**Priority**: HIGH  
**Effort**: Low (2-4 hours)  
**Breaking Change**: No (transparent to users)

#### Specification

```rust
// BEFORE:
use std::rc::Rc;

pub struct Gf2mField {
    params: Rc<FieldParams>,
}

// AFTER:
use std::sync::Arc;

pub struct Gf2mField {
    params: Arc<FieldParams>,  // ✅ Send + Sync
}
```

#### Implementation Notes

- **Clone semantics**: Identical (both use reference counting)
- **Performance**: Arc has atomic operations (slight overhead, negligible in practice)
- **Memory**: No change (same reference counting)
- **API**: No changes required (internal implementation detail)

#### Testing

```rust
#[test]
fn test_field_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    
    assert_send::<Gf2mField>();
    assert_sync::<Gf2mField>();
    assert_send::<Gf2mElement>();
    assert_sync::<Gf2mElement>();
}

#[test]
fn test_field_clone_across_threads() {
    use std::thread;
    
    let field = Gf2mField::new(8, 0b100011101).with_tables();
    
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let field = field.clone();
            thread::spawn(move || {
                // Use field in separate thread
                let a = field.element(0x42);
                let b = field.element(0x17);
                &a * &b
            })
        })
        .collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
}
```

### REQ-2: Update `Gf2mElement` Clone Implementation

**Priority**: HIGH  
**Effort**: Trivial (30 minutes)

`Gf2mElement` also contains `Rc<FieldParams>`:

```rust
pub struct Gf2mElement {
    value: u64,
    params: Rc<FieldParams>,  // ❌ Also needs Arc
}
```

Must be updated consistently with `Gf2mField`.

### REQ-3: Performance Validation

**Priority**: MEDIUM  
**Effort**: Low (1-2 hours)

#### Benchmark Arc vs Rc Overhead

```rust
// Measure clone performance
#[bench]
fn bench_field_clone_rc(b: &mut Bencher) {
    let field = Gf2mField::new(8, 0b100011101);
    b.iter(|| black_box(field.clone()));
}

#[bench]
fn bench_field_clone_arc(b: &mut Bencher) {
    let field = Gf2mField::new(8, 0b100011101);
    b.iter(|| black_box(field.clone()));
}
```

**Expected result**: Arc adds ~2-5ns per clone (atomic operations). Negligible compared to polynomial operations (100s-1000s of ns).

## Impact Analysis

### BCH Code Parallelization (gf2-coding)

**After Fix:**
```rust
impl BchDecoder {
    pub fn decode_batch(&self, received: &[BitVec]) -> Vec<BitVec> {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            let code = self.code.clone();
            (0..received.len())
                .into_par_iter()
                .map(|i| {
                    let decoder = BchDecoder::new(code.clone());  // ✅ Now Send+Sync
                    decoder.decode(&received[i])
                })
                .collect()
        }
        
        #[cfg(not(feature = "parallel"))]
        {
            received.iter().map(|cw| self.decode(cw)).collect()
        }
    }
}
```

### Expected Performance Gains

**DVB-T2 BCH (16200, 16008, t=12)** batch of 100 codewords:

| Implementation | Throughput | Speedup |
|----------------|------------|---------|
| Sequential (current) | ~50 Mbps | 1.0× |
| Parallel (after fix) | ~400 Mbps | 8.0× |

### Breaking Changes

**None**. This is an internal implementation change. All existing code continues to work.

### Migration Path

1. **gf2-core**: Change `Rc` to `Arc` in `Gf2mField` and `Gf2mElement`
2. **gf2-core**: Add Send/Sync tests
3. **gf2-core**: Run full test suite (all 452 tests must pass)
4. **gf2-coding**: Enable BCH parallel decode_batch
5. **gf2-coding**: Add parallel consistency tests

## Alternative Approaches Considered

### Option 1: Thread-Local Fields (Rejected)

```rust
pub fn decode_batch(&self, received: &[BitVec]) -> Vec<BitVec> {
    use rayon::prelude::*;
    thread_local! {
        static FIELD_CACHE: RefCell<HashMap<FieldKey, Gf2mField>> = ...;
    }
    // Complex caching logic
}
```

**Pros**: Works with Rc  
**Cons**: 
- Complex caching logic
- Memory overhead (multiple field copies)
- Hard to reason about lifetime

### Option 2: Arc from Day One (Best)

Just use Arc from the start. The performance difference is negligible for mathematical operations.

**Chosen**: This option (REQ-1).

### Option 3: Feature Flag `thread-safe` (Rejected)

```toml
[features]
thread-safe = []  # Use Arc instead of Rc
```

**Pros**: Maximum performance for single-threaded use  
**Cons**:
- API complexity (two build variants)
- Confusing for users
- Maintenance burden
- Negligible performance gain in practice

## Implementation Plan

### Phase 1: Core Changes (Week 1)

1. **Day 1**: Change `Rc` to `Arc` in gf2-core
   - Update `Gf2mField` struct
   - Update `Gf2mElement` struct
   - Update `FieldParams` usage
   - Run all gf2-core tests (452 tests must pass)

2. **Day 2**: Add thread safety tests
   - Send + Sync trait bounds
   - Cross-thread field cloning
   - Concurrent element operations
   - Run under miri (Rust's undefined behavior detector)

3. **Day 3**: Performance validation
   - Benchmark Arc vs Rc clone overhead
   - Benchmark field operations (no regression expected)
   - Document results

### Phase 2: Enable BCH Parallelism (Week 1)

4. **Day 4**: Update gf2-coding BCH decoder
   - Enable parallel decode_batch
   - Add parallel consistency tests
   - Run all 226 gf2-coding tests

5. **Day 5**: Benchmark BCH parallel performance
   - Measure 1, 2, 4, 8, 12, 24 thread scaling
   - Document speedup vs sequential
   - Update PARALLELIZATION_STRATEGY.md

## Success Criteria

- ✅ All 452 gf2-core tests pass
- ✅ All 226 gf2-coding tests pass
- ✅ `Gf2mField` is `Send + Sync`
- ✅ `Gf2mElement` is `Send + Sync`
- ✅ BCH decode_batch uses rayon parallelism
- ✅ No performance regression in single-threaded benchmarks
- ✅ 6-8× speedup in BCH batch decoding on 12-core CPU
- ✅ Zero breaking changes to public API

## References

### Related Issues

- gf2-coding: BCH batch API blocked by Rc (this document)
- gf2-coding: Reed-Solomon batch API (future, same issue)
- Technical debt: Move poly_from_exponents to gf2-core

### Documentation

- [Rust Book: Rc vs Arc](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Send and Sync traits](https://doc.rust-lang.org/nomicon/send-and-sync.html)
- [Rayon parallel iterator requirements](https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html)

### Similar Work

- LDPC codes use BitMatrix/SparseMatrix (no Rc) → works with rayon ✅
- BCH codes use Gf2mField (with Rc) → blocked ❌
- Convolutional codes (no field arithmetic) → works with rayon ✅

## Open Questions

1. **Should Arc be behind a feature flag?**
   - **Answer**: No. Always use Arc. Performance difference is negligible.

2. **Will this break no_std support?**
   - **Answer**: No. Arc is available in `alloc` crate (no_std compatible).

3. **Should we make FieldParams Clone instead of using Arc?**
   - **Answer**: No. FieldParams can be 64KB+ (tables), Arc is more efficient.

4. **Impact on existing code?**
   - **Answer**: Zero. Rc and Arc have identical clone semantics for users.

## Timeline

- **Week 1, Day 1-3**: Implement Arc migration in gf2-core (3 days)
- **Week 1, Day 4-5**: Enable BCH parallelism in gf2-coding (2 days)
- **Total**: 5 days (1 week)

**Dependencies**: None (can start immediately)

**Priority**: HIGH (unblocks all GF(2^m) code parallelization)
