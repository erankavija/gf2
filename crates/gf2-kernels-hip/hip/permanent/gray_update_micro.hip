// Dependency-chained Gray-update micro-kernels for F_3, F_5, and F_7.
//
// The update kernel owns one accumulator.  Each iteration reads the value its
// predecessor wrote, making this a latency chain rather than independent-op
// throughput.  The paired compiler-barrier control executes the same loop
// geometry without field arithmetic; the harness reports their event-time
// difference per operation.

#include <hip/hip_runtime.h>
#include <stdint.h>

__device__ __forceinline__ uint8_t add_mod(uint8_t left, uint8_t right, int q) {
    uint8_t sum = left + right;
    return sum >= q ? (uint8_t)(sum - q) : sum;
}

__device__ __forceinline__ uint8_t sub_mod(uint8_t left, uint8_t right, int q) {
    return left >= right ? (uint8_t)(left - right) : (uint8_t)(left + q - right);
}

__device__ __forceinline__ void bipedal3_add(uint64_t am, uint64_t asg,
                                              uint64_t bm, uint64_t bsg,
                                              uint64_t* out_m, uint64_t* out_s) {
    uint64_t t = am ^ asg ^ bsg;
    uint64_t u = bm & t;
    *out_m = u | (am ^ bm);
    *out_s = u ^ asg;
}

__device__ __forceinline__ void bipedal3_sub(uint64_t am, uint64_t asg,
                                              uint64_t bm, uint64_t bsg,
                                              uint64_t* out_m, uint64_t* out_s) {
    uint64_t t = asg ^ bsg;
    uint64_t u = am & t;
    *out_m = u | (am ^ bm);
    *out_s = u ^ (bm ^ bsg);
}

__global__ void gray_update_micro_kernel(const unsigned char* __restrict__ byte_column,
                                         const unsigned long long* __restrict__ bipedal_column,
                                         int q,
                                         int n,
                                         unsigned long long steps,
                                         unsigned long long* __restrict__ out) {
    if (threadIdx.x != 0 || blockIdx.x != 0) return;
    if (q == 3) {
        uint64_t mag = 0;
        uint64_t sgn = 0;
        const uint64_t column_mag = bipedal_column[0];
        const uint64_t column_sgn = bipedal_column[1];
        for (uint64_t step = 0; step < steps; ++step) {
            if ((step & 1ULL) == 0ULL) {
                bipedal3_add(mag, sgn, column_mag, column_sgn, &mag, &sgn);
            } else {
                bipedal3_sub(mag, sgn, column_mag, column_sgn, &mag, &sgn);
            }
        }
        out[0] = mag;
        out[1] = sgn;
        return;
    }

    uint8_t accumulator[63] = {};
    for (uint64_t step = 0; step < steps; ++step) {
        bool add = (step & 1ULL) == 0ULL;
        for (int row = 0; row < n; ++row) {
            accumulator[row] = add ? add_mod(accumulator[row], byte_column[row], q)
                                   : sub_mod(accumulator[row], byte_column[row], q);
        }
    }
    uint64_t checksum = 0;
    for (int row = 0; row < n; ++row) checksum += accumulator[row];
    out[0] = checksum;
}

__global__ void gray_update_compiler_barrier_baseline_kernel(
    int q,
    int n,
    unsigned long long steps,
    unsigned long long* __restrict__ out) {
    if (threadIdx.x != 0 || blockIdx.x != 0) return;
    // F_3's packed update is one two-plane operation; F_5/F_7 follow their
    // shipped byte controls, which traverse n active row values per update.
    const int inner = q == 3 ? 1 : n;
    for (uint64_t step = 0; step < steps; ++step) {
        for (int lane = 0; lane < inner; ++lane) {
            // Prevent loop elimination without adding deliberate field or
            // integer work to the baseline. Clang/hipcc accepts this empty
            // compiler barrier for device code. The distinct unused immediate
            // operands keep the selector arms separate without emitting work.
            if ((step & 1ULL) == 0ULL) {
                asm volatile("" : : "n"(0) : "memory");
            } else {
                asm volatile("" : : "n"(1) : "memory");
            }
        }
    }
    out[0] = 0;
}

extern "C" int launch_gray_update_micro(const unsigned char* byte_column,
                                         const unsigned long long* bipedal_column,
                                         int q,
                                         int n,
                                         unsigned long long steps,
                                         unsigned long long* out,
                                         hipStream_t stream) {
    if (out == nullptr || !(q == 3 || q == 5 || q == 7) || n < 1 || n > 63 || steps == 0 ||
        (q == 3 && bipedal_column == nullptr) || (q != 3 && byte_column == nullptr)) {
        return hipErrorInvalidValue;
    }
    hipLaunchKernelGGL(gray_update_micro_kernel, dim3(1), dim3(1), 0, stream,
                       byte_column, bipedal_column, q, n, steps, out);
    return (int)hipGetLastError();
}

extern "C" int launch_gray_update_compiler_barrier_baseline(
    int q,
    int n,
    unsigned long long steps,
    unsigned long long* out,
    hipStream_t stream) {
    if (out == nullptr || !(q == 3 || q == 5 || q == 7) || n < 1 || n > 63 || steps == 0) {
        return hipErrorInvalidValue;
    }
    hipLaunchKernelGGL(gray_update_compiler_barrier_baseline_kernel, dim3(1), dim3(1), 0, stream,
                       q, n, steps, out);
    return (int)hipGetLastError();
}
