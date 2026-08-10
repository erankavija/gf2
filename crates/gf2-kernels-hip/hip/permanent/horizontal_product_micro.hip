// Branch-aware horizontal-product micro-kernels.
//
// Each launch receives already-sampled row sums in the representation selected
// by its circuit ID. The fast and slow inputs are selected by the harness only
// after their unconditioned frequency has been counted. Device events surround
// the product launch and its paired compiler-barrier launch, never upload,
// download, allocation, submission, or host grouping work.

#include <hip/hip_runtime.h>
#include <stdint.h>

// This file is included by permanent_bipedal7.hip after that file defines and
// initializes d_MUL_LUT. Keeping the lookup circuit in the same HIP
// translation unit preserves that established device-resident multiplication
// table without a duplicate or cross-unit device-symbol dependency.

enum HorizontalCircuit : int {
    kBipedal3Halving = 1,
    kBipedal3ZeroMaskSignPopcount = 2,
    kF5Byte = 3,
    kF5ThreePlane = 4,
    kF7Lookup = 5,
    kF7ThreePlane = 6,
};

__device__ __forceinline__ uint64_t active_mask_for_order(int n) {
    return (UINT64_C(1) << n) - 1;
}

__device__ __forceinline__ unsigned fp5_mul(unsigned left, unsigned right) {
    return (left * right) % 5U;
}

__device__ __forceinline__ unsigned bipedal3_halving_product(uint64_t magnitude,
                                                               uint64_t sign, int n) {
    const auto active = active_mask_for_order(n);
    magnitude |= ~active;
    sign &= active;
    magnitude &= magnitude >> 32;
    sign ^= sign >> 32;
    magnitude &= magnitude >> 16;
    sign ^= sign >> 16;
    magnitude &= magnitude >> 8;
    sign ^= sign >> 8;
    magnitude &= magnitude >> 4;
    sign ^= sign >> 4;
    magnitude &= magnitude >> 2;
    sign ^= sign >> 2;
    magnitude &= magnitude >> 1;
    sign ^= sign >> 1;
    if ((magnitude & 1) == 0) {
        return 0;
    }
    return (sign & 1) == 0 ? 1 : 2;
}

__device__ __forceinline__ unsigned bipedal3_zero_mask_sign_popcount_product(
    uint64_t magnitude, uint64_t sign, int n) {
    const auto active = active_mask_for_order(n);
    if ((~magnitude & active) != 0) {
        return 0;
    }
    return (__popcll(sign & active) & 1) == 0 ? 1 : 2;
}

__device__ __forceinline__ unsigned f5_byte_product(const unsigned char* values, int n) {
    unsigned product = 1;
    for (int row = 0; row < n; ++row) {
        product = fp5_mul(product, values[row]);
        if (product == 0) {
            break;
        }
    }
    return product;
}

__device__ __forceinline__ unsigned f5_three_plane_product(
    const unsigned long long* planes, int n) {
    const auto active = active_mask_for_order(n);
    const auto b0 = planes[0] & active;
    const auto b1 = planes[1] & active;
    const auto b2 = planes[2] & active;
    if ((~(b0 | b1 | b2) & active) != 0) {
        return 0;
    }
    const auto exponent = __popcll(b1 & ~b0 & ~b2) +
                          2 * __popcll(b2 & ~b0 & ~b1) +
                          3 * __popcll(b0 & b1 & ~b2);
    constexpr unsigned kExponentToValue[4] = {1, 2, 4, 3};
    return kExponentToValue[exponent % 4];
}

__device__ __forceinline__ unsigned f7_lookup_product(const unsigned char* values, int n) {
    unsigned product = 1;
    for (int row = 0; row < n; ++row) {
        const auto key = product | (static_cast<unsigned>(values[row]) << 8);
        product = d_MUL_LUT[key];
        if (product == 0) {
            break;
        }
    }
    return product;
}

__device__ __forceinline__ unsigned f7_three_plane_product(
    const unsigned long long* planes, int n) {
    const auto active = active_mask_for_order(n);
    const auto b0 = planes[0] & active;
    const auto b1 = planes[1] & active;
    const auto b2 = planes[2] & active;
    if ((~(b0 | b1 | b2) & active) != 0) {
        return 0;
    }
    const auto not_b0 = ~b0;
    const auto not_b1 = ~b1;
    const auto not_b2 = ~b2;
    const auto exponent = __popcll(b0 & b1 & not_b2) +
                          2 * __popcll(not_b0 & b1 & not_b2) +
                          3 * __popcll(not_b0 & b1 & b2) +
                          4 * __popcll(not_b0 & not_b1 & b2) +
                          5 * __popcll(b0 & not_b1 & b2);
    constexpr unsigned kExponentToValue[6] = {1, 3, 2, 6, 4, 5};
    return kExponentToValue[exponent % 6];
}

