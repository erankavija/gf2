//! HIP stream RAII wrapper and a fixed-size stream pool.
//!
//! A [`HipStream`] owns one `hipStream_t` and destroys it on drop. A
//! [`HipStreamPool`] owns `n` streams bound to a device and hands them out by
//! fixed index, round-robin, or oldest-idle acquisition.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{check_hip, ffi, HipError, HIP_ERROR_NOT_READY};

/// RAII wrapper over a non-default `hipStream_t`.
///
/// The handle is an opaque pointer managed by the thread-safe HIP runtime and
/// never dereferenced on the host. [`HipStream`] is [`Send`] and [`Sync`]: its
/// `&self` methods only issue HIP runtime calls or return the handle.
pub struct HipStream {
    raw: *mut c_void,
    device_id: i32,
}

impl HipStream {
    /// Creates a new HIP stream on the currently selected device.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if `hipGetDevice` or `hipStreamCreate` fails.
    pub fn new() -> Result<Self, HipError> {
        let mut device_id = 0;
        // SAFETY: `device_id` is a valid writable out-pointer. The HIP runtime
        // writes the currently selected device index on success; recording it
        // binds this stream's host-side metadata to the device it is created on.
        check_hip(
            unsafe { ffi::hip_get_device(&mut device_id) },
            "hipGetDevice",
        )?;
        let mut raw: *mut c_void = ptr::null_mut();
        // SAFETY: `hip_stream_create` writes a valid hipStream_t handle to
        // `raw` on success and leaves it untouched on failure. We pass a valid
        // out-pointer. The handle is freed once in `Drop`.
        check_hip(
            unsafe { ffi::hip_stream_create(&mut raw) },
            "hipStreamCreate",
        )?;
        Ok(Self { raw, device_id })
    }

    /// Returns the raw `hipStream_t` handle for passing to kernel-launch FFI.
    ///
    /// The handle is valid for the lifetime of this [`HipStream`]. Callers must
    /// not destroy it; `Drop` owns that.
    pub fn as_raw(&self) -> *mut c_void {
        self.raw
    }

    /// Returns the HIP device selected when this stream was created.
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Blocks the calling host thread until all work on this stream completes.
    /// Other streams are not waited on.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if `hipStreamSynchronize` fails.
    pub fn synchronize(&self) -> Result<(), HipError> {
        // SAFETY: `self.raw` is a valid stream handle for our lifetime.
        check_hip(
            unsafe { ffi::hip_stream_synchronize(self.raw) },
            "hipStreamSynchronize",
        )
    }

    /// Returns `true` if the stream has no pending work (`hipStreamQuery`
    /// reports `hipSuccess`), `false` if work is still in flight
    /// (`hipErrorNotReady`).
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] for a query failure other than
    /// `hipErrorNotReady`.
    pub fn is_idle(&self) -> Result<bool, HipError> {
        // SAFETY: `self.raw` is a valid stream handle for our lifetime.
        let code = unsafe { ffi::hip_stream_query(self.raw) };
        match code {
            0 => Ok(true),                    // hipSuccess
            HIP_ERROR_NOT_READY => Ok(false), // hipErrorNotReady
            _ => Err(HipError::Hip {
                code,
                context: "hipStreamQuery",
            }),
        }
    }
}

