// Gray square-QAM (and BPSK) batch soft demapper kernel.
//
// Max-log LLR computation: per received symbol, for each bit k,
//     LLR_k = min_{L: bit_k(L)=1} d(L) - min_{L: bit_k(L)=0} d(L)
// where d(L) is the noise-weighted squared distance from the sample to
// the constellation point labeled L. For AWGN without channel gain the
// two axes separate, so we compute two 1D Gray-PAM distance tables of
// length `axis_len = 1 << m_half` (or 2 for BPSK) and reduce per axis.
//
// Kernel strategy: one thread per received symbol. `axis_len <= 16` for
// all supported presets (up to 256-QAM, m=8, m_half=4), so the per-thread
// work is small and we avoid block-level reductions entirely. This
// matches the prototype's "one kernel launch" target and is sufficient
// to obtain a real crossover measurement against the CPU fast path.
//
// Layout conventions (must match CPU):
// - Input (rx_i, rx_q, noise_var) length num_symbols.
// - Optional per-symbol channel gain: gain_i, gain_q (length num_symbols
//   when gains_present == 1; unused and may be nullptr otherwise). When
//   provided the kernel pre-rotates by conj(h) and rescales the effective
//   noise variance by |h|^2, exactly as on the CPU fast path.
// - pam_levels length axis_len, same table reused for I and Q axes
//   (Gray-QAM presets are symmetric, enforced on the CPU constructor).
// - Output llrs length num_symbols * m, symbol-major, MSB-first within a
//   symbol: first m_half bits are I-axis, remaining m_half bits are
//   Q-axis (for BPSK m=1 and only the I axis is used).

#include <hip/hip_runtime.h>
#include <cmath>
#include <cfloat>

// axis_len <= 16 for all supported presets (m in {1,2,4,6,8}).
static constexpr int MAX_AXIS_LEN = 16;

// Returns bit `bit_idx` of an MSB-first label of width `m_bits`.
__device__ __forceinline__ int bit_at_msb_first(int label, int bit_idx, int m_bits) {
    return (label >> (m_bits - 1 - bit_idx)) & 1;
}

// Compute the 1D Gray-PAM max-log LLR for bit `bit_idx` (MSB-first within
// the `m_bits`-wide half-label) given per-level squared distances `d` of
// length `1 << m_bits`. Label == index on a Gray-PAM axis.
__device__ __forceinline__ float pam_axis_max_log_llr(
    const float* d, int m_bits, int bit_idx
) {
    int n_labels = 1 << m_bits;
    float d_min0 = FLT_MAX;
    float d_min1 = FLT_MAX;
    for (int j = 0; j < n_labels; ++j) {
        float dj = d[j];
        int bit = bit_at_msb_first(j, bit_idx, m_bits);
        if (bit == 0) {
            if (dj < d_min0) d_min0 = dj;
        } else {
            if (dj < d_min1) d_min1 = dj;
        }
    }
    // max-log LLR: -d_min0 + d_min1 (sign convention: positive = bit 0
    // more likely, matching gf2_coding::llr::Llr).
    return -d_min0 + d_min1;
}

__global__ void gray_qam_demap_kernel(
    const float* __restrict__ rx_i,         // [num_symbols]
    const float* __restrict__ rx_q,         // [num_symbols]
    const float* __restrict__ gain_i,       // [num_symbols] or nullptr
    const float* __restrict__ gain_q,       // [num_symbols] or nullptr
    const float* __restrict__ noise_var,    // [num_symbols]
    const float* __restrict__ pam_levels,   // [axis_len]
    float* __restrict__ out_llrs,           // [num_symbols * m]
    int num_symbols,
    int axis_len,
    int m,
    int m_half,
    int is_bpsk,
    int gains_present
) {
    int s = blockIdx.x * blockDim.x + threadIdx.x;
    if (s >= num_symbols) return;

    float y_i = rx_i[s];
    float y_q = rx_q[s];
    float h_i, h_q;
    if (gains_present) {
        h_i = gain_i[s];
        h_q = gain_q[s];
    } else {
        h_i = 1.0f;
        h_q = 0.0f;
    }
    float n0 = noise_var[s];

    // Pre-rotate by conj(h): z = conj(h) * y. For the AWGN path (gains
    // absent, h=1+0j) this reduces to the identity transform.
    float g = h_i * h_i + h_q * h_q;
    float z_i = h_i * y_i + h_q * y_q;
    float z_q = h_i * y_q - h_q * y_i;

    // Zero-gain guard: matches CPU fast path contract. An all-zero
    // distance slab yields LLR = 0 for every bit on balanced Gray
    // presets.
    float n0_eq = n0 * g;
    float inv_n0_eq = (n0_eq > FLT_MIN) ? (1.0f / n0_eq) : 0.0f;

    // Per-symbol distance slabs for I and Q axes.
    float d_i[MAX_AXIS_LEN];
    float d_q[MAX_AXIS_LEN];

    for (int l = 0; l < axis_len; ++l) {
        float level = pam_levels[l];
        float diff_i = z_i - g * level;
        d_i[l] = diff_i * diff_i * inv_n0_eq;
        if (!is_bpsk) {
            float diff_q = z_q - g * level;
            d_q[l] = diff_q * diff_q * inv_n0_eq;
        }
    }

    float* out = out_llrs + s * m;

    if (is_bpsk) {
        // m=1, m_half treated as 1 for the axis reduction (two-label set).
        out[0] = pam_axis_max_log_llr(d_i, 1, 0);
    } else {
        for (int b = 0; b < m_half; ++b) {
            out[b] = pam_axis_max_log_llr(d_i, m_half, b);
        }
        for (int b = 0; b < m_half; ++b) {
            out[m_half + b] = pam_axis_max_log_llr(d_q, m_half, b);
        }
    }
}

// Host-side launch wrapper callable from Rust via FFI.
extern "C" int launch_gray_qam_demap(
    const float* rx_i,
    const float* rx_q,
    const float* gain_i,        // may be nullptr
    const float* gain_q,        // may be nullptr
    const float* noise_var,
    const float* pam_levels,
    float* out_llrs,
    int num_symbols,
    int axis_len,
    int m,
    int m_half,
    int is_bpsk,
    int gains_present,
    void* stream
) {
    if (num_symbols <= 0) {
        return 0;
    }
    const int threads = 256;
    const int blocks = (num_symbols + threads - 1) / threads;
    dim3 grid(blocks);
    dim3 block(threads);

    hipLaunchKernelGGL(
        gray_qam_demap_kernel,
        grid, block,
        0,
        reinterpret_cast<hipStream_t>(stream),
        rx_i, rx_q, gain_i, gain_q, noise_var, pam_levels, out_llrs,
        num_symbols, axis_len, m, m_half, is_bpsk, gains_present
    );

    return static_cast<int>(hipGetLastError());
}
