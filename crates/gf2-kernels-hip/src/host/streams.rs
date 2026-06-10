//! HIP stream RAII wrapper and a fixed-size stream pool.
//!
//! A [`HipStream`] owns one `hipStream_t` and destroys it on drop. A
//! [`HipStreamPool`] owns `n` streams bound to a device and hands them out by
//! fixed index (deterministic per-worker ownership, the Phase C scheduler
//! model), round-robin (cheap, allocation-free, call-order cursor), or
//! oldest-idle (probes `hipStreamQuery`) acquisition. The pool is the
//! per-`HipDispatcher` resource the design doc §7 multi-GPU seam later
//! replicates per device.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{check_hip, ffi, HipError, HIP_ERROR_NOT_READY};

/// RAII wrapper over a non-default `hipStream_t`.
///
/// The contained handle is created by `hipStreamCreate` in [`HipStream::new`]
/// and destroyed by `hipStreamDestroy` in `Drop`. The handle is an opaque
/// pointer managed by the thread-safe HIP runtime; it is never dereferenced on
/// the host, so [`HipStream`] is both [`Send`] and [`Sync`]. `Sync` is what lets
/// a [`HipStreamPool`] hand out `&HipStream` borrows to concurrent rayon workers
/// (see the pool's `unsafe impl Sync` SAFETY note): none of `HipStream`'s
/// `&self` methods mutate host state — they only issue HIP runtime calls
/// (`hipStreamSynchronize`, `hipStreamQuery`), which the runtime serializes
/// internally, or return the opaque handle.
pub struct HipStream {
    raw: *mut c_void,
}

impl HipStream {
    /// Creates a new HIP stream on the *currently selected* device.
    ///
    /// Callers that need a specific device must `hipSetDevice` first;
    /// [`HipStreamPool::new`] does this before creating its streams.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStream;
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// let stream = HipStream::new().expect("create a HIP stream");
    /// stream.synchronize().expect("drain the (empty) stream");
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStream;
    ///
    /// let stream = HipStream::new().expect("create a HIP stream");
    /// let raw = stream.as_raw(); // hand to a kernel-launch FFI call
    /// assert!(!raw.is_null());
    /// ```
    pub fn as_raw(&self) -> *mut c_void {
        self.raw
    }

    /// Blocks the calling host thread until all work on this stream completes.
    ///
    /// This is the per-stream synchronization used by the drain-for-checkpoint
    /// contract (design doc §4); it does **not** block unrelated streams, unlike
    /// `hipDeviceSynchronize`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStream;
    ///
    /// let stream = HipStream::new().expect("create a HIP stream");
    /// // ... enqueue async work on `stream` ...
    /// stream.synchronize().expect("wait for this stream's work to finish");
    /// ```
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
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStream;
    ///
    /// let stream = HipStream::new().expect("create a HIP stream");
    /// // A freshly created stream has no pending work, so it reports idle.
    /// assert!(stream.is_idle().expect("query the stream"));
    /// ```
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
/// The pool owns `n` streams and hands them out three ways: by fixed index
/// via [`get`](HipStreamPool::get) (deterministic ownership — the Phase C
/// scheduler model), round-robin via [`acquire`](HipStreamPool::acquire)
/// (call-order cursor, for serial or order-insensitive callers), or
/// oldest-idle via [`acquire_idle`](HipStreamPool::acquire_idle) (probes
/// `hipStreamQuery` to prefer a drained stream). All return a borrow whose
/// lifetime is tied to the pool — the executor keeps the pool alive for the
/// duration of a campaign.
///
/// # Thread safety
///
/// `HipStreamPool` is both [`Send`] and [`Sync`] (the latter is
/// auto-derived: its fields are an `i32`, an `AtomicUsize`, and a
/// `Vec<HipStream>` over the `Sync` [`HipStream`]). This is what makes the
/// Phase C scheduler model (`75c22fa8`) sound: a *single* pool is shared by
/// reference (`&HipStreamPool`) across the rayon worker pool, and worker `i`
/// calls [`get(i % len)`](HipStreamPool::get) so it deterministically OWNS
/// that stream regardless of how the thread pool interleaves the workers.
/// The scheduler does **not** use [`acquire`](HipStreamPool::acquire) for
/// worker-stream binding: the atomic cursor advances in *call* order, which
/// under a parallel iterator is scheduler-dependent, so worker `i` would not
/// be guaranteed stream `i % n` and a recorded `stream_id` could describe a
/// different stream than the one used. Two callers may legitimately share a
/// `&HipStream` (e.g. more workers than streams), which is sound precisely
/// because [`HipStream`] is `Sync` (its `&self` methods are read-only or
/// thread-safe HIP calls). No caller ever drives a stream through a mutable
/// alias.
///
/// A [compile-time assertion](self) in the test module enforces the `Send +
/// Sync` bound so the documented concurrency contract cannot silently regress.
pub struct HipStreamPool {
    device_id: i32,
    streams: Vec<HipStream>,
    /// Round-robin cursor. Atomic so `&self` acquisition is usable concurrently
    /// from the rayon worker pool without external locking (see the type-level
    /// "Thread safety" note).
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
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// let pool = HipStreamPool::new(0, 4).expect("create a 4-stream pool");
    /// assert_eq!(pool.len(), 4);
    /// assert_eq!(pool.device_id(), 0);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 2).expect("create a 2-stream pool");
    /// assert_eq!(pool.device_id(), 0);
    /// ```
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Returns the number of streams in the pool.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 3).expect("create a 3-stream pool");
    /// assert_eq!(pool.len(), 3);
    /// ```
    pub fn len(&self) -> usize {
        self.streams.len()
    }

