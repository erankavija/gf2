// Permanent computation kernel for GF(3) (bipedal-3) on gfx1030.
//
// Implements `permanent_bipedal3_hip_batch`: a batched entry point that
// computes the Ryser permanent over F_3 for M independent n×n matrices in a
// single kernel launch. One HIP block is assigned to each matrix; only
// thread 0 of each block executes the Gray walk (sequential by design —
// the per-step work is a few u64 ops + one O(1) fold). GPU parallelism
// comes from having many matrices in flight simultaneously.
//
// Algorithm mirrors `permanent_bipedal3_singleword` at
// `crates/gf2-algebra/src/permanent/bipedal3.rs:242` (SSOT). The SSOT
// must be kept bit-identical to this device kernel.
//
// Bipedal3 encoding (SSOT: `crates/gf2-algebra/src/packed/bipedal3.rs`):
//   (mag=0, *)  → F_3 value 0
//   (mag=1, sgn=0) → F_3 value 1
//   (mag=1, sgn=1) → F_3 value 2
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
// Bipedal3 arithmetic helpers
//
// All of these mirror the Rust impl in
// `crates/gf2-algebra/src/packed/bipedal3.rs`.
// SSOT for add: `Bipedal3::add` at bipedal3.rs:491
// SSOT for sub: `Bipedal3::sub` at bipedal3.rs:530
// ---------------------------------------------------------------------------

// bipedal3_add_mag — magnitude word of (a + b) over F_3.
// Mirrors `Bipedal3::add` at `crates/gf2-algebra/src/packed/bipedal3.rs:491` (SSOT).
// 6 ops with CSE: t = am ^ as ^ bs; u = bm & t; mag = u | (am ^ bm)
__device__ __forceinline__ uint64_t bipedal3_add_mag(uint64_t am, uint64_t asg,
                                                      uint64_t bm, uint64_t bsg) {
    uint64_t t = am ^ asg ^ bsg;
    uint64_t u = bm & t;
    return u | (am ^ bm);
}

// bipedal3_add_sgn — sign word of (a + b) over F_3.
// Mirrors `Bipedal3::add` at `crates/gf2-algebra/src/packed/bipedal3.rs:491` (SSOT).
// 6 ops with CSE (t, u shared with mag): sgn = u ^ as
__device__ __forceinline__ uint64_t bipedal3_add_sgn(uint64_t am, uint64_t asg,
                                                      uint64_t bm, uint64_t bsg) {
    uint64_t t = am ^ asg ^ bsg;
    uint64_t u = bm & t;
    return u ^ asg;
}

// bipedal3_sub_mag — magnitude word of (a - b) over F_3.
// Mirrors `Bipedal3::sub` at `crates/gf2-algebra/src/packed/bipedal3.rs:530` (SSOT).
// 6 ops: t = as ^ bs; u = am & t; mag = u | (am ^ bm)
__device__ __forceinline__ uint64_t bipedal3_sub_mag(uint64_t am, uint64_t asg,
                                                      uint64_t bm, uint64_t bsg) {
    uint64_t t = asg ^ bsg;
    uint64_t u = am & t;
    return u | (am ^ bm);
}

// bipedal3_sub_sgn — sign word of (a - b) over F_3.
// Mirrors `Bipedal3::sub` at `crates/gf2-algebra/src/packed/bipedal3.rs:530` (SSOT).
// 6 ops (t, u shared with mag): sgn = u ^ (bm ^ bs)
__device__ __forceinline__ uint64_t bipedal3_sub_sgn(uint64_t am, uint64_t asg,
                                                      uint64_t bm, uint64_t bsg) {
    uint64_t t = asg ^ bsg;
    uint64_t u = am & t;
    return u ^ (bm ^ bsg);
}

