// Device-side LDPC belief-propagation batch decoder (design doc §6, §10, §11).
//
// Reproduces, bit-for-bit in the hard-decision verdict, the CPU
// `gf2_coding::ldpc::core::LdpcDecoder` flooding belief-propagation schedule:
// init var-to-check messages with the channel LLRs, then alternate a
// check-node update and a variable-node update, with an optional per-iteration
// syndrome early-termination test (design §11: the hard-decision verdict is
// robust to the 1-3 ULP RDNA2 transcendental drift, so `fer`/`frames`/`errors`
// are byte-identical CPU-vs-GPU even though `mean_iters` may differ).
//
// # Determinism (design doc §11)
//
// All message ordering matches the CPU exactly:
//   * Check-node gather order is the parity-check matrix CSR row order
//     (`SpBitMatrixDual::row_iter`); the SumProduct `tanh` product is
//     accumulated left-to-right in that order so the order-dependent f32
//     product is reproduced bit-for-bit (MinSum / NormalizedMinSum use only
//     sign / min / scalar-multiply, which are order-independent and exact).
//   * Variable-node belief sum is accumulated left-to-right in the CSC column
//     order (`SpBitMatrixDual::col_iter`), matching the CPU's
//     `variable_node_update` accumulation order.
// There are no atomics and no cross-thread reductions on f32: one thread owns
// each check (check-update kernel) and each variable (variable-update kernel),
// each writing disjoint per-edge / per-node outputs. Per-edge messages are
// stored in a per-edge scratch and consumed by an ordered gather (the §-Algorithm
// "per-edge ordered accumulation into a per-edge scratch followed by an ordered
// reduction" contract).
//
// # Graph layout (uploaded once by the host wrapper)
//
// The Tanner graph is stored as a double CSR (check-major and variable-major),
// matching the CPU decoder's `check_neighbors` / `var_neighbors`:
//
//   check_row_ptr[c]   .. check_row_ptr[c+1]    : edges of check c (CSR row)
//   check_edge_var[e]                            : the variable of check-edge e
//   check_edge_to_var_edge[e]                    : the matching var-edge index f
//
//   var_col_ptr[v]     .. var_col_ptr[v+1]       : edges of variable v (CSC col)
//   var_edge_check[f]                            : the check of var-edge f
//   var_edge_to_check_edge[f]                    : the matching check-edge index e
//
// Per-edge message arrays (length = number of edges E, one per Tanner edge),
// laid out batch-major (`batch_idx * E + edge`):
//
//   v2c[b*E + f]   variable-to-check message on var-edge f      (CSC order)
//   c2v[b*E + e]   check-to-variable message on check-edge e     (CSR order)
//
// Because check-edge e and var-edge f index the SAME Tanner edge through the two
// cross-maps, the two arrays describe the same E messages from the two views.
//
// The shift table (`shift_table` / `shift_len`) is accepted at launch time but
// unused by the DVB-T2 path (the graph is already fully expanded into the CSR
// above). It is the per-`i_LS` 5G-NR parameterisation seam (Phase E `23d3525f`):
// a future BG1/BG2 launch can pass the lifting-set shift row so the same kernel
// expands a quasi-cyclic base graph on the fly. Passing it now keeps the FFI
// signature stable across both standards.

#include <hip/hip_runtime.h>
#include <stdint.h>

// Algorithm selector (mirrors `gf2_coding::ldpc::DecoderAlgorithm`).
#define LDPC_ALG_MIN_SUM        0
#define LDPC_ALG_NORM_MIN_SUM   1
#define LDPC_ALG_OFFSET_MIN_SUM 2
#define LDPC_ALG_SUM_PRODUCT    3

// Matches `Llr::MAX_TANH_PRODUCT = 1 - f32::EPSILON` (f32::EPSILON = 2^-23).
__device__ __forceinline__ float max_tanh_product() {
    return 1.0f - 1.1920929e-7f; // 1.0f - FLT_EPSILON
}

