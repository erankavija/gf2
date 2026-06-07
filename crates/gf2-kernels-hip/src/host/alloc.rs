//! Typed RAII device and pinned-host buffers.
//!
//! [`DeviceBuffer<T>`] owns a `hipMalloc` allocation of `len` `T`s and frees it
//! on drop. [`PinnedHostBuffer<T>`] owns page-locked host memory (`hipHostMalloc`)
//! used to stage H2D / D2H transfers that overlap with kernel execution.
//!
//! Both report out-of-memory as the distinguished [`HipError::OutOfMemory`]
//! variant (carrying `device_id` and `bytes_requested`) rather than a generic
//! error, so the `gf2-sim` executor can catch it and substitute a CPU fallback
//! (design doc §8). The mapping to `gf2_sim::RecoverableError::OutOfMemory`
//! happens at the `gf2-sim` boundary (`crates/gf2-sim/src/gpu/mod.rs`); this
//! crate has no dependency on `gf2-sim`.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr;

use crate::host::streams::HipStream;
use crate::{ffi, HipError, HIP_ERROR_OUT_OF_MEMORY};

/// Returns the free / total device memory (bytes) for the current device.
///
/// Thin safe wrapper over `hipMemGetInfo`, exposed so the allocator can
/// pre-flight large requests and the dispatcher can report headroom.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_hip::host::alloc::device_mem_info;
///
/// // Requires a real HIP device, so this is `no_run`.
/// let (free, total) = device_mem_info().expect("query device memory");
/// assert!(free <= total);
/// ```
///
/// # Errors
///
/// Returns [`HipError::Hip`] if `hipMemGetInfo` fails.
pub fn device_mem_info() -> Result<(usize, usize), HipError> {
    let mut free_bytes: usize = 0;
    let mut total_bytes: usize = 0;
    // SAFETY: both out-pointers are valid; the runtime writes them on success.
    let code = unsafe { ffi::hip_mem_get_info(&mut free_bytes, &mut total_bytes) };
    if code == 0 {
        Ok((free_bytes, total_bytes))
    } else {
        Err(HipError::Hip {
            code,
            context: "hipMemGetInfo",
        })
    }
}

/// An RAII device allocation of `len` values of type `T`.
///
/// The allocation is made by `hipMalloc` and freed by `hipFree` on drop. `T`
/// must be `Copy` and plain-old-data (no destructor is run on device elements;
/// the host never constructs a `T` in the device buffer).
pub struct DeviceBuffer<T> {
    ptr: *mut c_void,
    len: usize,
    device_id: i32,
    _marker: PhantomData<T>,
}

impl<T> DeviceBuffer<T> {
    /// Allocates a device buffer for `len` elements of `T` on `device_id`.
    ///
    /// On allocation failure this returns [`HipError::OutOfMemory`] when the
    /// HIP runtime reports `hipErrorOutOfMemory`, or [`HipError::Hip`] for any
    /// other failure. It never panics on a runtime allocation failure — that is
    /// the contract the executor relies on for OOM substitution (design doc §8).
    ///
    /// # Arguments
    ///
    /// * `len` - Number of `T` elements. `len == 0` is permitted and yields an
    ///   empty buffer with a null device pointer (no `hipMalloc` is issued).
    /// * `device_id` - The HIP device to allocate on. Recorded so the OOM error
    ///   carries it; the caller is responsible for having selected the device.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// let buf = DeviceBuffer::<f32>::new(256, 0).expect("allocate 256 f32 on device 0");
    /// assert_eq!(buf.len(), 256);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`HipError::OutOfMemory`] on `hipErrorOutOfMemory`, otherwise
    /// [`HipError::Hip`].
    ///
    /// # Complexity
    ///
    /// O(1) host-side; one `hipMalloc`.
    pub fn new(len: usize, device_id: i32) -> Result<Self, HipError> {
        let bytes = len.saturating_mul(std::mem::size_of::<T>());
        if len == 0 || bytes == 0 {
            return Ok(Self {
                ptr: ptr::null_mut(),
                len,
                device_id,
                _marker: PhantomData,
            });
        }
        let mut ptr: *mut c_void = ptr::null_mut();
        // SAFETY: `hip_malloc` writes a valid device pointer to `ptr` on
        // success and leaves it null on failure. The pointer is freed once in
        // `Drop`. The runtime validates `bytes`.
        let code = unsafe { ffi::hip_malloc(&mut ptr, bytes) };
        if code == 0 {
            Ok(Self {
                ptr,
                len,
                device_id,
                _marker: PhantomData,
            })
        } else if code == HIP_ERROR_OUT_OF_MEMORY {
            Err(HipError::OutOfMemory {
                device_id,
                bytes_requested: bytes,
            })
        } else {
            Err(HipError::Hip {
                code,
                context: "hipMalloc",
            })
        }
    }

