// Device-side arithmetic conformance for the standalone F_7 accumulator.
//
// This is intentionally one thread and does not implement a permanent kernel.
// It validates only the Mersenne add/subtract and active-mask/C6 product
// circuits that a later permanent-shaped kernel (cc162697) will consume.

#include <hip/hip_runtime.h>

#include <cstdint>
#include <cstdio>

struct ThreePlane {
  std::uint64_t b0;
  std::uint64_t b1;
  std::uint64_t b2;
};

__device__ __forceinline__ ThreePlane add7(ThreePlane left, ThreePlane right) {
  const auto s0 = left.b0 ^ right.b0;
  const auto c1 = left.b0 & right.b0;
  const auto axb1 = left.b1 ^ right.b1;
  const auto s1 = axb1 ^ c1;
  const auto c2 = (left.b1 & right.b1) | (c1 & axb1);
  const auto axb2 = left.b2 ^ right.b2;
  const auto s2 = axb2 ^ c2;
  const auto c3 = (left.b2 & right.b2) | (c2 & axb2);

  const auto s1_and_s0 = s1 & s0;
  const auto is_seven = s1_and_s0 & s2;
  const auto no_seven = ~is_seven;
  const auto no_carry = ~c3;
  return {
      ((s0 & no_seven) & no_carry) | ((~s0) & c3),
      ((s1 & no_seven) & no_carry) | ((s1 ^ s0) & c3),
      ((s2 & no_seven) & no_carry) | ((s2 ^ s1_and_s0) & c3),
  };
}

__device__ __forceinline__ ThreePlane sub7(ThreePlane left, ThreePlane right) {
  const auto nonzero = right.b0 | right.b1 | right.b2;
  return add7(left, {(~right.b0) & nonzero, (~right.b1) & nonzero,
                     (~right.b2) & nonzero});
}

__device__ __forceinline__ unsigned lane0(ThreePlane value) {
  return static_cast<unsigned>((value.b0 & 1) | ((value.b1 & 1) << 1) |
                               ((value.b2 & 1) << 2));
}

__device__ __forceinline__ unsigned horizontal_product(ThreePlane value,
                                                        std::uint64_t active) {
  const auto b0 = value.b0 & active;
  const auto b1 = value.b1 & active;
  const auto b2 = value.b2 & active;
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
  constexpr unsigned exponent_to_value[6] = {1, 3, 2, 6, 4, 5};
  return exponent_to_value[exponent % 6];
}

__device__ __forceinline__ ThreePlane three_lanes(unsigned first, unsigned second,
                                                   unsigned third) {
  return {
      (first & 1U) | ((second & 1U) << 1) | ((third & 1U) << 2),
      ((first >> 1) & 1U) | (((second >> 1) & 1U) << 1) |
          (((third >> 1) & 1U) << 2),
      ((first >> 2) & 1U) | (((second >> 2) & 1U) << 1) |
          (((third >> 2) & 1U) << 2),
  };
}

__global__ void f7_three_plane_equivalence(int* failures) {
  if (blockIdx.x != 0 || threadIdx.x != 0) {
    return;
  }

  int count = 0;
  for (unsigned left = 0; left < 7; ++left) {
    for (unsigned right = 0; right < 7; ++right) {
      const auto lhs = three_lanes(left, 0, 0);
      const auto rhs = three_lanes(right, 0, 0);
      count += lane0(add7(lhs, rhs)) != (left + right) % 7;
      count += lane0(sub7(lhs, rhs)) != (left + 7 - right) % 7;
    }
  }
  for (unsigned first = 0; first < 7; ++first) {
    for (unsigned second = 0; second < 7; ++second) {
      for (unsigned third = 0; third < 7; ++third) {
        count += horizontal_product(three_lanes(first, second, third), 0x7) !=
                 (first * second * third) % 7;
      }
    }
  }
  *failures = count;
}

namespace {

bool check(hipError_t status, const char* operation) {
  if (status == hipSuccess) {
    return true;
  }
  std::fprintf(stderr, "%s: %s\n", operation, hipGetErrorString(status));
  return false;
}

}  // namespace

int main() {
  int* device_failures = nullptr;
  int host_failures = -1;
  if (!check(hipMalloc(&device_failures, sizeof(host_failures)), "hipMalloc")) {
    return 2;
  }
  if (!check(hipMemset(device_failures, 0, sizeof(host_failures)), "hipMemset")) {
    (void)hipFree(device_failures);
    return 3;
  }
  hipLaunchKernelGGL(f7_three_plane_equivalence, dim3(1), dim3(1), 0, 0,
                     device_failures);
  if (!check(hipGetLastError(), "kernel launch") ||
      !check(hipDeviceSynchronize(), "hipDeviceSynchronize") ||
      !check(hipMemcpy(&host_failures, device_failures, sizeof(host_failures),
                       hipMemcpyDeviceToHost),
             "hipMemcpy")) {
    (void)hipFree(device_failures);
    return 4;
  }
  if (!check(hipFree(device_failures), "hipFree")) {
    return 5;
  }
  if (host_failures != 0) {
    std::fprintf(stderr, "F_7 three-plane device arithmetic failures: %d\n",
                 host_failures);
    return 6;
  }
  return 0;
}