// fold_mul_first_n — horizontal product of the first n active lanes.
//
// Mirrors `Bipedal3::fold_mul_first_n` at
// `crates/gf2-algebra/src/packed/bipedal3.rs:346` (SSOT).
//
// Steps:
//   1. Build used_mask = bits 0..n-1 all set (for n==64, all bits set).
//   2. Pad inactive lanes to mul-identity: acc_m |= ~used_mask (mag=1);
//      acc_s &= used_mask (sgn=0).
//   3. Six halving-fold steps (32, 16, 8, 4, 2, 1):
//        acc_m &= acc_m >> step;
//        acc_s ^= acc_s >> step;
//   4. Decode bit 0 of (acc_m, acc_s):
//        acc_m bit 0 == 0         → return 0
//        acc_m bit 0 == 1, acc_s bit 0 == 0 → return 1
//        acc_m bit 0 == 1, acc_s bit 0 == 1 → return 2
//
// Returns: uint64_t in {0, 1, 2} (the F_3 value).
__device__ __forceinline__ uint64_t fold_mul_first_n(uint64_t acc_m, uint64_t acc_s, int n) {
    // Build used_mask: bits 0..n-1 set. For n==64, all bits = UINT64_MAX.
    uint64_t used_mask = (n < 64) ? ((UINT64_C(1) << n) - 1ULL) : UINT64_MAX;

    // Pad inactive lanes (bits n..63) to the multiplicative identity: mag=1, sgn=0.
    acc_m |= ~used_mask;
    acc_s &= used_mask;

    // Six halving-fold steps. Uses Bipedal3 mul: mag' = mag & (mag>>step);
    // sgn' = sgn ^ (sgn>>step).
    acc_m &= acc_m >> 32;
    acc_s ^= acc_s >> 32;
    acc_m &= acc_m >> 16;
    acc_s ^= acc_s >> 16;
    acc_m &= acc_m >> 8;
    acc_s ^= acc_s >> 8;
    acc_m &= acc_m >> 4;
    acc_s ^= acc_s >> 4;
    acc_m &= acc_m >> 2;
    acc_s ^= acc_s >> 2;
    acc_m &= acc_m >> 1;
    acc_s ^= acc_s >> 1;

    // Decode bit 0.
    if ((acc_m & 1ULL) == 0ULL) {
        return 0ULL;
    } else if ((acc_s & 1ULL) == 0ULL) {
        return 1ULL;
    } else {
        return 2ULL;
    }
}

// ---------------------------------------------------------------------------
// F_3 scalar arithmetic (for accumulating the Ryser total mod 3)
// ---------------------------------------------------------------------------

// Add two F_3 scalars (values in {0, 1, 2}).
__device__ __forceinline__ uint64_t fp3_add(uint64_t a, uint64_t b) {
    // Simple modular add: (a + b) % 3. At most two additions under 6, so no
    // overflow for uint64_t.
    uint64_t s = a + b;
    return s >= 3ULL ? s - 3ULL : s;
}

// Subtract two F_3 scalars (values in {0, 1, 2}): returns (a - b) mod 3.
__device__ __forceinline__ uint64_t fp3_sub(uint64_t a, uint64_t b) {
    return fp3_add(a, 3ULL - b);
}

// Negate an F_3 scalar: -(0)=0, -(1)=2, -(2)=1.
__device__ __forceinline__ uint64_t fp3_neg(uint64_t a) {
    return a == 0ULL ? 0ULL : 3ULL - a;
}

