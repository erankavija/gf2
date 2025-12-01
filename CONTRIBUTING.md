# Contributing to gf2

Thank you for your interest in contributing to the gf2 project! This document provides guidelines and information for contributors.

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and constructive in all interactions.

## Getting Started

### Prerequisites

- Rust 1.80 or later (MSRV)
- Git for version control
- Familiarity with coding theory and linear algebra (helpful but not required)

### Setting Up the Development Environment

```bash
# Clone the repository
git clone https://github.com/yourusername/gf2.git
cd gf2

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run tests with all features
cargo test --workspace --all-features

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --workspace --all-targets --all-features
```

## Project Structure

```
gf2/
├── crates/
│   ├── gf2-core/          # Core bit manipulation and GF(2) linear algebra
│   ├── gf2-coding/        # Error-correcting codes (BCH, LDPC, convolutional)
│   └── gf2-kernels-simd/  # SIMD-optimized kernels
├── docs/                   # Project-wide documentation
└── .github/workflows/      # CI/CD configuration
```

## Development Workflow

### 1. Test-Driven Development (TDD)

We follow TDD principles strictly:

1. **Write tests first** before implementing functionality
2. **Start with failing tests** that define expected behavior
3. **Implement minimal code** to make tests pass
4. **Refactor** while keeping tests green
5. **Add property-based tests** for mathematical invariants

Example workflow:
```rust
// Step 1: Write the test
#[test]
fn test_bit_count_empty_vector() {
    let bv = BitVec::new();
    assert_eq!(bv.count_ones(), 0);
}

// Step 2: Implement to pass
impl BitVec {
    pub fn count_ones(&self) -> u64 {
        self.words.iter().map(|w| w.count_ones() as u64).sum()
    }
}

// Step 3: Add property tests
proptest! {
    #[test]
    fn count_ones_matches_naive(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let expected = bytes.iter().map(|b| b.count_ones()).sum::<u32>() as u64;
        assert_eq!(bv.count_ones(), expected);
    }
}
```

### 2. Coding Standards

#### Functional Programming Style

Prefer functional programming patterns for high-level code:

```rust
// GOOD: Functional style
let sum: u32 = words.iter().map(|w| w.count_ones()).sum();

// AVOID: Imperative loops when functional alternatives exist
let mut sum = 0;
for word in &words {
    sum += word.count_ones();
}
```

**Exception:** Performance-critical code in `kernels/` may use imperative style and mutation when benchmarks show clear benefits.

#### Safety

- **No unsafe code**: The project uses `#![deny(unsafe_code)]`
- **Validate all inputs**: Check preconditions and panic with clear messages
- **Maintain invariants**: Document and preserve invariants (e.g., tail masking)

#### Documentation

All public APIs must have:
- Doc comments explaining functionality
- Parameter descriptions
- Return value documentation
- Example usage (tested via `cargo test --doc`)
- Panic conditions documented
- Complexity notes for non-trivial operations

Example:
```rust
/// Performs a logical left shift of the bit vector by `k` positions.
///
/// Bits shifted off the left end are discarded. New bits on the right are zeros.
///
/// # Arguments
///
/// * `k` - Number of positions to shift left
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

### 3. Testing Requirements

Every contribution must include:

#### Unit Tests
- Test individual functions in isolation
- Cover happy path, edge cases, and error conditions
- Test boundary values (0, 1, 63, 64, 65 bits for word boundaries)

#### Property-Based Tests
- Use `proptest` for mathematical properties
- Validate invariants across random inputs
- Test roundtrip properties

Example:
```rust
proptest! {
    #[test]
    fn roundtrip_bytes_to_bitvec(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let roundtrip = bv.to_bytes_le();
        assert_eq!(bytes, roundtrip);
    }
}
```

#### Integration Tests
- Test interactions between modules
- Verify end-to-end workflows

### 4. Performance Considerations

- **Profile before optimizing**: Use `cargo bench` and profiling tools
- **Optimize hot paths**: Focus on word-level operations over bit-level
- **Document performance**: Include complexity analysis in doc comments
- **Benchmark critical changes**: Add benchmarks for performance-sensitive code

### 5. Git Workflow

#### Branches
- `main` - stable, production-ready code
- `copilot/**` - feature branches (e.g., `copilot/add-polar-codes`)

#### Commit Messages
Follow conventional commits format:
```
type(scope): brief description

Longer explanation if needed

Fixes #123
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`

Example:
```
feat(ldpc): add DVB-T2 short frame support

Implements LDPC encoding/decoding for DVB-T2 short frames (n=16200).
Includes property tests and test vector validation.

Refs #45
```

#### Pull Request Process

1. **Create a feature branch** from `main`
2. **Write tests first** (TDD)
3. **Implement the feature**
4. **Ensure all tests pass**: `cargo test --workspace --all-features`
5. **Check formatting**: `cargo fmt --all`
6. **Run clippy**: `cargo clippy --workspace --all-targets --all-features`
7. **Update documentation** if needed
8. **Submit PR** with clear description

PR template:
```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Added unit tests
- [ ] Added property tests
- [ ] Added integration tests
- [ ] All tests pass

## Checklist
- [ ] Code follows project style guidelines
- [ ] Documentation updated
- [ ] CHANGELOG updated (if applicable)
```

## Specific Contribution Areas

### Adding New Error-Correcting Codes

When adding a new code type (e.g., Polar codes, Turbo codes):

1. **Implement traits**: `BlockEncoder`, `HardDecisionDecoder`, or `SoftDecoder`
2. **Add factory methods**: Standard-specific constructors (e.g., `PolarCode::nr_5g()`)
3. **Write comprehensive tests**:
   - Unit tests for encoding/decoding
   - Property tests for mathematical properties
   - Test vectors from standards (if available)
4. **Add benchmarks**: Encoding/decoding throughput
5. **Document thoroughly**: Algorithm description, complexity, references
6. **Add example**: Demonstrate usage in `examples/`

### Performance Optimization

1. **Profile first**: Use `cargo bench` and `perf`/`flamegraph`
2. **Benchmark before and after**: Demonstrate improvement
3. **Consider SIMD**: Add kernels to `gf2-kernels-simd` if appropriate
4. **Maintain correctness**: Ensure property tests still pass
5. **Document trade-offs**: Explain complexity vs performance

### Documentation Improvements

- Fix typos and clarify explanations
- Add more examples to doc comments
- Improve README or guides in `docs/`
- Add educational content for coding theory concepts

## Questions or Issues?

- **Found a bug?** Open an issue with minimal reproducible example
- **Have a question?** Open a discussion or issue
- **Want to propose a feature?** Open an issue for discussion first

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (see LICENSE file).

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Project ROADMAP](crates/gf2-coding/ROADMAP.md)
- [Project Guidelines](.github/copilot-instructions.md)

Thank you for contributing to gf2! 🎉
