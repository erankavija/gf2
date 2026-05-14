// Permanent computation kernel for GF(3) (bipedal-3) — placeholder.
//
// SAFETY: This file is a scaffold placeholder. The actual device kernel
// will be implemented by the downstream issue (ad55b777). All entry
// points compile cleanly under hipcc and link against the HIP runtime, but
// do not perform any real computation: out_ptr is not written and the
// function returns 0 (hipSuccess) unconditionally.
//
// Entry point contract (to be refined by downstream):
//   permanent_bipedal3_hip(matrix_ptr, n, out_ptr)
//   - matrix_ptr: device pointer to an n*n row-major matrix of uint8 elements
//                 in GF(3) (values 0, 1, 2).
//   - n: matrix dimension (square, n x n).
//   - out_ptr: device pointer to a single uint64_t output (the permanent mod 3).
//
// TODO(ad55b777): replace the stub body with the real Ryser/BPAS kernel.

#include <hip/hip_runtime.h>

// SAFETY: Caller must ensure:
//   - `matrix_ptr` is a valid device allocation of at least n*n bytes.
//   - `out_ptr` is a valid device allocation of at least 8 bytes.
//   - `n >= 0`.
// Placeholder: does not write to out_ptr; returns 0 (hipSuccess).
extern "C" int permanent_bipedal3_hip(const unsigned char* matrix_ptr,
                                       int n,
                                       unsigned long long* out_ptr) {
    (void)matrix_ptr;
    (void)n;
    (void)out_ptr;
    // Placeholder — no computation. Downstream issue ad55b777 will
    // launch a Ryser/BPAS device kernel here and store the result.
    return 0; // hipSuccess
}
