// BCJR forward-backward batch kernel for linear block codes.
//
// One thread block per SISO decode. 1024 threads per block, each thread
// handles 2 trellis states (for 2048-state codes like dRM(32,21)).
//
// Key insight: the XOR-indexed scatter is reformulated as a conflict-free
// read-permutation. Thread s reads alpha[s] and alpha[s ^ h_col], then
// writes to alpha_next[s]. No atomics needed.

#include <hip/hip_runtime.h>
#include <cmath>
#include <cfloat>

static constexpr int MAX_STATES = 2048;
static constexpr float NEG_INF = -INFINITY;

// Jacobian logarithm: ln(e^a + e^b) = max(a,b) + ln(1 + exp(-|a-b|))
__device__ __forceinline__ float max_star(float a, float b) {
    if (a == NEG_INF) return b;
    if (b == NEG_INF) return a;
    float m = fmaxf(a, b);
    float d = fabsf(a - b);
    return (d > 8.0f) ? m : m + log1pf(expf(-d));
}

// Warp-level max reduction (wavefront=32 on RDNA2, wavefront=64 on CDNA)
__device__ __forceinline__ float warp_reduce_max(float val) {
    for (int offset = warpSize / 2; offset > 0; offset >>= 1) {
        val = fmaxf(val, __shfl_xor(val, offset));
    }
    return val;
}

// Warp-level max_star reduction (log-sum-exp across warp)
__device__ __forceinline__ float warp_reduce_max_star(float val) {
    for (int offset = warpSize / 2; offset > 0; offset >>= 1) {
        val = max_star(val, __shfl_xor(val, offset));
    }
    return val;
}

// Block-level max reduction using shared memory
__device__ float block_reduce_max(float val, float* smem) {
    int tid = threadIdx.x;
    int warp_id = tid / warpSize;
    int lane = tid % warpSize;
    int num_warps = blockDim.x / warpSize;

    val = warp_reduce_max(val);
    if (lane == 0) smem[warp_id] = val;
    __syncthreads();

    // First warp reduces across warps
    if (warp_id == 0) {
        val = (lane < num_warps) ? smem[lane] : NEG_INF;
        val = warp_reduce_max(val);
    }
    __syncthreads();
    // Broadcast from thread 0
    if (tid == 0) smem[0] = val;
    __syncthreads();
    return smem[0];
}

// Block-level max_star reduction (log-sum-exp)
__device__ float block_reduce_max_star(float val, float* smem) {
    int tid = threadIdx.x;
    int warp_id = tid / warpSize;
    int lane = tid % warpSize;
    int num_warps = blockDim.x / warpSize;

    val = warp_reduce_max_star(val);
    if (lane == 0) smem[warp_id] = val;
    __syncthreads();

    if (warp_id == 0) {
        val = (lane < num_warps) ? smem[lane] : NEG_INF;
        val = warp_reduce_max_star(val);
    }
    __syncthreads();
    if (tid == 0) smem[0] = val;
    __syncthreads();
    return smem[0];
}

