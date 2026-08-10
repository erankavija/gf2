// Device conformance for the paired F_5 wave-owned permanent candidates.
//
// Both kernels assign one matrix to one block and partition [0, 2^n) among
// its active wave lanes with the shared `wave_ryser_mapping.h` convention.
// Each lane retains a complete accumulator for its own interval, reconstructs
// it from g(interval.start), and exchanges only its scalar partial Ryser sum.
// The lane-zero reduction reads those partials in increasing lane order.
//
// Register/shared-memory hypotheses before compiler allocation:
// * f5_byte_control_kernel holds n accumulator bytes, u64 cursor/end bounds,
//   and a u32 partial: n + 20 bytes of stable lane state. Its shared read-only
//   byte column table is n^2 bytes per block.
// * f5_three_plane_kernel holds three u64 value planes, u64 cursor/end bounds,
//   and a u32 partial: 11 32-bit register units of stable lane state. Its
//   shared read-only column table is 3 u64 words per column, or 24n bytes.
//
// Loop temporaries and compiler-introduced scratch are deliberately excluded
// from those source-level models. The gfx1030 resource report captured with
// `-Rpass-analysis=kernel-resource-usage` is authoritative for registers,
// scratch, LDS, and spill counts. After the implementation is committed, a
// clean-revision capture must first go to /tmp:
//
// /opt/rocm/bin/hipcc --offload-arch=gfx1030 -O3 \
//     -Rpass-analysis=kernel-resource-usage hip/f5_wave_equivalence.hip \
//     -o /tmp/56c69654-f5-wave-resource \
//     2> /tmp/56c69654-f5-wave-resource.log
//
// Validate that revision with `f5-wave-device-evidence` before copying the log
// byte-for-byte beside this source in a separate receipt commit. That receipt
// records the clean revision, host, ROCm toolchain, direct-driver outcome, and
// the report's registers, private scratch, LDS, and spill counts. A dirty
// capture must never overwrite the canonical artifact. This executable is a
// correctness check, not a timing benchmark.

#include <hip/hip_runtime.h>

#include "wave_ryser_mapping.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <map>
#include <numeric>
#include <string>
#include <vector>

