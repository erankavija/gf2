// Device conformance for the F_3 wave-cooperative Ryser prototype.
//
// One block owns one matrix. Its active lanes partition the sequential
// Gray-index range [0, 2^n) into balanced, contiguous, disjoint intervals.
// Lane i reconstructs the packed column sum for g(interval.start), performs
// only its own Gray walk, and exposes only its scalar F_3 partial Ryser sum to
// the wave-level reduction. The lane-0 reduction reads sources in increasing
// lane order, so repeated launches have no schedule-dependent reduction order.
//
// Register/shared-memory accounting hypothesis (before compiler allocation):
// each lane keeps five live u64 state words -- (magnitude, sign), Gray cursor,
// interval end, and partial sum -- i.e. a 10 x 32-bit-register lower-bound
// model. The only dynamic shared allocation is the read-only column table:
// two u64 words per column = 16n bytes (384 B at n=24, 512 B at n=32). The
// compiler's actual gfx1030 resource report is committed in
// `wave_gf3_resource_usage.log` beside this source after the checked build.
//
// The device executable is intentionally a self-checking conformance binary,
// not a benchmark. It checks independent permutation references for small
// structural matrices, both Gray transition directions via its n=4 case,
// multi-index lane intervals at n=6, and the n=63 active-mask product probe.

#include <hip/hip_runtime.h>

#include "wave_ryser_mapping.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <numeric>
#include <vector>

