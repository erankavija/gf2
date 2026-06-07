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

// Writes the calling host thread's currently-active HIP device index to
// `*device_id`. Used to save/restore the current device around a device-scoped
// query (e.g. per-device hipMemGetInfo) so the caller's selection is preserved.
int hip_get_device(int* device_id) {
    return static_cast<int>(hipGetDevice(device_id));
}

// Writes the number of visible HIP devices to `*count`. Returns hipSuccess
// even when count == 0 (the caller maps 0 → DeviceUnavailable per §8).
int hip_device_get_count(int* count) {
    return static_cast<int>(hipGetDeviceCount(count));
}

// Writes the device's GCN arch name (e.g. "gfx1030", "gfx940", "gfx942") into
// the caller-provided buffer `buf` of capacity `buf_len` bytes. The name is
// the canonical arch identifier from hipDeviceProp_t.gcnArchName and is the
// authoritative discriminator for kernel-blob selection (design doc §6) — it
// distinguishes steppings that share a compute capability (gfx940 vs gfx942).
// The buffer is always NUL-terminated on success (truncated if too small).
// Returns hipSuccess (0) on success, the hipError_t cast to int otherwise.
int hip_device_get_arch_name(int device_id, char* buf, size_t buf_len) {
    if (buf == nullptr || buf_len == 0) {
        return static_cast<int>(hipErrorInvalidValue);
    }
    hipDeviceProp_t props;
    hipError_t e = hipGetDeviceProperties(&props, device_id);
    if (e != hipSuccess) {
        return static_cast<int>(e);
    }
    // gcnArchName may include a feature suffix (e.g. "gfx942:sramecc+:xnack-");
    // copy the full string and let the Rust side split on ':'.
    size_t i = 0;
    for (; i + 1 < buf_len && props.gcnArchName[i] != '\0'; ++i) {
        buf[i] = props.gcnArchName[i];
    }
    buf[i] = '\0';
    return static_cast<int>(hipSuccess);
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
