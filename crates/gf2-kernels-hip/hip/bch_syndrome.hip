// Device-side batch BCH syndrome evaluator (design doc §5, §6, §10).
//
// Reproduces, bit-for-bit, the CPU `gf2_coding::bch::BchDecoder::compute_syndromes`
// path: for each received polynomial r(x) and each evaluation point α^(i+1)
// (i = 0..2t-1), the syndrome S_{i+1} = r(α^(i+1)) is computed by Horner's rule
// over GF(2^m). Berlekamp-Massey and Chien search stay on the CPU (issue
// background); this kernel is ONLY the syndrome-evaluation sub-step.
//
// # GF(2^m) multiplication — uploaded-table, bit-identical (design doc §5)
//
// The host uploads the EXACT CPU `exp` / `log` tables from the live
// `Gf2mField` (`d_exp`: 2^m - 1 entries, `d_log`: 2^m entries). The device
// multiply is then byte-identical to the CPU table path
// (`gf2-core::gf2m::field::Mul`):
//
//   mul(a, b) = (a == 0 || b == 0) ? 0 : exp[(log[a] + log[b]) % order]
//
// with `order = 2^m - 1`. Since `log[a], log[b] < order`, their sum is
// `< 2*order`, so the modulus is a single conditional subtract. Addition in
// GF(2^m) is XOR. There is NO on-device table regeneration — the tables ARE
// the CPU tables, so equality is exact by construction.
//
// # Coefficient layout — host-side reorder, packed bits (design doc §6)
//
// The host reorders each received BitVec into the design-doc §3.1 `coeffs`
// order (parity bits reversed, then message bits reversed) and uploads it as a
// packed little-endian bit stream of `n` bits per frame
// (`words_per_frame = ceil(n/64)` u64 words). The kernel does a PURE Horner
// pass with no knowledge of the parity/message split: `coeff_bit(f, d)` is one
// word load + shift from frame `f`'s packed stream. This keeps the exactness
// (the reorder lives in tested Rust) and the upload small (bits, not expanded
// field elements).
//
// # Thread mapping — one thread per (frame, point) (design doc §6)
//
// A flat grid of `batch * 2t` threads. Thread `gid = f * (2t) + i` evaluates
// `S_{i+1}` for frame `f` at point `d_points[i] = α^(i+1)` (uploaded as u16):
//
//   acc = coeff_bit(f, n-1)                 // leading coefficient (0 or 1)
//   for d = n-2 down to 0:
//       acc = gf_mul(acc, point) ^ coeff_bit(f, d)
//   d_syndromes[f * (2t) + i] = acc         // u16 field element
//
// The recurrence is strictly sequential per (f, i), in the identical order to
// the CPU `FieldPoly::eval` Horner fold (leading coeff first, folding down), so
// the result is exact and total (the all-zero polynomial yields zero in every
// lane, matching the CPU `eval_batch` zero-polynomial contract). There are no
// atomics and no cross-thread reductions: each thread writes one disjoint
// `d_syndromes` element.

#include <hip/hip_runtime.h>
#include <stdint.h>

// GF(2^m) multiply via the uploaded exp/log tables. Byte-identical to the CPU
// `Gf2mField` table path (design doc §3.3 / §5). `order = 2^m - 1`.
__device__ __forceinline__ uint16_t gf_mul(uint16_t a, uint16_t b,
                                           const uint16_t* __restrict__ d_log,
                                           const uint16_t* __restrict__ d_exp,
                                           uint32_t order) {
    if (a == 0u || b == 0u) return 0u;
    uint32_t s = (uint32_t)d_log[a] + (uint32_t)d_log[b];
    if (s >= order) s -= order; // (log_a + log_b) % order: single subtract
    return d_exp[s];
}

// One bit of frame `f`'s packed coefficient stream at coefficient index `d`.
// The stream is `words_per_frame` u64 words per frame, little-endian bit order
// (bit i in word i>>6, mask 1<<(i&63)) — matching `gf2_core::BitVec`.
__device__ __forceinline__ uint16_t coeff_bit(const uint64_t* __restrict__ coeffs,
                                              long long fbase, int d) {
    uint64_t word = coeffs[fbase + (long long)(d >> 6)];
    return (uint16_t)((word >> (d & 63)) & 1ull);
}

