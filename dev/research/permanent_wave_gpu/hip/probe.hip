// Compile-only probe for the registry crate's HIP source discovery. Candidate
// kernels replace neither this probe nor the source-discovery convention.
#include <hip/hip_runtime.h>

extern "C" __global__ void permanent_wave_gpu_probe() {}