namespace {

constexpr unsigned kFieldOrder = 5;
constexpr int kMaxRows = 63;

struct Case {
  std::string id;
  int n;
  std::vector<unsigned char> matrix;
  unsigned expected = 0;
  bool uses_streamed_expected = false;
};

struct IntervalObservation {
  std::uint64_t start;
  std::uint64_t end;
  std::uint64_t subset;
};

struct DirectionObservation {
  std::uint64_t index;
  std::uint64_t subset;
  unsigned flipped_column;
  unsigned added;
};

struct ThreePlane {
  std::uint64_t b0;
  std::uint64_t b1;
  std::uint64_t b2;
};

struct Selectors {
  std::uint64_t e0;
  std::uint64_t e1;
  std::uint64_t e2;
  std::uint64_t e3;
  std::uint64_t e4;
};

__device__ __forceinline__ unsigned lane0(ThreePlane value) {
  return static_cast<unsigned>((value.b0 & 1) | ((value.b1 & 1) << 1) |
                               ((value.b2 & 1) << 2));
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

__host__ __device__ __forceinline__ unsigned fp5_add(unsigned left, unsigned right) {
  const auto sum = left + right;
  return sum >= kFieldOrder ? sum - kFieldOrder : sum;
}

__host__ __device__ __forceinline__ unsigned fp5_sub(unsigned left, unsigned right) {
  return left >= right ? left - right : left + kFieldOrder - right;
}

__host__ __device__ __forceinline__ unsigned fp5_mul(unsigned left, unsigned right) {
  return (left * right) % kFieldOrder;
}

__device__ __forceinline__ Selectors decode5(ThreePlane value) {
  const auto not_b0 = ~value.b0;
  const auto not_b1 = ~value.b1;
  const auto not_b2 = ~value.b2;
  const auto not_b2_not_b1 = not_b2 & not_b1;
  const auto not_b2_b1 = not_b2 & value.b1;
  const auto not_b1_not_b0 = not_b1 & not_b0;
  return {
      not_b2_not_b1 & not_b0,
      not_b2_not_b1 & value.b0,
      not_b2_b1 & not_b0,
      not_b2_b1 & value.b0,
      value.b2 & not_b1_not_b0,
  };
}

__device__ __forceinline__ ThreePlane encode5(std::uint64_t r1, std::uint64_t r2,
                                                std::uint64_t r3, std::uint64_t r4) {
  return {r1 | r3, r2 | r3, r4};
}

// This is the same canonical 5-way decode/cross-product circuit as Packed5.
__device__ __forceinline__ ThreePlane add5(ThreePlane left, ThreePlane right) {
  const auto a = decode5(left);
  const auto b = decode5(right);
  const auto r1 = (a.e0 & b.e1) | (a.e1 & b.e0) | (a.e2 & b.e4) |
                  (a.e3 & b.e3) | (a.e4 & b.e2);
  const auto r2 = (a.e0 & b.e2) | (a.e1 & b.e1) | (a.e2 & b.e0) |
                  (a.e3 & b.e4) | (a.e4 & b.e3);
  const auto r3 = (a.e0 & b.e3) | (a.e1 & b.e2) | (a.e2 & b.e1) |
                  (a.e3 & b.e0) | (a.e4 & b.e4);
  const auto r4 = (a.e0 & b.e4) | (a.e1 & b.e3) | (a.e2 & b.e2) |
                  (a.e3 & b.e1) | (a.e4 & b.e0);
  return encode5(r1, r2, r3, r4);
}

__device__ __forceinline__ ThreePlane sub5(ThreePlane left, ThreePlane right) {
  const auto a = decode5(left);
  const auto b = decode5(right);
  const auto r1 = (a.e0 & b.e4) | (a.e1 & b.e0) | (a.e2 & b.e1) |
                  (a.e3 & b.e2) | (a.e4 & b.e3);
  const auto r2 = (a.e0 & b.e3) | (a.e1 & b.e4) | (a.e2 & b.e0) |
                  (a.e3 & b.e1) | (a.e4 & b.e2);
  const auto r3 = (a.e0 & b.e2) | (a.e1 & b.e3) | (a.e2 & b.e4) |
                  (a.e3 & b.e0) | (a.e4 & b.e1);
  const auto r4 = (a.e0 & b.e1) | (a.e1 & b.e2) | (a.e2 & b.e3) |
                  (a.e3 & b.e4) | (a.e4 & b.e0);
  return encode5(r1, r2, r3, r4);
}

__device__ __forceinline__ unsigned byte_product(const std::uint8_t* sums, int n) {
  unsigned product = 1;
  for (int row = 0; row < n; ++row) {
    product = fp5_mul(product, sums[row]);
    if (product == 0) {
      break;
    }
  }
  return product;
}

// If Z = ~(b0 | b1 | b2) & active is nonzero, the product is zero. Otherwise
// 2 generates F_5*: 1, 2, 4, 3 have exponents 0, 1, 2, 3 respectively.
__device__ __forceinline__ unsigned c4_product(ThreePlane value, std::uint64_t active) {
  const auto b0 = value.b0 & active;
  const auto b1 = value.b1 & active;
  const auto b2 = value.b2 & active;
  const auto zero_mask = ~(b0 | b1 | b2) & active;
  if (zero_mask != 0) {
    return 0;
  }
  const auto exponent = __popcll(b1 & ~b0 & ~b2) +
                        2 * __popcll(b2 & ~b0 & ~b1) +
                        3 * __popcll(b0 & b1 & ~b2);
  constexpr unsigned kExponentToValue[4] = {1, 2, 4, 3};
  return kExponentToValue[exponent % 4];
}

__device__ __forceinline__ ThreePlane column_from_global_bytes(
    const std::uint8_t* matrix, int n, int column) {
  ThreePlane result{0, 0, 0};
  for (int row = 0; row < n; ++row) {
    const auto value = matrix[row * n + column];
    const auto mask = UINT64_C(1) << row;
    if ((value & 1) != 0) {
      result.b0 |= mask;
    }
    if ((value & 2) != 0) {
      result.b1 |= mask;
    }
    if ((value & 4) != 0) {
      result.b2 |= mask;
    }
  }
  return result;
}

// Both kernels use the exact same active-lane geometry. The byte path keeps
// the shipped F_5 kernel's elementwise update and product loops, but obtains
// the complete row-major column table from block-local shared memory.
__global__ void f5_byte_control_kernel(const std::uint8_t* __restrict__ matrices, int n,
                                        int batch_size, unsigned* __restrict__ results) {
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

  extern __shared__ std::uint8_t staged_matrix[];
  const auto* const matrix = matrices + static_cast<std::size_t>(matrix_index) * n * n;
  for (int offset = static_cast<int>(lane); offset < n * n;
       offset += static_cast<int>(blockDim.x)) {
    staged_matrix[offset] = matrix[offset];
  }
  __syncthreads();

  const auto total_indices = UINT64_C(1) << n;
  const auto lane_count = static_cast<std::uint64_t>(blockDim.x);
  const auto interval = gf2_wave_mapping::balanced_interval(total_indices, lane, lane_count);
  std::uint64_t cursor = interval.start;
  const std::uint64_t interval_end = interval.end;
  std::uint8_t sums[kMaxRows] = {};
  std::uint64_t subset = gf2_wave_mapping::gray_subset(cursor);
  for (int column = 0; column < n; ++column) {
    if (((subset >> column) & 1) != 0) {
      for (int row = 0; row < n; ++row) {
        sums[row] = static_cast<std::uint8_t>(fp5_add(sums[row], staged_matrix[row * n + column]));
      }
    }
  }

  unsigned partial = 0;
  unsigned term = byte_product(sums, n);
  partial = (__popcll(subset) & 1) != 0 ? fp5_sub(partial, term)
                                           : fp5_add(partial, term);
  for (++cursor; cursor < interval_end; ++cursor) {
    const auto transition = gf2_wave_mapping::gray_transition(cursor);
    subset = transition.subset;
    for (int row = 0; row < n; ++row) {
      const auto column_value = staged_matrix[row * n + transition.flipped_column];
      sums[row] = static_cast<std::uint8_t>(transition.added
                                                 ? fp5_add(sums[row], column_value)
                                                 : fp5_sub(sums[row], column_value));
    }
    term = byte_product(sums, n);
    partial = (__popcll(subset) & 1) != 0 ? fp5_sub(partial, term)
                                             : fp5_add(partial, term);
  }

  const auto reduced = gf2_wave_mapping::reduce_partials_in_lane_order<5>(partial, lane_count);
  if (lane == 0) {
    results[matrix_index] = gf2_wave_mapping::apply_outer_ryser_sign<5>(reduced, n);
  }
}

// This path stages a packed read-only column table. A transition makes exactly
// one constant-time add5/sub5 call on its three u64 value planes.
__global__ void f5_three_plane_kernel(const std::uint8_t* __restrict__ matrices, int n,
                                      int batch_size, unsigned* __restrict__ results) {
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

  extern __shared__ std::uint64_t staged_columns[];
  auto* const column_b0 = staged_columns;
  auto* const column_b1 = staged_columns + n;
  auto* const column_b2 = staged_columns + 2 * n;
  const auto* const matrix = matrices + static_cast<std::size_t>(matrix_index) * n * n;
  for (int column = static_cast<int>(lane); column < n;
       column += static_cast<int>(blockDim.x)) {
    const auto packed = column_from_global_bytes(matrix, n, column);
    column_b0[column] = packed.b0;
    column_b1[column] = packed.b1;
    column_b2[column] = packed.b2;
  }
  __syncthreads();

  const auto total_indices = UINT64_C(1) << n;
  const auto lane_count = static_cast<std::uint64_t>(blockDim.x);
  const auto interval = gf2_wave_mapping::balanced_interval(total_indices, lane, lane_count);
  std::uint64_t cursor = interval.start;
  const std::uint64_t interval_end = interval.end;
  ThreePlane sums{0, 0, 0};
  std::uint64_t subset = gf2_wave_mapping::gray_subset(cursor);
  for (int column = 0; column < n; ++column) {
    if (((subset >> column) & 1) != 0) {
      const ThreePlane packed_column = {column_b0[column], column_b1[column], column_b2[column]};
      sums = add5(sums, packed_column);
    }
  }

  const auto active = (UINT64_C(1) << n) - 1;
  unsigned partial = 0;
  unsigned term = c4_product(sums, active);
  partial = (__popcll(subset) & 1) != 0 ? fp5_sub(partial, term)
                                           : fp5_add(partial, term);
  for (++cursor; cursor < interval_end; ++cursor) {
    const auto transition = gf2_wave_mapping::gray_transition(cursor);
    subset = transition.subset;
    const ThreePlane column = {column_b0[transition.flipped_column],
                               column_b1[transition.flipped_column],
                               column_b2[transition.flipped_column]};
    sums = transition.added ? add5(sums, column) : sub5(sums, column);
    term = c4_product(sums, active);
    partial = (__popcll(subset) & 1) != 0 ? fp5_sub(partial, term)
                                             : fp5_add(partial, term);
  }

  const auto reduced = gf2_wave_mapping::reduce_partials_in_lane_order<5>(partial, lane_count);
  if (lane == 0) {
    results[matrix_index] = gf2_wave_mapping::apply_outer_ryser_sign<5>(reduced, n);
  }
}

__global__ void n63_mapping_probe(IntervalObservation* observations) {
  if (blockIdx.x != 0 || threadIdx.x >= gf2_wave_mapping::kWaveLanes) {
    return;
  }
  constexpr std::uint64_t kTotal = UINT64_C(1) << 63;
  const auto interval = gf2_wave_mapping::balanced_interval(
      kTotal, threadIdx.x, gf2_wave_mapping::kWaveLanes);
  observations[threadIdx.x] = {interval.start, interval.end,
                               gf2_wave_mapping::gray_subset(interval.start)};
}

__global__ void n4_direction_probe(DirectionObservation* observations) {
  if (blockIdx.x != 0 || threadIdx.x >= 2) {
    return;
  }
  const std::uint64_t index = threadIdx.x == 0 ? UINT64_C(8) : UINT64_C(15);
  const auto transition = gf2_wave_mapping::gray_transition(index);
  observations[threadIdx.x] = {index, transition.subset, transition.flipped_column,
                               static_cast<unsigned>(transition.added)};
}

__global__ void f5_active_mask_probe(unsigned* results) {
  if (blockIdx.x != 0 || threadIdx.x != 0) {
    return;
  }
  constexpr int kOrder = 63;
  const auto active = (UINT64_C(1) << kOrder) - 1;
  std::uint8_t bytes[kMaxRows];
  for (int lane = 0; lane < kMaxRows; ++lane) {
    bytes[lane] = 1;
  }
  results[0] = byte_product(bytes, kOrder);
  bytes[kMaxRows - 1] = 0;
  results[1] = byte_product(bytes, kOrder);
  results[2] = c4_product({active, 0, UINT64_C(1) << 63}, active);
  results[3] = c4_product({active & ~(UINT64_C(1) << 62), 0, 0}, active);
}

__global__ void f5_field_and_c4_probe(unsigned* failures) {
  if (blockIdx.x != 0 || threadIdx.x != 0) {
    return;
  }
  unsigned failure_count = 0;
  for (unsigned left = 0; left < kFieldOrder; ++left) {
    for (unsigned right = 0; right < kFieldOrder; ++right) {
      const auto lhs = three_lanes(left, 0, 0);
      const auto rhs = three_lanes(right, 0, 0);
      failure_count += lane0(add5(lhs, rhs)) != fp5_add(left, right);
      failure_count += lane0(sub5(lhs, rhs)) != fp5_sub(left, right);
    }
  }
  for (unsigned first = 0; first < kFieldOrder; ++first) {
    for (unsigned second = 0; second < kFieldOrder; ++second) {
      for (unsigned third = 0; third < kFieldOrder; ++third) {
        std::uint8_t direct_values[3] = {static_cast<std::uint8_t>(first),
                                         static_cast<std::uint8_t>(second),
                                         static_cast<std::uint8_t>(third)};
        const auto state = three_lanes(first, second, third);
        if (c4_product(state, 0b111) != byte_product(direct_values, 3)) {
          ++failure_count;
        }
      }
    }
  }
  failures[0] = failure_count;
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
      product = fp5_mul(product, test_case.matrix[row * test_case.n + permutation[row]]);
    }
    total = fp5_add(total, product);
  } while (std::next_permutation(permutation.begin(), permutation.end()));
  return total;
}

