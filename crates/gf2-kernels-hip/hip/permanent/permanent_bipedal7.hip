// Permanent computation kernel for GF(7) (F_7) on gfx1030.
//
// Implements `permanent_bipedal7_hip_batch`: a batched entry point that
// computes the Ryser permanent over F_7 for M independent n×n matrices in a
// single kernel launch. One HIP block is assigned to each matrix; only
// thread 0 of each block executes the Gray walk (sequential by design —
// the per-step work is O(n) LUT lookups + one O(n) fold). GPU parallelism
// comes from having many matrices in flight simultaneously.
//
// Algorithm mirrors `permanent_bipedal7_singleword` at
// `crates/gf2-algebra/src/permanent/bipedal7.rs:163` (SSOT). The SSOT
// must be kept bit-identical to this device kernel.
//
// Encoding choice (F_7 GPU kernel):
//   Direct byte arithmetic — each GF(7) element is a uint8_t in {0,1,...,6}.
//   Add and subtract use ADD_LUT / SUB_LUT (key = a | (b << 8)); multiply
//   in fold_mul uses MUL_LUT (key = a | (b << 8)).
//
// LUT layout (SSOT: `crates/gf2-algebra/src/packed/packed7.rs`):
//   ADD_LUT[key] where key = (a_byte as usize) | ((b_byte as usize) << 8).
//   Each byte packs TWO 4-bit-aligned nibble results (low nibble = op on
//   low nibbles of a/b, high nibble = op on high nibbles of a/b).
//
//   For scalar byte arithmetic in this kernel each element is one full byte
//   in {0..6}, so the low nibble of that byte IS the element value (since
//   values 0..6 < 8 = 2^3, they fit in 4 bits). The high nibble of the
//   byte input is always 0 (no second element stacked there).
//
//   Scalar key form: key = a | (b << 8)  (a and b each in 0..6).
//   Result = ADD_LUT[key] & 0x0f  (low nibble = (a_lo + b_lo) % 7).
//
//   The high nibble of the result is also 0 when a_hi = b_hi = 0 (which
//   is always the case here since the high nibble of each input byte is 0).
//   So the full byte result is correct without masking.
//
// LUT placement:
//   - d_MUL_LUT: __constant__ memory (64 KiB — fills the entire constant
//     cache on gfx1030). Populated once per process via hipMemcpyToSymbol
//     from the Rust shim's `init_permanent_gf7` call.
//   - d_ADD_LUT, d_SUB_LUT: __device__ global memory (64 KiB each).
//     Populated by the same init call. Accesses go through L1/L2.
//
//   Criterion 3: the `test_permanent_bipedal7_constant_lut_checksum_matches_host`
//   test sums all 65536 bytes of d_MUL_LUT on the GPU and compares to the
//   host MUL_LUT sum, verifying byte-identical population.
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
// LUT declarations
//
// d_MUL_LUT: __constant__ — 64 KiB, populated via hipMemcpyToSymbol.
// d_ADD_LUT, d_SUB_LUT: __device__ global — 64 KiB each, populated via
//   hipMemcpy from the Rust init shim.
// ---------------------------------------------------------------------------

/// MUL_LUT in __constant__ memory (64 KiB). Lives in the dedicated constant
/// cache (hardware limit exactly 64 KiB on gfx1030). Populated by
/// `permanent_bipedal7_hip_init` before the first kernel launch.
__constant__ unsigned char d_MUL_LUT[65536];

/// ADD_LUT in __device__ global memory (64 KiB). Accessed via L1/L2.
__device__ unsigned char d_ADD_LUT[65536];

/// SUB_LUT in __device__ global memory (64 KiB). Accessed via L1/L2.
__device__ unsigned char d_SUB_LUT[65536];