    /// Allocates a device buffer, pre-flighting the request against the device's
    /// reported free memory so an over-large request fails fast as
    /// [`HipError::OutOfMemory`].
    ///
    /// The pre-flight via [`device_mem_info`] catches requests that exceed the
    /// device's *total* memory even when the driver might otherwise lazily
    /// over-commit; the subsequent `hipMalloc` catches genuine pressure. Both
    /// paths surface [`HipError::OutOfMemory`] so the executor can substitute a
    /// CPU fallback (design doc §8) — never a panic.
    ///
    /// # Arguments
    ///
    /// * `len` - Number of `T` elements to allocate.
    /// * `device_id` - The HIP device to allocate on.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    /// use gf2_kernels_hip::HipError;
    ///
    /// // An absurd request fails fast as a recoverable OOM rather than panicking.
    /// let huge = 256usize * 1024 * 1024 * 1024;
    /// match DeviceBuffer::<u8>::new_with_fallback(huge, 0) {
    ///     Err(HipError::OutOfMemory { .. }) => { /* executor substitutes CPU */ }
    ///     Err(other) => panic!("expected OOM, got a different error: {other}"),
    ///     Ok(_) => panic!("a 256 GiB allocation should not succeed"),
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`HipError::OutOfMemory`] if the request exceeds total device
    /// memory or `hipMalloc` reports `hipErrorOutOfMemory`; otherwise
    /// [`HipError::Hip`].
    pub fn new_with_fallback(len: usize, device_id: i32) -> Result<Self, HipError> {
        let bytes = len.saturating_mul(std::mem::size_of::<T>());
        if bytes > 0 {
            // Pre-flight: if the request is larger than the device's total
            // memory, report OOM without troubling the driver. `hipMemGetInfo`
            // errors are tolerated (we fall through to the real hipMalloc).
            if let Ok((_free, total)) = device_mem_info() {
                if bytes > total {
                    return Err(HipError::OutOfMemory {
                        device_id,
                        bytes_requested: bytes,
                    });
                }
            }
        }
        Self::new(len, device_id)
    }

    /// Number of `T` elements the buffer holds.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// let buf = DeviceBuffer::<f32>::new(128, 0).expect("allocate");
    /// assert_eq!(buf.len(), 128);
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer has zero elements.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// // A zero-length buffer is a valid empty handle (no `hipMalloc` issued).
    /// let buf = DeviceBuffer::<f32>::new(0, 0).expect("empty buffer");
    /// assert!(buf.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Size of the allocation in bytes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// let buf = DeviceBuffer::<f32>::new(64, 0).expect("allocate");
    /// assert_eq!(buf.size_bytes(), 64 * std::mem::size_of::<f32>());
    /// ```
    pub fn size_bytes(&self) -> usize {
        self.len * std::mem::size_of::<T>()
    }

    /// The device this buffer was allocated on.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// let buf = DeviceBuffer::<f32>::new(16, 0).expect("allocate on device 0");
    /// assert_eq!(buf.device_id(), 0);
    /// ```
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Raw const device pointer for kernel-launch FFI.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// let buf = DeviceBuffer::<f32>::new(16, 0).expect("allocate");
    /// let ptr = buf.as_ptr(); // hand to a kernel-launch FFI argument
    /// assert!(!ptr.is_null());
    /// ```
    pub fn as_ptr(&self) -> *const c_void {
        self.ptr as *const c_void
    }

    /// Raw mut device pointer for kernel-launch FFI.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// let buf = DeviceBuffer::<f32>::new(16, 0).expect("allocate");
    /// let ptr = buf.as_mut_ptr(); // hand to a kernel-launch output argument
    /// assert!(!ptr.is_null());
    /// ```
    pub fn as_mut_ptr(&self) -> *mut c_void {
        self.ptr
    }

    /// Copies `src` (host) into this device buffer (synchronous H2D).
    ///
    /// # Arguments
    ///
    /// * `src` - Host slice to upload; `src.len()` must be `<= self.len()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// let buf = DeviceBuffer::<f32>::new(4, 0).expect("allocate");
    /// buf.copy_from_host(&[1.0, 2.0, 3.0, 4.0]).expect("upload H2D");
    ///
    /// let mut out = [0.0f32; 4];
    /// buf.copy_to_host(&mut out).expect("download D2H");
    /// assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `src.len() > self.len()`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] on memcpy failure.
    pub fn copy_from_host(&self, src: &[T]) -> Result<(), HipError>
    where
        T: Copy,
    {
        assert!(
            src.len() <= self.len,
            "DeviceBuffer::copy_from_host: src.len() {} > buffer len {}",
            src.len(),
            self.len
        );
        if src.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::size_of_val(src);
        // SAFETY: `self.ptr` is a valid device allocation of at least `bytes`
        // (src.len() <= self.len asserted). `src` is a valid host slice; we copy
        // exactly its byte length H→D.
        let code = unsafe { ffi::hip_memcpy_h2d(self.ptr, src.as_ptr() as *const c_void, bytes) };
        crate::check_hip(code, "hipMemcpy H2D")
    }

    /// Copies this device buffer into `dst` (host, synchronous D2H).
    ///
    /// # Arguments
    ///
    /// * `dst` - Host slice to fill; `dst.len()` must be `<= self.len()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::DeviceBuffer;
    ///
    /// let buf = DeviceBuffer::<u32>::new(3, 0).expect("allocate");
    /// buf.copy_from_host(&[10, 20, 30]).expect("upload H2D");
    ///
    /// let mut out = [0u32; 3];
    /// buf.copy_to_host(&mut out).expect("download D2H");
    /// assert_eq!(out, [10, 20, 30]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `dst.len() > self.len()`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] on memcpy failure.
    pub fn copy_to_host(&self, dst: &mut [T]) -> Result<(), HipError>
    where
        T: Copy,
    {
        assert!(
            dst.len() <= self.len,
            "DeviceBuffer::copy_to_host: dst.len() {} > buffer len {}",
            dst.len(),
            self.len
        );
        if dst.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::size_of_val(dst);
        // SAFETY: `self.ptr` is a valid device allocation of at least `bytes`.
        // `dst` is a valid mutable host slice; we copy exactly its byte length
        // D→H.
        let code = unsafe { ffi::hip_memcpy_d2h(dst.as_mut_ptr() as *mut c_void, self.ptr, bytes) };
        crate::check_hip(code, "hipMemcpy D2H")
    }

    /// Enqueues a stream-ordered H2D copy from a pinned host buffer.
    ///
    /// The copy is asynchronous: it is ordered on `stream` and returns as soon
    /// as it is enqueued. The caller must keep `src` alive and not mutate it
    /// until the stream is synchronized (`stream.synchronize()`). Staging
    /// through pinned host memory is what lets the transfer overlap with kernel
    /// work on other streams (design doc §6).
    ///
    /// # Arguments
    ///
    /// * `src` - Pinned host source; `src.len()` must be `<= self.len()`.
    /// * `stream` - The stream the copy is ordered on.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::{DeviceBuffer, HipStream, PinnedHostBuffer};
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// let stream = HipStream::new().expect("create a stream");
    /// let mut staging = PinnedHostBuffer::<f32>::new(4, 0).expect("pinned host");
    /// staging.as_mut_slice().copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
    ///
    /// let dev = DeviceBuffer::<f32>::new(4, 0).expect("device buffer");
    /// dev.copy_from_pinned_async(&staging, &stream).expect("enqueue H2D");
    /// stream.synchronize().expect("wait for the transfer");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `src.len() > self.len()`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if the async memcpy fails to enqueue.
    pub fn copy_from_pinned_async(
        &self,
        src: &PinnedHostBuffer<T>,
        stream: &HipStream,
    ) -> Result<(), HipError>
    where
        T: Copy + Default,
    {
        assert!(
            src.len() <= self.len,
            "DeviceBuffer::copy_from_pinned_async: src.len() {} > buffer len {}",
            src.len(),
            self.len
        );
        if src.is_empty() {
            return Ok(());
        }
        let bytes = src.len() * std::mem::size_of::<T>();
        // SAFETY: `self.ptr` is a valid device allocation of at least `bytes`;
        // `src` is pinned host memory of exactly `src.len()` elements; `stream`
        // is a live HIP stream. The HIP runtime owns the async copy until the
        // stream is synchronized.
        let code =
            unsafe { ffi::hip_memcpy_h2d_async(self.ptr, src.as_ptr(), bytes, stream.as_raw()) };
        crate::check_hip(code, "hipMemcpyAsync H2D")
    }

    /// Enqueues a stream-ordered D2H copy into a pinned host buffer.
    ///
    /// Asynchronous, ordered on `stream`. The destination data is valid only
    /// after `stream.synchronize()` returns.
    ///
    /// # Arguments
    ///
    /// * `dst` - Pinned host destination; `dst.len()` must be `<= self.len()`.
    /// * `stream` - The stream the copy is ordered on.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::{DeviceBuffer, HipStream, PinnedHostBuffer};
    ///
    /// let stream = HipStream::new().expect("create a stream");
    /// let dev = DeviceBuffer::<f32>::new(4, 0).expect("device buffer");
    /// let mut staging = PinnedHostBuffer::<f32>::new(4, 0).expect("pinned host");
    ///
    /// dev.copy_to_pinned_async(&mut staging, &stream).expect("enqueue D2H");
    /// stream.synchronize().expect("wait for the transfer");
    /// // `staging.as_slice()` is now valid to read.
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `dst.len() > self.len()`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if the async memcpy fails to enqueue.
    pub fn copy_to_pinned_async(
        &self,
        dst: &mut PinnedHostBuffer<T>,
        stream: &HipStream,
    ) -> Result<(), HipError>
    where
        T: Copy + Default,
    {
        assert!(
            dst.len() <= self.len,
            "DeviceBuffer::copy_to_pinned_async: dst.len() {} > buffer len {}",
            dst.len(),
            self.len
        );
        if dst.is_empty() {
            return Ok(());
        }
        let bytes = dst.len() * std::mem::size_of::<T>();
        // SAFETY: `self.ptr` is a valid device allocation of at least `bytes`;
        // `dst` is pinned host memory of exactly `dst.len()` elements; `stream`
        // is a live HIP stream. The destination is valid once the stream is
        // synchronized.
        let code = unsafe {
            ffi::hip_memcpy_d2h_async(
                dst.as_mut_ptr(),
                self.ptr as *const c_void,
                bytes,
                stream.as_raw(),
            )
        };
        crate::check_hip(code, "hipMemcpyAsync D2H")
    }
}

