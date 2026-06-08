// Device-side ChaCha20 + Box-Muller AWGN noise sampler (design doc §3, §11).
//
// This kernel reproduces, on the GPU, the *exact* per-frame ChaCha20 32-bit
// word stream that the CPU path draws via `rand_chacha 0.9`'s
// `ChaCha20Rng::seed_from_u64(seed)` + `set_word_pos(worker_offset(...))`, so
// that the raw device bytes are byte-identical to the host stream at the same
// `(seed, snr_idx, worker_idx, frame_idx)` (criterion 1, [hard]). On top of
// that identical word stream it applies the same Box-Muller cosine transform
// the CPU uses (`gf2_coding::dvb_t2_bicm_harness::box_muller_cos`), producing
// f32 standard-normal samples that agree with the CPU to <= 1 ulp f32 — the
// transform is not bit-exact across hardware sin/cos, so the contract is a 1
// ulp tolerance, not bit-equality (design doc §11 GPU softmath).
//
// # Stream reproduction contract
//
// rand_chacha 0.9 builds its 32-byte ChaCha key from the u64 `seed` via
// rand_core's PCG32 expansion (rand_core-0.9.3/src/lib.rs:466), seeds the
// cipher with a zero nonce, and exposes the keystream in 32-bit-word units
// (`BLOCK_WORDS = 16`). `set_word_pos(p)` selects block `p / 16` and word
// `p % 16`. A standard normal draw consumes two f64 uniforms; each f64 is
// `rng.next_u64() >> 11` scaled by 2^-53, and each `next_u64` consumes two
// consecutive 32-bit words as `(w[idx+1] << 32) | w[idx]` (low word first,
// rand_core-0.9.3/src/block.rs:197). So one Gaussian sample consumes exactly
// 4 words; one complex symbol (I and Q axes) consumes 8 words. Frame strides
// (`FRAME_STRIDE = 2^20`) are multiples of 16, so every frame begins on a
// ChaCha block boundary.
//
// # Determinism (design doc §6/§11)
//
// The launch geometry is a pure function of the problem size (one thread per
// noise SAMPLE; see `launch_chacha20_awgn`). Each thread computes its own
// absolute ChaCha word position from `worker_offset` and the per-thread sample
// index, generates exactly the ChaCha block(s) it needs, and writes one f32 to
// a fixed output index. There are no atomics, no cross-thread reductions, and
// no occupancy-derived dimensions, so the output is reproducible run-to-run.

#include <hip/hip_runtime.h>
#include <stdint.h>

// ---------------------------------------------------------------------------
// ChaCha20 block function (RFC 8439 layout; the layout rand_chacha uses)
// ---------------------------------------------------------------------------

__device__ __forceinline__ uint32_t rotl32(uint32_t x, int n) {
    return (x << n) | (x >> (32 - n));
}

// One ChaCha quarter-round on four state words (by reference).
__device__ __forceinline__ void quarter_round(
    uint32_t& a, uint32_t& b, uint32_t& c, uint32_t& d) {
    a += b; d ^= a; d = rotl32(d, 16);
    c += d; b ^= c; b = rotl32(b, 12);
    a += b; d ^= a; d = rotl32(d, 8);
    c += d; b ^= c; b = rotl32(b, 7);
}

