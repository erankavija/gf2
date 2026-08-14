// Streamed batch-evaluation boundary shared by the wave-prototype executables.
//
// The measurement harness reaches a prototype kernel through the same
// host/device boundary this crate's fixture evidence already uses: a prebuilt
// HIP executable that reads a framed stream on stdin. This header owns that
// frame format, the device allocation and transfer around one batch, the event
// instrumentation, and the response encoding, so each candidate source supplies
// only its own launch and no candidate restates the boundary.
//
// A worker stays resident across many batches: the harness writes one evaluate
// frame per batch and reads one response frame back, so a measured batch pays
// the pipe transfer and this executable's per-batch device allocation, never a
// process start.
//
// Placement note: each candidate source includes this header *after* its kernel
// definitions. The committed compiler resource remarks beside those sources
// cite their kernels by line, and an include above them would move every one.
//
// Frame format, little-endian throughout:
//
//   request:  magic "GF2BEVAL" once, then repeated frames
//               u32 kind = 0 evaluate: u32 n, u32 batch, u8[batch*n*n]
//               u32 kind = 1 shutdown
//               u32 kind = 2 lookup tables: u8[3 * 65536] add, sub, mul
//   response: u32 status (0 ok, 1 failed)
//               evaluate and status 0 additionally:
//                 f64 h2d_s, f64 kernel_s, f64 d2h_s, f64 submission_to_kernel_s
//                 u32 values[batch]
//
// Matrix bytes are canonical row-major field values. This boundary does not
// re-validate them: the harness draws them from its rejection sampler, and a
// per-batch validation pass would enter the span the harness times.

#pragma once

#include <hip/hip_runtime.h>

#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <vector>