enum class Path { kByteControl, kThreePlane };

const char* path_name(Path path) {
  return path == Path::kByteControl ? "F_5 byte control" : "F_5 three-plane";
}

bool run_batch(Path path, const std::vector<Case>& cases) {
  const int n = cases.front().n;
  for (const auto& test_case : cases) {
    if (test_case.n != n) {
      std::fprintf(stderr, "%s batch mixed matrix orders\n", path_name(path));
      return false;
    }
  }
  std::vector<std::uint8_t> host_matrices;
  std::vector<unsigned> expected;
  for (const auto& test_case : cases) {
    host_matrices.insert(host_matrices.end(), test_case.matrix.begin(), test_case.matrix.end());
    expected.push_back(test_case.uses_streamed_expected ? test_case.expected
                                                         : cpu_permanent(test_case));
  }

  std::uint8_t* device_matrices = nullptr;
  unsigned* device_results = nullptr;
  std::vector<unsigned> actual(cases.size(), 0);
  const auto bytes = host_matrices.size() * sizeof(std::uint8_t);
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
    const auto lanes = gf2_wave_mapping::active_lanes_for_order(n);
    const std::size_t shared_bytes = path == Path::kByteControl
                                         ? static_cast<std::size_t>(n) * n
                                         : 3 * static_cast<std::size_t>(n) * sizeof(std::uint64_t);
    if (path == Path::kByteControl) {
      hipLaunchKernelGGL(f5_byte_control_kernel, dim3(cases.size()), dim3(lanes), shared_bytes,
                         0, device_matrices, n, static_cast<int>(cases.size()), device_results);
    } else {
      hipLaunchKernelGGL(f5_three_plane_kernel, dim3(cases.size()), dim3(lanes), shared_bytes,
                         0, device_matrices, n, static_cast<int>(cases.size()), device_results);
    }
  }
  const bool launched = copied && check(hipGetLastError(), "F_5 kernel launch") &&
                        check(hipDeviceSynchronize(), "F_5 kernel synchronize") &&
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
      std::fprintf(stderr, "%s %s: device=%u expected=%u\n", path_name(path),
                   cases[index].id.c_str(), actual[index], expected[index]);
      return false;
    }
  }
  return true;
}

