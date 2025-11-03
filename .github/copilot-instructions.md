# GitHub Copilot Instructions for gf2

This document provides guidelines for GitHub Copilot when working on the `gf2` codebase - a high-performance Rust library for bit string manipulation with focus on GF(2) operations and coding theory.

## Project Overview

`gf2` is a high-performance bit manipulation library that implements:
- Dense bit vector operations (BitVec)
- GF(2) linear algebra with bit-packed matrices (BitMatrix)
- M4RM (Method of the Four Russians) matrix multiplication
- Gauss-Jordan matrix inversion
- Future: GF(2) polynomial arithmetic for coding theory

## Core Design Principles

### 1. Functional Programming Paradigm

**Prefer functional programming patterns for high-level code:**
- Use immutability where practical; prefer returning new values over mutation
- Leverage iterators and functional combinators (map, filter, fold) over explicit loops
- Write pure functions without side effects whenever possible
- Use expression-oriented code rather than statement-oriented
- Employ higher-order functions and closures for abstraction
- Avoid mutable state; when mutation is necessary for performance, encapsulate it well
- Use type-driven design with strong type safety

**⚠️ Performance has absolute priority for low-level implementations:**
- In `kernels/` and performance-critical paths, prioritize speed over functional style
- Use imperative loops, mutation, and manual optimizations when benchmarks show benefits
- Profile first, optimize second - measure before sacrificing functional style
- Encapsulate low-level optimizations behind clean, functional high-level APIs

**Examples of preferred patterns:**
```rust
// GOOD: Functional style with iterator combinators
let sum: u32 = words.iter().map(|w| w.count_ones()).sum();

// AVOID: Imperative loops when functional alternatives exist
let mut sum = 0;
for word in &words {
    sum += word.count_ones();
}

// GOOD: Pure functions that return new values
fn transpose(matrix: &BitMatrix) -> BitMatrix { ... }

// GOOD: Expression-oriented code
let result = if condition { value_a } else { value_b };
```

### 2. Test-Driven Development (TDD)

**Always follow TDD principles:**
1. **Write tests first** before implementing functionality
2. **Start with failing tests** that define the expected behavior
3. **Implement minimal code** to make tests pass
4. **Refactor** while keeping tests green
5. **Ensure comprehensive coverage** including edge cases

**Test Categories:**
- **Unit tests**: Test individual functions and methods in isolation
- **Property-based tests**: Use `proptest` to verify properties across random inputs
- **Integration tests**: Test interactions between modules
- **Edge case tests**: Cover boundary conditions (0, 1, 63, 64, 65 bits, etc.)
- **Invariant tests**: Verify design invariants (e.g., tail masking)

**Example TDD workflow:**
```rust
// Step 1: Write the test first
#[test]
fn test_bit_count_empty_vector() {
    let bv = BitVec::new();
    assert_eq!(bv.count_ones(), 0);
}

// Step 2: Implement minimal code to pass
impl BitVec {
    pub fn count_ones(&self) -> u64 {
        self.words.iter().map(|w| w.count_ones() as u64).sum()
    }
}

// Step 3: Add more comprehensive tests
#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn count_ones_matches_naive(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
            let bv = BitVec::from_bytes_le(&bytes);
            let expected = bytes.iter().map(|b| b.count_ones()).sum::<u32>() as u64;
            assert_eq!(bv.count_ones(), expected);
        }
    }
}
```

## Code Style and Conventions

### Safety and Correctness
- **No unsafe code**: The crate uses `#![deny(unsafe_code)]`
- **Validate all inputs**: Check preconditions and panic with clear messages
- **Maintain invariants**: Always preserve tail masking (padding bits must be zero)
- **Comprehensive documentation**: All public APIs must have doc comments with examples

### Naming Conventions
- Use clear, descriptive names that convey intent
- Method names should be verbs for actions (e.g., `shift_left`, `count_ones`)
- Predicates should start with `is_` or `has_` (e.g., `is_empty`)
- Use conventional Rust naming: `snake_case` for functions/variables, `PascalCase` for types

### Performance Considerations
- **Performance is the absolute priority for low-level code** (especially in `kernels/`)
- Optimize hot paths with word-level operations over bit-level
- Minimize branching in tight loops
- Use appropriate data structures (Vec<u64> for dense storage)
- Profile before optimizing; correctness comes first, then performance
- Use imperative, mutating code in kernels when benchmarks show clear benefits
- Encapsulate performance-critical code behind clean, functional high-level APIs
- Document performance characteristics in doc comments

### GF(2) Domain-Specific Guidelines
- Operations are over the binary field GF(2) where addition is XOR
- Matrix multiplication uses M4RM algorithm for efficiency
- Maintain mathematical correctness - verify against reference implementations
- Use appropriate terminology from coding theory and linear algebra