// Main BCJR forward-backward kernel.
// Grid: batch_size blocks. Block: 1024 threads.
__global__ void bcjr_batch_kernel(
    const float* __restrict__ g_combined_llrs,  // [batch_size][n]
    const unsigned int* __restrict__ g_h_cols,   // [n]
    float* __restrict__ g_app_llrs,              // [batch_size][n]
    float* __restrict__ g_alpha_all,             // [batch_size][n+1][num_states]
    int n,
    int num_states
) {
    int batch_idx = blockIdx.x;
    int tid = threadIdx.x;
    int bdim = blockDim.x;

    const float* llrs = g_combined_llrs + batch_idx * n;
    float* out_app = g_app_llrs + batch_idx * n;
    float* alpha_store = g_alpha_all + batch_idx * (n + 1) * num_states;

    __shared__ float alpha[MAX_STATES];
    __shared__ float beta[MAX_STATES];
    __shared__ float scratch[64]; // For reductions (max 64 warps per block)

    // ====== FORWARD PASS ======

    // Initialize alpha: state 0 = 0.0, rest = -INF
    for (int s = tid; s < num_states; s += bdim) {
        float val = (s == 0) ? 0.0f : NEG_INF;
        alpha[s] = val;
        alpha_store[s] = val;  // Store level 0
    }
    __syncthreads();

    for (int i = 0; i < n; i++) {
        float g0 = llrs[i] * 0.5f;
        float g1 = -llrs[i] * 0.5f;
        unsigned int h = g_h_cols[i];

        // Compute next alpha via read-permutation (no write conflicts)
        float local_max = NEG_INF;
        for (int s = tid; s < num_states; s += bdim) {
            float a_stay = alpha[s];
            float a_xor  = alpha[s ^ h];
            float c0 = (a_stay > NEG_INF) ? a_stay + g0 : NEG_INF;
            float c1 = (a_xor  > NEG_INF) ? a_xor  + g1 : NEG_INF;
            float nxt = max_star(c0, c1);
            // Write to beta temporarily (avoid alpha read-after-write)
            beta[s] = nxt;
            local_max = fmaxf(local_max, nxt);
        }
        __syncthreads();

        // Block-level max for normalization
        float max_val = block_reduce_max(local_max, scratch);

        // Normalize and copy to alpha, store for APP computation
        float* store_row = alpha_store + (i + 1) * num_states;
        for (int s = tid; s < num_states; s += bdim) {
            float val = beta[s];
            val = (val > NEG_INF) ? val - max_val : NEG_INF;
            alpha[s] = val;
            store_row[s] = val;
        }
        __syncthreads();
    }

    // ====== BACKWARD PASS + APP COMPUTATION ======

    // Initialize beta: state 0 = 0.0, rest = -INF
    for (int s = tid; s < num_states; s += bdim) {
        beta[s] = (s == 0) ? 0.0f : NEG_INF;
    }
    __syncthreads();

    for (int i = n - 1; i >= 0; i--) {
        float g0 = llrs[i] * 0.5f;
        float g1 = -llrs[i] * 0.5f;
        unsigned int h = g_h_cols[i];

        // --- APP computation for bit i ---
        // P(c_i=0 | y) ~ sum_s alpha[i][s] * gamma(c=0) * beta[i+1][s]
        // P(c_i=1 | y) ~ sum_s alpha[i][s] * gamma(c=1) * beta[i+1][s^h]
        const float* alpha_i = alpha_store + i * num_states;

        float local_p0 = NEG_INF;
        float local_p1 = NEG_INF;
        for (int s = tid; s < num_states; s += bdim) {
            float a = alpha_i[s];
            if (a > NEG_INF) {
                local_p0 = max_star(local_p0, a + g0 + beta[s]);
                local_p1 = max_star(local_p1, a + g1 + beta[s ^ h]);
            }
        }

        // Block-level log-sum-exp reduction for p0 and p1
        float p0 = block_reduce_max_star(local_p0, scratch);
        float p1 = block_reduce_max_star(local_p1, scratch);

        if (tid == 0) {
            out_app[i] = p0 - p1;  // APP LLR
        }

        // --- Update beta for next stage ---
        // Use alpha[] array as temp buffer (forward pass is done)
        float local_bmax = NEG_INF;
        for (int s = tid; s < num_states; s += bdim) {
            float b_stay = beta[s];
            float b_xor  = beta[s ^ h];
            float c0 = (b_stay > NEG_INF) ? b_stay + g0 : NEG_INF;
            float c1 = (b_xor  > NEG_INF) ? b_xor  + g1 : NEG_INF;
            float nxt = max_star(c0, c1);
            alpha[s] = nxt;  // Using alpha as temp
            local_bmax = fmaxf(local_bmax, nxt);
        }
        __syncthreads();

        float bmax = block_reduce_max(local_bmax, scratch);

        for (int s = tid; s < num_states; s += bdim) {
            float val = alpha[s];
            beta[s] = (val > NEG_INF) ? val - bmax : NEG_INF;
        }
        __syncthreads();
    }
}

// Host-side launch wrapper callable from Rust via FFI.
extern "C" int launch_bcjr_batch(
    const float* combined_llrs,    // Device ptr: [batch_size][n]
    const unsigned int* h_cols,    // Device ptr: [n]
    float* app_llrs,               // Device ptr: [batch_size][n]
    float* alpha_workspace,        // Device ptr: [batch_size][n+1][num_states]
    int batch_size,
    int n,
    int num_states,
    void* stream                   // hipStream_t (nullptr for default)
) {
    dim3 grid(batch_size);
    dim3 block(1024);

    hipLaunchKernelGGL(
        bcjr_batch_kernel,
        grid, block,
        0,  // shared memory (statically allocated)
        reinterpret_cast<hipStream_t>(stream),
        combined_llrs, h_cols, app_llrs, alpha_workspace,
        n, num_states
    );

    return static_cast<int>(hipGetLastError());
}

// Convenience wrappers for HIP memory management.
extern "C" int hip_malloc(void** ptr, size_t size) {
    return static_cast<int>(hipMalloc(ptr, size));
}

extern "C" int hip_free(void* ptr) {
    return static_cast<int>(hipFree(ptr));
}

extern "C" int hip_memcpy_h2d(void* dst, const void* src, size_t size) {
    return static_cast<int>(hipMemcpy(dst, src, size, hipMemcpyHostToDevice));
}

extern "C" int hip_memcpy_d2h(void* dst, const void* src, size_t size) {
    return static_cast<int>(hipMemcpy(dst, src, size, hipMemcpyDeviceToHost));
}

extern "C" int hip_device_synchronize() {
    return static_cast<int>(hipDeviceSynchronize());
}
