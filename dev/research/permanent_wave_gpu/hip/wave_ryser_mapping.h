// Shared, intentionally small wave-Ryser control mapping.
//
// Packed column staging and packed field arithmetic stay in each candidate
// kernel. This header owns only the cross-field convention: balanced
// sequential Gray ranges, the canonical g(k) subset image, lane-index-ordered
// scalar modular reduction, and the one outer Ryser sign.

#pragma once

#include <hip/hip_runtime.h>

#include <cstdint>

namespace gf2_wave_mapping {

constexpr unsigned kWaveLanes = 32;

struct GrayInterval {
  std::uint64_t start;
  std::uint64_t end;
};

struct GrayTransition {
  std::uint64_t subset;
  unsigned flipped_column;
  bool added;
};

__host__ __device__ __forceinline__ unsigned active_lanes_for_order(int n) {
  return n < 5 ? (1U << n) : kWaveLanes;
}

__host__ __device__ __forceinline__ GrayInterval balanced_interval(
    std::uint64_t total, std::uint64_t lane, std::uint64_t lanes) {
  const auto base = total / lanes;
  const auto remainder = total % lanes;
  const auto extra_before = lane < remainder ? lane : remainder;
  const auto start = lane * base + extra_before;
  return {start, start + base + (lane < remainder ? UINT64_C(1) : UINT64_C(0))};
}

__host__ __device__ __forceinline__ std::uint64_t gray_subset(std::uint64_t index) {
  return index ^ (index >> 1);
}

/// Canonical transition at a nonzero sequential Gray index.
__host__ __device__ __forceinline__ GrayTransition gray_transition(std::uint64_t index) {
  const auto subset = gray_subset(index);
  const auto flipped_column = static_cast<unsigned>(__builtin_ctzll(index));
  return {subset, flipped_column, ((subset >> flipped_column) & 1) != 0};
}

template <unsigned Modulus>
__device__ __forceinline__ unsigned add_scalar(unsigned left, unsigned right) {
  const auto sum = left + right;
  return sum >= Modulus ? sum - Modulus : sum;
}

template <unsigned Modulus>
__device__ __forceinline__ unsigned negate_scalar(unsigned value) {
  return value == 0 ? 0 : Modulus - value;
}

// Every lane executes each shuffle; only lane zero accumulates the fetched
// scalar partials. Therefore the sole cross-lane exchange is the partial sum,
// and its additions have a fixed source order regardless of scheduling.
template <unsigned Modulus>
__device__ __forceinline__ unsigned reduce_partials_in_lane_order(unsigned partial,
                                                                    unsigned lane_count) {
  unsigned total = 0;
  for (unsigned source = 0; source < lane_count; ++source) {
    const auto source_partial = __shfl(partial, source, kWaveLanes);
    if (threadIdx.x == 0) {
      total = add_scalar<Modulus>(total, source_partial);
    }
  }
  return total;
}

template <unsigned Modulus>
__device__ __forceinline__ unsigned apply_outer_ryser_sign(unsigned reduced_total, int n) {
  return n % 2 == 0 ? reduced_total : negate_scalar<Modulus>(reduced_total);
}

}  // namespace gf2_wave_mapping