impl<T> Drop for DeviceBuffer<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: `self.ptr` was allocated by `hip_malloc` in `new` and is
            // freed exactly once here (Drop runs once). Return code ignored —
            // no recovery during teardown.
            unsafe {
                let _ = ffi::hip_free(self.ptr);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

// SAFETY: device pointers are opaque handles managed by the thread-safe HIP
// runtime and never dereferenced on the host. Moving across threads is sound.
//
// `DeviceBuffer` is deliberately `Send`-only and NOT `Sync`: `copy_from_host`
// and `copy_from_pinned_async` mutate device memory through a *shared* `&self`,
// so handing two threads a `&DeviceBuffer` to the SAME buffer would allow
// concurrent H2D writes — a genuine data race. The concurrency model is
// per-worker-owned buffers (each worker owns its own `DeviceBuffer`, moved in
// via `Send`), never a buffer shared by `&` across workers.
unsafe impl<T: Send> Send for DeviceBuffer<T> {}

/// An RAII page-locked (pinned) host allocation of `len` values of `T`.
///
/// Pinned host memory enables asynchronous, overlap-capable `hipMemcpyAsync`
/// transfers. Allocated by `hipHostMalloc`, freed by `hipHostFree` on drop.
pub struct PinnedHostBuffer<T> {
    ptr: *mut T,
    len: usize,
}

impl<T: Copy + Default> PinnedHostBuffer<T> {
    /// Allocates `len` pinned (page-locked) host elements, zero-initialized.
    ///
    /// # Arguments
    ///
    /// * `len` - Number of `T` elements to page-lock. `len == 0` yields an empty
    ///   buffer with a null pointer (no `hipHostMalloc` is issued).
    /// * `device_id` - Recorded so an OOM error carries it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// let staging = PinnedHostBuffer::<f32>::new(256, 0).expect("pinned host alloc");
    /// assert_eq!(staging.len(), 256);
    /// // Freshly allocated pinned memory is zero-initialized.
    /// assert!(staging.as_slice().iter().all(|&x| x == 0.0));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`HipError::OutOfMemory`] on `hipErrorOutOfMemory`, otherwise
    /// [`HipError::Hip`].
    ///
    /// # Complexity
    ///
    /// O(`len`) to zero-initialize after a single `hipHostMalloc`.
    pub fn new(len: usize, device_id: i32) -> Result<Self, HipError> {
        let bytes = len.saturating_mul(std::mem::size_of::<T>());
        if len == 0 || bytes == 0 {
            return Ok(Self {
                ptr: ptr::null_mut(),
                len,
            });
        }
        let mut raw: *mut c_void = ptr::null_mut();
        // SAFETY: `hip_host_malloc` writes a valid pinned host pointer to `raw`
        // on success; freed once in `Drop`.
        let code = unsafe { ffi::hip_host_malloc(&mut raw, bytes) };
        if code == HIP_ERROR_OUT_OF_MEMORY {
            return Err(HipError::OutOfMemory {
                device_id,
                bytes_requested: bytes,
            });
        }
        crate::check_hip(code, "hipHostMalloc")?;
        let ptr = raw as *mut T;
        // SAFETY: `ptr` points to `bytes == len * size_of::<T>()` valid,
        // suitably aligned, freshly allocated (pinned) host bytes. Writing
        // `T::default()` into each `len` slot initializes the region; `T: Copy`
        // means no drop semantics are skipped.
        unsafe {
            for i in 0..len {
                ptr.add(i).write(T::default());
            }
        }
        Ok(Self { ptr, len })
    }

    /// Number of `T` elements.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// let staging = PinnedHostBuffer::<f32>::new(64, 0).expect("pinned host alloc");
    /// assert_eq!(staging.len(), 64);
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer has zero elements.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// let staging = PinnedHostBuffer::<f32>::new(0, 0).expect("empty pinned host");
    /// assert!(staging.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Borrows the pinned region as a slice.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// let staging = PinnedHostBuffer::<u8>::new(8, 0).expect("pinned host alloc");
    /// assert_eq!(staging.as_slice().len(), 8);
    /// ```
    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() {
            return &[];
        }
        // SAFETY: `ptr` is valid for `len` initialized `T`s for our lifetime;
        // the borrow is tied to `&self`.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Mutably borrows the pinned region as a slice.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// let mut staging = PinnedHostBuffer::<f32>::new(4, 0).expect("pinned host alloc");
    /// staging.as_mut_slice().copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
    /// assert_eq!(staging.as_slice(), &[1.0, 2.0, 3.0, 4.0]);
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.ptr.is_null() {
            return &mut [];
        }
        // SAFETY: `ptr` is valid for `len` initialized `T`s; the mutable borrow
        // is tied to `&mut self`, so no aliasing.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    /// Raw const pointer for `hipMemcpyAsync` FFI.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// let staging = PinnedHostBuffer::<f32>::new(16, 0).expect("pinned host alloc");
    /// let ptr = staging.as_ptr(); // hand to a `hipMemcpyAsync` source argument
    /// assert!(!ptr.is_null());
    /// ```
    pub fn as_ptr(&self) -> *const c_void {
        self.ptr as *const c_void
    }

    /// Raw mut pointer for `hipMemcpyAsync` FFI.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::PinnedHostBuffer;
    ///
    /// let mut staging = PinnedHostBuffer::<f32>::new(16, 0).expect("pinned host alloc");
    /// let ptr = staging.as_mut_ptr(); // hand to a `hipMemcpyAsync` dest argument
    /// assert!(!ptr.is_null());
    /// ```
    pub fn as_mut_ptr(&mut self) -> *mut c_void {
        self.ptr as *mut c_void
    }
}

impl<T> Drop for PinnedHostBuffer<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: `self.ptr` was allocated by `hipHostMalloc` in `new` and
            // is freed exactly once here. The elements are `Copy`, so no
            // per-element drop is required.
            unsafe {
                let _ = ffi::hip_host_free(self.ptr as *mut c_void);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

// SAFETY: the pinned host pointer is owned exclusively by this buffer; moving
// it across threads is sound for `T: Send` because no aliasing handle escapes.
//
// `PinnedHostBuffer` is deliberately `Send`-only and NOT `Sync`. It is a
// staging buffer mutated in place (`as_mut_slice`, async D2H into it) and is
// owned per-worker (e.g. inside a `HipDispatcher`'s `StageScratch`), never
// shared by `&` across workers. Keeping it `Send`-only matches the buffer
// concurrency doctrine documented on `DeviceBuffer` and keeps `HipDispatcher`
// (which embeds one) from auto-deriving a `Sync` its scratch cannot honour.
unsafe impl<T: Send> Send for PinnedHostBuffer<T> {}

#[cfg(all(test, feature = "hip"))]
mod tests {
    use super::*;
    use crate::host::streams::HipStream;

    /// A zero-length DeviceBuffer is a valid empty handle with no allocation
    /// (no GPU required for the empty path).
    #[test]
    fn test_device_buffer_empty_is_null() {
        let buf = DeviceBuffer::<f32>::new(0, 0).expect("empty buffer");
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.as_ptr().is_null());
    }

    /// Synchronous H2D then D2H through a DeviceBuffer round-trips the payload
    /// byte-for-byte (Finding 7). Gated to the gfx1030 host.
    #[test]
    fn test_device_buffer_h2d_d2h_roundtrip() {
        let src: Vec<f32> = (0..64).map(|i| i as f32 * 1.5).collect();
        let buf = DeviceBuffer::<f32>::new(src.len(), 0).expect("alloc device buffer");
        buf.copy_from_host(&src).expect("H2D");
        let mut dst = vec![0.0f32; src.len()];
        buf.copy_to_host(&mut dst).expect("D2H");
        assert_eq!(src, dst, "device round-trip must be byte-identical");
    }

    /// Staged async round-trip: host → pinned → device → pinned → host over a
    /// stream, synchronized before reading back (Finding 7).
    #[test]
    fn test_pinned_async_roundtrip_over_stream() {
        let n = 32usize;
        let stream = HipStream::new().expect("create stream");
        let mut h2d = PinnedHostBuffer::<f32>::new(n, 0).expect("pinned in");
        for (i, slot) in h2d.as_mut_slice().iter_mut().enumerate() {
            *slot = i as f32 - 7.0;
        }
        let dev = DeviceBuffer::<f32>::new(n, 0).expect("device buffer");
        dev.copy_from_pinned_async(&h2d, &stream)
            .expect("async H2D");
        let mut d2h = PinnedHostBuffer::<f32>::new(n, 0).expect("pinned out");
        dev.copy_to_pinned_async(&mut d2h, &stream)
            .expect("async D2H");
        stream.synchronize().expect("drain stream");
        assert_eq!(
            h2d.as_slice(),
            d2h.as_slice(),
            "staged async round-trip must preserve the payload"
        );
    }

    /// `new_with_fallback` pre-flights an over-large request against total
    /// device memory and reports a structured OOM (not a panic).
    #[test]
    fn test_device_buffer_oom_is_structured() {
        let huge = 256usize * 1024 * 1024 * 1024; // 256 GiB of u8
        let err = DeviceBuffer::<u8>::new_with_fallback(huge, 0)
            .err()
            .expect("256 GiB allocation must fail");
        match err {
            HipError::OutOfMemory {
                bytes_requested, ..
            } => assert_eq!(bytes_requested, huge),
            other => panic!("expected structured OOM, got {other}"),
        }
    }
}