    /// Returns `true` if the pool has no streams. Always `false` for a pool
    /// built by [`HipStreamPool::new`] (which rejects `n == 0`).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 1).expect("create a 1-stream pool");
    /// assert!(!pool.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    /// Acquires the next stream by round-robin.
    ///
    /// Allocation-free and lock-free: advances an atomic cursor modulo the pool
    /// size, so successive calls visit streams in a fixed order **per call
    /// sequence**. Note the cursor orders *calls*, not *callers*: under
    /// concurrent callers (e.g. a parallel iterator) the call interleaving is
    /// scheduler-dependent, so a given caller is not guaranteed any particular
    /// stream. For deterministic per-worker stream ownership use
    /// [`get`](Self::get) instead (the Phase C scheduler model).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 2).expect("create a 2-stream pool");
    /// // Successive acquisitions round-robin through the pool's streams.
    /// let s0 = pool.acquire();
    /// let s1 = pool.acquire();
    /// assert_ne!(s0.as_raw(), s1.as_raw());
    /// // The third acquisition wraps back to the first stream.
    /// assert_eq!(pool.acquire().as_raw(), s0.as_raw());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn acquire(&self) -> &HipStream {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.streams.len();
        &self.streams[idx]
    }

    /// Returns the stream at a fixed index — deterministic worker-to-stream
    /// ownership.
    ///
    /// Unlike [`acquire`](Self::acquire), which advances a shared atomic
    /// cursor in *call* order (scheduler-dependent under concurrent callers),
    /// `get` binds the caller to one specific stream: worker `i` calling
    /// `pool.get(i % pool.len())` owns that stream regardless of how the
    /// thread pool interleaves the calls. The `gf2-sim` hybrid scheduler
    /// (`75c22fa8`) uses this so the `stream_id` it records in tracing spans
    /// is always the stream actually used.
    ///
    /// # Arguments
    ///
    /// * `idx` — the stream index, `0 <= idx < self.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 2).expect("create a 2-stream pool");
    /// // Indexed access is stable: the same index is the same stream.
    /// assert_eq!(pool.get(1).as_raw(), pool.get(1).as_raw());
    /// assert_ne!(pool.get(0).as_raw(), pool.get(1).as_raw());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn get(&self, idx: usize) -> &HipStream {
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
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 4).expect("create a 4-stream pool");
    /// // Prefer a drained stream; falls back to round-robin if all are busy.
    /// let stream = pool.acquire_idle().expect("query streams without a fault");
    /// stream.synchronize().expect("drain before reuse");
    /// ```
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
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::HipStreamPool;
    ///
    /// let pool = HipStreamPool::new(0, 4).expect("create a 4-stream pool");
    /// // ... dispatch work across the pool's streams ...
    /// pool.synchronize_all().expect("drain every stream for a checkpoint");
    /// ```
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

/// Compile-time enforcement of the [`HipStreamPool`] / [`HipStream`]
/// thread-safety contract. These run on every build (no GPU required): if a
/// future change removes `Sync` from `HipStream` (or otherwise breaks the
/// pool's auto-`Sync`), this module fails to compile, catching the regression
/// before it reaches the docs/types-consistency reviewer.
#[cfg(test)]
mod sync_contract {
    use super::*;

    const fn _assert_send<T: Send>() {}
    const fn _assert_sync<T: Sync>() {}

    // The pool is shared by `&` across rayon workers (Phase C scheduler), so it
    // must be both Send and Sync; a single HipStream is shared the same way.
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