__global__ void horizontal_product_micro_kernel(const unsigned char* __restrict__ byte_values,
                                                const unsigned long long* __restrict__ plane_values,
                                                int circuit, int n, int sample_count,
                                                unsigned long long* __restrict__ out) {
    const int sample = static_cast<int>(blockIdx.x);
    if (sample >= sample_count || threadIdx.x != 0) {
        return;
    }
    unsigned product = 0;
    switch (circuit) {
        case kBipedal3Halving: {
            const auto* planes = plane_values + 2 * sample;
            product = bipedal3_halving_product(planes[0], planes[1], n);
            break;
        }
        case kBipedal3ZeroMaskSignPopcount: {
            const auto* planes = plane_values + 2 * sample;
            product = bipedal3_zero_mask_sign_popcount_product(planes[0], planes[1], n);
            break;
        }
        case kF5Byte:
            product = f5_byte_product(byte_values + sample * n, n);
            break;
        case kF5ThreePlane:
            product = f5_three_plane_product(plane_values + 3 * sample, n);
            break;
        case kF7Lookup:
            product = f7_lookup_product(byte_values + sample * n, n);
            break;
        case kF7ThreePlane:
            product = f7_three_plane_product(plane_values + 3 * sample, n);
            break;
        default:
            return;
    }
    out[sample] = product;
}

__global__ void horizontal_product_compiler_barrier_baseline_kernel(
    int circuit, int branch, int n, int sample_count,
    const unsigned char* __restrict__ iterations, unsigned long long* __restrict__ out) {
    const int sample = static_cast<int>(blockIdx.x);
    if (sample >= sample_count || threadIdx.x != 0) {
        return;
    }
    // The host derives `iterations` from each selected input's real execution
    // geometry: direct-byte products stop at their first zero; zero-mask C4/C6
    // paths use their fixed mask-plus-reduction geometry. The loop performs no
    // field arithmetic, table access, or memory access beyond this geometry
    // descriptor, matching the Gray micro-mode's compiler-barrier policy.
    const int geometry = static_cast<int>(iterations[sample]);
    for (int operation = 0; operation < geometry; ++operation) {
        if (((operation + circuit + branch + n) & 1) == 0) {
            asm volatile("" : : "n"(0) : "memory");
        } else {
            asm volatile("" : : "n"(1) : "memory");
        }
    }
    out[sample] = 0;
}

extern "C" int launch_horizontal_product_micro(
    const unsigned char* byte_values, const unsigned long long* plane_values, int circuit,
    int branch, int n, int sample_count, unsigned long long* out, hipStream_t stream) {
    if (out == nullptr || !(branch == 0 || branch == 1) || n < 1 || n > 63 || sample_count < 1 ||
        circuit < kBipedal3Halving || circuit > kF7ThreePlane ||
        ((circuit == kF5Byte || circuit == kF7Lookup) && byte_values == nullptr) ||
        ((circuit != kF5Byte && circuit != kF7Lookup) && plane_values == nullptr)) {
        return hipErrorInvalidValue;
    }
    hipLaunchKernelGGL(horizontal_product_micro_kernel, dim3(sample_count), dim3(1), 0, stream,
                       byte_values, plane_values, circuit, n, sample_count, out);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_horizontal_product_compiler_barrier_baseline(
    int circuit, int branch, int n, int sample_count, const unsigned char* iterations,
    unsigned long long* out, hipStream_t stream) {
    if (out == nullptr || iterations == nullptr || !(branch == 0 || branch == 1) || n < 1 ||
        n > 63 || sample_count < 1 || circuit < kBipedal3Halving || circuit > kF7ThreePlane) {
        return hipErrorInvalidValue;
    }
    hipLaunchKernelGGL(horizontal_product_compiler_barrier_baseline_kernel, dim3(sample_count),
                       dim3(1), 0, stream, circuit, branch, n, sample_count, iterations, out);
    return static_cast<int>(hipGetLastError());
}
