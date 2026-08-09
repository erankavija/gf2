// Permanent computation kernel for GF(5) (F_5) on gfx1030.
//
// Implements `permanent_bipedal5_hip_batch`: a batched entry point that
// computes the Ryser permanent over F_5 for M independent n×n matrices in a
// single kernel launch. One HIP block is assigned to each matrix; only
// thread 0 of each block executes the Gray walk (sequential by design —
// the per-step work is O(n) byte ops + one O(n) fold). GPU parallelism
// comes from having many matrices in flight simultaneously.
//
// Algorithm mirrors `permanent_bipedal5_singleword` at
// `crates/gf2-algebra/src/permanent/bipedal5.rs:173` (SSOT). The SSOT
// must be kept bit-identical to this device kernel.
//
// Encoding choice (F_5 GPU kernel):
//   Direct byte arithmetic — each GF(5) element is a uint8_t in {0,1,2,3,4}.
//   No bit-plane encoding (Packed5) is used on the GPU; byte-by-byte F_5
//   arithmetic is simpler and faithful to the same mathematical computation.
//   Bit-identity is guaranteed because both sides compute the same Ryser sum
//   over F_5 (the mathematical result is the same regardless of representation).
//
// Gray-code enumeration (SSOT: `crates/gf2-algebra/src/gray.rs`):
//   At step k (k = 1 .. 2^n - 1):
//     flip   = trailing_zeros(k)    (which bit toggled)
//     g_k    = k ^ (k >> 1)        (current subset bitmask)
//     parity = (g_k >> flip) & 1   (+1 if added, 0 if removed)
//
// CAUTION: Do NOT use `(k >> flip) & 1` as the parity test — with
// `flip = trailing_zeros(k)`, bit `flip` of `k` is always 1, so that
// predicate would always return +1 and the kernel would only ever add,
// never subtract. Use `(g_k >> flip) & 1` instead (see gray.rs §6 note).
//
// Compile with: hipcc --offload-arch=gfx1030 -fPIC -O3

#include <hip/hip_runtime.h>
#include <stdint.h>

// ---------------------------------------------------------------------------
// F_5 scalar arithmetic helpers (for column sums and accumulating totals).
//
// All values are in {0, 1, 2, 3, 4}.
// ---------------------------------------------------------------------------

// fp5_add — (a + b) mod 5 for a, b in {0, 1, 2, 3, 4}.
__device__ __forceinline__ uint8_t fp5_add(uint8_t a, uint8_t b) {
    uint8_t s = a + b;
    return (s >= 5) ? (s - 5) : s;
}

// fp5_sub — (a - b) mod 5 for a, b in {0, 1, 2, 3, 4}.
__device__ __forceinline__ uint8_t fp5_sub(uint8_t a, uint8_t b) {
    // (a - b) mod 5 = (a + (5 - b)) mod 5.
    return fp5_add(a, (uint8_t)(5 - b));
}

// fp5_neg — (-a) mod 5 for a in {0, 1, 2, 3, 4}.
__device__ __forceinline__ uint8_t fp5_neg(uint8_t a) {
    return (a == 0) ? (uint8_t)0 : (uint8_t)(5 - a);
}

// fp5_mul — (a * b) mod 5 for a, b in {0, 1, 2, 3, 4}.
__device__ __forceinline__ uint8_t fp5_mul(uint8_t a, uint8_t b) {
    return (uint8_t)((unsigned int)a * (unsigned int)b % 5u);
}

// fp5_scalar_add — add two F_5 scalars stored as uint64_t in {0..4}.
// Used for the Ryser accumulator total.
__device__ __forceinline__ uint64_t fp5_scalar_add(uint64_t a, uint64_t b) {
    uint64_t s = a + b;
    return (s >= 5ULL) ? (s - 5ULL) : s;
}

// fp5_scalar_sub — subtract two F_5 scalars stored as uint64_t.
__device__ __forceinline__ uint64_t fp5_scalar_sub(uint64_t a, uint64_t b) {
    return fp5_scalar_add(a, 5ULL - b);
}

// fp5_scalar_neg — negate an F_5 scalar stored as uint64_t.
__device__ __forceinline__ uint64_t fp5_scalar_neg(uint64_t a) {
    return (a == 0ULL) ? 0ULL : (5ULL - a);
}