bool run_both_paths(const std::vector<Case>& cases) {
  return run_batch(Path::kByteControl, cases) && run_batch(Path::kThreePlane, cases);
}

bool run_active_mask_probe() {
  unsigned* device_results = nullptr;
  std::array<unsigned, 4> host_results{};
  if (!check(hipMalloc(&device_results, sizeof(host_results)), "hipMalloc mask probe")) {
    return false;
  }
  hipLaunchKernelGGL(f5_active_mask_probe, dim3(1), dim3(1), 0, 0, device_results);
  const bool launched = check(hipGetLastError(), "F_5 mask probe launch") &&
                        check(hipDeviceSynchronize(), "F_5 mask probe synchronize") &&
                        check(hipMemcpy(host_results.data(), device_results, sizeof(host_results),
                                        hipMemcpyDeviceToHost),
                              "hipMemcpy F_5 mask probe");
  const bool freed = check(hipFree(device_results), "hipFree F_5 mask probe");
  if (!launched || !freed) {
    return false;
  }
  if (host_results != std::array<unsigned, 4>{1, 0, 1, 0}) {
    std::fprintf(stderr, "F_5 n=63 active-mask probe got {%u, %u, %u, %u}\n",
                 host_results[0], host_results[1], host_results[2], host_results[3]);
    return false;
  }
  return true;
}