namespace {

struct Case {
  const char* name;
  int n;
  std::vector<unsigned char> matrix;
};

__device__ __forceinline__ void add3(std::uint64_t& magnitude,
                                     std::uint64_t& sign,
                                     std::uint64_t right_magnitude,
                                     std::uint64_t right_sign) {
  const auto t = magnitude ^ sign ^ right_sign;
  const auto u = right_magnitude & t;
  sign = u ^ sign;
  magnitude = u | (magnitude ^ right_magnitude);
}

__device__ __forceinline__ void sub3(std::uint64_t& magnitude,
                                     std::uint64_t& sign,
                                     std::uint64_t right_magnitude,
                                     std::uint64_t right_sign) {
  const auto t = sign ^ right_sign;
  const auto u = magnitude & t;
  sign = u ^ (right_magnitude ^ right_sign);
  magnitude = u | (magnitude ^ right_magnitude);
}

__device__ __forceinline__ unsigned fp3_sub(unsigned left, unsigned right) {
  return left >= right ? left - right : left + 3 - right;
}

// This reduction stays inside the lane's packed words. It neither reads nor
// writes another lane's accumulator; the active mask neutralizes every tail
// bit as the Bipedal3 multiplicative identity before the six halving folds.
__device__ __forceinline__ unsigned fold_product(std::uint64_t magnitude,
                                                 std::uint64_t sign, int n) {
  const auto active = (UINT64_C(1) << n) - 1;
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

// Exactly one matrix is assigned to each block. `blockDim.x` is the active
// lane count selected solely from n; `gridDim.x` is the caller's batch size.
// There is deliberately no occupancy query or device-state-dependent shape.
__global__ void wave_gf3_kernel(const unsigned char* __restrict__ matrices,
                                int n, int batch_size,
                                unsigned* __restrict__ results) {
  const unsigned lane = threadIdx.x;
  const unsigned matrix_index = blockIdx.x;
  if (matrix_index >= static_cast<unsigned>(batch_size)) {
    return;
  }
  if (n == 0) {
    if (lane == 0) {
      results[matrix_index] = 1;
    }
    return;
  }

  // Dynamic allocation is exactly two packed words per input column.
  extern __shared__ std::uint64_t staged_columns[];
  auto* const column_magnitude = staged_columns;
  auto* const column_sign = staged_columns + n;
  const auto* const matrix =
      matrices + static_cast<std::size_t>(matrix_index) * n * n;

  // Cooperative, strided global-to-shared staging happens once per matrix.
  for (int column = static_cast<int>(lane); column < n;
       column += static_cast<int>(blockDim.x)) {
    std::uint64_t magnitude = 0;
    std::uint64_t sign = 0;
    for (int row = 0; row < n; ++row) {
      const unsigned char value = matrix[row * n + column];
      if (value != 0) {
        magnitude |= UINT64_C(1) << row;
        if (value == 2) {
          sign |= UINT64_C(1) << row;
        }
      }
    }
    column_magnitude[column] = magnitude;
    column_sign[column] = sign;
  }
  __syncthreads();

  const std::uint64_t total_indices = UINT64_C(1) << n;
  const std::uint64_t lane_count = blockDim.x;
  const auto interval = gf2_wave_mapping::balanced_interval(total_indices, lane, lane_count);

  // `cursor`, `interval_end`, and `partial` are the three scalar u64 live
  // state words. Together with the packed (magnitude, sign) pair they are the
  // documented five-word per-lane mapping budget.
  std::uint64_t cursor = interval.start;
  const std::uint64_t interval_end = interval.end;
  std::uint64_t magnitude = 0;
  std::uint64_t sign = 0;
  std::uint64_t subset = gf2_wave_mapping::gray_subset(cursor);
  for (int column = 0; column < n; ++column) {
    if (((subset >> column) & 1) != 0) {
      add3(magnitude, sign, column_magnitude[column], column_sign[column]);
    }
  }

  unsigned partial = 0;
  unsigned term = fold_product(magnitude, sign, n);
  partial = (__popcll(subset) & 1) != 0
                ? fp3_sub(partial, term)
                : gf2_wave_mapping::add_scalar<3>(partial, term);
  for (++cursor; cursor < interval_end; ++cursor) {
    const unsigned flipped_column = __builtin_ctzll(cursor);
    subset = gf2_wave_mapping::gray_subset(cursor);
    if (((subset >> flipped_column) & 1) != 0) {
      add3(magnitude, sign, column_magnitude[flipped_column],
           column_sign[flipped_column]);
    } else {
      sub3(magnitude, sign, column_magnitude[flipped_column],
           column_sign[flipped_column]);
    }
    term = fold_product(magnitude, sign, n);
    partial = (__popcll(subset) & 1) != 0
                  ? fp3_sub(partial, term)
                  : gf2_wave_mapping::add_scalar<3>(partial, term);
  }

  // Partial sums are the only cross-lane quantity. `__shfl` makes lane 0 read
  // sources in ascending lane order without allocating reduction scratch in
  // shared memory. All source lanes are active because blockDim.x is exactly
  // the deterministic partition count (a power of two no greater than 32).
  const unsigned reduced =
      gf2_wave_mapping::reduce_partials_in_lane_order<3>(partial, lane_count);
  if (lane == 0) {
    results[matrix_index] = gf2_wave_mapping::apply_outer_ryser_sign<3>(reduced, n);
  }
}

__global__ void active_mask_product_probe(unsigned* results) {
  if (blockIdx.x != 0 || threadIdx.x != 0) {
    return;
  }
  constexpr int kPartialWordOrder = 63;
  const auto active = (UINT64_C(1) << kPartialWordOrder) - 1;
  const unsigned all_active_ones = fold_product(active, UINT64_C(1) << 63,
                                                 kPartialWordOrder);
  const unsigned active_zero = fold_product(active & ~(UINT64_C(1) << 62), 0,
                                            kPartialWordOrder);
  results[0] = all_active_ones;
  results[1] = active_zero;
}

bool check(hipError_t status, const char* operation) {
  if (status == hipSuccess) {
    return true;
  }
  std::fprintf(stderr, "%s: %s\n", operation, hipGetErrorString(status));
  return false;
}

unsigned cpu_permanent(const Case& test_case) {
  if (test_case.n == 0) {
    return 1;
  }
  std::vector<int> permutation(test_case.n);
  std::iota(permutation.begin(), permutation.end(), 0);
  unsigned total = 0;
  do {
    unsigned product = 1;
    for (int row = 0; row < test_case.n; ++row) {
      product = (product * test_case.matrix[row * test_case.n + permutation[row]]) % 3;
    }
    total = (total + product) % 3;
  } while (std::next_permutation(permutation.begin(), permutation.end()));
  return total;
}

bool run_batch(const std::vector<Case>& cases) {
  const int n = cases.front().n;
  for (const auto& test_case : cases) {
    if (test_case.n != n) {
      std::fprintf(stderr, "batch mixed matrix orders\n");
      return false;
    }
  }
  std::vector<unsigned char> host_matrices;
  std::vector<unsigned> expected;
  for (const auto& test_case : cases) {
    host_matrices.insert(host_matrices.end(), test_case.matrix.begin(), test_case.matrix.end());
    expected.push_back(cpu_permanent(test_case));
  }

  unsigned char* device_matrices = nullptr;
  unsigned* device_results = nullptr;
  std::vector<unsigned> actual(cases.size(), 0);
  const auto bytes = host_matrices.size() * sizeof(unsigned char);
  const bool allocated_matrices =
      n == 0 || check(hipMalloc(&device_matrices, bytes), "hipMalloc matrices");
  if (!allocated_matrices ||
      !check(hipMalloc(&device_results, actual.size() * sizeof(actual.front())),
             "hipMalloc results")) {
    (void)hipFree(device_matrices);
    (void)hipFree(device_results);
    return false;
  }
  const bool copied = n == 0 || check(hipMemcpy(device_matrices, host_matrices.data(), bytes,
                                                 hipMemcpyHostToDevice),
                                       "hipMemcpy matrices");
  if (copied) {
    hipLaunchKernelGGL(wave_gf3_kernel, dim3(cases.size()),
                       dim3(gf2_wave_mapping::active_lanes_for_order(n)),
                       2 * n * sizeof(std::uint64_t), 0,
                       device_matrices, n, static_cast<int>(cases.size()), device_results);
  }
  const bool launched = copied && check(hipGetLastError(), "wave_gf3 kernel launch") &&
                        check(hipDeviceSynchronize(), "wave_gf3 synchronize") &&
                        check(hipMemcpy(actual.data(), device_results,
                                        actual.size() * sizeof(actual.front()),
                                        hipMemcpyDeviceToHost),
                              "hipMemcpy results");
  const bool freed_matrices = n == 0 || check(hipFree(device_matrices), "hipFree matrices");
  const bool freed_results = check(hipFree(device_results), "hipFree results");
  if (!launched || !freed_matrices || !freed_results) {
    return false;
  }
  for (std::size_t index = 0; index < cases.size(); ++index) {
    if (actual[index] != expected[index]) {
      std::fprintf(stderr, "%s: device=%u cpu-permutation=%u\n", cases[index].name,
                   actual[index], expected[index]);
      return false;
    }
  }
  return true;
}

bool run_active_mask_probe() {
  unsigned* device_results = nullptr;
  std::array<unsigned, 2> host_results{};
  if (!check(hipMalloc(&device_results, sizeof(host_results)), "hipMalloc mask probe")) {
    return false;
  }
  hipLaunchKernelGGL(active_mask_product_probe, dim3(1), dim3(1), 0, 0,
                     device_results);
  const bool launched = check(hipGetLastError(), "active-mask probe launch") &&
                        check(hipDeviceSynchronize(), "active-mask probe synchronize") &&
                        check(hipMemcpy(host_results.data(), device_results,
                                        sizeof(host_results), hipMemcpyDeviceToHost),
                              "hipMemcpy mask probe");
  const bool freed = check(hipFree(device_results), "hipFree mask probe");
  if (!launched || !freed) {
    return false;
  }
  if (host_results != std::array<unsigned, 2>{1, 0}) {
    std::fprintf(stderr, "n=63 active-mask probe got {%u, %u}, expected {1, 0}\n",
                 host_results[0], host_results[1]);
    return false;
  }
  return true;
}

Case patterned_case(const char* name, int n, unsigned offset) {
  Case test_case{name, n, std::vector<unsigned char>(n * n)};
  for (int row = 0; row < n; ++row) {
    for (int column = 0; column < n; ++column) {
      test_case.matrix[row * n + column] =
          static_cast<unsigned char>((row * 2 + column + offset) % 3);
    }
  }
  return test_case;
}

}  // namespace

int main() {
  const std::vector<Case> empty_cases = {
      {"n0-empty", 0, {}},
  };
  // n=4 covers every one-index lane interval, including the add at index 8
  // and subtraction at index 15 named by the committed structural corpus.
  const std::vector<Case> n4_cases = {
      patterned_case("n4-gray-boundaries-a", 4, 1),
      patterned_case("n4-gray-boundaries-b", 4, 2),
  };
  // n=6 gives each of 32 lanes a two-index interval, exercising the local
  // incremental update after interval-start subset reconstruction.
  const std::vector<Case> n6_cases = {
      patterned_case("n6-two-index-intervals", 6, 1),
  };
  // The zero row makes every nonempty row-product zero without relying on the
  // GPU's result to establish the expected value.
  const std::vector<Case> zero_cases = {
      {"n3-zero-product", 3, {0, 0, 0, 0, 0, 0, 0, 1, 2}},
  };

  if (!run_batch(empty_cases) || !run_batch(n4_cases) || !run_batch(n6_cases) ||
      !run_batch(zero_cases) || !run_active_mask_probe()) {
    return 1;
  }
  return 0;
}
