//! Safe host wrappers for the device ChaCha20 + Box-Muller AWGN kernel
//! (`hip/chacha20_awgn.hip`, design doc §3 / §11).
//!
//! These wrappers seed the device kernel's ChaCha20 state from a `u64` seed
//! using the **exact** key-derivation `rand_chacha 0.9` performs
//! ([`ChaCha20Rng::seed_from_u64`](rand_chacha::ChaCha20Rng) → rand_core's
//! PCG32 32-byte seed expansion → the key *is* that 32-byte seed, read
//! little-endian as 8 `u32` words). Reproducing the key host-side and uploading
//! it lets the device kernel emit a keystream byte-identical to the host RNG at
//! the same `worker_offset(...)` word position (criterion 1, [hard]).
//!
//! # Why the key is derived here, not in `gf2-sim`
//!
//! `gf2-kernels-hip` owns all device FFI and the SAFETY-annotated launch path,
//! so the key-upload + launch is a single reviewed unit here. The `gf2-sim`
//! `GpuAwgn` stage (the §8 fallback-bearing consumer) calls
//! [`GpuChaChaAwgn`] without touching FFI, preserving `gf2-sim`'s
//! `#![deny(unsafe_code)]`.

use std::ffi::c_void;

use crate::host::{DeviceBuffer, HipStream};
use crate::{check_hip, ffi, HipError};

/// Derives the 32-byte ChaCha20 key `rand_chacha 0.9` builds from a `u64` seed.
///
/// This reproduces `rand_core 0.9`'s `SeedableRng::seed_from_u64`
/// (rand_core-0.9.3/src/lib.rs): a PCG32 generator advanced from `seed` emits
/// eight `u32` words via the PCG-XSH-RR output function, each written
/// little-endian, filling the 32-byte ChaCha key. `ChaCha20Rng` then uses that
/// 32-byte buffer directly as its key with a zero nonce, so reproducing it here
/// (and uploading it) is what makes the device keystream match the host RNG.
///
/// Returned as 8 `u32` words in the order the device kernel consumes them
/// (`key[i]` = the little-endian `u32` at byte offset `4*i` of the 32-byte
/// seed).
///
/// # Arguments
///
/// * `seed` — the base `u64` seed passed to `ChaCha20Rng::seed_from_u64`.
///
/// # Returns
///
/// The 8-word (256-bit) ChaCha key.
///
/// # Complexity
///
/// O(1) — eight PCG32 steps.
///
/// # Examples
///
/// ```
/// use gf2_kernels_hip::chacha20_key_from_seed;
///
/// // Deterministic: the same seed always derives the same key.
/// assert_eq!(chacha20_key_from_seed(42), chacha20_key_from_seed(42));
/// // Different seeds (almost surely) derive different keys.
/// assert_ne!(chacha20_key_from_seed(0), chacha20_key_from_seed(1));
/// ```
#[must_use]
pub fn chacha20_key_from_seed(seed: u64) -> [u32; 8] {
    // PCG32 constants from rand_core-0.9.3/src/lib.rs::seed_from_u64.
    const MUL: u64 = 6364136223846793005;
    const INC: u64 = 11634580027462260723;

    let mut state = seed;
    let mut key = [0u32; 8];
    for slot in key.iter_mut() {
        // Advance first (away from a low-Hamming-weight input), then apply the
        // PCG-XSH-RR output function; the result `x.to_le_bytes()` fills four
        // seed bytes, i.e. one little-endian key word.
        state = state.wrapping_mul(MUL).wrapping_add(INC);
        let s = state;
        let xorshifted = (((s >> 18) ^ s) >> 27) as u32;
        let rot = (s >> 59) as u32;
        // `to_le_bytes` then `from_le_bytes` is the identity, so the key word
        // is exactly the PCG output (the device reads the bytes little-endian).
        *slot = xorshifted.rotate_right(rot);
    }
    key
}

