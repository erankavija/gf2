// Device equivalence for the paired F_7 permanent-shaped wave kernels.
//
// Both kernels use the one canonical mapping in wave_ryser_mapping.h: one
// block owns each matrix, active lanes own contiguous Gray-index intervals,
// reconstruct their row sums from g(interval.start), and reduce only scalar
// Ryser partials in increasing lane order. No packed state crosses lanes.
//
// Source-level state and shared-memory budgets, before compiler allocation:
//
// * The three-plane path keeps b0, b1, and b2 as three u64 registers for its
//   entire interval, plus u64 cursor/end and u32 partial: 11 32-bit units.
//   Its shared read-only column table is three u64 words per column, 24n
//   bytes/block (576 B at n=24).
// * The lookup-table control keeps ceil(n/16) packed u64 words, plus the same
//   cursor/end/partial. Its lower bound is 7 32-bit units at n=16 and 9 at
//   n=20,24. Its shared read-only column table is
//   8n ceil(n/16) bytes/block (384 B at n=24).
//
// Loop temporaries, spills, and compiler allocation are deliberately not
// inferred from these lower bounds. After a real gfx1030 build, commit that
// compiler's exact resource output beside this source as
// wave_gf7_resource_usage.log; it must name registers/thread, private
// scratch/thread, shared memory/block, and SGPR/VGPR spill counts.
//
// The control retains the current canonical two-nibble F_7 table layout:
// MUL is a 64 KiB __constant__ table, ADD and SUB are 64 KiB __device__
// global tables. Fixture evidence receives all three public canonical Rust
// Packed7 table byte arrays over stdin and uploads those exact bytes before a
// control launch. The three-plane path has no multiplication tables. Its byte-to-plane transpose is the
// separately addressable prepare_three_plane_columns kernel. The executable's
// `--stage three-plane-preparation --fixture-stdin` route invokes and verifies
// only that symbol; a receipt worker can event/profile this launch without
// including the Gray-walk kernel.

#include <hip/hip_runtime.h>

#include "wave_ryser_mapping.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>

