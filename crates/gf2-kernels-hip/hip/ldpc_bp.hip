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
// # Early-termination = per-frame freeze (matches the CPU loop structurally)
//
// `LdpcDecoder::decode_to_codeword(.., early_termination=true)` freezes a
// frame's hard decision at the FIRST iteration its syndrome passes (it `break`s
// out of the iteration loop the moment `is_valid_codeword` holds), so the final
// hard decision is the codeword at first convergence — NOT whatever later
// flooding-BP iterations would produce. To reproduce that bit-for-bit across a
// batch (where different frames converge at different iterations), every kernel
// here takes a per-frame `frame_done` flag array (length `batch`):
//
//   * The syndrome kernel SETS `frame_done[b]` the iteration frame `b`'s
//     syndrome first passes (early-termination only).
//   * The check-node, variable-node, and syndrome kernels SKIP any frame whose
//     `frame_done[b]` is already set — so its `hard_bits`, `v2c`, and `c2v`
//     are frozen at the first-convergence state (also a perf win: converged
//     frames stop doing work). When `frame_done` is `nullptr` (early-term off)
//     no frame is ever frozen and all `max_iters` run, matching the CPU
//     `early_termination == false` path.
//
// The host loop stops when every frame is done (all `frame_done[b] == 1`) or
// `max_iters` is reached.
//
// # Determinism (design doc §11)
//
// All message ordering matches the CPU exactly:
//   * Check-node gather order is the parity-check matrix CSR row order
//     (`SpBitMatrixDual::row_iter`); the SumProduct `tanh` product is
//     accumulated left-to-right in that order so the order-dependent f32
//     product is reproduced bit-for-bit.
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
// The min-sum-family sign is taken from the value's SIGN BIT
// (`signbit(lv) ? -1 : +1`) so that `-0.0` counts as NEGATIVE, matching the CPU
// default build's AVX2 vectorized min-sum lanes (`gf2-kernels-simd/src/llr.rs`),
// whose dominant vectorized path keys the sign on the sign bit (the SIMD scalar
// remainder uses `< 0.0`, but the vectorized lanes dominate for n=64800, and
// the byte-identity test compares against the CPU with `simd` on). A `-0.0` v2c
// message is rare (needs `belief == incoming`); aligning on the sign bit removes
// that latent bit-flip. The min magnitude is `min(|lv|)` and the sign is a
// product of `±1`, both exactly representable in f32 and order-independent, so
// the min-sum family is reproduced exactly regardless of gather order; only the
// SumProduct `tanh`/`atanh` path is order-sensitive (handled above).
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
// # Per-`i_LS` shift table (the 5G NR seam, design doc §6, Phase E `23d3525f`)
//
// `ldpc_check_update_kernel` and its launcher take a `shift_table`
// (`const int*`) + `shift_len` (`int`) parameter pair, threaded all the way
// through the Rust host wrapper and `LdpcGraphLayout`, so a Phase E BG1/BG2
// caller can supply the lifting-set cyclic-shift row without ANY breaking FFI
// signature change. For the fully-expanded DVB-T2 graph the host passes
// `shift_table == nullptr` and `shift_len == 0`, and the kernel ignores it (the
// DVB-T2 graph is already expanded into the CSR above, so message routing needs
// no per-block shift). When `shift_len > 0` a future lifted-graph variant reads
// the per-block shift here to re-route messages across the Z-circulant; that
// expansion is out of scope for this task (DVB-T2 only) but the parameter is
// wired now so the seam is real, not cosmetic.

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

// True if frame `b` is frozen (converged) under early termination. When
// `frame_done == nullptr` (early termination off) no frame is ever frozen.
__device__ __forceinline__ bool frame_frozen(const uint8_t* frame_done, int b) {
    return frame_done != nullptr && frame_done[b] != 0u;
}