bool run_field_and_c4_probe() {
  unsigned* device_failures = nullptr;
  unsigned host_failures = 0;
  if (!check(hipMalloc(&device_failures, sizeof(host_failures)), "hipMalloc F_5 field probe")) {
    return false;
  }
  hipLaunchKernelGGL(f5_field_and_c4_probe, dim3(1), dim3(1), 0, 0, device_failures);
  const bool launched = check(hipGetLastError(), "F_5 field probe launch") &&
                        check(hipDeviceSynchronize(), "F_5 field probe synchronize") &&
                        check(hipMemcpy(&host_failures, device_failures, sizeof(host_failures),
                                        hipMemcpyDeviceToHost),
                              "hipMemcpy F_5 field probe");
  const bool freed = check(hipFree(device_failures), "hipFree F_5 field probe");
  if (!launched || !freed) {
    return false;
  }
  if (host_failures != 0) {
    std::fprintf(stderr, "F_5 add/sub or C4 product probe found %u failures\n", host_failures);
    return false;
  }
  return true;
}

bool run_mapping_probes() {
  IntervalObservation* device_intervals = nullptr;
  DirectionObservation* device_directions = nullptr;
  std::array<IntervalObservation, gf2_wave_mapping::kWaveLanes> intervals{};
  std::array<DirectionObservation, 2> directions{};
  if (!check(hipMalloc(&device_intervals, sizeof(intervals)), "hipMalloc n=63 mapping") ||
      !check(hipMalloc(&device_directions, sizeof(directions)), "hipMalloc n=4 directions")) {
    (void)hipFree(device_intervals);
    (void)hipFree(device_directions);
    return false;
  }
  hipLaunchKernelGGL(n63_mapping_probe, dim3(1), dim3(gf2_wave_mapping::kWaveLanes), 0, 0,
                     device_intervals);
  const bool n63_launched = check(hipGetLastError(), "n=63 mapping probe launch") &&
                            check(hipDeviceSynchronize(), "n=63 mapping probe synchronize") &&
                            check(hipMemcpy(intervals.data(), device_intervals, sizeof(intervals),
                                            hipMemcpyDeviceToHost),
                                  "hipMemcpy n=63 mapping");
  hipLaunchKernelGGL(n4_direction_probe, dim3(1), dim3(2), 0, 0, device_directions);
  const bool n4_launched = check(hipGetLastError(), "n=4 direction probe launch") &&
                           check(hipDeviceSynchronize(), "n=4 direction probe synchronize") &&
                           check(hipMemcpy(directions.data(), device_directions, sizeof(directions),
                                           hipMemcpyDeviceToHost),
                                 "hipMemcpy n=4 directions");
  const bool freed_intervals = check(hipFree(device_intervals), "hipFree n=63 mapping");
  const bool freed_directions = check(hipFree(device_directions), "hipFree n=4 directions");
  if (!n63_launched || !n4_launched || !freed_intervals || !freed_directions) {
    return false;
  }

  constexpr std::uint64_t kTotal = UINT64_C(1) << 63;
  constexpr std::uint64_t kLaneWidth = UINT64_C(1) << 58;
  std::uint64_t next = 0;
  for (unsigned lane = 0; lane < gf2_wave_mapping::kWaveLanes; ++lane) {
    const auto& observed = intervals[lane];
    const std::uint64_t expected_start = lane * kLaneWidth;
    const std::uint64_t expected_end = (lane + 1) * kLaneWidth;
    if (observed.start != next || observed.start != expected_start ||
        observed.end != expected_end ||
        observed.subset != (observed.start ^ (observed.start >> 1))) {
      std::fprintf(stderr, "F_5 n=63 mapping mismatch at lane %u\n", lane);
      return false;
    }
    next = observed.end;
  }
  if (next != kTotal) {
    std::fprintf(stderr, "F_5 n=63 intervals do not cover 2^63\n");
    return false;
  }
  const std::array<DirectionObservation, 2> expected = {{{8, 12, 3, 1}, {15, 8, 0, 0}}};
  for (std::size_t index = 0; index < directions.size(); ++index) {
    const auto& observed = directions[index];
    if (observed.index != expected[index].index || observed.subset != expected[index].subset ||
        observed.flipped_column != expected[index].flipped_column ||
        observed.added != expected[index].added) {
      std::fprintf(stderr, "F_5 n=4 device direction probe mismatch\n");
      return false;
    }
  }
  return true;
}