namespace {

constexpr unsigned kFieldOrder = 7;
constexpr unsigned kLutBytes = 65536;
constexpr unsigned kMaxEvidenceOrder = 24;
constexpr unsigned kMaxControlWords = 2;

enum class KernelPath { kLookupTableControl, kThreePlane };

struct Fixture {
  std::string id;
  int n;
  std::uint64_t expected;
  std::vector<unsigned char> matrix;
};

// The control intentionally retains the production packed7 placement: MUL is
// constant-cached, while ADD/SUB are read through global memory. Each lookup
// key is a | (b << 8) where each byte holds two canonical F_7 nibbles.
__constant__ unsigned char d_mul_lut[kLutBytes];
__device__ unsigned char d_add_lut[kLutBytes];
__device__ unsigned char d_sub_lut[kLutBytes];

__device__ __forceinline__ unsigned sub_scalar(unsigned left, unsigned right) {
  return left >= right ? left - right : left + kFieldOrder - right;
}

struct ThreePlane {
  std::uint64_t b0;
  std::uint64_t b1;
  std::uint64_t b2;
};

// Canonical Mersenne three-plane add/subtract from f7_three_plane.rs.
__device__ __forceinline__ ThreePlane add_three_plane(ThreePlane left,
                                                       ThreePlane right) {
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

__device__ __forceinline__ ThreePlane sub_three_plane(ThreePlane left,
                                                       ThreePlane right) {
  const auto nonzero = right.b0 | right.b1 | right.b2;
  return add_three_plane(left, {(~right.b0) & nonzero, (~right.b1) & nonzero,
                                (~right.b2) & nonzero});
}

__device__ __forceinline__ unsigned three_plane_product(ThreePlane value,
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
  constexpr unsigned kExponentToValue[6] = {1, 3, 2, 6, 4, 5};
  return kExponentToValue[exponent % 6];
}

// This transpose is intentionally a separate launch. The permanent kernel
// below receives only prepared three-plane columns, not source matrix bytes.
__global__ void prepare_three_plane_columns(const unsigned char* __restrict__ matrices,
                                            int n, int batch_size,
                                            std::uint64_t* __restrict__ prepared_columns) {
  const unsigned matrix_index = blockIdx.x;
  if (matrix_index >= static_cast<unsigned>(batch_size)) {
    return;
  }
  const auto* const matrix =
      matrices + static_cast<std::size_t>(matrix_index) * n * n;
  auto* const prepared = prepared_columns + static_cast<std::size_t>(matrix_index) * 3 * n;
  for (int column = static_cast<int>(threadIdx.x); column < n;
       column += static_cast<int>(blockDim.x)) {
    ThreePlane encoded{0, 0, 0};
    for (int row = 0; row < n; ++row) {
      const auto value = matrix[row * n + column];
      const auto mask = UINT64_C(1) << row;
      if ((value & 1U) != 0) {
        encoded.b0 |= mask;
      }
      if ((value & 2U) != 0) {
        encoded.b1 |= mask;
      }
      if ((value & 4U) != 0) {
        encoded.b2 |= mask;
      }
    }
    prepared[column] = encoded.b0;
    prepared[n + column] = encoded.b1;
    prepared[2 * n + column] = encoded.b2;
  }
}

__global__ void wave_gf7_three_plane_kernel(
    const std::uint64_t* __restrict__ prepared_columns, int n, int batch_size,
    unsigned* __restrict__ results) {
  const unsigned lane = threadIdx.x;
  const unsigned matrix_index = blockIdx.x;
  if (matrix_index >= static_cast<unsigned>(batch_size)) {
    return;
  }

  // Exactly 3n u64 words are staged once per block, then read-only.
  extern __shared__ std::uint64_t staged_columns[];
  const auto* const source = prepared_columns + static_cast<std::size_t>(matrix_index) * 3 * n;
  for (int word = static_cast<int>(lane); word < 3 * n;
       word += static_cast<int>(blockDim.x)) {
    staged_columns[word] = source[word];
  }
  __syncthreads();

  const auto total_indices = UINT64_C(1) << n;
  const auto interval = gf2_wave_mapping::balanced_interval(total_indices, lane, blockDim.x);
  const auto start_subset = gf2_wave_mapping::gray_subset(interval.start);
  const auto active = (UINT64_C(1) << n) - 1;

  // Persistent lane state: three u64 planes, cursor/end, and u32 partial.
  ThreePlane sums{0, 0, 0};
  for (int column = 0; column < n; ++column) {
    if (((start_subset >> column) & 1) != 0) {
      sums = add_three_plane(
          sums, {staged_columns[column], staged_columns[n + column],
                 staged_columns[2 * n + column]});
    }
  }

  std::uint64_t cursor = interval.start;
  const std::uint64_t interval_end = interval.end;
  unsigned partial = three_plane_product(sums, active);
  if ((__popcll(start_subset) & 1) != 0) {
    partial = sub_scalar(0, partial);
  }
  for (++cursor; cursor < interval_end; ++cursor) {
    const auto transition = gf2_wave_mapping::gray_transition(cursor);
    const ThreePlane column = {staged_columns[transition.flipped_column],
                               staged_columns[n + transition.flipped_column],
                               staged_columns[2 * n + transition.flipped_column]};
    sums = transition.added ? add_three_plane(sums, column) : sub_three_plane(sums, column);
    unsigned term = three_plane_product(sums, active);
    if ((__popcll(transition.subset) & 1) != 0) {
      term = sub_scalar(0, term);
    }
    partial = gf2_wave_mapping::add_scalar<kFieldOrder>(partial, term);
  }

  // The only cross-lane exchange is this lane-order scalar partial reduction.
  const unsigned reduced =
      gf2_wave_mapping::reduce_partials_in_lane_order<kFieldOrder>(partial, blockDim.x);
  if (lane == 0) {
    results[matrix_index] =
        gf2_wave_mapping::apply_outer_ryser_sign<kFieldOrder>(reduced, n);
  }
}

__device__ __forceinline__ std::uint64_t packed_binary_word(
    std::uint64_t left, std::uint64_t right, const unsigned char* table) {
  std::uint64_t result = 0;
  for (unsigned byte = 0; byte < 8; ++byte) {
    const auto shift = 8 * byte;
    const auto a = static_cast<unsigned>((left >> shift) & 0xffU);
    const auto b = static_cast<unsigned>((right >> shift) & 0xffU);
    result |= static_cast<std::uint64_t>(table[a | (b << 8)]) << shift;
  }
  return result;
}

template <unsigned Words>
struct PackedControl {
  std::uint64_t words[Words];
};

template <unsigned Words>
__device__ __forceinline__ void update_control(PackedControl<Words>* sums,
                                               const std::uint64_t* column,
                                               bool added) {
  const auto* const table = added ? d_add_lut : d_sub_lut;
  for (unsigned word = 0; word < Words; ++word) {
    sums->words[word] = packed_binary_word(sums->words[word], column[word], table);
  }
}

template <unsigned Words>
__device__ __forceinline__ unsigned control_product(PackedControl<Words> sums, int n) {
  unsigned product = 1;
  for (int row = 0; row < n; ++row) {
    const auto word = sums.words[row / 16];
    const auto value = static_cast<unsigned>((word >> (4 * (row % 16))) & 0xfU);
    product = d_mul_lut[product | (value << 8)];
    if (product == 0) {
      break;
    }
  }
  return product;
}

template <unsigned Words>
__global__ void wave_gf7_lookup_table_kernel(const unsigned char* __restrict__ matrices, int n,
                                              int batch_size,
                                              unsigned* __restrict__ results) {
  const unsigned lane = threadIdx.x;
  const unsigned matrix_index = blockIdx.x;
  if (matrix_index >= static_cast<unsigned>(batch_size)) {
    return;
  }

  // Exactly n*Words u64 words are staged once per block, then read-only.
  extern __shared__ std::uint64_t staged_columns[];
  const auto* const matrix =
      matrices + static_cast<std::size_t>(matrix_index) * n * n;
  for (int column = static_cast<int>(lane); column < n;
       column += static_cast<int>(blockDim.x)) {
    for (unsigned word = 0; word < Words; ++word) {
      std::uint64_t packed = 0;
      const int first_row = static_cast<int>(word) * 16;
      const int last_row = first_row + 16 < n ? first_row + 16 : n;
      for (int row = first_row; row < last_row; ++row) {
        packed |= static_cast<std::uint64_t>(matrix[row * n + column])
                  << (4 * (row - first_row));
      }
      staged_columns[column * Words + word] = packed;
    }
  }
  __syncthreads();

  const auto total_indices = UINT64_C(1) << n;
  const auto interval = gf2_wave_mapping::balanced_interval(total_indices, lane, blockDim.x);
  const auto start_subset = gf2_wave_mapping::gray_subset(interval.start);

  // Persistent state is Words u64 accumulator words, cursor/end, and partial.
  PackedControl<Words> sums{};
  for (int column = 0; column < n; ++column) {
    if (((start_subset >> column) & 1) != 0) {
      update_control(&sums, staged_columns + column * Words, true);
    }
  }

  std::uint64_t cursor = interval.start;
  const std::uint64_t interval_end = interval.end;
  unsigned partial = control_product(sums, n);
  if ((__popcll(start_subset) & 1) != 0) {
    partial = sub_scalar(0, partial);
  }
  for (++cursor; cursor < interval_end; ++cursor) {
    const auto transition = gf2_wave_mapping::gray_transition(cursor);
    update_control(&sums, staged_columns + transition.flipped_column * Words,
                   transition.added);
    unsigned term = control_product(sums, n);
    if ((__popcll(transition.subset) & 1) != 0) {
      term = sub_scalar(0, term);
    }
    partial = gf2_wave_mapping::add_scalar<kFieldOrder>(partial, term);
  }

  const unsigned reduced =
      gf2_wave_mapping::reduce_partials_in_lane_order<kFieldOrder>(partial, blockDim.x);
  if (lane == 0) {
    results[matrix_index] =
        gf2_wave_mapping::apply_outer_ryser_sign<kFieldOrder>(reduced, n);
  }
}

bool check(hipError_t status, const char* operation) {
  if (status == hipSuccess) {
    return true;
  }
  std::fprintf(stderr, "%s: %s\n", operation, hipGetErrorString(status));
  return false;
}

unsigned lut_component(unsigned left, unsigned right, char operation) {
  if (operation == '+') {
    return (left + right) % kFieldOrder;
  }
  if (operation == '-') {
    return (left + kFieldOrder - right) % kFieldOrder;
  }
  return (left * right) % kFieldOrder;
}

std::array<unsigned char, kLutBytes> canonical_lut(char operation) {
  std::array<unsigned char, kLutBytes> table{};
  for (unsigned a = 0; a < 256; ++a) {
    for (unsigned b = 0; b < 256; ++b) {
      const auto a_low = a & 0xfU;
      const auto a_high = (a >> 4) & 0xfU;
      const auto b_low = b & 0xfU;
      const auto b_high = (b >> 4) & 0xfU;
      if (a_low < kFieldOrder && a_high < kFieldOrder && b_low < kFieldOrder &&
          b_high < kFieldOrder) {
        const auto low = lut_component(a_low, b_low, operation);
        const auto high = lut_component(a_high, b_high, operation);
        table[a | (b << 8)] = static_cast<unsigned char>(low | (high << 4));
      }
    }
  }
  return table;
}

// The no-stdin structural smoke route cannot receive Rust-owned bytes. Its
// formula-generated tables are explicitly non-authoritative; fixture evidence
// always uses initialise_lookup_tables_from_stream below instead.
bool initialise_smoke_lookup_tables() {
  static const auto add = canonical_lut('+');
  static const auto sub = canonical_lut('-');
  static const auto mul = canonical_lut('*');
  return check(hipMemcpyToSymbol(HIP_SYMBOL(d_mul_lut), mul.data(), mul.size(), 0,
                                 hipMemcpyHostToDevice),
               "hipMemcpyToSymbol MUL_LUT") &&
         check(hipMemcpyToSymbol(HIP_SYMBOL(d_add_lut), add.data(), add.size(), 0,
                                 hipMemcpyHostToDevice),
               "hipMemcpyToSymbol ADD_LUT") &&
         check(hipMemcpyToSymbol(HIP_SYMBOL(d_sub_lut), sub.data(), sub.size(), 0,
                                 hipMemcpyHostToDevice),
               "hipMemcpyToSymbol SUB_LUT");
}

bool launch_three_plane_preparation(const unsigned char* device_matrices, int n,
                                    int batch_size, std::uint64_t* device_columns) {
  const auto active_lanes = gf2_wave_mapping::active_lanes_for_order(n);
  hipLaunchKernelGGL(prepare_three_plane_columns, dim3(batch_size), dim3(active_lanes), 0, 0,
                     device_matrices, n, batch_size, device_columns);
  return check(hipGetLastError(), "three-plane preparation launch");
}

bool verify_prepared_columns(const Fixture& fixture,
                             const std::vector<std::uint64_t>& prepared) {
  for (int column = 0; column < fixture.n; ++column) {
    ThreePlane expected{0, 0, 0};
    for (int row = 0; row < fixture.n; ++row) {
      const auto value = fixture.matrix[row * fixture.n + column];
      const auto mask = UINT64_C(1) << row;
      if ((value & 1U) != 0) {
        expected.b0 |= mask;
      }
      if ((value & 2U) != 0) {
        expected.b1 |= mask;
      }
      if ((value & 4U) != 0) {
        expected.b2 |= mask;
      }
    }
    if (prepared[column] != expected.b0 || prepared[fixture.n + column] != expected.b1 ||
        prepared[2 * fixture.n + column] != expected.b2) {
      std::fprintf(stderr, "%s: three-plane preparation mismatch at column %d\n",
                   fixture.id.c_str(), column);
      return false;
    }
  }
  return true;
}

bool run_fixture(const Fixture& fixture, KernelPath path, bool preparation_only) {
  if (fixture.n < 1 || fixture.n > static_cast<int>(kMaxEvidenceOrder) ||
      fixture.matrix.size() != static_cast<std::size_t>(fixture.n) * fixture.n ||
      fixture.expected >= kFieldOrder ||
      !std::all_of(fixture.matrix.begin(), fixture.matrix.end(),
                   [](unsigned char value) { return value < kFieldOrder; })) {
    std::fprintf(stderr, "%s has invalid F_7 fixture data\n", fixture.id.c_str());
    return false;
  }

  unsigned char* device_matrices = nullptr;
  std::uint64_t* device_columns = nullptr;
  unsigned* device_result = nullptr;
  unsigned actual = 0;
  std::vector<std::uint64_t> prepared(3 * fixture.n, 0);
  const auto matrix_bytes = fixture.matrix.size() * sizeof(unsigned char);
  bool ok = check(hipMalloc(&device_matrices, matrix_bytes), "hipMalloc matrices") &&
            check(hipMemcpy(device_matrices, fixture.matrix.data(), matrix_bytes,
                            hipMemcpyHostToDevice),
                  "hipMemcpy matrices");
  if (ok && !preparation_only) {
    ok = check(hipMalloc(&device_result, sizeof(actual)), "hipMalloc result");
  }
  const auto active_lanes = gf2_wave_mapping::active_lanes_for_order(fixture.n);
  if (ok && path == KernelPath::kThreePlane) {
    ok = check(hipMalloc(&device_columns,
                         static_cast<std::size_t>(3 * fixture.n) * sizeof(std::uint64_t)),
               "hipMalloc three-plane columns") &&
         launch_three_plane_preparation(device_matrices, fixture.n, 1, device_columns);
    if (ok && !preparation_only) {
      hipLaunchKernelGGL(wave_gf7_three_plane_kernel, dim3(1), dim3(active_lanes),
                         3 * fixture.n * sizeof(std::uint64_t), 0, device_columns, fixture.n,
                         1, device_result);
      ok = check(hipGetLastError(), "three-plane Gray-walk launch");
    }
  } else if (ok && !preparation_only) {
    if (fixture.n <= 16) {
      hipLaunchKernelGGL((wave_gf7_lookup_table_kernel<1>), dim3(1), dim3(active_lanes),
                         fixture.n * sizeof(std::uint64_t), 0, device_matrices, fixture.n, 1,
                         device_result);
    } else {
      hipLaunchKernelGGL((wave_gf7_lookup_table_kernel<kMaxControlWords>), dim3(1),
                         dim3(active_lanes),
                         fixture.n * kMaxControlWords * sizeof(std::uint64_t), 0,
                         device_matrices, fixture.n, 1, device_result);
    }
    ok = check(hipGetLastError(), "lookup-table Gray-walk launch");
  }
  if (ok && preparation_only) {
    ok = check(hipDeviceSynchronize(), "three-plane preparation synchronize") &&
         check(hipMemcpy(prepared.data(), device_columns,
                         prepared.size() * sizeof(prepared.front()), hipMemcpyDeviceToHost),
               "hipMemcpy prepared three-plane columns");
  } else if (ok) {
    ok = check(hipDeviceSynchronize(), "F_7 wave synchronize") &&
         check(hipMemcpy(&actual, device_result, sizeof(actual), hipMemcpyDeviceToHost),
               "hipMemcpy result");
  }
  const bool freed_columns =
      device_columns == nullptr || check(hipFree(device_columns), "hipFree three-plane columns");
  const bool freed_matrices =
      device_matrices == nullptr || check(hipFree(device_matrices), "hipFree matrices");
  const bool freed_result =
      device_result == nullptr || check(hipFree(device_result), "hipFree result");
  if (!ok || !freed_columns || !freed_matrices || !freed_result) {
    return false;
  }
  if (preparation_only) {
    return verify_prepared_columns(fixture, prepared);
  }
  if (!preparation_only && actual != fixture.expected) {
    std::fprintf(stderr, "%s: device=%u expected=%llu\n", fixture.id.c_str(), actual,
                 static_cast<unsigned long long>(fixture.expected));
    return false;
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

bool read_canonical_lookup_tables_from_stream(KernelPath path) {
  std::array<unsigned char, kLutBytes> add{};
  std::array<unsigned char, kLutBytes> sub{};
  std::array<unsigned char, kLutBytes> mul{};
  if (!read_exact(add.data(), add.size()) || !read_exact(sub.data(), sub.size()) ||
      !read_exact(mul.data(), mul.size())) {
    std::fprintf(stderr, "missing canonical Packed7 lookup-table bytes\n");
    return false;
  }
  if (path != KernelPath::kLookupTableControl) {
    return true;
  }
  return check(hipMemcpyToSymbol(HIP_SYMBOL(d_mul_lut), mul.data(), mul.size(), 0,
                                 hipMemcpyHostToDevice),
               "hipMemcpyToSymbol streamed MUL_LUT") &&
         check(hipMemcpyToSymbol(HIP_SYMBOL(d_add_lut), add.data(), add.size(), 0,
                                 hipMemcpyHostToDevice),
               "hipMemcpyToSymbol streamed ADD_LUT") &&
         check(hipMemcpyToSymbol(HIP_SYMBOL(d_sub_lut), sub.data(), sub.size(), 0,
                                 hipMemcpyHostToDevice),
               "hipMemcpyToSymbol streamed SUB_LUT");
}

bool run_streamed_fixture_evidence(KernelPath path, bool preparation_only) {
  constexpr std::array<char, 8> kMagic = {'G', 'F', '2', 'W', 'A', 'V', 'E', '1'};
  std::array<char, kMagic.size()> magic{};
  std::uint32_t count = 0;
  if (!read_exact(magic.data(), magic.size()) || magic != kMagic) {
    std::fprintf(stderr, "invalid F_7 wave fixture evidence stream header\n");
    return false;
  }
  if (!read_canonical_lookup_tables_from_stream(path) || !read_u32_le(&count) || count == 0) {
    return false;
  }
  for (std::uint32_t record = 0; record < count; ++record) {
    std::uint32_t id_size = 0;
    std::uint32_t n = 0;
    std::uint64_t expected = 0;
    std::uint64_t matrix_size = 0;
    if (!read_u32_le(&id_size) || id_size == 0 || id_size > 1024) {
      std::fprintf(stderr, "F_7 fixture %u has invalid evidence metadata\n", record);
      return false;
    }
    std::string id(id_size, '\0');
    if (!read_exact(id.data(), id.size()) || !read_u32_le(&n) || !read_u64_le(&expected) ||
        !read_u64_le(&matrix_size) || (n != 16 && n != 20 && n != 24) ||
        expected >= kFieldOrder || matrix_size != static_cast<std::uint64_t>(n) * n) {
      std::fprintf(stderr, "F_7 fixture %u has invalid evidence metadata\n", record);
      return false;
    }
    Fixture fixture{id, static_cast<int>(n), expected,
                    std::vector<unsigned char>(matrix_size)};
    if (!read_exact(fixture.matrix.data(), fixture.matrix.size()) ||
        !run_fixture(fixture, path, preparation_only)) {
      std::fprintf(stderr, "F_7 fixture %u failed\n", record);
      return false;
    }
  }
  return true;
}

bool run_streamed_preparation_evidence() {
  constexpr std::array<char, 8> kMagic = {'G', 'F', '2', 'P', 'R', 'E', 'P', '1'};
  std::array<char, kMagic.size()> magic{};
  std::uint32_t count = 0;
  if (!read_exact(magic.data(), magic.size()) || magic != kMagic || !read_u32_le(&count) ||
      count == 0) {
    std::fprintf(stderr, "invalid F_7 preparation evidence stream header\n");
    return false;
  }
  for (std::uint32_t record = 0; record < count; ++record) {
    std::uint32_t id_size = 0;
    std::uint32_t n = 0;
    std::uint64_t matrix_size = 0;
    if (!read_u32_le(&id_size) || id_size == 0 || id_size > 1024) {
      std::fprintf(stderr, "F_7 preparation fixture %u has invalid metadata\n", record);
      return false;
    }
    std::string id(id_size, '\0');
    if (!read_exact(id.data(), id.size()) || !read_u32_le(&n) || !read_u64_le(&matrix_size) ||
        (n != 16 && n != 20 && n != 24) ||
        matrix_size != static_cast<std::uint64_t>(n) * n) {
      std::fprintf(stderr, "F_7 preparation fixture %u has invalid metadata\n", record);
      return false;
    }
    Fixture fixture{id, static_cast<int>(n), 0,
                    std::vector<unsigned char>(matrix_size)};
    if (!read_exact(fixture.matrix.data(), fixture.matrix.size()) ||
        !run_fixture(fixture, KernelPath::kThreePlane, true)) {
      std::fprintf(stderr, "F_7 preparation fixture %u failed\n", record);
      return false;
    }
  }
  return true;
}

KernelPath parse_path(const char* value, bool* valid) {
  if (std::strcmp(value, "f7-lookup-table-control") == 0) {
    *valid = true;
    return KernelPath::kLookupTableControl;
  }
  if (std::strcmp(value, "f7-three-plane-permanent") == 0) {
    *valid = true;
    return KernelPath::kThreePlane;
  }
  *valid = false;
  return KernelPath::kLookupTableControl;
}

}  // namespace

int main(int argc, char** argv) {
  KernelPath path = KernelPath::kLookupTableControl;
  bool reads_fixture_stdin = false;
  bool preparation_only = false;
  for (int argument = 1; argument < argc; ++argument) {
    if (std::strcmp(argv[argument], "--fixture-stdin") == 0) {
      reads_fixture_stdin = true;
    } else if (std::strcmp(argv[argument], "--stage") == 0 && argument + 1 < argc &&
               std::strcmp(argv[argument + 1], "three-plane-preparation") == 0) {
      preparation_only = true;
      ++argument;
    } else if (std::strcmp(argv[argument], "--path") == 0 && argument + 1 < argc) {
      bool valid = false;
      path = parse_path(argv[++argument], &valid);
      if (!valid) {
        std::fprintf(stderr, "unknown F_7 path: %s\n", argv[argument]);
        return 2;
      }
    } else {
      std::fprintf(stderr,
                   "usage: %s [--path f7-lookup-table-control|f7-three-plane-permanent] "
                   "[--stage three-plane-preparation] [--fixture-stdin]\n",
                   argv[0]);
      return 2;
    }
  }
  if (preparation_only && path != KernelPath::kThreePlane) {
    std::fprintf(stderr, "three-plane preparation requires f7-three-plane-permanent\n");
    return 2;
  }
  if (!reads_fixture_stdin && path == KernelPath::kLookupTableControl &&
      !initialise_smoke_lookup_tables()) {
    return 1;
  }
  if (reads_fixture_stdin) {
    if (preparation_only) {
      return run_streamed_preparation_evidence() ? 0 : 1;
    }
    return run_streamed_fixture_evidence(path, preparation_only) ? 0 : 1;
  }
  const Fixture structural{"f7-n1-one", 1, 1, {1}};
  return run_fixture(structural, path, preparation_only) ? 0 : 1;
}
