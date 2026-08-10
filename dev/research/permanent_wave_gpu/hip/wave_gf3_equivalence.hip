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
// each lane owns two u64 packed words plus u64 Gray cursor/end bounds and one
// u32 partial sum: a 9 x 32-bit-register lower-bound model. The subset and
// product value are loop temporaries, not part of that stable state model; the
// compiler's actual resource report is authoritative. The only dynamic shared
// allocation is the read-only column table: two u64 words per column = 16n
// bytes (384 B at n=24, 512 B at n=32). The compiler's gfx1030 report is
// committed in `wave_gf3_resource_usage.log` beside this source.
//
// The device executable is intentionally a self-checking conformance binary,
// not a benchmark. It checks independent permutation references for small
// structural matrices, both Gray transition directions via an explicit device
// probe, multi-index lane intervals at n=6, and the n=63 mapping/mask probes.

#include <hip/hip_runtime.h>

#include "wave_ryser_mapping.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <iostream>
#include <map>
#include <numeric>
#include <string>
#include <vector>

namespace {

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

  // The stable state is the packed (magnitude, sign) u64 pair, u64 cursor/end,
  // and u32 partial: nine 32-bit register units before compiler allocation.
  // `subset` and `term` below are loop temporaries; the resource report, not
  // this lower bound, determines actual allocation.
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
    const auto transition = gf2_wave_mapping::gray_transition(cursor);
    subset = transition.subset;
    if (transition.added) {
      add3(magnitude, sign, column_magnitude[transition.flipped_column],
           column_sign[transition.flipped_column]);
    } else {
      sub3(magnitude, sign, column_magnitude[transition.flipped_column],
           column_sign[transition.flipped_column]);
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

// Return every lane's full n=63 interval observation. The host validates the
// returned sequence exactly rather than inferring coverage from the mapping
// formula on its own.
__global__ void n63_mapping_probe(IntervalObservation* observations) {
  if (blockIdx.x != 0 || threadIdx.x >= gf2_wave_mapping::kWaveLanes) {
    return;
  }
  constexpr int kOrder = 63;
  constexpr std::uint64_t kTotal = UINT64_C(1) << kOrder;
  const auto interval = gf2_wave_mapping::balanced_interval(
      kTotal, threadIdx.x, gf2_wave_mapping::kWaveLanes);
  observations[threadIdx.x] = {interval.start, interval.end,
                               gf2_wave_mapping::gray_subset(interval.start)};
}

// Observe both canonical Gray directions at the tractable n=4 fixture
// boundary indices: index 8 enters column 3 and index 15 removes column 0.
__global__ void n4_direction_probe(DirectionObservation* observations) {
  if (blockIdx.x != 0 || threadIdx.x >= 2) {
    return;
  }
  const std::uint64_t index = threadIdx.x == 0 ? UINT64_C(8) : UINT64_C(15);
  const auto transition = gf2_wave_mapping::gray_transition(index);
  observations[threadIdx.x] = {index, transition.subset,
                               transition.flipped_column,
                               static_cast<unsigned>(transition.added)};
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
    expected.push_back(test_case.uses_streamed_expected ? test_case.expected
                                                         : cpu_permanent(test_case));
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
      std::fprintf(stderr, "%s: device=%u expected=%u\n", cases[index].id.c_str(),
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
    const std::uint64_t expected_start = lane * kLaneWidth;
    const std::uint64_t expected_end = (lane + 1) * kLaneWidth;
    const auto& observed = intervals[lane];
    if (observed.start != next || observed.start != expected_start ||
        observed.end != expected_end ||
        observed.subset != (observed.start ^ (observed.start >> 1))) {
      std::fprintf(stderr,
                   "n=63 lane %u: got [%llu, %llu) subset=%#llx; expected [%llu, %llu)\n",
                   lane, static_cast<unsigned long long>(observed.start),
                   static_cast<unsigned long long>(observed.end),
                   static_cast<unsigned long long>(observed.subset),
                   static_cast<unsigned long long>(expected_start),
                   static_cast<unsigned long long>(expected_end));
      return false;
    }
    next = observed.end;
  }
  if (next != kTotal) {
    std::fprintf(stderr, "n=63 intervals end at %llu instead of 2^63\n",
                 static_cast<unsigned long long>(next));
    return false;
  }

  const std::array<DirectionObservation, 2> expected_directions = {{
      {8, 12, 3, 1},
      {15, 8, 0, 0},
  }};
  for (std::size_t index = 0; index < directions.size(); ++index) {
    const auto& observed = directions[index];
    const auto& expected = expected_directions[index];
    if (observed.index != expected.index || observed.subset != expected.subset ||
        observed.flipped_column != expected.flipped_column || observed.added != expected.added) {
      std::fprintf(stderr, "n=4 device direction probe did not retain add/sub boundaries\n");
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
    std::fprintf(stderr, "invalid F_3 wave fixture evidence stream header\n");
    return false;
  }

  std::map<int, std::vector<Case>> batches;
  for (std::uint32_t record = 0; record < count; ++record) {
    std::uint32_t id_size = 0;
    std::uint32_t n = 0;
    std::uint64_t expected = 0;
    std::uint64_t matrix_size = 0;
    if (!read_u32_le(&id_size) || id_size == 0 || id_size > 1024) {
      std::fprintf(stderr, "fixture %u has invalid identifier length\n", record);
      return false;
    }
    std::string id(id_size, '\0');
    if (!read_exact(id.data(), id.size()) || !read_u32_le(&n) || !read_u64_le(&expected) ||
        !read_u64_le(&matrix_size) || n > 63 || expected >= 3 ||
        matrix_size != static_cast<std::uint64_t>(n) * n) {
      std::fprintf(stderr, "fixture %u has invalid F_3 evidence metadata\n", record);
      return false;
    }
    Case test_case{id, static_cast<int>(n), std::vector<unsigned char>(matrix_size),
                   static_cast<unsigned>(expected), true};
    if ((!test_case.matrix.empty() &&
         !read_exact(test_case.matrix.data(), test_case.matrix.size())) ||
        !std::all_of(test_case.matrix.begin(), test_case.matrix.end(),
                     [](unsigned char value) { return value < 3; })) {
      std::fprintf(stderr, "%s has invalid canonical F_3 row-major bytes\n", id.c_str());
      return false;
    }
    batches[static_cast<int>(n)].push_back(std::move(test_case));
  }
  for (const auto& batch : batches) {
    const auto& cases = batch.second;
    if (!run_batch(cases)) {
      return false;
    }
  }
  if (!run_mapping_probes() || !run_active_mask_probe()) {
    return false;
  }
  std::fprintf(stdout, "F_3 wave device evidence matched %u streamed fixtures\n", count);
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

int main(int argc, char** argv) {
  if (argc == 2 && std::strcmp(argv[1], "--fixture-stdin") == 0) {
    return run_streamed_fixture_evidence() ? 0 : 1;
  }
  if (argc != 1) {
    std::fprintf(stderr, "usage: %s [--fixture-stdin]\n", argv[0]);
    return 2;
  }
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
      !run_batch(zero_cases) || !run_mapping_probes() || !run_active_mask_probe()) {
    return 1;
  }
  return 0;
}