namespace gf2_wave_batch {

constexpr std::array<char, 8> kRequestMagic = {'G', 'F', '2', 'B', 'E', 'V', 'A', 'L'};

constexpr std::uint32_t kFrameEvaluate = 0;
constexpr std::uint32_t kFrameShutdown = 1;
constexpr std::uint32_t kFrameLookupTables = 2;

constexpr std::uint32_t kStatusOk = 0;
constexpr std::uint32_t kStatusFailed = 1;

// Three 64 KiB two-nibble tables, in the add/sub/mul order the F_7 control's
// canonical Packed7 upload uses.
constexpr std::size_t kLookupTableBytes = 3u * 65536u;

// Refuses an implausible request rather than attempting a multi-gigabyte
// allocation. The campaign's largest cell is far below this.
constexpr std::uint64_t kMaxRequestBytes = UINT64_C(1) << 32;

inline bool stream_check(hipError_t status, const char* operation) {
  if (status == hipSuccess) {
    return true;
  }
  std::fprintf(stderr, "batch stream %s: %s\n", operation, hipGetErrorString(status));
  return false;
}

inline bool read_exact(void* destination, std::size_t bytes) {
  if (bytes == 0) {
    return true;
  }
  std::cin.read(static_cast<char*>(destination), static_cast<std::streamsize>(bytes));
  return static_cast<bool>(std::cin);
}

inline bool read_u32(std::uint32_t* value) {
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

inline void write_u32(std::uint32_t value) {
  std::array<unsigned char, 4> bytes{};
  for (unsigned shift = 0; shift < 4; ++shift) {
    bytes[shift] = static_cast<unsigned char>((value >> (8 * shift)) & 0xffU);
  }
  std::cout.write(reinterpret_cast<const char*>(bytes.data()),
                  static_cast<std::streamsize>(bytes.size()));
}

inline void write_f64(double value) {
  std::array<unsigned char, 8> bytes{};
  std::memcpy(bytes.data(), &value, bytes.size());
  std::cout.write(reinterpret_cast<const char*>(bytes.data()),
                  static_cast<std::streamsize>(bytes.size()));
}

inline void write_status(std::uint32_t status) {
  write_u32(status);
  std::cout.flush();
}

/// Device-clock spans of one batch, in seconds.
struct DeviceSpans {
  double h2d_s;
  double kernel_s;
  double d2h_s;
  double submission_to_kernel_s;
};

/// Five stream markers bracketing one batch, released together.
class BatchEvents {
 public:
  bool create() {
    for (auto& event : events_) {
      if (!stream_check(hipEventCreate(&event), "hipEventCreate")) {
        return false;
      }
    }
    return true;
  }

  ~BatchEvents() {
    for (auto& event : events_) {
      if (event != nullptr) {
        (void)hipEventDestroy(event);
      }
    }
  }

  bool record(std::size_t marker) {
    return stream_check(hipEventRecord(events_[marker], 0), "hipEventRecord");
  }

  bool spans(DeviceSpans* out) const {
    float h2d = 0.0F;
    float kernel = 0.0F;
    float d2h = 0.0F;
    float submission = 0.0F;
    if (!stream_check(hipEventElapsedTime(&h2d, events_[kSubmit], events_[kH2dEnd]),
                      "hipEventElapsedTime h2d") ||
        !stream_check(
            hipEventElapsedTime(&kernel, events_[kKernelStart], events_[kKernelEnd]),
            "hipEventElapsedTime kernel") ||
        !stream_check(hipEventElapsedTime(&d2h, events_[kKernelEnd], events_[kD2hEnd]),
                      "hipEventElapsedTime d2h") ||
        !stream_check(
            hipEventElapsedTime(&submission, events_[kH2dEnd], events_[kKernelStart]),
            "hipEventElapsedTime submission")) {
      return false;
    }
    out->h2d_s = static_cast<double>(h2d) / 1000.0;
    out->kernel_s = static_cast<double>(kernel) / 1000.0;
    out->d2h_s = static_cast<double>(d2h) / 1000.0;
    out->submission_to_kernel_s = static_cast<double>(submission) / 1000.0;
    return true;
  }

  static constexpr std::size_t kSubmit = 0;
  static constexpr std::size_t kH2dEnd = 1;
  static constexpr std::size_t kKernelStart = 2;
  static constexpr std::size_t kKernelEnd = 3;
  static constexpr std::size_t kD2hEnd = 4;

 private:
  std::array<hipEvent_t, 5> events_{};
};

/// Evaluate one batch through `launch`, returning its values and device spans.
///
/// `launch(device_matrices, n, batch, device_results)` enqueues this
/// candidate's own kernels on the null stream and returns false on a device
/// failure. The kernel span brackets that whole enqueue, so a candidate whose
/// mapping needs a staged preparation launch reports both of its launches.
template <typename Launch>
bool evaluate_batch(Launch& launch, int n, int batch,
                    const std::vector<unsigned char>& host_matrices,
                    std::vector<unsigned>* values, DeviceSpans* spans) {
  unsigned char* device_matrices = nullptr;
  unsigned* device_results = nullptr;
  BatchEvents events;
  values->assign(static_cast<std::size_t>(batch), 0U);
  const auto matrix_bytes = host_matrices.size() * sizeof(unsigned char);
  const auto result_bytes = values->size() * sizeof(unsigned);

  bool ok = events.create() &&
            stream_check(hipMalloc(&device_matrices, matrix_bytes), "hipMalloc matrices") &&
            stream_check(hipMalloc(&device_results, result_bytes), "hipMalloc results");
  if (ok) {
    ok = events.record(BatchEvents::kSubmit) &&
         stream_check(hipMemcpyAsync(device_matrices, host_matrices.data(), matrix_bytes,
                                     hipMemcpyHostToDevice, 0),
                      "hipMemcpyAsync matrices") &&
         events.record(BatchEvents::kH2dEnd) && events.record(BatchEvents::kKernelStart) &&
         launch(device_matrices, n, batch, device_results) &&
         stream_check(hipGetLastError(), "kernel launch") &&
         events.record(BatchEvents::kKernelEnd) &&
         stream_check(hipMemcpyAsync(values->data(), device_results, result_bytes,
                                     hipMemcpyDeviceToHost, 0),
                      "hipMemcpyAsync results") &&
         events.record(BatchEvents::kD2hEnd) &&
         stream_check(hipStreamSynchronize(0), "hipStreamSynchronize") &&
         events.spans(spans);
  }

  const bool freed_matrices =
      device_matrices == nullptr || stream_check(hipFree(device_matrices), "hipFree matrices");
  const bool freed_results =
      device_results == nullptr || stream_check(hipFree(device_results), "hipFree results");
  return ok && freed_matrices && freed_results;
}

/// Serve batch-evaluation frames on stdin until shutdown or end of input.
///
/// `max_order` is the largest matrix order this candidate's kernel accepts.
/// `lookup_tables(bytes)` receives the canonical two-nibble table upload for a
/// candidate that needs one and returns false on a device failure; a candidate
/// with no tables accepts and ignores the frame, exactly as the fixture
/// evidence stream does.
template <typename Launch, typename LookupTables>
int serve_batch_stream(int max_order, Launch&& launch, LookupTables&& lookup_tables) {
  std::array<char, kRequestMagic.size()> magic{};
  if (!read_exact(magic.data(), magic.size()) || magic != kRequestMagic) {
    std::fprintf(stderr, "invalid batch evaluation stream header\n");
    return 2;
  }

  std::vector<unsigned char> host_matrices;
  std::vector<unsigned> values;
  std::vector<unsigned char> tables(kLookupTableBytes, 0);
  while (true) {
    std::uint32_t kind = 0;
    if (!read_u32(&kind)) {
      // End of input is the harness closing the worker, not a failure.
      return 0;
    }
    if (kind == kFrameShutdown) {
      return 0;
    }
    if (kind == kFrameLookupTables) {
      if (!read_exact(tables.data(), tables.size())) {
        std::fprintf(stderr, "truncated canonical lookup-table frame\n");
        return 2;
      }
      write_status(lookup_tables(tables.data()) ? kStatusOk : kStatusFailed);
      continue;
    }
    if (kind != kFrameEvaluate) {
      std::fprintf(stderr, "unknown batch evaluation frame kind %u\n", kind);
      return 2;
    }

    std::uint32_t n = 0;
    std::uint32_t batch = 0;
    if (!read_u32(&n) || !read_u32(&batch)) {
      std::fprintf(stderr, "truncated batch evaluation frame header\n");
      return 2;
    }
    const auto requested_bytes =
        static_cast<std::uint64_t>(batch) * static_cast<std::uint64_t>(n) * n;
    if (n < 1 || static_cast<int>(n) > max_order || batch < 1 ||
        requested_bytes > kMaxRequestBytes) {
      std::fprintf(stderr, "batch evaluation frame requests n=%u batch=%u beyond this kernel\n",
                   n, batch);
      return 2;
    }
    host_matrices.resize(static_cast<std::size_t>(requested_bytes));
    if (!read_exact(host_matrices.data(), host_matrices.size())) {
      std::fprintf(stderr, "truncated batch evaluation matrix bytes\n");
      return 2;
    }

    DeviceSpans spans{0.0, 0.0, 0.0, 0.0};
    if (!evaluate_batch(launch, static_cast<int>(n), static_cast<int>(batch), host_matrices,
                        &values, &spans)) {
      write_status(kStatusFailed);
      continue;
    }
    write_u32(kStatusOk);
    write_f64(spans.h2d_s);
    write_f64(spans.kernel_s);
    write_f64(spans.d2h_s);
    write_f64(spans.submission_to_kernel_s);
    for (const auto value : values) {
      write_u32(value);
    }
    std::cout.flush();
  }
}

/// A candidate whose kernel needs no lookup tables still accepts the frame.
struct NoLookupTables {
  bool operator()(const unsigned char*) const { return true; }
};

}  // namespace gf2_wave_batch
