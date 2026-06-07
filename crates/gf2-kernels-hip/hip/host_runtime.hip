// Host-side HIP runtime FFI wrappers for the gf2-sim pipeline.
//
// These thin `extern "C"` shims back the host infrastructure in
// `src/host/` (stream pool, allocator, multi-arch dispatch). They mirror
// the convention of the memory wrappers already in `bcjr_kernel.hip`:
// every function returns the `hipError_t` cast to `int` (0 == hipSuccess),
// and out-parameters are written only on success.
//
// This file is compiled unconditionally for gfx1030 by build.rs (it has no
// device kernels, only host runtime calls), so it is available on every
// build of this crate regardless of the `hip` Cargo feature.

#include <hip/hip_runtime.h>

extern "C" {

// ---- Stream management ----------------------------------------------------

// Creates a new (non-default) HIP stream on the current device. Writes the
// opaque hipStream_t handle to `*stream` on success.
int hip_stream_create(void** stream) {
    return static_cast<int>(hipStreamCreate(reinterpret_cast<hipStream_t*>(stream)));
}

// Destroys a stream previously created by hip_stream_create.
int hip_stream_destroy(void* stream) {
    return static_cast<int>(hipStreamDestroy(reinterpret_cast<hipStream_t>(stream)));
}

// Blocks the host until all work enqueued on `stream` has completed. Used by
// the drain-for-checkpoint contract (design doc §4) which synchronizes each
// worker's owned stream rather than the whole device.
int hip_stream_synchronize(void* stream) {
    return static_cast<int>(hipStreamSynchronize(reinterpret_cast<hipStream_t>(stream)));
}

// Non-blocking query: returns hipSuccess (0) if the stream is idle,
// hipErrorNotReady (>0) if work is still pending, or another error code.
int hip_stream_query(void* stream) {
    return static_cast<int>(hipStreamQuery(reinterpret_cast<hipStream_t>(stream)));
}

// ---- Device selection / introspection ------------------------------------

// Sets the active HIP device for the calling host thread.
int hip_set_device(int device_id) {
    return static_cast<int>(hipSetDevice(device_id));
}

// Writes the number of visible HIP devices to `*count`. Returns hipSuccess
// even when count == 0 (the caller maps 0 → DeviceUnavailable per §8).
int hip_device_get_count(int* count) {
    return static_cast<int>(hipGetDeviceCount(count));
}

// Reads the compute-capability major/minor of `device_id` via
// hipDeviceGetAttribute. Both out-params are written on success. This is the
// arch-detection primitive of design doc §6.
int hip_device_compute_capability(int device_id, int* major, int* minor) {
    hipError_t e = hipDeviceGetAttribute(
        major, hipDeviceAttributeComputeCapabilityMajor, device_id);
    if (e != hipSuccess) {
        return static_cast<int>(e);
    }
    return static_cast<int>(hipDeviceGetAttribute(
        minor, hipDeviceAttributeComputeCapabilityMinor, device_id));
}

// Writes free and total device memory (bytes) for the current device.
int hip_mem_get_info(size_t* free_bytes, size_t* total_bytes) {
    return static_cast<int>(hipMemGetInfo(free_bytes, total_bytes));
}

// ---- Pinned host memory ---------------------------------------------------

// Allocates `size` bytes of page-locked (pinned) host memory for fast staged
// H2D/D2H transfers. Writes the host pointer to `*ptr` on success.
int hip_host_malloc(void** ptr, size_t size) {
    return static_cast<int>(hipHostMalloc(ptr, size, hipHostMallocDefault));
}

// Frees memory allocated by hip_host_malloc.
int hip_host_free(void* ptr) {
    return static_cast<int>(hipHostFree(ptr));
}

// ---- Asynchronous (stream-ordered) transfers -----------------------------

// Stream-ordered H2D copy. With pinned host memory this overlaps with kernel
// execution on other streams.
int hip_memcpy_h2d_async(void* dst, const void* src, size_t size, void* stream) {
    return static_cast<int>(hipMemcpyAsync(
        dst, src, size, hipMemcpyHostToDevice,
        reinterpret_cast<hipStream_t>(stream)));
}

// Stream-ordered D2H copy.
int hip_memcpy_d2h_async(void* dst, const void* src, size_t size, void* stream) {
    return static_cast<int>(hipMemcpyAsync(
        dst, src, size, hipMemcpyDeviceToHost,
        reinterpret_cast<hipStream_t>(stream)));
}

}  // extern "C"