// ---------------------------------------------------------------------------
// Batch BCH syndrome evaluation: one thread per (frame, syndrome point).
//
//   d_coeffs     [batch * words_per_frame] packed coeff streams (§3.1 order)
//   d_points     [two_t] u16 evaluation points α^1 .. α^(2t)
//   d_log        [2^m]     uploaded discrete-log table
//   d_exp        [2^m - 1] uploaded antilog table
//   d_syndromes  [batch * two_t] u16 syndromes (out), row-major per frame
// ---------------------------------------------------------------------------
extern "C" __global__ void bch_syndrome_kernel(
    const uint64_t* __restrict__ d_coeffs,
    const uint16_t* __restrict__ d_points,
    const uint16_t* __restrict__ d_log,
    const uint16_t* __restrict__ d_exp,
    uint16_t* __restrict__ d_syndromes,
    int n,
    int two_t,
    int words_per_frame,
    uint32_t order,
    int batch_size) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)two_t;
    if (gid >= total) return;
    int f = (int)(gid / (long long)two_t);
    int i = (int)(gid % (long long)two_t);

    uint16_t point = d_points[i];
    long long fbase = (long long)f * (long long)words_per_frame;

    // Horner: start from the leading coefficient (index n-1) and fold down,
    // matching CPU `FieldPoly::eval` exactly.
    uint16_t acc = coeff_bit(d_coeffs, fbase, n - 1);
    for (int d = n - 2; d >= 0; --d) {
        acc = (uint16_t)(gf_mul(acc, point, d_log, d_exp, order) ^ coeff_bit(d_coeffs, fbase, d));
    }
    d_syndromes[gid] = acc;
}

// ---------------------------------------------------------------------------
// Standalone device gf_mul, exposed for the exhaustive GF(2^m) correctness
// rung (design doc §10 rung 1). Computes `out[j] = gf_mul(a[j], b[j])` for
// arbitrary u16 inputs using the SAME `gf_mul` the syndrome kernel uses, so the
// exhaustive (a, b) check exercises the real device multiply (BCH coefficients
// are binary, so the Horner path alone cannot feed an arbitrary leading
// accumulator). One thread per pair.
// ---------------------------------------------------------------------------
extern "C" __global__ void gf_mul_test_kernel(
    const uint16_t* __restrict__ d_a,
    const uint16_t* __restrict__ d_b,
    const uint16_t* __restrict__ d_log,
    const uint16_t* __restrict__ d_exp,
    uint16_t* __restrict__ d_out,
    uint32_t order,
    int count) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    if (gid >= (long long)count) return;
    d_out[gid] = gf_mul(d_a[gid], d_b[gid], d_log, d_exp, order);
}

// ---------------------------------------------------------------------------
// Host-side launch wrapper (callable from Rust via FFI). Fixed block size,
// grid = ceil(work / block) — a pure function of the problem size (design §6).
// ---------------------------------------------------------------------------

static constexpr int BLOCK_THREADS = 256;

extern "C" int launch_bch_syndrome(
    const uint64_t* d_coeffs,
    const uint16_t* d_points,
    const uint16_t* d_log,
    const uint16_t* d_exp,
    uint16_t* d_syndromes,
    int n,
    int two_t,
    int words_per_frame,
    uint32_t order,
    int batch_size,
    void* stream) {
    long long work = (long long)batch_size * (long long)two_t;
    if (work <= 0) return 0;
    long long g = (work + BLOCK_THREADS - 1) / BLOCK_THREADS;
    hipLaunchKernelGGL(
        bch_syndrome_kernel,
        dim3((unsigned int)g), dim3(BLOCK_THREADS), 0,
        reinterpret_cast<hipStream_t>(stream),
        d_coeffs, d_points, d_log, d_exp, d_syndromes,
        n, two_t, words_per_frame, order, batch_size);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_gf_mul_test(
    const uint16_t* d_a,
    const uint16_t* d_b,
    const uint16_t* d_log,
    const uint16_t* d_exp,
    uint16_t* d_out,
    uint32_t order,
    int count,
    void* stream) {
    if (count <= 0) return 0;
    long long g = ((long long)count + BLOCK_THREADS - 1) / BLOCK_THREADS;
    hipLaunchKernelGGL(
        gf_mul_test_kernel,
        dim3((unsigned int)g), dim3(BLOCK_THREADS), 0,
        reinterpret_cast<hipStream_t>(stream),
        d_a, d_b, d_log, d_exp, d_out, order, count);
    return static_cast<int>(hipGetLastError());
}