/// A reusable device-side ChaCha20 + Box-Muller AWGN noise generator.
///
/// Holds the persistent device-resident ChaCha key (derived from a `u64` seed
/// via [`chacha20_key_from_seed`]) plus reusable output buffers, so repeated
/// per-frame generation reuses one `hipMalloc`/upload. The two entry points
/// mirror the two kernels in `hip/chacha20_awgn.hip`:
///
/// * [`raw_words`](Self::raw_words) — the criterion-1 byte-identity oracle:
///   emits the raw ChaCha 32-bit word stream from an absolute word position.
/// * [`noise_samples`](Self::noise_samples) — the criterion-2 path: emits
///   Box-Muller standard-normal f32 samples from a frame's `worker_offset`.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_hip::GpuChaChaAwgn;
///
/// // Requires a real HIP device, so this is `no_run`.
/// // Seed 42, device 0, room for up to 1024 samples / 4096 words per call.
/// let gen = GpuChaChaAwgn::new(42, 0, 4096).expect("build generator");
/// // Raw words at stream position 0 (one frame's start when worker_offset == 0).
/// let words = gen.raw_words(0, 64).expect("raw words");
/// assert_eq!(words.len(), 64);
/// // 32 standard-normal samples from frame offset 0.
/// let noise = gen.noise_samples(0, 32).expect("noise");
/// assert_eq!(noise.len(), 32);
/// ```
pub struct GpuChaChaAwgn {
    d_key: DeviceBuffer<u32>,
    /// Output scratch (`u32` lanes); reinterpreted as `f32` for the noise
    /// kernel since both are 4 bytes wide with identical layout.
    d_out: DeviceBuffer<u32>,
    device_id: i32,
    capacity: usize,
}

impl GpuChaChaAwgn {
    /// Builds a generator for `seed` on `device_id`, with output buffers sized
    /// for up to `capacity` words (or samples) per call.
    ///
    /// The 8-word ChaCha key is derived once via [`chacha20_key_from_seed`] and
    /// uploaded; the output buffer holds `capacity` 4-byte lanes (the max of a
    /// `raw_words`/`noise_samples` request).
    ///
    /// # Arguments
    ///
    /// * `seed` — the base `u64` seed (selects the ChaCha stream).
    /// * `device_id` — the HIP device to allocate on.
    /// * `capacity` — maximum words / samples per call (sizes the output buffer).
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if device allocation or the one-time key upload
    /// fails (an OOM is the distinguished [`HipError::OutOfMemory`]).
    ///
    /// # Complexity
    ///
    /// O(`capacity`) device memory plus one 32-byte key upload.
    pub fn new(seed: u64, device_id: i32, capacity: usize) -> Result<Self, HipError> {
        let key = chacha20_key_from_seed(seed);
        let d_key = DeviceBuffer::<u32>::new(8, device_id)?;
        d_key.copy_from_host(&key)?;
        let d_out = DeviceBuffer::<u32>::new(capacity, device_id)?;
        Ok(Self {
            d_key,
            d_out,
            device_id,
            capacity,
        })
    }

    /// The device this generator's buffers are bound to.
    #[must_use]
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// The maximum words / samples a single call may request.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Emits `n_words` raw ChaCha20 32-bit words starting at absolute stream
    /// position `base_word_pos` (criterion-1 byte-identity oracle).
    ///
    /// The returned words equal a host `ChaCha20Rng::seed_from_u64(seed)`
    /// repositioned via `set_word_pos(base_word_pos)` and read `n_words` times
    /// via `next_u32`.
    ///
    /// # Arguments
    ///
    /// * `base_word_pos` — absolute ChaCha 32-bit-word position to start at.
    /// * `n_words` — number of words to emit; must be `<= capacity`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, synchronization, or D2H failure.
    ///
    /// # Panics
    ///
    /// Panics if `n_words > capacity`.
    pub fn raw_words(&self, base_word_pos: u128, n_words: usize) -> Result<Vec<u32>, HipError> {
        assert!(
            n_words <= self.capacity,
            "GpuChaChaAwgn::raw_words: n_words {n_words} > capacity {}",
            self.capacity
        );
        if n_words == 0 {
            return Ok(Vec::new());
        }
        let base = u64::try_from(base_word_pos)
            .expect("base_word_pos exceeds u64 (frame index far beyond any practical run)");
        // SAFETY: `d_key` holds the uploaded 8-word key; `d_out` is sized for
        // `capacity >= n_words` u32 lanes (asserted). The kernel reads the key
        // and writes exactly `n_words` words; the FFI returns hipGetLastError.
        check_hip(
            unsafe {
                ffi::launch_chacha20_words(
                    self.d_key.as_ptr() as *const u32,
                    base,
                    self.d_out.as_mut_ptr() as *mut u32,
                    n_words as i32,
                    std::ptr::null_mut(),
                )
            },
            "launch_chacha20_words",
        )?;
        // SAFETY: hipDeviceSynchronize blocks until the default-stream launch
        // above completes; no preconditions.
        check_hip(
            unsafe { ffi::hip_device_synchronize() },
            "hipDeviceSynchronize",
        )?;
        let mut out = vec![0u32; n_words];
        self.d_out_to_host_u32(&mut out)?;
        Ok(out)
    }

