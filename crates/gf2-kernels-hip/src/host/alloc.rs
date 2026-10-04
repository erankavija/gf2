//! Typed RAII device and pinned-host buffers: [`DeviceBuffer<T>`] owns a
//! `hipMalloc` allocation and [`PinnedHostBuffer<T>`] owns page-locked host
//! memory (`hipHostMalloc`) for stream-ordered transfers. Both report
//! out-of-memory as [`HipError::OutOfMemory`], carrying `device_id` and
//! `bytes_requested`.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr;

use crate::host::streams::HipStream;
use crate::{ffi, HipError, HIP_ERROR_OUT_OF_MEMORY};

/// Returns the free / total device memory (bytes) for the device the calling
/// host thread last selected; [`device_mem_info_for`] queries a device by id.
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

/// Returns the free / total device memory (bytes) for `device_id`.
///
/// Selects `device_id` around the query and restores the device that was
/// current even when the query fails; if the initial `hipGetDevice` fails, no
/// device switch is performed.
///
/// # Errors
///
/// Returns [`HipError::Hip`] if `hipGetDevice`, `hipSetDevice`, or
/// `hipMemGetInfo` fails.
pub fn device_mem_info_for(device_id: i32) -> Result<(usize, usize), HipError> {
    let prev = select_device(device_id)?;

    let mut free_bytes: usize = 0;
    let mut total_bytes: usize = 0;
    // SAFETY: both out-pointers are valid; the runtime writes them on success.
    let query_code = unsafe { ffi::hip_mem_get_info(&mut free_bytes, &mut total_bytes) };

    let restore_code = restore_device(prev);

    if query_code != 0 {
        return Err(HipError::Hip {
            code: query_code,
            context: "hipMemGetInfo",
        });
    }
    if restore_code != 0 {
        return Err(HipError::Hip {
            code: restore_code,
            context: "hipSetDevice(restore)",
        });
    }
    Ok((free_bytes, total_bytes))
}

/// Selects `device_id` as the current HIP device and returns the device that
/// was current, for [`restore_device`].
///
/// If `hipGetDevice` fails no switch is performed; if `hipSetDevice` fails the
/// current device is unchanged.
pub(crate) fn select_device(device_id: i32) -> Result<i32, HipError> {
    let mut prev: i32 = 0;
    // SAFETY: `&mut prev` is a valid out-pointer the runtime writes on success.
    let code = unsafe { ffi::hip_get_device(&mut prev) };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "hipGetDevice",
        });
    }
    // SAFETY: `hip_set_device` takes a device index; the runtime validates it.
    let code = unsafe { ffi::hip_set_device(device_id) };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "hipSetDevice",
        });
    }
    Ok(prev)
}

