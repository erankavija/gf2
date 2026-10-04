//! Safe host wrappers for the device ChaCha20 + Box-Muller AWGN kernel
//! (`hip/chacha20_awgn.hip`).

use std::ffi::c_void;

use crate::host::{DeviceBuffer, HipStream, PinnedHostBuffer};
use crate::{check_hip, ffi, HipError};

/// Derives the 8-word ChaCha20 key by the PCG32 seed expansion of
/// `SeedableRng::seed_from_u64` in `@/citation/RandCore2025`.
///
/// A PCG32 generator advanced from `seed` emits eight `u32` words through the
/// PCG-XSH-RR output function. `key[i]` is the little-endian `u32` at byte
/// offset `4*i` of the 32-byte seed.
#[must_use]
pub fn chacha20_key_from_seed(seed: u64) -> [u32; 8] {
    // PCG32 constants of `seed_from_u64` in `@/citation/RandCore2025`.
    const MUL: u64 = 6364136223846793005;
    const INC: u64 = 11634580027462260723;

    let mut state = seed;
    let mut key = [0u32; 8];
    for slot in key.iter_mut() {
        state = state.wrapping_mul(MUL).wrapping_add(INC);
        let s = state;
        let xorshifted = (((s >> 18) ^ s) >> 27) as u32;
        let rot = (s >> 59) as u32;
        *slot = xorshifted.rotate_right(rot);
    }
    key
}

/// Pinned host staging for [`GpuChaChaAwgn::noise_samples_into_on_stream`],
/// sized for one generator's `capacity` and `Send`-only: one per worker thread.
pub struct AwgnStreamScratch {
    out: PinnedHostBuffer<u32>,
    capacity: usize,
}

impl AwgnStreamScratch {
    /// The `capacity` (4-byte lanes) of the [`GpuChaChaAwgn`] this scratch was
    /// sized for.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// A reusable device-side ChaCha20 + Box-Muller AWGN noise generator.
///
/// Holds the device-resident ChaCha key and a reusable output buffer.
/// [`raw_words`](Self::raw_words) emits the raw ChaCha 32-bit word stream from
/// an absolute word position; [`noise_samples`](Self::noise_samples) emits
/// Box-Muller standard-normal f32 samples.
pub struct GpuChaChaAwgn {
    d_key: DeviceBuffer<u32>,
    /// Output scratch (`u32` lanes); reinterpreted as `f32` for the noise
    /// kernel since both are 4 bytes wide with identical layout.
    d_out: DeviceBuffer<u32>,
    device_id: i32,
    capacity: usize,
}

impl GpuChaChaAwgn {
    /// Builds a generator for `seed` on `device_id`, with an output buffer of
    /// `capacity` 4-byte lanes: the maximum words or samples per call.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if device allocation or the key upload fails (an
    /// OOM is [`HipError::OutOfMemory`]).
    ///
    /// # Complexity
    ///
    /// O(`capacity`) device memory.
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
    /// position `base_word_pos`.
    ///
    /// `gf2-sim`'s `test_gpu_chacha_raw_words_full_range_byte_identical`
    /// compares the words with a host `ChaCha20Rng::seed_from_u64(seed)`
    /// positioned by `set_word_pos(base_word_pos)`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, synchronization, or D2H failure.
    ///
    /// # Panics
    ///
    /// Panics if `n_words > capacity` or `base_word_pos` exceeds `u64`.
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

    /// Emits `n_samples` Box-Muller standard-normal `N(0, 1)` f32 samples.
    /// Sample `s` consumes the 4 ChaCha words at `base_word_pos + 4*s`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, synchronization, or D2H failure.
    ///
    /// # Panics
    ///
    /// Panics if `n_samples > capacity` or `base_word_pos` exceeds `u64`.
    pub fn noise_samples(
        &self,
        base_word_pos: u128,
        n_samples: usize,
    ) -> Result<Vec<f32>, HipError> {
        let mut out = vec![0.0f32; n_samples];
        self.noise_samples_into(base_word_pos, &mut out)?;
        Ok(out)
    }

    /// [`noise_samples`](Self::noise_samples) into a caller-provided buffer of
    /// `out.len()` samples.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, synchronization, or D2H failure.
    ///
    /// # Panics
    ///
    /// Panics if `out.len() > capacity` or `base_word_pos` exceeds `u64`.
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
        self.copy_out::<u32>(dst.as_mut_ptr().cast::<c_void>(), dst.len())
    }