// ---------------------------------------------------------------------------
// Kernel: permanent_bipedal7_kernel
//
// Grid: blockDim.x = 1 (or any value; only thread 0 does work), gridDim.x = M.
// Each block computes the F_7 permanent of one n×n matrix.
//
// Input layout: matrices stored row-major, consecutive in memory.
//   matrix i starts at matrices_ptr + i * n * n.
//   matrices_ptr[i*n*n + row*n + col] = A[row][col] ∈ {0, 1, ..., 6}.
//
// Output: out_ptr[blockIdx.x] = permanent(A) mod 7, as a uint64_t.
// ---------------------------------------------------------------------------
__global__ void permanent_bipedal7_kernel(const unsigned char* __restrict__ matrices_ptr,
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
    // narrowing). GPU sequential Gray walk at n=64 would take ~600 years
    // on gfx1030 (2^64 ≈ 1.8×10^19 steps) — infeasible.
    //
    // Empty-matrix case (n == 0): permanent of the empty matrix is 1
    // (the empty permutation has product 1 — vacuous product). The host
    // wrapper rejects n <= 0 before launching, so this branch is
    // defensive against direct kernel invocation.
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
    // columns[j][i] = A[i][j] ∈ {0,1,...,6} — direct byte representation.
    // For n ≤ 63, each column fits in a 63-byte array.
    // -----------------------------------------------------------------------
    uint8_t columns[63][63];
    for (int j = 0; j < n; j++) {
        for (int i = 0; i < n; i++) {
            columns[j][i] = mat[i * n + j];
        }
    }

    // -----------------------------------------------------------------------
    // Column-sum accumulator: col_sum[i] = (Σ_{j ∈ S} A[i,j]) mod 7.
    // Starts at zero (empty subset).
    //
    // Add/sub use the LUT: key = (a_byte) | (b_byte << 8).
    // Since values are in {0..6} (nibble values), the full byte IS the
    // element (high nibble is 0). The LUT result byte also has the result
    // in the low nibble (high nibble 0 when both inputs have high nibble 0).
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

    // Ryser running total (scalar F_7 value in {0, 1, ..., 6}) and subset size.
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
            // col_sum += columns[flip] elementwise mod 7 via ADD_LUT.
            // key = col_sum[i] | (columns[flip][i] << 8).
            // result = d_ADD_LUT[key] (full byte; high nibble is 0).
            subset_size++;
            for (int i = 0; i < n; i++) {
                unsigned int key = (unsigned int)col_sum[i] | ((unsigned int)columns[flip][i] << 8);
                col_sum[i] = d_ADD_LUT[key];
            }
        } else {
            // col_sum -= columns[flip] elementwise mod 7 via SUB_LUT.
            // key = col_sum[i] | (columns[flip][i] << 8).
            // result = d_SUB_LUT[key] (full byte; high nibble is 0).
            subset_size--;
            for (int i = 0; i < n; i++) {
                unsigned int key = (unsigned int)col_sum[i] | ((unsigned int)columns[flip][i] << 8);
                col_sum[i] = d_SUB_LUT[key];
            }
        }

        // Horizontal product of the column-sum via MUL_LUT:
        //   prod = 1; for i in 0..n: prod = MUL_LUT[(prod) | (col_sum[i] << 8)].
        //
        // Mirrors `Packed7::fold_mul_first_n` SSOT at
        // `crates/gf2-algebra/src/packed/packed7.rs` (SSOT).
        //
        // MUL_LUT key = (a_byte) | (b_byte << 8), same byte layout as ADD/SUB.
        uint8_t prod = 1;
        for (int i = 0; i < n; i++) {
            unsigned int key = (unsigned int)prod | ((unsigned int)col_sum[i] << 8);
            prod = d_MUL_LUT[key];
            // Short-circuit: if product is zero, no further multiply can change it.
            if (prod == 0) break;
        }
        uint64_t term = (uint64_t)prod;

        // Ryser sign: (-1)^|S|. Odd subset_size → subtract; even → add.
        // Subtraction mod 7: (total + (7 - term)) mod 7.
        // Addition mod 7: (total + term) mod 7.
        if (subset_size % 2 == 1) {
            // total -= term  (mod 7)
            uint64_t s = total + (7ULL - term);
            total = (s >= 7ULL) ? (s - 7ULL) : s;
        } else {
            // total += term  (mod 7)
            uint64_t s = total + term;
            total = (s >= 7ULL) ? (s - 7ULL) : s;
        }
    }

    // Apply outer (-1)^n factor from Ryser's formula.
    // In F_7, -1 == 6. Negate: (7 - total) mod 7.
    if (n % 2 == 1) {
        total = (total == 0ULL) ? 0ULL : (7ULL - total);
    }

    out_ptr[mat_idx] = total;
}

// ---------------------------------------------------------------------------
// LUT checksum kernel (for criterion-3 test)
//
// Sums all 65536 bytes of d_MUL_LUT (__constant__) and writes the u64
// result to *out_ptr. Used by
// `test_permanent_bipedal7_constant_lut_checksum_matches_host`.
// ---------------------------------------------------------------------------
__global__ void permanent_bipedal7_lut_checksum_kernel(unsigned long long* __restrict__ out_ptr) {
    if (threadIdx.x != 0 || blockIdx.x != 0) return;
    uint64_t s = 0ULL;
    for (int i = 0; i < 65536; i++) {
        s += (uint64_t)d_MUL_LUT[i];
    }
    *out_ptr = s;
}