bool read_exact(void* destination, std::size_t bytes) {
  std::cin.read(static_cast<char*>(destination), static_cast<std::streamsize>(bytes));
  return static_cast<bool>(std::cin);
}

bool read_u32_le(std::uint32_t* value) {
  std::array<unsigned char, 4> bytes{};
  if (!read_exact(bytes.data(), bytes.size())) {
    return false;
  }
  *value = static_cast<std::uint32_t>(bytes[0]) |
           (static_cast<std::uint32_t>(bytes[1]) << 8) |
           (static_cast<std::uint32_t>(bytes[2]) << 16) |
           (static_cast<std::uint32_t>(bytes[3]) << 24);
  return true;
}

bool read_u64_le(std::uint64_t* value) {
  std::array<unsigned char, 8> bytes{};
  if (!read_exact(bytes.data(), bytes.size())) {
    return false;
  }
  *value = 0;
  for (unsigned shift = 0; shift < 8; ++shift) {
    *value |= static_cast<std::uint64_t>(bytes[shift]) << (8 * shift);
  }
  return true;
}

bool run_streamed_fixture_evidence() {
  constexpr std::array<char, 8> kMagic = {'G', 'F', '2', 'W', 'A', 'V', 'E', '1'};
  std::array<char, kMagic.size()> magic{};
  std::uint32_t count = 0;
  if (!read_exact(magic.data(), magic.size()) || magic != kMagic || !read_u32_le(&count) ||
      count == 0) {
    std::fprintf(stderr, "invalid F_5 wave fixture evidence stream header\n");
    return false;
  }
  std::map<int, std::vector<Case>> batches;
  for (std::uint32_t record = 0; record < count; ++record) {
    std::uint32_t id_size = 0;
    std::uint32_t n = 0;
    std::uint64_t expected = 0;
    std::uint64_t matrix_size = 0;
    if (!read_u32_le(&id_size) || id_size == 0 || id_size > 1024) {
      std::fprintf(stderr, "F_5 fixture %u has invalid identifier length\n", record);
      return false;
    }
    std::string id(id_size, '\0');
    if (!read_exact(id.data(), id.size()) || !read_u32_le(&n) || !read_u64_le(&expected) ||
        !read_u64_le(&matrix_size) || n > kMaxRows || expected >= kFieldOrder ||
        matrix_size != static_cast<std::uint64_t>(n) * n) {
      std::fprintf(stderr, "F_5 fixture %u has invalid metadata\n", record);
      return false;
    }
    Case test_case{id, static_cast<int>(n), std::vector<unsigned char>(matrix_size),
                   static_cast<unsigned>(expected), true};
    if ((!test_case.matrix.empty() &&
         !read_exact(test_case.matrix.data(), test_case.matrix.size())) ||
        !std::all_of(test_case.matrix.begin(), test_case.matrix.end(),
                     [](unsigned char value) { return value < kFieldOrder; })) {
      std::fprintf(stderr, "%s has invalid canonical F_5 row-major bytes\n", id.c_str());
      return false;
    }
    batches[static_cast<int>(n)].push_back(std::move(test_case));
  }
  for (const auto& entry : batches) {
    if (!run_both_paths(entry.second)) {
      return false;
    }
  }
  if (!run_mapping_probes() || !run_active_mask_probe() || !run_field_and_c4_probe()) {
    return false;
  }
  std::fprintf(stdout, "F_5 wave device evidence matched %u streamed fixtures\n", count);
  return true;
}