## Module Structure

- **bitvec.rs**: Core bit vector operations (get, set, push, pop, shifts, bitwise ops) - functional style preferred
- **matrix.rs**: Bit-packed boolean matrices for GF(2) linear algebra - functional style preferred  
- **alg/**: Algorithms including M4RM multiplication and Gauss-Jordan inversion - functional style preferred
- **kernels/**: **Performance-critical** low-level operation kernels with potential SIMD implementations - **imperative style and mutation encouraged for speed**
- **tests/**: Comprehensive test suites including property-based tests

## Testing Guidelines

### Property-Based Testing
Use `proptest` for validating properties:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn roundtrip_bytes_to_bitvec(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let roundtrip = bv.to_bytes_le();
        assert_eq!(bytes, roundtrip);
    }
}
```

### Test Organization
- Keep unit tests in the same file as implementation using `#[cfg(test)] mod tests`
- Put integration tests in `tests/` directory
- Use descriptive test names: `test_<operation>_<scenario>` (e.g., `test_shift_left_word_boundary`)
- Test boundary conditions explicitly (0, 1, word boundaries at 63, 64, 65 bits)

### Test Coverage
Aim for comprehensive coverage:
- Happy path scenarios
- Edge cases and boundary conditions
- Error conditions (panics with clear messages)
- Invariant preservation (especially tail masking)
- Mathematical properties (e.g., A × I = A for matrices)

## Documentation Standards

Every public item must have:
- A doc comment explaining what it does
- Parameter descriptions (using `///` or `/** */`)
- Return value documentation
- Example usage in doc comments (tested via `cargo test`)
- Panics section if applicable
- Complexity notes for non-trivial operations

Example:
```rust
/// Performs a logical left shift of the bit vector by `k` positions.
///
/// Bits shifted off the left end are discarded. New bits on the right are filled with zeros.
/// This operation maintains the tail masking invariant.
///
/// # Arguments
///
/// * `k` - The number of positions to shift left
///
/// # Examples
///
/// ```
/// use gf2::BitVec;
///
/// let mut bv = BitVec::from_bytes_le(&[0b00001111]);
/// bv.shift_left(2);
/// assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
/// ```
///
/// # Complexity
///
/// O(n) where n is the number of words in the bit vector.
pub fn shift_left(&mut self, k: usize) { ... }
```

## Common Patterns

### Bit Manipulation
```rust
// Extracting a bit at position i
let word = i >> 6;  // i / 64
let mask = 1u64 << (i & 63);  // 1u64 << (i % 64)
let bit = (self.words[word] & mask) != 0;

// Setting a bit at position i
if value {
    self.words[word] |= mask;
} else {
    self.words[word] &= !mask;
}
```

### Tail Masking (Critical Invariant)
```rust
// Always mask padding bits after mutation
fn mask_tail(&mut self) {
    if let Some(&last_word) = self.words.last() {
        let used_bits = self.len_bits & 63;
        if used_bits > 0 {
            let mask = (1u64 << used_bits) - 1;
            *self.words.last_mut().unwrap() &= mask;
        }
    }
}
```

### Iterator Usage (Functional Style)
```rust
// Prefer functional combinators
let ones: usize = self.words.iter()
    .map(|w| w.count_ones() as usize)
    .sum();

// Chain operations functionally
let result = input.iter()
    .filter(|&x| condition(x))
    .map(|x| transform(x))
    .collect();
```

## MSRV and Dependencies

- **Minimum Supported Rust Version (MSRV)**: 1.74
- Keep dependencies minimal
- Use standard library features when possible
- Dev dependencies: `proptest` for property tests, `criterion` for benchmarks

## When Adding Features

1. **Write tests first** (TDD approach)
2. **Consider functional design**: Can this be implemented with pure functions and immutability?
3. **Document invariants**: What must remain true after this operation?
4. **Add examples**: Include doc comment examples that are tested
5. **Benchmark if performance-critical**: Add to `benches/` if this is a hot path
6. **Update README**: If adding significant functionality, update usage examples

## Review Checklist

Before suggesting code, ensure:
- [ ] Tests written before implementation (TDD)
- [ ] Functional programming principles applied where practical
- [ ] No unsafe code
- [ ] Tail masking invariant maintained
- [ ] Public APIs have comprehensive doc comments with examples
- [ ] Property-based tests for non-trivial logic
- [ ] Edge cases covered (0, 1, word boundaries)
- [ ] Clear panic messages for invalid inputs
- [ ] Performance considerations documented

## Additional Resources

- See `README.md` for API overview and examples
- See `ROADMAP.md` for planned features and performance phases
- Run `cargo test` for full test suite
- Run `cargo bench` for performance benchmarks
- Run `cargo doc --no-deps --open` for full API documentation