impl Drop for HipStream {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            // SAFETY: `self.raw` was created by `hipStreamCreate` in `new` and
            // is destroyed exactly once here (Drop runs once). We ignore the
            // return code — there is no meaningful recovery from a failed
            // stream destroy during teardown.
            unsafe {
                let _ = ffi::hip_stream_destroy(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

// SAFETY: a hipStream_t is an opaque handle managed by the thread-safe HIP
// runtime and is never dereferenced on the host. Moving it across threads is
// sound; all access goes through HIP API calls that synchronize internally.
unsafe impl Send for HipStream {}

// SAFETY: sharing a `&HipStream` across threads is sound. None of `HipStream`'s
// `&self` methods mutate host-visible state: `as_raw` only copies the opaque
// handle, and `synchronize` / `is_idle` issue `hipStreamSynchronize` /
// `hipStreamQuery`, which the HIP runtime documents as thread-safe for
// concurrent calls (it serializes them internally). `Drop` runs once with
// exclusive ownership, so there is no shared-`&` destroy race. Two threads
// calling these on the SAME stream observe well-defined HIP-runtime behaviour
// (e.g. both block until the stream drains), not a Rust-level data race.
unsafe impl Sync for HipStream {}

/// A fixed-size pool of [`HipStream`]s bound to a single device.
///
/// Streams are handed out by fixed index ([`get`](HipStreamPool::get)),
/// round-robin ([`acquire`](HipStreamPool::acquire)), or oldest-idle
/// ([`acquire_idle`](HipStreamPool::acquire_idle)); each returns a borrow tied
/// to the pool.
///
/// # Thread safety
///
/// `HipStreamPool` is [`Send`] and [`Sync`], so one pool is shared by
/// reference across worker threads. Worker `i` calling
/// [`get(i % len)`](HipStreamPool::get) is bound to one stream regardless of
/// scheduling; the [`acquire`](HipStreamPool::acquire) cursor advances in call
/// order, which is scheduler-dependent under concurrent callers.
pub struct HipStreamPool {
    device_id: i32,
    streams: Vec<HipStream>,
    /// Round-robin cursor; atomic so `&self` acquisition needs no lock.
    next: AtomicUsize,
}

impl HipStreamPool {
    /// Creates a pool of `n` streams on `device_id`, which it selects with
    /// `hipSetDevice` and leaves current.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if `hipSetDevice` or any `hipStreamCreate`
    /// fails; streams created before the failure are destroyed.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`.
    pub fn new(device_id: i32, n: usize) -> Result<Self, HipError> {
        assert!(n > 0, "HipStreamPool::new: n must be non-zero");
        // SAFETY: `hip_set_device` only takes a device index; the runtime
        // validates it and returns an error code we check.
        check_hip(unsafe { ffi::hip_set_device(device_id) }, "hipSetDevice")?;
        let mut streams = Vec::with_capacity(n);
        for _ in 0..n {
            streams.push(HipStream::new()?);
        }
        Ok(Self {
            device_id,
            streams,
            next: AtomicUsize::new(0),
        })
    }

    /// Returns the device this pool's streams are bound to.
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Returns the number of streams in the pool.
    pub fn len(&self) -> usize {
        self.streams.len()
    }

    /// Returns `true` if the pool has no streams. Always `false` for a pool
    /// built by [`HipStreamPool::new`] (which rejects `n == 0`).
    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    /// Acquires the next stream by round-robin.
    ///
    /// An atomic cursor advances modulo the pool size per call. Under
    /// concurrent callers the call order is scheduler-dependent; use
    /// [`get`](Self::get) for a fixed worker-to-stream binding.
    pub fn acquire(&self) -> &HipStream {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.streams.len();
        &self.streams[idx]
    }

    /// Returns the stream at index `idx`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    pub fn get(&self, idx: usize) -> &HipStream {
        &self.streams[idx]
    }

    /// Acquires the oldest idle stream, falling back to round-robin.
    ///
    /// Probes streams in round-robin order and returns the first that
    /// `hipStreamQuery` reports idle. If every stream is busy, the stream at
    /// the round-robin cursor is returned.
    ///
    /// # Errors
    ///
    /// Returns the first non-`hipErrorNotReady` [`HipError`] reported by
    /// `hipStreamQuery` while probing.
    ///
    /// # Complexity
    ///
    /// O(`n`) stream queries in the worst case.
    pub fn acquire_idle(&self) -> Result<&HipStream, HipError> {
        let n = self.streams.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed) % n;
        for offset in 0..n {
            let idx = (start + offset) % n;
            match self.streams[idx].is_idle() {
                Ok(true) => return Ok(&self.streams[idx]),
                Ok(false) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(&self.streams[start])
    }

    /// Synchronizes every stream in the pool.
    ///
    /// # Errors
    ///
    /// Returns the first [`HipError`] encountered; later streams are not
    /// synchronized.
    pub fn synchronize_all(&self) -> Result<(), HipError> {
        for s in &self.streams {
            s.synchronize()?;
        }
        Ok(())
    }
}

/// Compile-time check of the [`HipStreamPool`] / [`HipStream`] `Send + Sync`
/// contract.
#[cfg(test)]
mod sync_contract {
    use super::*;

    const fn _assert_send<T: Send>() {}
    const fn _assert_sync<T: Sync>() {}

    const _: () = {
        _assert_send::<HipStream>();
        _assert_sync::<HipStream>();
        _assert_send::<HipStreamPool>();
        _assert_sync::<HipStreamPool>();
    };
}

#[cfg(all(test, feature = "hip"))]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_round_robin_order() {
        let pool = HipStreamPool::new(0, 3).expect("create 3-stream pool");
        let a = pool.acquire().as_raw();
        let b = pool.acquire().as_raw();
        let c = pool.acquire().as_raw();
        let d = pool.acquire().as_raw();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_eq!(a, d, "round-robin must wrap after n acquisitions");
    }

    #[test]
    fn test_acquire_idle_returns_drained_stream() {
        let pool = HipStreamPool::new(0, 2).expect("create 2-stream pool");
        pool.synchronize_all().expect("drain");
        let s = pool
            .acquire_idle()
            .expect("idle acquisition must not error");
        assert!(
            s.is_idle().expect("query a drained stream"),
            "returned stream must be idle after a full drain"
        );
    }

    #[test]
    fn test_is_idle_on_fresh_stream() {
        let s = HipStream::new().expect("create stream");
        assert!(s.is_idle().expect("query fresh stream is not an error"));
    }
}
