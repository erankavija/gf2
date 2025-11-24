# gf2-core

`gf2-core` provides the high-performance primitives that power the `gf2` workspace. It focuses on bit-level data structures and linear algebra over the binary field (GF(2)).

## Highlights

- Dense, tail-masked `BitVec` backed by `Vec<u64>`
- Bit-packed `BitMatrix` with M4RM multiplication and Gauss-Jordan inversion
- Polar transform operations for polar code encoding/decoding
- Sparse matrices in CSR/CSC formats for low-density matrices
- Extension field GF(2^m) arithmetic with table-based and SIMD multiplication
- Primitive polynomial generation and verification (exhaustive, trinomial, parallel)
- SIMD-accelerated operations (AVX2) for supported platforms
- Strict safety: `#![deny(unsafe_code)]`

## Features

- **default**: Scalar implementations with random generation (`rand`)
- **simd**: Enables SIMD-accelerated operations (opt-in)
  - AVX2 logical ops, popcount, scans, shifts on x86_64
  - PCLMULQDQ field multiplication for GF(2^m) when m > 16
- **rand**: Random BitVec and BitMatrix generation (enabled by default)
  - To opt-out, use `default-features = false`
- **visualization**: Save BitMatrix as PNG images (opt-in)
  - Useful for debugging and visual inspection of matrix patterns

## Architecture

**Three-layer design** optimizes for both ergonomics and performance:

1. **Public API** (`BitVec`, `BitMatrix`) - Functional, ergonomic operations
2. **Kernel Ops** (`kernels::ops`) - Smart dispatch based on operation size
3. **Backends** - Pluggable implementations:
   - **Scalar**: Pure Rust baseline (always available)
   - **SIMD**: AVX2/NEON acceleration (optional, runtime detected)
   - **Future**: GPU/FPGA backends planned

Operations automatically use the fastest backend available. Small operations (<512 bytes) use scalar code to avoid dispatch overhead. Large operations leverage SIMD when available (3.4-3.6x speedup validated).

**Documentation:**
- [`docs/KERNEL_OPTIMIZATION.md`](docs/KERNEL_OPTIMIZATION.md) - Complete kernel architecture guide
- [`docs/BENCHMARK_RESULTS_SIMD_VS_SCALAR.md`](docs/BENCHMARK_RESULTS_SIMD_VS_SCALAR.md) - SIMD performance analysis

## Performance

Benchmarked against SageMath and specialized C/C++ libraries (NTL, M4RI, FLINT). Competitive to superior performance across most operations, with continuous optimization efforts.

**Recent work**: M4RM matrix multiplication optimization (gray code ordering + flat buffer reuse) and comprehensive performance gap analysis.

See **[`docs/BENCHMARKS.md`](docs/BENCHMARKS.md)** for comprehensive performance analysis, comparisons, and optimization details.

## Usage

Add the crate to your project (from crates.io or via `git` path):

```toml
[dependencies]
gf2-core = "0.1"  # rand enabled by default

# With SIMD acceleration
gf2-core = { version = "0.1", features = ["simd"] }

# Without rand (minimal build)
gf2-core = { version = "0.1", default-features = false }
```

Example:

```rust
use gf2_core::BitVec;

let mut bits = BitVec::new();
bits.push_bit(true);
bits.push_bit(false);
assert_eq!(bits.count_ones(), 1);

// Scan operations
let bv = BitVec::from_bytes_le(&[0b0001_0000]);
assert_eq!(bv.find_first_one(), Some(4));
```

### Random Generation (enabled by default)

```rust
use gf2_core::{BitVec, matrix::BitMatrix};
use rand::rngs::StdRng;
use rand::SeedableRng;

// Create random bit vector (p=0.5 for each bit)
let mut rng = StdRng::seed_from_u64(42);
let bv = BitVec::random(1000, &mut rng);

// Create random matrix with custom probability
let sparse_matrix = BitMatrix::random_with_probability(100, 100, 0.1, &mut rng);

// Deterministic random generation
let bv = BitVec::random_seeded(500, 0x1234);
```

### Sparse Matrices

```rust
use gf2_core::sparse::{SparseMatrix, SparseMatrixDual};
use gf2_core::BitVec;

// Build from coordinate (COO) format
let coo = vec![(0, 1), (0, 3), (1, 2), (1, 4)];
let sparse = SparseMatrix::from_coo(2, 5, &coo);

// Matrix-vector multiply for syndrome computation
let x = BitVec::random(5, &mut rng);
let y = sparse.matvec(&x);

// Row iteration
for col in sparse.row_iter(0) {
    println!("Row 0 has nonzero at column {}", col);
}

// For bidirectional access patterns, use dual representation
let dual = SparseMatrixDual::from_coo(2, 5, &coo);

// Fast row access (CSR)
for col in dual.row_iter(0) {
    println!("Row 0, column {}", col);
}

// Fast column access (CSC - no transpose!)
for row in dual.col_iter(3) {
    println!("Column 3, row {}", row);
}

// Both A×x and A^T×x are efficient
let y = dual.matvec(&x);
let yt = dual.matvec_transpose(&y);
```

### Polar Transform Operations

Fast Hadamard Transform for polar code encoding/decoding:

```rust
use gf2_core::BitVec;

// Create information bits for polar encoding
let mut info = BitVec::zeros(1024);
info.set(0, true);
info.set(512, true);

// Apply polar transform G_N = [1 0; 1 1]^⊗log2(N)
let encoded = info.polar_transform(1024);

// Decode via inverse transform
let decoded = encoded.polar_transform_inverse(1024);
assert_eq!(decoded, info);

// In-place operations for efficiency
let mut bits = BitVec::from_bytes_le(&[0xFF, 0x00, 0xFF, 0x00]);
bits.polar_transform_into(32);

// Bit-reversal permutation (used in polar code construction)
let mut bv = BitVec::from_bytes_le(&[0b11001010]);
bv.bit_reverse_into(8);
```

### Matrix Visualization (Optional)

Enable the `visualization` feature to save matrices as PNG images for debugging and inspection:

```toml
[dependencies]
gf2-core = { version = "0.1", features = ["visualization"] }
```

```rust
use gf2_core::matrix::BitMatrix;

// Create and visualize an identity matrix
let m = BitMatrix::identity(100);
m.save_image("identity.png").unwrap();

// Visualize matrix structure
let h = BitMatrix::random_with_probability(50, 100, 0.2, &mut rng);
h.save_image("parity_check_matrix.png").unwrap();
```

Each bit becomes a pixel: unset (0) → black, set (1) → white.  
To change colors, edit the `ZERO_COLOR` and `ONE_COLOR` constants in `src/matrix.rs`.

## Benchmarking

Run Rust benchmarks:
```bash
cargo bench --bench <name>
# Examples: primitive_poly, generation, polynomial, matmul, sparse, polar
```

For C/C++ library comparisons (requires NTL, M4RI, FLINT):
```bash
cd benchmarks-cpp/build && cmake .. && make
./bench_ntl_field
./bench_m4ri_matrix  
./bench_flint_poly
```

See [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) for detailed performance analysis.

---

For more details, see the workspace-level [README](../../README.md) and the inlined Rustdocs.