    /// Emits `n_samples` Box-Muller standard-normal `N(0, 1)` f32 samples for a
    /// frame whose noise region begins at `base_word_pos` (a `worker_offset`).
    ///
    /// Sample `s` consumes the 4 ChaCha words at `base_word_pos + 4*s`, matching
    /// the CPU `draw_standard_normal` order. The samples agree with the CPU
    /// Box-Muller transform to <= 1 ulp f32 (design doc §11).
    ///
    /// # Arguments
    ///
    /// * `base_word_pos` — the frame's `worker_offset(...)` (a multiple of 16).
    /// * `n_samples` — number of standard-normal samples; must be `<= capacity`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, synchronization, or D2H failure.
    ///
    /// # Panics
    ///
    /// Panics if `n_samples > capacity`.
    pub fn noise_samples(
        &self,
        base_word_pos: u128,
        n_samples: usize,
    ) -> Result<Vec<f32>, HipError> {
        let mut out = vec![0.0f32; n_samples];
        self.noise_samples_into(base_word_pos, &mut out)?;
        Ok(out)
    }

    /// Emits `out.len()` Box-Muller standard-normal samples into a
    /// caller-provided buffer (no per-call allocation), draining the frame whose
    /// noise region begins at `base_word_pos`.
    ///
    /// Identical semantics to [`noise_samples`](Self::noise_samples) but writes
    /// into `out` so a per-worker host buffer can be reused across frames (the
    /// `gf2-sim` `GpuAwgn` stage threads its scratch buffer here).
    ///
    /// # Arguments
    ///
    /// * `base_word_pos` — the frame's `worker_offset(...)` (a multiple of 16).
    /// * `out` — destination for the samples; `out.len()` must be `<= capacity`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, synchronization, or D2H failure.
    ///
    /// # Panics
    ///
    /// Panics if `out.len() > capacity`.
    pub fn noise_samples_into(&self, base_word_pos: u128, out: &mut [f32]) -> Result<(), HipError> {
        let n_samples = out.len();
        assert!(
            n_samples <= self.capacity,
            "GpuChaChaAwgn::noise_samples_into: n_samples {n_samples} > capacity {}",
            self.capacity
        );
        if n_samples == 0 {
            return Ok(());
        }
        let base = u64::try_from(base_word_pos)
            .expect("base_word_pos exceeds u64 (frame index far beyond any practical run)");
        // SAFETY: `d_key` holds the uploaded 8-word key; `d_out` is sized for
        // `capacity >= n_samples` 4-byte lanes (asserted), reinterpreted as
        // f32. The kernel reads the key and writes exactly `n_samples` f32s.
        check_hip(
            unsafe {
                ffi::launch_chacha20_awgn(
                    self.d_key.as_ptr() as *const u32,
                    base,
                    self.d_out.as_mut_ptr() as *mut f32,
                    n_samples as i32,
                    std::ptr::null_mut(),
                )
            },
            "launch_chacha20_awgn",
        )?;
        // SAFETY: hipDeviceSynchronize blocks until the launch completes.
        check_hip(
            unsafe { ffi::hip_device_synchronize() },
            "hipDeviceSynchronize",
        )?;
        self.d_out_to_host_f32(out)?;
        Ok(())
    }

    /// D2H copy of the leading `dst.len()` u32 output lanes.
    fn d_out_to_host_u32(&self, dst: &mut [u32]) -> Result<(), HipError> {
        // `DeviceBuffer<u32>::copy_to_host` copies `dst.len()` u32s D2H.
        // Borrow a length-checked subslice of the device buffer by copying only
        // the leading `dst.len()` elements (the buffer is `>= dst.len()`).
        self.copy_out::<u32>(dst.as_mut_ptr().cast::<c_void>(), dst.len())
    }

    /// D2H copy of the leading `dst.len()` lanes reinterpreted as f32.
    fn d_out_to_host_f32(&self, dst: &mut [f32]) -> Result<(), HipError> {
        self.copy_out::<f32>(dst.as_mut_ptr().cast::<c_void>(), dst.len())
    }