/// Restores `prev` as the current HIP device and returns the raw status code
/// (`0` = success), so the caller applies its own error precedence.
pub(crate) fn restore_device(prev: i32) -> i32 {
    // SAFETY: `prev` was obtained from `hipGetDevice`, so it is a valid index.
    unsafe { ffi::hip_set_device(prev) }
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
    /// `device_id` is selected for the `hipMalloc` and the device that was
    /// current is restored afterward. `len == 0` yields an empty buffer with a
    /// null device pointer and issues no `hipMalloc`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::OutOfMemory`] on `hipErrorOutOfMemory`, otherwise
    /// [`HipError::Hip`].
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

        let prev = select_device(device_id)?;

        let mut ptr: *mut c_void = ptr::null_mut();
        // SAFETY: `hip_malloc` writes a valid device pointer to `ptr` on success
        // and leaves it null on failure. The pointer is freed once in `Drop`. The
        // runtime validates `bytes`. The allocation lands on `device_id` (just
        // selected above).
        let alloc_code = unsafe { ffi::hip_malloc(&mut ptr, bytes) };

        let restore_code = restore_device(prev);

        if alloc_code == HIP_ERROR_OUT_OF_MEMORY {
            return Err(HipError::OutOfMemory {
                device_id,
                bytes_requested: bytes,
            });
        }
        if alloc_code != 0 {
            return Err(HipError::Hip {
                code: alloc_code,
                context: "hipMalloc",
            });
        }

        // Bind the buffer before checking the restore status so that a failed
        // restore frees the allocation on the error path.
        let buf = Self {
            ptr,
            len,
            device_id,
            _marker: PhantomData,
        };
        if restore_code != 0 {
            return Err(HipError::Hip {
                code: restore_code,
                context: "hipSetDevice(restore)",
            });
        }
        Ok(buf)
    }

    /// Allocates a device buffer after checking the request against the total
    /// memory of `device_id`, so a request larger than the device fails as
    /// [`HipError::OutOfMemory`] without reaching `hipMalloc`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::OutOfMemory`] if the request exceeds total device
    /// memory or `hipMalloc` reports `hipErrorOutOfMemory`; otherwise
    /// [`HipError::Hip`].
    pub fn new_with_fallback(len: usize, device_id: i32) -> Result<Self, HipError> {
        let bytes = len.saturating_mul(std::mem::size_of::<T>());
        if bytes > 0 {
            // A `hipMemGetInfo` error is tolerated: the request falls through
            // to `hipMalloc`.
            if let Ok((_free, total)) = device_mem_info_for(device_id) {
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
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer has zero elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Size of the allocation in bytes.
    pub fn size_bytes(&self) -> usize {
        self.len * std::mem::size_of::<T>()
    }

    /// The device this buffer was allocated on.
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Raw const device pointer for kernel-launch FFI.
    pub fn as_ptr(&self) -> *const c_void {
        self.ptr as *const c_void
    }

    /// Raw mut device pointer for kernel-launch FFI.
    pub fn as_mut_ptr(&self) -> *mut c_void {
        self.ptr
    }

    /// Copies `src` (host) into this device buffer (synchronous H2D).
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
    /// Returns as soon as the copy is enqueued. The caller must keep `src`
    /// alive and unmodified until `stream` is synchronized.
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
    /// The destination data is valid only after `stream.synchronize()`
    /// returns.
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
// `DeviceBuffer` is not `Sync`: `copy_from_host` and `copy_from_pinned_async`
// write device memory through `&self`, so two threads sharing a `&DeviceBuffer`
// could race on H2D writes. Each worker owns its buffers.
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
    /// Allocates `len` pinned (page-locked) host elements, each initialized to
    /// `T::default()`.
    ///
    /// `len == 0` yields an empty buffer with a null pointer and issues no
    /// `hipHostMalloc`. `device_id` is carried by an OOM error only.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::OutOfMemory`] on `hipErrorOutOfMemory`, otherwise
    /// [`HipError::Hip`].
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
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer has zero elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Borrows the pinned region as a slice.
    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() {
            return &[];
        }
        // SAFETY: `ptr` is valid for `len` initialized `T`s for our lifetime;
        // the borrow is tied to `&self`.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Mutably borrows the pinned region as a slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.ptr.is_null() {
            return &mut [];
        }
        // SAFETY: `ptr` is valid for `len` initialized `T`s; the mutable borrow
        // is tied to `&mut self`, so no aliasing.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    /// Raw const pointer for `hipMemcpyAsync` FFI.
    pub fn as_ptr(&self) -> *const c_void {
        self.ptr as *const c_void
    }

    /// Raw mut pointer for `hipMemcpyAsync` FFI.
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
// `PinnedHostBuffer` is not `Sync`: it is a staging buffer mutated in place
// and owned per worker.
unsafe impl<T: Send> Send for PinnedHostBuffer<T> {}

#[cfg(all(test, feature = "hip"))]
mod tests {
    use super::*;
    use crate::host::streams::HipStream;

    #[test]
    fn test_device_buffer_empty_is_null() {
        let buf = DeviceBuffer::<f32>::new(0, 0).expect("empty buffer");
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.as_ptr().is_null());
    }

    #[test]
    fn test_device_buffer_h2d_d2h_roundtrip() {
        let src: Vec<f32> = (0..64).map(|i| i as f32 * 1.5).collect();
        let buf = DeviceBuffer::<f32>::new(src.len(), 0).expect("alloc device buffer");
        buf.copy_from_host(&src).expect("H2D");
        let mut dst = vec![0.0f32; src.len()];
        buf.copy_to_host(&mut dst).expect("D2H");
        assert_eq!(src, dst, "device round-trip must be byte-identical");
    }

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

    #[test]
    fn test_device_buffer_oom_is_structured() {
        let huge = 256usize * 1024 * 1024 * 1024; // 256 GiB of u8
        let err = DeviceBuffer::<u8>::new_with_fallback(huge, 0)
            .err()
            .expect("256 GiB allocation must fail");
        match err {
            HipError::OutOfMemory {
                bytes_requested,
                device_id,
            } => {
                assert_eq!(bytes_requested, huge);
                assert_eq!(device_id, 0, "OOM must carry the requested device id");
            }
            other => panic!("expected structured OOM, got {other}"),
        }
    }

    /// Assumes a single-GPU host, where device 0 is the only valid index.
    #[test]
    fn test_device_mem_info_for_honors_device_id() {
        let (free0, total0) = device_mem_info_for(0).expect("device 0 mem info");
        assert!(free0 <= total0, "free must not exceed total");
        assert!(total0 > 0, "a real GPU reports nonzero total memory");

        let (_free_cur, total_cur) = device_mem_info().expect("current device mem info");
        assert_eq!(
            total_cur, total0,
            "device 0 must remain current after the scoped query"
        );
    }
}