// ---------------------------------------------------------------------------
// Kernel: permanent_bipedal5_kernel
//
// Grid: blockDim.x = 1 (or any value; only thread 0 does work), gridDim.x = M.
// Each block computes the F_5 permanent of one n×n matrix.
//
// Input layout: matrices stored row-major, consecutive in memory.
//   matrix i starts at matrices_ptr + i * n * n.
//   matrices_ptr[i*n*n + row*n + col] = A[row][col] ∈ {0, 1, 2, 3, 4}.
//
// Output: out_ptr[blockIdx.x] = permanent(A) mod 5, as a uint64_t.
// ---------------------------------------------------------------------------
__global__ void permanent_bipedal5_kernel(const unsigned char* __restrict__ matrices_ptr,
                                           int n,
                                           int m,
                                           unsigned long long* __restrict__ out_ptr) {
    // Each block handles exactly one matrix. Only thread 0 runs.
    if (threadIdx.x != 0) return;

    int mat_idx = (int)blockIdx.x;
    if (mat_idx >= m) return;

    const unsigned char* mat = matrices_ptr + (size_t)mat_idx * (size_t)n * (size_t)n;

    // Range guards ordered so n == 0 is handled correctly before the
    // out-of-range rejection.
    //
    // Supported range: 1 <= n <= 63 (post 2026-05-15 CPU/GPU consistency
    // narrowing; the CPU reference permanent_bipedal5_singleword was
    // correspondingly narrowed). GPU sequential Gray walk at n=64 would
    // take ~600 years on gfx1030 — infeasible.
    //
    // Empty-matrix case (n == 0): permanent of the empty matrix is 1
    // (vacuous product). The host wrapper rejects n <= 0 before launch,
    // so this branch is defensive against direct kernel invocation.
    if (n == 0) {
        out_ptr[mat_idx] = 1ULL;
        return;
    }
    if (n < 0 || n > 63) {
        out_ptr[mat_idx] = 0ULL;
        return;
    }

    // -----------------------------------------------------------------------
    // Column extraction.
    //
    // columns[j][i] = A[i][j] ∈ {0,1,2,3,4} — direct byte representation.
    // For n ≤ 63, each column fits in a 63-byte array.
    // -----------------------------------------------------------------------
    uint8_t columns[63][63];
    for (int j = 0; j < n; j++) {
        for (int i = 0; i < n; i++) {
            columns[j][i] = mat[i * n + j];
        }
    }

    // -----------------------------------------------------------------------
    // Column-sum accumulator: col_sum[i] = (Σ_{j ∈ S} A[i,j]) mod 5.
    // Starts at zero (empty subset).
    // -----------------------------------------------------------------------
    uint8_t col_sum[63];
    for (int i = 0; i < n; i++) {
        col_sum[i] = 0;
    }

    // -----------------------------------------------------------------------
    // Gray walk: enumerate all 2^n - 1 non-empty subsets of [n].
    //
    // At Gray step k (k = 1 .. 2^n - 1):
    //   flip   = ctz(k)         — which column just entered or left S
    //   g_k    = k ^ (k >> 1)  — current subset bitmask
    //   parity = (g_k >> flip) & 1:
    //            1 → column `flip` just entered S (ADD col_sum += col[flip])
    //            0 → column `flip` just left  S (SUB col_sum -= col[flip])
    //
    // SSOT for the Gray iteration: `crates/gf2-algebra/src/gray.rs:123`.
    // WARNING: do NOT use `(k >> flip) & 1` as parity — that is always 1
    // for any k because flip = ctz(k). See gray.rs §6 note.
    // -----------------------------------------------------------------------

    // Ryser running total (scalar F_5 value in {0, 1, 2, 3, 4}) and subset size.
    uint64_t total = 0ULL;
    int subset_size = 0;

    // Step counter. For n <= 63, upper = 2^n fits in uint64_t (max 2^63).
    uint64_t upper = UINT64_C(1) << n;

    for (uint64_t k = 1ULL; k != upper; k++) {
        // Count trailing zeros to find the flipped bit.
        int flip = __builtin_ctzll(k);

        // Current subset bitmask.
        uint64_t g_k = k ^ (k >> 1);

        // Parity: did bit `flip` just enter (+1) or leave (0) S?
        int parity = (int)((g_k >> flip) & 1ULL);  // 1 = entered, 0 = left

        if (parity == 1) {
            // col_sum += columns[flip] elementwise mod 5.
            subset_size++;
            for (int i = 0; i < n; i++) {
                col_sum[i] = fp5_add(col_sum[i], columns[flip][i]);
            }
        } else {
            // col_sum -= columns[flip] elementwise mod 5.
            subset_size--;
            for (int i = 0; i < n; i++) {
                col_sum[i] = fp5_sub(col_sum[i], columns[flip][i]);
            }
        }

        // Horizontal product of the column-sum: prod = Π_{i=0}^{n-1} col_sum[i] mod 5.
        // Mirrors `Packed5::fold_mul_first_n` at
        // `crates/gf2-algebra/src/packed/packed5.rs` (SSOT).
        uint8_t prod = 1;
        for (int i = 0; i < n; i++) {
            prod = fp5_mul(prod, col_sum[i]);
            // Short-circuit: if product is zero, no further multiply can change it.
            if (prod == 0) break;
        }
        uint64_t term = (uint64_t)prod;

        // Ryser sign: (-1)^|S|. Odd subset_size → subtract; even → add.
        if (subset_size % 2 == 1) {
            total = fp5_scalar_sub(total, term);
        } else {
            total = fp5_scalar_add(total, term);
        }
    }

    // Apply outer (-1)^n factor from Ryser's formula.
    // In F_5, -1 == 4. (-1)^n: even n → factor=1 (no change); odd n → negate.
    if (n % 2 == 1) {
        total = fp5_scalar_neg(total);
    }

    out_ptr[mat_idx] = total;
}