    /// Shared D2H of `len` 4-byte lanes from `d_out` into `dst_ptr`.
    ///
    /// `T` is `u32` or `f32` (both 4 bytes, no padding); the device buffer
    /// stores raw lanes, so the host reinterprets them by `T`. Routed through
    /// the canonical `ffi::hip_memcpy_d2h` (the same primitive `DeviceBuffer`
    /// uses) so there is no second memcpy implementation.
    fn copy_out<T>(&self, dst_ptr: *mut c_void, len: usize) -> Result<(), HipError> {
        debug_assert!(len <= self.d_out.len());
        let bytes = len * std::mem::size_of::<T>();
        // SAFETY: `d_out` holds at least `len` 4-byte lanes (debug-asserted);
        // `dst_ptr` is a valid host buffer of `len` `T`s supplied by the caller.
        // We copy exactly `bytes` D→H.
        check_hip(
            unsafe { ffi::hip_memcpy_d2h(dst_ptr, self.d_out.as_ptr(), bytes) },
            "hipMemcpy D2H",
        )
    }

    /// Emits `n_samples` noise samples ordered on `stream` instead of the
    /// default stream, leaving the result on the device buffer.
    ///
    /// This is the stream-aware launch the Phase C executor uses to overlap
    /// generation with other GPU work; the caller synchronizes `stream` and
    /// reads back via its own staging. Unlike [`noise_samples`](Self::noise_samples)
    /// this does **not** synchronize or copy back — it only enqueues the launch.
    ///
    /// # Arguments
    ///
    /// * `base_word_pos` — the frame's `worker_offset(...)`.
    /// * `n_samples` — number of samples; must be `<= capacity`.
    /// * `stream` — the stream to order the launch on.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if the kernel fails to launch.
    ///
    /// # Panics
    ///
    /// Panics if `n_samples > capacity`.
    pub fn enqueue_noise_samples(
        &self,
        base_word_pos: u128,
        n_samples: usize,
        stream: &HipStream,
    ) -> Result<(), HipError> {
        assert!(
            n_samples <= self.capacity,
            "GpuChaChaAwgn::enqueue_noise_samples: n_samples {n_samples} > capacity {}",
            self.capacity
        );
        if n_samples == 0 {
            return Ok(());
        }
        let base = u64::try_from(base_word_pos)
            .expect("base_word_pos exceeds u64 (frame index far beyond any practical run)");
        // SAFETY: as in `noise_samples`, but ordered on `stream` (a live HIP
        // stream); the launch returns hipGetLastError and does not synchronize.
        check_hip(
            unsafe {
                ffi::launch_chacha20_awgn(
                    self.d_key.as_ptr() as *const u32,
                    base,
                    self.d_out.as_mut_ptr() as *mut f32,
                    n_samples as i32,
                    stream.as_raw(),
                )
            },
            "launch_chacha20_awgn",
        )
    }
}

// `GpuChaChaAwgn` is `Send` by auto-derive: both fields are `DeviceBuffer<_>`
// (which is `Send`) and the rest are `Copy` scalars. It is deliberately NOT
// `Sync` — its launch methods mutate `d_out` device memory through `&self`, so
// it follows the per-worker-owned-buffer doctrine documented on `DeviceBuffer`.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<GpuChaChaAwgn>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore as _;
    use rand::SeedableRng as _;
    use rand_chacha::ChaCha20Rng;

    /// The host key derivation must match `rand_chacha`'s: a `ChaCha20Rng`
    /// seeded from the same `u64` and read word-by-word must reproduce the
    /// keystream our derived key + block function would (verified end-to-end on
    /// the GPU in the integration test; here we check the derivation is
    /// deterministic and seed-sensitive without a GPU).
    #[test]
    fn test_key_derivation_is_deterministic_and_seed_sensitive() {
        assert_eq!(chacha20_key_from_seed(7), chacha20_key_from_seed(7));
        assert_ne!(chacha20_key_from_seed(7), chacha20_key_from_seed(8));
    }

    /// The first ChaCha word the host RNG produces at position 0 is a pure
    /// function of the seed; record that the RNG is reproducible at a fixed
    /// seed (the device kernel is checked against this same RNG in the
    /// gfx1030-gated integration test `tests/gpu_rng_byte_identity.rs`).
    #[test]
    fn test_host_rng_reproducible_at_seed() {
        let mut a = ChaCha20Rng::seed_from_u64(123);
        let mut b = ChaCha20Rng::seed_from_u64(123);
        for _ in 0..16 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