// ---------------------------------------------------------------------------
// Check-node update: one thread per (batch frame, check) pair.
//
// For each edge e of check c (in CSR order), gathers the var-to-check messages
// of all OTHER edges of c (also CSR order) and writes the check-to-var message
// c2v[e] using the configured box-plus rule. The gathered v2c values come from
// the matching var-edge `check_edge_to_var_edge[e']`.
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_check_update_kernel(
    const float* __restrict__ v2c,          // [batch * E] var-to-check (CSC view)
    float* __restrict__ c2v,                 // [batch * E] check-to-var (CSR view) (out)
    const int* __restrict__ check_row_ptr,   // [m+1]
    const int* __restrict__ check_edge_to_var_edge, // [E]
    int m,
    int edges,
    int batch_size,
    int algorithm,
    float alpha,
    float beta) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)m;
    if (gid >= total) return;
    int b = (int)(gid / (long long)m);
    int c = (int)(gid % (long long)m);

    int start = check_row_ptr[c];
    int end = check_row_ptr[c + 1];
    long long base = (long long)b * (long long)edges;

    for (int e = start; e < end; ++e) {
        // Accumulate over all OTHER edges of this check, in CSR order.
        if (algorithm == LDPC_ALG_SUM_PRODUCT) {
            // Ordered f32 product of tanh(L/2), matching `Llr::boxplus_n`.
            float product = 1.0f;
            bool any = false;
            for (int o = start; o < end; ++o) {
                if (o == e) continue;
                int f = check_edge_to_var_edge[o];
                float lv = v2c[base + (long long)f];
                product *= tanhf(lv * 0.5f);
                any = true;
            }
            float msg;
            if (!any) {
                msg = 0.0f;
            } else {
                float clamped = product;
                float mx = max_tanh_product();
                if (clamped > mx) clamped = mx;
                if (clamped < -mx) clamped = -mx;
                msg = 2.0f * atanhf(clamped);
            }
            c2v[base + (long long)e] = msg;
        } else {
            // Min-sum family: sign product * min magnitude (order-independent
            // and exactly representable), then the per-algorithm correction.
            float sign = 1.0f;
            float minmag = INFINITY;
            bool any = false;
            for (int o = start; o < end; ++o) {
                if (o == e) continue;
                int f = check_edge_to_var_edge[o];
                float lv = v2c[base + (long long)f];
                // CPU: sign = +1 when value >= 0 else -1 (`boxplus_minsum_n_scalar`
                // and the SIMD path agree on this convention).
                sign *= (lv >= 0.0f) ? 1.0f : -1.0f;
                float mag = fabsf(lv);
                if (mag < minmag) minmag = mag;
                any = true;
            }
            float msg;
            if (!any) {
                msg = 0.0f;
            } else if (algorithm == LDPC_ALG_NORM_MIN_SUM) {
                msg = alpha * (sign * minmag);
            } else if (algorithm == LDPC_ALG_OFFSET_MIN_SUM) {
                float om = minmag - beta;
                if (om < 0.0f) om = 0.0f;
                msg = sign * om;
            } else {
                // LDPC_ALG_MIN_SUM
                msg = sign * minmag;
            }
            c2v[base + (long long)e] = msg;
        }
    }
}

// ---------------------------------------------------------------------------
// Variable-node update: one thread per (batch frame, variable) pair.
//
// belief = channel_llr + sum over CSC-ordered incoming c2v messages, then
// v2c[f] = belief - incoming[f]. The accumulation order is the CSC column order,
// matching the CPU `variable_node_update`. Also writes the hard decision
// (belief < 0 -> bit 1).
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_var_update_kernel(
    const float* __restrict__ channel_llrs,  // [batch * n]
    float* __restrict__ v2c,                  // [batch * E] var-to-check (out)
    const float* __restrict__ c2v,            // [batch * E] check-to-var (in)
    const int* __restrict__ var_col_ptr,      // [n+1]
    const int* __restrict__ var_edge_to_check_edge, // [E]
    uint8_t* __restrict__ hard_bits,          // [batch * n] (out): 1 if bit 1
    int n,
    int edges,
    int batch_size) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)n;
    if (gid >= total) return;
    int b = (int)(gid / (long long)n);
    int v = (int)(gid % (long long)n);

    int start = var_col_ptr[v];
    int end = var_col_ptr[v + 1];
    long long ebase = (long long)b * (long long)edges;

    // belief = channel + sum of incoming check-to-var messages (CSC order).
    float belief = channel_llrs[(long long)b * (long long)n + v];
    for (int f = start; f < end; ++f) {
        int e = var_edge_to_check_edge[f];
        belief += c2v[ebase + (long long)e];
    }

    // v2c[f] = belief - incoming[f].
    for (int f = start; f < end; ++f) {
        int e = var_edge_to_check_edge[f];
        float incoming = c2v[ebase + (long long)e];
        v2c[ebase + (long long)f] = belief - incoming;
    }

    // Hard decision: bit 1 iff belief < 0 (matches `Llr::hard_decision`).
    hard_bits[(long long)b * (long long)n + v] = (belief < 0.0f) ? 1u : 0u;
}

// ---------------------------------------------------------------------------
// Initialise the var-to-check messages with the channel LLRs (every edge of
// variable v gets channel_llrs[v]), matching the CPU init loop.
// One thread per (batch frame, variable).
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_init_kernel(
    const float* __restrict__ channel_llrs,  // [batch * n]
    float* __restrict__ v2c,                  // [batch * E] (out)
    const int* __restrict__ var_col_ptr,      // [n+1]
    int n,
    int edges,
    int batch_size) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)n;
    if (gid >= total) return;
    int b = (int)(gid / (long long)n);
    int v = (int)(gid % (long long)n);
    float llr = channel_llrs[(long long)b * (long long)n + v];
    long long ebase = (long long)b * (long long)edges;
    int start = var_col_ptr[v];
    int end = var_col_ptr[v + 1];
    for (int f = start; f < end; ++f) {
        v2c[ebase + (long long)f] = llr;
    }
}