// ---------------------------------------------------------------------------
// Batched entry point
//
// Launches permanent_bipedal5_kernel with M blocks, one per matrix.
// Thread-block size is fixed at 1 (only thread 0 does work per block).
//
// Parameters:
//   matrices_ptr — host-mapped or device pointer to M row-major n×n matrices
//                  of uint8 values in GF(5) (0, 1, 2, 3, 4).
//   n            — matrix dimension (square, n×n); must satisfy 1 <= n <= 63.
//                  n=64 is excluded: the GPU sequential Gray walk at n=64 would
//                  take ~600 years on gfx1030 (2^64 ≈ 1.8×10^19 steps).
//                  The CPU reference was narrowed to n ≤ 63 on 2026-05-15 for
//                  CPU/GPU consistency.
//   m            — batch size (number of matrices).
//   out_ptr      — device pointer to M uint64_t outputs.
//
// Returns: 0 on success (hipSuccess), non-zero on HIP error.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal5_hip_batch_on_stream(const unsigned char* matrices_ptr,
                                                       int n,
                                                       int m,
                                                       unsigned long long* out_ptr,
                                                       void* stream) {
    if (m <= 0) return 0; // hipSuccess — nothing to do
    if (n <= 0 || n > 63) return 1; // hipErrorInvalidValue

    // One block per matrix, one thread per block.
    // Thread 0 of each block executes the Gray walk; all other threads return
    // immediately (idle). The GPU win comes from M blocks in flight.
    dim3 grid((unsigned int)m, 1, 1);
    dim3 block(1, 1, 1);

    hipLaunchKernelGGL(permanent_bipedal5_kernel,
                       grid,
                       block,
                       /*sharedMemBytes=*/0,
                       reinterpret_cast<hipStream_t>(stream),
                       matrices_ptr,
                       n,
                       m,
                       out_ptr);

    return (int)hipGetLastError();
}

// Default-stream compatibility entry point. Keep this symbol and its
// behaviour unchanged for existing Rust and C callers; stream-aware callers
// use permanent_bipedal5_hip_batch_on_stream directly.
extern "C" int permanent_bipedal5_hip_batch(const unsigned char* matrices_ptr,
                                             int n,
                                             int m,
                                             unsigned long long* out_ptr) {
    return permanent_bipedal5_hip_batch_on_stream(matrices_ptr, n, m, out_ptr, nullptr);
}

// ---------------------------------------------------------------------------
// Single-matrix entry point (delegates to the batched variant with m=1 to
// avoid code duplication).
//
// SSOT: the real computation now lives in permanent_bipedal5_hip_batch.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal5_hip(const unsigned char* matrix_ptr,
                                       int n,
                                       unsigned long long* out_ptr) {
    return permanent_bipedal5_hip_batch(matrix_ptr, n, 1, out_ptr);
}
