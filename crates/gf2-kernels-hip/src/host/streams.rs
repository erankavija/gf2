//! HIP stream RAII wrapper and a fixed-size stream pool.
//!
//! A [`HipStream`] owns one `hipStream_t` and destroys it on drop. A
//! [`HipStreamPool`] owns `n` streams bound to a device and hands them out by
//! round-robin (cheap, allocation-free) or oldest-idle (probes
//! `hipStreamQuery`) acquisition. The pool is the per-`HipDispatcher` resource
//! the design doc §7 multi-GPU seam later replicates per device.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{check_hip, ffi, HipError, HIP_ERROR_NOT_READY};

/// RAII wrapper over a non-default `hipStream_t`.
///
/// The contained handle is created by `hipStreamCreate` in [`HipStream::new`]
/// and destroyed by `hipStreamDestroy` in `Drop`. The handle is an opaque
/// pointer managed by the thread-safe HIP runtime; it is never dereferenced on
/// the host, so [`HipStream`] is [`Send`].
pub struct HipStream {
    raw: *mut c_void,
}

impl HipStream {
    /// Creates a new HIP stream on the *currently selected* device.
    ///
    /// Callers that need a specific device must `hipSetDevice` first;
    /// [`HipStreamPool::new`] does this before creating its streams.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if `hipStreamCreate` fails.
    pub fn new() -> Result<Self, HipError> {
        let mut raw: *mut c_void = ptr::null_mut();
        // SAFETY: `hip_stream_create` writes a valid hipStream_t handle to
        // `raw` on success and leaves it untouched on failure. We pass a valid
        // out-pointer. The handle is freed once in `Drop`.
        check_hip(
            unsafe { ffi::hip_stream_create(&mut raw) },
            "hipStreamCreate",
        )?;
        Ok(Self { raw })
    }

    /// Returns the raw `hipStream_t` handle for passing to kernel-launch FFI.
    ///
    /// The handle is valid for the lifetime of this [`HipStream`]. Callers must
    /// not destroy it; `Drop` owns that.
    pub fn as_raw(&self) -> *mut c_void {
        self.raw
    }

    /// Blocks the calling host thread until all work on this stream completes.
    ///
    /// This is the per-stream synchronization used by the drain-for-checkpoint
    /// contract (design doc §4); it does **not** block unrelated streams, unlike
    /// `hipDeviceSynchronize`.
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
    /// Any other HIP error is surfaced as `Err`.
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

/// A fixed-size pool of [`HipStream`]s bound to a single device.
///
/// The pool owns `n` streams and hands them out round-robin via
/// [`acquire`](HipStreamPool::acquire) (the default, allocation-free path) or
/// oldest-idle via [`acquire_idle`](HipStreamPool::acquire_idle) (probes
/// `hipStreamQuery` to prefer a drained stream). Both return a borrow whose
/// lifetime is tied to the pool — the executor keeps the pool alive for the
/// duration of a campaign.
pub struct HipStreamPool {
    device_id: i32,
    streams: Vec<HipStream>,
    /// Round-robin cursor. Atomic so `&self` acquisition is usable from the
    /// rayon worker pool without external locking.
    next: AtomicUsize,
}

impl HipStreamPool {
    /// Creates a pool of `n` streams on `device_id`.
    ///
    /// Selects the device with `hipSetDevice` before creating the streams so
    /// every stream is bound to the same device (the design doc §7 multi-GPU
    /// seam replicates one pool per device).
    ///
    /// # Arguments
    ///
    /// * `device_id` - The HIP device the streams are created on.
    /// * `n` - The number of streams to create. Must be non-zero.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if `hipSetDevice` or any `hipStreamCreate`
    /// fails. Streams created before a mid-loop failure are dropped (and thus
    /// destroyed) when the partially built `Vec` unwinds.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0` — an empty pool has no acquirable stream.
    ///
    /// # Complexity
    ///
    /// O(`n`) HIP stream creations.
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
    /// Allocation-free and lock-free: advances an atomic cursor modulo the pool
    /// size. This is the deterministic default — successive calls visit streams
    /// in a fixed, reproducible order, which keeps multi-stream dispatch
    /// consistent with the determinism contract (design doc §11).
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn acquire(&self) -> &HipStream {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.streams.len();
        &self.streams[idx]
    }