// ---------------------------------------------------------------------------
// Check-node update: one thread per (batch frame, check) pair.
//
// For each edge e of check c (in CSR order), gathers the var-to-check messages
// of all OTHER edges of c (also CSR order) and writes the check-to-var message
// c2v[e] using the configured box-plus rule. The gathered v2c values come from
// the matching var-edge `check_edge_to_var_edge[e']`. Frozen frames are skipped
// so their c2v stays at the first-convergence state.
//
// `shift_table` / `shift_len`: the per-`i_LS` 5G NR lifting-set shift row (the
// Phase E seam). Ignored when `shift_len == 0` (the DVB-T2 fully-expanded path).
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_check_update_kernel(
    const float* __restrict__ v2c,          // [batch * E] var-to-check (CSC view)
    float* __restrict__ c2v,                 // [batch * E] check-to-var (CSR view) (out)
    const int* __restrict__ check_row_ptr,   // [m+1]
    const int* __restrict__ check_edge_to_var_edge, // [E]
    const uint8_t* __restrict__ frame_done,  // [batch] or nullptr
    const int* __restrict__ shift_table,     // [shift_len] or nullptr (5G NR seam)
    int shift_len,
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
    if (frame_frozen(frame_done, b)) return; // frozen: keep first-convergence c2v

    // The per-`i_LS` shift table is reserved for the Phase E lifted-graph
    // (BG1/BG2) variant; the DVB-T2 graph is fully expanded, so `shift_len == 0`
    // and no per-block shift is applied. Touch the params so they are part of
    // the live signature (not dead-stripped) without changing DVB-T2 behaviour.
    (void)shift_table;
    (void)shift_len;

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
            // Min-sum family: sign product * min magnitude. The sign is keyed on
            // the SIGN BIT (so -0.0 is negative), matching the CPU default
            // build's AVX2 vectorized min-sum lanes; both the sign product
            // (±1) and the min magnitude are exactly representable and
            // order-independent.
            float sign = 1.0f;
            float minmag = INFINITY;
            bool any = false;
            for (int o = start; o < end; ++o) {
                if (o == e) continue;
                int f = check_edge_to_var_edge[o];
                float lv = v2c[base + (long long)f];
                sign *= signbit(lv) ? -1.0f : 1.0f;
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
// (belief < 0 -> bit 1). Frozen frames are skipped so their v2c and hard_bits
// stay at the first-convergence state.
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_var_update_kernel(
    const float* __restrict__ channel_llrs,  // [batch * n]
    float* __restrict__ v2c,                  // [batch * E] var-to-check (out)
    const float* __restrict__ c2v,            // [batch * E] check-to-var (in)
    const int* __restrict__ var_col_ptr,      // [n+1]
    const int* __restrict__ var_edge_to_check_edge, // [E]
    uint8_t* __restrict__ hard_bits,          // [batch * n] (out): 1 if bit 1
    const uint8_t* __restrict__ frame_done,   // [batch] or nullptr
    int n,
    int edges,
    int batch_size) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)n;
    if (gid >= total) return;
    int b = (int)(gid / (long long)n);
    int v = (int)(gid % (long long)n);
    if (frame_frozen(frame_done, b)) return; // frozen: keep first-convergence state

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
// One thread per (batch frame, variable). Runs once before iteration, so no
// freeze check is needed (no frame is converged yet).
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
// Per-frame syndrome check: one thread per (batch frame, check).
//
// Computes per-check parity. A frame converges when none of its checks are
// violated. To detect that without a cross-thread reduction we use a small
// per-frame `unsatisfied` flag: it is cleared by the host before each launch,
// and any violated check stores 1 (an idempotent OR; no atomic needed). The
// host reads it back and, when a frame has NO violated check (`unsatisfied == 0`),
// sets `frame_done[b]` (host-side) to freeze the frame from the next iteration.
//
// Frozen frames (`frame_done[b] != 0`) are skipped: they neither read stale
// `hard_bits` nor (re)mark `unsatisfied`, so a frozen-and-satisfied frame stays
// satisfied across the remaining iterations regardless of other frames' state.
// `frame_done` is `nullptr` when early termination is off (no freezing).
// ---------------------------------------------------------------------------
extern "C" __global__ void ldpc_syndrome_kernel(
    const uint8_t* __restrict__ hard_bits,    // [batch * n]
    const int* __restrict__ check_row_ptr,    // [m+1]
    const int* __restrict__ check_edge_var,   // [E]
    uint8_t* __restrict__ frame_unsatisfied,  // [batch] (out)
    const uint8_t* __restrict__ frame_done,   // [batch] or nullptr
    int m,
    int n,
    int batch_size) {
    long long gid = (long long)blockIdx.x * blockDim.x + threadIdx.x;
    long long total = (long long)batch_size * (long long)m;
    if (gid >= total) return;
    int b = (int)(gid / (long long)m);
    int c = (int)(gid % (long long)m);
    if (frame_frozen(frame_done, b)) return; // already converged: stays satisfied

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
    const uint8_t* frame_done,   // nullptr when early termination is off
    const int* shift_table,      // nullptr for DVB-T2 (5G NR seam)
    int shift_len,               // 0 for DVB-T2
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
        v2c, c2v, check_row_ptr, check_edge_to_var_edge, frame_done,
        shift_table, shift_len, m, edges, batch_size, algorithm, alpha, beta);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_ldpc_var_update(
    const float* channel_llrs,
    float* v2c,
    const float* c2v,
    const int* var_col_ptr,
    const int* var_edge_to_check_edge,
    uint8_t* hard_bits,
    const uint8_t* frame_done,   // nullptr when early termination is off
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
        hard_bits, frame_done, n, edges, batch_size);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_ldpc_syndrome(
    const uint8_t* hard_bits,
    const int* check_row_ptr,
    const int* check_edge_var,
    uint8_t* frame_unsatisfied,
    const uint8_t* frame_done,   // nullptr when early termination is off
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
        frame_done, m, n, batch_size);
    return static_cast<int>(hipGetLastError());
}