// Computes one 16-word ChaCha20 keystream block for `block_counter`, writing
// the 16 output u32 words into `out`. `key` is the 8-word (256-bit) key
// (little-endian u32 words, exactly as rand_chacha reads the 32-byte seed);
// the nonce/stream is zero (matching `seed_from_u64` + default stream).
//
// State layout (RFC 8439):
//   words 0..3  : ChaCha constants "expand 32-byte k"
//   words 4..11 : key[0..8]
//   word  12    : block_counter low  32 bits
//   word  13    : block_counter high 32 bits
//   words 14,15 : nonce/stream = 0
__device__ void chacha20_block(const uint32_t key[8], uint64_t block_counter,
                               uint32_t out[16]) {
    uint32_t state[16];
    state[0] = 0x61707865u;
    state[1] = 0x3320646eu;
    state[2] = 0x79622d32u;
    state[3] = 0x6b206574u;
    for (int i = 0; i < 8; ++i) {
        state[4 + i] = key[i];
    }
    state[12] = (uint32_t)(block_counter & 0xffffffffull);
    state[13] = (uint32_t)(block_counter >> 32);
    state[14] = 0u;
    state[15] = 0u;

    uint32_t x[16];
    for (int i = 0; i < 16; ++i) {
        x[i] = state[i];
    }
    // 20 rounds = 10 double rounds (column round + diagonal round).
    for (int i = 0; i < 10; ++i) {
        // Column rounds.
        quarter_round(x[0], x[4], x[8], x[12]);
        quarter_round(x[1], x[5], x[9], x[13]);
        quarter_round(x[2], x[6], x[10], x[14]);
        quarter_round(x[3], x[7], x[11], x[15]);
        // Diagonal rounds.
        quarter_round(x[0], x[5], x[10], x[15]);
        quarter_round(x[1], x[6], x[11], x[12]);
        quarter_round(x[2], x[7], x[8], x[13]);
        quarter_round(x[3], x[4], x[9], x[14]);
    }
    for (int i = 0; i < 16; ++i) {
        out[i] = x[i] + state[i];
    }
}

// ---------------------------------------------------------------------------
// Word-stream reads matching rand_chacha 0.9 / rand 0.9 exactly
// ---------------------------------------------------------------------------

// Returns the 32-bit ChaCha word at absolute stream position `word_pos`,
// caching the most recently generated block in `cache` (keyed by
// `*cached_block`, initialized to UINT64_MAX = "no block cached"). This mirrors
// rand_chacha's block-at-a-time generation: word_pos identifies block
// `word_pos / 16` and word index `word_pos % 16`.
__device__ __forceinline__ uint32_t chacha_word(
    const uint32_t key[8], uint64_t word_pos,
    uint32_t cache[16], uint64_t* cached_block) {
    uint64_t block = word_pos >> 4;       // / 16
    uint32_t idx = (uint32_t)(word_pos & 15ull); // % 16
    if (block != *cached_block) {
        chacha20_block(key, block, cache);
        *cached_block = block;
    }
    return cache[idx];
}

// Draws one u64 the way rand_core's BlockRng does: low word first, then high.
// Consumes the two words at `word_pos` and `word_pos + 1`.
__device__ __forceinline__ uint64_t next_u64_at(
    const uint32_t key[8], uint64_t word_pos,
    uint32_t cache[16], uint64_t* cached_block) {
    uint32_t lo = chacha_word(key, word_pos, cache, cached_block);
    uint32_t hi = chacha_word(key, word_pos + 1, cache, cached_block);
    return ((uint64_t)hi << 32) | (uint64_t)lo;
}

// Draws one f64 uniform in [0, 1) the way rand 0.9's StandardUniform does:
// take the top 53 bits of a u64 and scale by 2^-53
// (rand-0.9.x/src/distr/float.rs). Consumes 2 ChaCha words.
__device__ __forceinline__ double next_f64_at(
    const uint32_t key[8], uint64_t word_pos,
    uint32_t cache[16], uint64_t* cached_block) {
    uint64_t v = next_u64_at(key, word_pos, cache, cached_block);
    v >>= (64 - 53); // keep the top 53 bits
    const double scale = 1.0 / 9007199254740992.0; // 1 / 2^53
    return scale * (double)v;
}

// Box-Muller cosine branch, matching
// `gf2_coding::dvb_t2_bicm_harness::box_muller_cos`:
//   u1 = max(u1, 1e-15); ((-2 ln u1).sqrt() * cos(2*pi*u2)) as f32
// The CPU computes this in f64 then narrows to f32; we do the same so the
// only divergence is the hardware-vs-CPU transcendental rounding (<= 1 ulp f32,
// design doc §11).
__device__ __forceinline__ float box_muller_cos(double u1, double u2) {
    if (u1 < 1e-15) u1 = 1e-15;
    const double TWO_PI = 6.283185307179586; // 2 * pi
    double r = sqrt(-2.0 * log(u1));
    double n = r * cos(TWO_PI * u2);
    return (float)n;
}