// ---------------------------------------------------------------------------
// Init entry point
//
// Copies the three host LUTs into GPU memory. Safe to call multiple times
// (idempotent). The Rust shim memoises this function's return code in
// `GF7_INIT_RC` so a failure here propagates to subsequent compute-batch
// callers instead of silently producing wrong results.
//
// Parameters:
//   host_add_lut — host pointer to 65536 bytes (ADD_LUT from gf2-algebra).
//   host_sub_lut — host pointer to 65536 bytes (SUB_LUT from gf2-algebra).
//   host_mul_lut — host pointer to 65536 bytes (MUL_LUT from gf2-algebra).
//
// Returns: 0 on success (hipSuccess), non-zero on HIP error.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal7_hip_init(const unsigned char* host_add_lut,
                                            const unsigned char* host_sub_lut,
                                            const unsigned char* host_mul_lut) {
    // Populate __constant__ d_MUL_LUT via hipMemcpyToSymbol.
    // hipMemcpyToSymbol signature: (symbol, src, count, offset, kind)
    hipError_t rc = hipMemcpyToSymbol(HIP_SYMBOL(d_MUL_LUT), host_mul_lut,
                                       65536, 0, hipMemcpyHostToDevice);
    if (rc != hipSuccess) return (int)rc;

    // Populate __device__ d_ADD_LUT via hipMemcpyToSymbol.
    rc = hipMemcpyToSymbol(HIP_SYMBOL(d_ADD_LUT), host_add_lut,
                            65536, 0, hipMemcpyHostToDevice);
    if (rc != hipSuccess) return (int)rc;

    // Populate __device__ d_SUB_LUT via hipMemcpyToSymbol.
    rc = hipMemcpyToSymbol(HIP_SYMBOL(d_SUB_LUT), host_sub_lut,
                            65536, 0, hipMemcpyHostToDevice);
    if (rc != hipSuccess) return (int)rc;

    return (int)hipSuccess;
}

// ---------------------------------------------------------------------------
// LUT checksum entry point (criterion-3 test support)
//
// Launches `permanent_bipedal7_lut_checksum_kernel` with a single block/thread
// and writes the 64-bit sum of all bytes of d_MUL_LUT to *out_ptr (a device
// pointer allocated by the caller).
//
// Returns: 0 on success (hipSuccess), non-zero on HIP error.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal7_hip_lut_checksum(unsigned long long* out_ptr) {
    dim3 grid(1, 1, 1);
    dim3 block(1, 1, 1);
    hipLaunchKernelGGL(permanent_bipedal7_lut_checksum_kernel,
                       grid,
                       block,
                       /*sharedMemBytes=*/0,
                       /*stream=*/0,
                       out_ptr);
    return (int)hipGetLastError();
}

// ---------------------------------------------------------------------------
// Batched entry point
//
// Launches permanent_bipedal7_kernel with M blocks, one per matrix.
// Thread-block size is fixed at 1 (only thread 0 does work per block).
//
// Parameters:
//   matrices_ptr — host-mapped or device pointer to M row-major n×n matrices
//                  of uint8 values in GF(7) (0, 1, ..., 6).
//   n            — matrix dimension (square, n×n); must satisfy 1 <= n <= 63.
//                  n=64 is excluded: the GPU sequential Gray walk at n=64 would
//                  take ~600 years on gfx1030 (2^64 ≈ 1.8×10^19 steps).
//                  The CPU reference (permanent_bipedal7_singleword) supports
//                  n <= 16 (Packed7::LANES); the GPU kernel supports n <= 63
//                  via direct byte arithmetic.
//   m            — batch size (number of matrices).
//   out_ptr      — device pointer to M uint64_t outputs.
//
// Returns: 0 on success (hipSuccess), non-zero on HIP error.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal7_hip_batch(const unsigned char* matrices_ptr,
                                             int n,
                                             int m,
                                             unsigned long long* out_ptr) {
    if (m <= 0) return 0; // hipSuccess — nothing to do
    if (n <= 0 || n > 63) return 1; // hipErrorInvalidValue

    // One block per matrix, one thread per block.
    // Thread 0 of each block executes the Gray walk; all other threads return
    // immediately (idle). The GPU win comes from M blocks in flight.
    dim3 grid((unsigned int)m, 1, 1);
    dim3 block(1, 1, 1);

    hipLaunchKernelGGL(permanent_bipedal7_kernel,
                       grid,
                       block,
                       /*sharedMemBytes=*/0,
                       /*stream=*/0,
                       matrices_ptr,
                       n,
                       m,
                       out_ptr);

    return (int)hipGetLastError();
}

// ---------------------------------------------------------------------------
// Single-matrix entry point (delegates to the batched variant with m=1 to
// avoid code duplication).
//
// SSOT: the real computation now lives in permanent_bipedal7_hip_batch.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal7_hip(const unsigned char* matrix_ptr,
                                       int n,
                                       unsigned long long* out_ptr) {
    return permanent_bipedal7_hip_batch(matrix_ptr, n, 1, out_ptr);
}