// ---------------------------------------------------------------------------
// Per-frame syndrome check: one thread per (batch frame, check). Sets
// frame_unsatisfied[b] = 1 if check c is violated by the current hard_bits.
// (A frame converges when none of its checks are violated.) Determinism: a
// boolean OR via a plain store of 1 — no f32 reduction, no read-modify-write
// dependence on order.
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_syndrome_kernel(
    const uint8_t* __restrict__ hard_bits,    // [batch * n]
    const int* __restrict__ check_row_ptr,    // [m+1]
    const int* __restrict__ check_edge_var,   // [E]
    uint8_t* __restrict__ frame_unsatisfied,  // [batch] (out)
    int m,
    int n,
    int batch_size) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)m;
    if (gid >= total) return;
    int b = (int)(gid / (long long)m);
    int c = (int)(gid % (long long)m);

    int start = check_row_ptr[c];
    int end = check_row_ptr[c + 1];
    long long nbase = (long long)b * (long long)n;
    uint32_t parity = 0u;
    for (int e = start; e < end; ++e) {
        int v = check_edge_var[e];
        parity ^= (uint32_t)hard_bits[nbase + (long long)v];
    }
    if (parity & 1u) {
        // At least one violated check in this frame: mark unsatisfied. Multiple
        // checks racing to write 1 is benign (idempotent store of the same
        // value), no atomic needed.
        frame_unsatisfied[b] = 1u;
    }
}

// ---------------------------------------------------------------------------
// Host-side launch wrappers (callable from Rust via FFI). Fixed block size,
// grid = ceil(work / block) — a pure function of the problem size (design §6).
// ---------------------------------------------------------------------------

static constexpr int BLOCK_THREADS = 256;

static inline dim3 grid_for(long long work) {
    long long g = (work + BLOCK_THREADS - 1) / BLOCK_THREADS;
    return dim3((unsigned int)g);
}

extern "C" int launch_ldpc_init(
    const float* channel_llrs,
    float* v2c,
    const int* var_col_ptr,
    int n,
    int edges,
    int batch_size,
    void* stream) {
    long long work = (long long)batch_size * (long long)n;
    if (work <= 0) return 0;
    hipLaunchKernelGGL(
        ldpc_init_kernel,
        grid_for(work), dim3(BLOCK_THREADS), 0,
        reinterpret_cast<hipStream_t>(stream),
        channel_llrs, v2c, var_col_ptr, n, edges, batch_size);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_ldpc_check_update(
    const float* v2c,
    float* c2v,
    const int* check_row_ptr,
    const int* check_edge_to_var_edge,
    int m,
    int edges,
    int batch_size,
    int algorithm,
    float alpha,
    float beta,
    void* stream) {
    long long work = (long long)batch_size * (long long)m;
    if (work <= 0) return 0;
    hipLaunchKernelGGL(
        ldpc_check_update_kernel,
        grid_for(work), dim3(BLOCK_THREADS), 0,
        reinterpret_cast<hipStream_t>(stream),
        v2c, c2v, check_row_ptr, check_edge_to_var_edge,
        m, edges, batch_size, algorithm, alpha, beta);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_ldpc_var_update(
    const float* channel_llrs,
    float* v2c,
    const float* c2v,
    const int* var_col_ptr,
    const int* var_edge_to_check_edge,
    uint8_t* hard_bits,
    int n,
    int edges,
    int batch_size,
    void* stream) {
    long long work = (long long)batch_size * (long long)n;
    if (work <= 0) return 0;
    hipLaunchKernelGGL(
        ldpc_var_update_kernel,
        grid_for(work), dim3(BLOCK_THREADS), 0,
        reinterpret_cast<hipStream_t>(stream),
        channel_llrs, v2c, c2v, var_col_ptr, var_edge_to_check_edge,
        hard_bits, n, edges, batch_size);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_ldpc_syndrome(
    const uint8_t* hard_bits,
    const int* check_row_ptr,
    const int* check_edge_var,
    uint8_t* frame_unsatisfied,
    int m,
    int n,
    int batch_size,
    void* stream) {
    long long work = (long long)batch_size * (long long)m;
    if (work <= 0) return 0;
    hipLaunchKernelGGL(
        ldpc_syndrome_kernel,
        grid_for(work), dim3(BLOCK_THREADS), 0,
        reinterpret_cast<hipStream_t>(stream),
        hard_bits, check_row_ptr, check_edge_var, frame_unsatisfied,
        m, n, batch_size);
    return static_cast<int>(hipGetLastError());
}