// ---------------------------------------------------------------------------
// Kernels
// ---------------------------------------------------------------------------

// Emits the raw ChaCha20 32-bit word stream starting at `base_word_pos` into
// `out` (`n_words` words). One thread per word. This is the criterion-1
// byte-identity oracle: it must equal a host `ChaCha20Rng::seed_from_u64(seed)`
// repositioned to `base_word_pos` and read `n_words` times via `next_u32`.
extern "C" __global__ void chacha20_words_kernel(
    const uint32_t* key,        // device ptr: [8] little-endian key words
    uint64_t base_word_pos,     // absolute ChaCha word position to start at
    uint32_t* out,              // device ptr (output): [n_words]
    int n_words) {
    int tid = blockIdx.x * blockDim.x + threadIdx.x;
    if (tid >= n_words) return;
    uint32_t k[8];
    for (int i = 0; i < 8; ++i) k[i] = key[i];
    uint32_t cache[16];
    uint64_t cached_block = 0xffffffffffffffffull;
    out[tid] = chacha_word(k, base_word_pos + (uint64_t)tid, cache, &cached_block);
}

// Emits `n_samples` Box-Muller standard-normal f32 samples into `out`, one
// thread per sample. Sample `s` consumes the 4 ChaCha words at
// `base_word_pos + 4*s` (u1 from words 0,1; u2 from words 2,3), matching the
// CPU `draw_standard_normal` order. `base_word_pos` is the frame's
// `worker_offset(...)` (a multiple of 16), supplied by the host wrapper.
extern "C" __global__ void chacha20_awgn_kernel(
    const uint32_t* key,        // device ptr: [8] little-endian key words
    uint64_t base_word_pos,     // frame's worker_offset (ChaCha word units)
    float* out,                 // device ptr (output): [n_samples] N(0,1)
    int n_samples) {
    int tid = blockIdx.x * blockDim.x + threadIdx.x;
    if (tid >= n_samples) return;
    uint32_t k[8];
    for (int i = 0; i < 8; ++i) k[i] = key[i];
    uint32_t cache[16];
    uint64_t cached_block = 0xffffffffffffffffull;
    uint64_t wp = base_word_pos + 4ull * (uint64_t)tid;
    double u1 = next_f64_at(k, wp, cache, &cached_block);
    double u2 = next_f64_at(k, wp + 2, cache, &cached_block);
    out[tid] = box_muller_cos(u1, u2);
}

// ---------------------------------------------------------------------------
// Host-side launch wrappers (callable from Rust via FFI). Fixed block size,
// grid = ceil(n / block) — a pure function of the problem size (design doc §6).
// ---------------------------------------------------------------------------

static constexpr int BLOCK_THREADS = 256;

extern "C" int launch_chacha20_words(
    const uint32_t* key,
    uint64_t base_word_pos,
    uint32_t* out,
    int n_words,
    void* stream) {
    if (n_words <= 0) return 0;
    dim3 block(BLOCK_THREADS);
    dim3 grid((n_words + BLOCK_THREADS - 1) / BLOCK_THREADS);
    hipLaunchKernelGGL(
        chacha20_words_kernel,
        grid, block, 0,
        reinterpret_cast<hipStream_t>(stream),
        key, base_word_pos, out, n_words);
    return static_cast<int>(hipGetLastError());
}

extern "C" int launch_chacha20_awgn(
    const uint32_t* key,
    uint64_t base_word_pos,
    float* out,
    int n_samples,
    void* stream) {
    if (n_samples <= 0) return 0;
    dim3 block(BLOCK_THREADS);
    dim3 grid((n_samples + BLOCK_THREADS - 1) / BLOCK_THREADS);
    hipLaunchKernelGGL(
        chacha20_awgn_kernel,
        grid, block, 0,
        reinterpret_cast<hipStream_t>(stream),
        key, base_word_pos, out, n_samples);
    return static_cast<int>(hipGetLastError());
}