    /// Acquires the oldest idle stream, falling back to round-robin.
    ///
    /// Probes streams in round-robin order and returns the first that reports
    /// idle via `hipStreamQuery` (`hipSuccess`). A stream that is merely still
    /// busy (`hipErrorNotReady`) is skipped — that is the normal "not idle"
    /// signal, not an error. If **every** stream is busy, the next round-robin
    /// stream is returned so the caller always makes progress.
    ///
    /// A genuine `hipStreamQuery` failure (any code other than `hipSuccess` or
    /// `hipErrorNotReady`) is **not** swallowed: it propagates as `Err` so a
    /// real HIP runtime fault surfaces instead of being masked as "busy"
    /// (Finding 4 / design doc §8).
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
            // `is_idle` already distinguishes hipSuccess (Ok(true)),
            // hipErrorNotReady (Ok(false) — still busy, skip), and any other
            // code (Err — a real fault we must surface).
            match self.streams[idx].is_idle() {
                Ok(true) => return Ok(&self.streams[idx]),
                Ok(false) => continue,
                Err(e) => return Err(e),
            }
        }
        // None idle, but no query errored: fall back to round-robin progress.
        Ok(&self.streams[start])
    }

    /// Synchronizes every stream in the pool.
    ///
    /// Used at the drain-for-checkpoint boundary (design doc §4) to ensure all
    /// in-flight work has committed before the executor latches worker counters.
    ///
    /// # Errors
    ///
    /// Returns the first [`HipError`] encountered; remaining streams are still
    /// synchronized on the happy path but short-circuit on the first error.
    pub fn synchronize_all(&self) -> Result<(), HipError> {
        for s in &self.streams {
            s.synchronize()?;
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "hip"))]
mod tests {
    use super::*;

    /// Round-robin acquisition visits streams in a fixed cyclic order
    /// (determinism contract, design doc §11). Gated to the gfx1030 host.
    #[test]
    fn test_acquire_round_robin_order() {
        let pool = HipStreamPool::new(0, 3).expect("create 3-stream pool");
        // Record the pointer identity of three successive acquisitions; with a
        // freshly-zeroed cursor they must be streams 0, 1, 2 then wrap to 0.
        let a = pool.acquire().as_raw();
        let b = pool.acquire().as_raw();
        let c = pool.acquire().as_raw();
        let d = pool.acquire().as_raw();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_eq!(a, d, "round-robin must wrap after n acquisitions");
    }

    /// `acquire_idle` returns an idle stream when all streams are drained, and
    /// never errors on the happy path (Finding 4: not-ready is skipped, only a
    /// genuine fault propagates — none occurs here).
    #[test]
    fn test_acquire_idle_returns_drained_stream() {
        let pool = HipStreamPool::new(0, 2).expect("create 2-stream pool");
        pool.synchronize_all().expect("drain");
        // All streams are idle, so this must succeed (Ok), not error.
        let s = pool
            .acquire_idle()
            .expect("idle acquisition must not error");
        assert!(
            s.is_idle().expect("query a drained stream"),
            "returned stream must be idle after a full drain"
        );
    }

    /// A freshly created, never-used stream reports idle (hipSuccess), exercising
    /// the `is_idle` Ok(true) arm — the hipErrorNotReady (Ok(false)) and Err
    /// arms are the not-ready-vs-error distinction acquire_idle relies on.
    #[test]
    fn test_is_idle_on_fresh_stream() {
        let s = HipStream::new().expect("create stream");
        assert!(s.is_idle().expect("query fresh stream is not an error"));
    }
}