    /// D2H copy of the leading `dst.len()` lanes reinterpreted as f32.
    fn d_out_to_host_f32(&self, dst: &mut [f32]) -> Result<(), HipError> {
        self.copy_out::<f32>(dst.as_mut_ptr().cast::<c_void>(), dst.len())
    }

    /// D2H of `len` 4-byte lanes from `d_out` into `dst_ptr`; `T` is `u32` or
    /// `f32`.
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

    /// Allocates the pinned host staging
    /// [`noise_samples_into_on_stream`](Self::noise_samples_into_on_stream)
    /// requires.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if the pinned allocation fails (an OOM is
    /// [`HipError::OutOfMemory`]).
    ///
    /// # Complexity
    ///
    /// O(`capacity`) pinned host memory.
    pub fn new_stream_scratch(&self) -> Result<AwgnStreamScratch, HipError> {
        Ok(AwgnStreamScratch {
            out: PinnedHostBuffer::new(self.capacity, self.device_id)?,
            capacity: self.capacity,
        })
    }

    /// Like [`noise_samples_into`](Self::noise_samples_into), with the kernel
    /// launch and the D2H read-back enqueued on the caller-owned `stream` and
    /// completion awaited with [`HipStream::synchronize`]. `scratch` comes
    /// from [`new_stream_scratch`](Self::new_stream_scratch).
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on kernel launch, stream synchronization, or D2H
    /// failure.
    ///
    /// # Panics
    ///
    /// Panics if `out.len() > capacity`, if `base_word_pos` exceeds `u64`, or
    /// if `scratch` was sized for a different generator.
    pub fn noise_samples_into_on_stream(
        &self,
        base_word_pos: u128,
        out: &mut [f32],
        stream: &HipStream,
        scratch: &mut AwgnStreamScratch,
    ) -> Result<(), HipError> {
        assert_eq!(
            scratch.capacity, self.capacity,
            "AwgnStreamScratch capacity {} does not match generator capacity {}",
            scratch.capacity, self.capacity
        );
        let n_samples = out.len();
        if n_samples == 0 {
            return Ok(());
        }
        self.enqueue_noise_samples(base_word_pos, n_samples, stream)?;
        self.d_out.copy_to_pinned_async(&mut scratch.out, stream)?;
        stream.synchronize()?;
        // The device lanes are IEEE-754 f32 bit patterns stored in raw 4-byte
        // lanes; `from_bits` is the exact, bit-preserving reinterpretation.
        for (dst, &bits) in out.iter_mut().zip(scratch.out.as_slice()) {
            *dst = f32::from_bits(bits);
        }
        Ok(())
    }

    /// Enqueues the noise kernel for `n_samples` samples on `stream` and
    /// returns without synchronizing or copying back; the samples stay in the
    /// device buffer.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if the kernel fails to launch.
    ///
    /// # Panics
    ///
    /// Panics if `n_samples > capacity` or `base_word_pos` exceeds `u64`.
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

// `GpuChaChaAwgn` is `Send` by auto-derive; this assertion keeps it so.
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

    #[test]
    fn test_key_derivation_is_deterministic_and_seed_sensitive() {
        assert_eq!(chacha20_key_from_seed(7), chacha20_key_from_seed(7));
        assert_ne!(chacha20_key_from_seed(7), chacha20_key_from_seed(8));
    }

    #[cfg(feature = "hip")]
    #[test]
    fn test_noise_on_stream_matches_default_stream() {
        let capacity = 256usize;
        let gen = GpuChaChaAwgn::new(42, 0, capacity).expect("build generator");
        let stream = HipStream::new().expect("create stream");
        let mut scratch = gen.new_stream_scratch().expect("pinned staging");
        assert_eq!(scratch.capacity(), capacity);

        // Several base positions, including a block-unaligned-frame-style
        // offset, and a partial-capacity request.
        for &(base, n) in &[(0u128, capacity), (16, capacity), (1 << 20, 100)] {
            let default = gen.noise_samples(base, n).expect("default-stream noise");
            let mut streamed = vec![0.0f32; n];
            gen.noise_samples_into_on_stream(base, &mut streamed, &stream, &mut scratch)
                .expect("stream-ordered noise");
            for (s, (d, st)) in default.iter().zip(streamed.iter()).enumerate() {
                assert_eq!(
                    d.to_bits(),
                    st.to_bits(),
                    "sample {s} at base {base} differs: default={d} stream={st}"
                );
            }
        }
    }

    #[test]
    fn test_host_rng_reproducible_at_seed() {
        let mut a = ChaCha20Rng::seed_from_u64(123);
        let mut b = ChaCha20Rng::seed_from_u64(123);
        for _ in 0..16 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