Case patterned_case(const char* name, int n, unsigned offset) {
  Case test_case{name, n, std::vector<unsigned char>(n * n)};
  for (int row = 0; row < n; ++row) {
    for (int column = 0; column < n; ++column) {
      test_case.matrix[row * n + column] =
          static_cast<unsigned char>((row * 3 + column + offset) % kFieldOrder);
    }
  }
  return test_case;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc == 2 && std::strcmp(argv[1], "--fixture-stdin") == 0) {
    return run_streamed_fixture_evidence() ? 0 : 1;
  }
  if (argc != 1) {
    std::fprintf(stderr, "usage: %s [--fixture-stdin]\n", argv[0]);
    return 2;
  }
  const std::vector<Case> empty_cases = {{"n0-empty", 0, {}}};
  const std::vector<Case> n1_exponents = {
      {"n1-exp0", 1, {1}}, {"n1-exp1", 1, {2}},
      {"n1-exp2", 1, {4}}, {"n1-exp3", 1, {3}},
  };
  const std::vector<Case> n4_cases = {
      patterned_case("n4-gray-boundaries-a", 4, 1),
      patterned_case("n4-gray-boundaries-b", 4, 2),
  };
  const std::vector<Case> n6_cases = {patterned_case("n6-two-index-intervals", 6, 1)};
  const std::vector<Case> zero_cases = {
      {"n3-zero-product", 3, {0, 0, 0, 0, 0, 0, 0, 1, 2}},
  };
  if (!run_both_paths(empty_cases) || !run_both_paths(n1_exponents) ||
      !run_both_paths(n4_cases) || !run_both_paths(n6_cases) ||
      !run_both_paths(zero_cases) || !run_mapping_probes() ||
      !run_active_mask_probe() || !run_field_and_c4_probe()) {
    return 1;
  }
  return 0;
}