// ---------------------------------------------------------------------------
// Kernel: permanent_bipedal3_kernel
//
// Grid: blockDim.x = 1 (or any value; only thread 0 does work), gridDim.x = M.
// Each block computes the F_3 permanent of one n×n matrix.
//
// Input layout: matrices stored row-major, consecutive in memory.
//   matrix i starts at matrices_ptr + i * n * n.
//   matrices_ptr[i*n*n + row*n + col] = A[row][col] ∈ {0, 1, 2}.
//
// Output: out_ptr[blockIdx.x] = permanent(A) mod 3, as a uint64_t.
// ---------------------------------------------------------------------------
__global__ void permanent_bipedal3_kernel(const unsigned char* __restrict__ matrices_ptr,
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
    // The supported range is 1 <= n <= 63 (post 2026-05-15 CPU/GPU
    // consistency narrowing). The GPU sequential Gray walk at n=64 would
    // take ~600 years on gfx1030 (2^64 ≈ 1.8×10^19 steps), so n=64 is
    // computationally infeasible and is excluded from the supported range.
    // The CPU reference (permanent_bipedal3_singleword) supports n=64 via
    // a u128 Gray-code counter; the GPU kernel does not need that range.
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
    // columns[j] = (col_m[j], col_s[j]), where lane i of col_m[j] and
    // col_s[j] encode A[i][j] as a Bipedal3 pair.
    //
    // Encoding (mirrors with_lane in bipedal3.rs):
    //   A[i][j] == 0 → mag bit i = 0, sgn bit i = 0
    //   A[i][j] == 1 → mag bit i = 1, sgn bit i = 0
    //   A[i][j] == 2 → mag bit i = 1, sgn bit i = 1
    // -----------------------------------------------------------------------
    uint64_t col_m[64];
    uint64_t col_s[64];
    for (int j = 0; j < n; j++) {
        col_m[j] = 0ULL;
        col_s[j] = 0ULL;
        for (int i = 0; i < n; i++) {
            unsigned char v = mat[i * n + j];
            if (v != 0) {
                col_m[j] |= (UINT64_C(1) << i);
                if (v == 2) {
                    col_s[j] |= (UINT64_C(1) << i);
                }
            }
        }
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

    // Column-sum accumulator: (sum_m, sum_s) as a Bipedal3 pair.
    uint64_t sum_m = 0ULL;
    uint64_t sum_s = 0ULL;

    // Ryser running total (scalar F_3 value in {0, 1, 2}) and subset size.
    uint64_t total = 0ULL;
    int subset_size = 0;

    // Step counter. For n <= 63, upper = 2^n fits in uint64_t (max 2^63).
    // n=64 is excluded (see the range check above), so no u64 wraparound
    // can occur and the loop termination condition k != upper is always safe.
    uint64_t upper = UINT64_C(1) << n;

    for (uint64_t k = 1ULL; k != upper; k++) {
        // Count trailing zeros to find the flipped bit.
        int flip = __builtin_ctzll(k);

        // Current subset bitmask.
        uint64_t g_k = k ^ (k >> 1);

        // Parity: did bit `flip` just enter (+1) or leave (-1) S?
        int parity = (int)((g_k >> flip) & 1ULL);  // 1 = entered, 0 = left

        if (parity == 1) {
            // col_sum += columns[flip]
            subset_size++;
            uint64_t new_m = bipedal3_add_mag(sum_m, sum_s, col_m[flip], col_s[flip]);
            uint64_t new_s = bipedal3_add_sgn(sum_m, sum_s, col_m[flip], col_s[flip]);
            sum_m = new_m;
            sum_s = new_s;
        } else {
            // col_sum -= columns[flip]
            subset_size--;
            uint64_t new_m = bipedal3_sub_mag(sum_m, sum_s, col_m[flip], col_s[flip]);
            uint64_t new_s = bipedal3_sub_sgn(sum_m, sum_s, col_m[flip], col_s[flip]);
            sum_m = new_m;
            sum_s = new_s;
        }

        // Horizontal product of the column-sum.
        uint64_t term = fold_mul_first_n(sum_m, sum_s, n);

        // Ryser sign: (-1)^|S|. Odd subset_size → subtract; even → add.
        if (subset_size % 2 == 1) {
            total = fp3_sub(total, term);
        } else {
            total = fp3_add(total, term);
        }
    }

    // Apply outer (-1)^n factor from Ryser's formula.
    if (n % 2 == 1) {
        total = fp3_neg(total);
    }

    out_ptr[mat_idx] = total;
}

// ---------------------------------------------------------------------------
// Batched entry point (new, per ad55b777)
//
// Launches permanent_bipedal3_kernel with M blocks, one per matrix.
// Thread-block size is fixed at 1 (only thread 0 does work per block).
//
// Parameters:
//   matrices_ptr — host-mapped or device pointer to M row-major n×n matrices
//                  of uint8 values in GF(3) (0, 1, 2).
//   n            — matrix dimension (square, n×n); must satisfy 1 <= n <= 63.
//                  n=64 is excluded: the GPU sequential Gray walk at n=64 would
//                  take ~600 years on gfx1030 (2^64 ≈ 1.8×10^19 steps).
//                  The CPU reference supports n=64 via a u128 Gray counter.
//   m            — batch size (number of matrices).
//   out_ptr      — device pointer to M uint64_t outputs.
//
// Returns: 0 on success (hipSuccess), non-zero on HIP error.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal3_hip_batch(const unsigned char* matrices_ptr,
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

    hipLaunchKernelGGL(permanent_bipedal3_kernel,
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
// Single-matrix entry point (retained from the scaffold; now delegates to
// the batched variant with m=1 to avoid code duplication).
//
// SSOT: the real computation now lives in permanent_bipedal3_hip_batch.
// ---------------------------------------------------------------------------
extern "C" int permanent_bipedal3_hip(const unsigned char* matrix_ptr,
                                       int n,
                                       unsigned long long* out_ptr) {
    return permanent_bipedal3_hip_batch(matrix_ptr, n, 1, out_ptr);
}
