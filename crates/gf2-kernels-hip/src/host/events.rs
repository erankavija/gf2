//! HIP timing events and stream-local event spans: [`HipEvent`] owns one
//! timing-enabled `hipEvent_t`, and [`HipEventSpan`] owns a start/stop pair and
//! reports a device-clock [`std::time::Duration`] only after its stop event is
//! complete. This module deliberately does not call `hipDeviceSynchronize`:
//! callers choose when to synchronize their own [`HipStream`] or poll the stop
//! event.

use std::ffi::c_void;
use std::ptr;
use std::time::Duration;

use crate::host::alloc::{restore_device, select_device};
use crate::host::streams::HipStream;
use crate::{check_hip, ffi, HipError, HIP_ERROR_NOT_READY};

/// An RAII wrapper over a timing-enabled `hipEvent_t`.
pub struct HipEvent {
    raw: *mut c_void,
}

impl HipEvent {
    /// Creates a timing-enabled HIP event on the current HIP device.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if `hipEventCreate` fails.
    pub fn new() -> Result<Self, HipError> {
        let mut raw = ptr::null_mut();
        // SAFETY: `raw` is a valid writable out-pointer. HIP writes a live
        // event handle on success; this wrapper takes sole ownership and its
        // Drop implementation destroys that handle exactly once.
        check_hip(unsafe { ffi::hip_event_create(&mut raw) }, "hipEventCreate")?;
        debug_assert!(
            !raw.is_null(),
            "hipEventCreate succeeded without returning an event handle"
        );
        Ok(Self { raw })
    }

    /// Creates a timing event in `device_id`'s HIP context, selecting the
    /// device for the call and then restoring the device that was current.
    pub(crate) fn new_on_device(device_id: i32) -> Result<Self, HipError> {
        let previous = select_device(device_id)?;
        let event = Self::new();
        let restore_code = restore_device(previous);

        match event {
            Err(error) => Err(error),
            Ok(event) if restore_code != 0 => {
                drop(event);
                Err(HipError::Hip {
                    code: restore_code,
                    context: "hipSetDevice(restore)",
                })
            }
            Ok(event) => Ok(event),
        }
    }

    /// Records this event on `stream` after all preceding stream work.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if the HIP runtime rejects the record call.
    pub fn record(&self, stream: &HipStream) -> Result<(), HipError> {
        // SAFETY: `self.raw` is a live event owned by this value and
        // `stream.as_raw()` is a live stream handle for the borrow's lifetime.
        // Both are passed unchanged to HIP and belong to the caller's context.
        check_hip(
            unsafe { ffi::hip_event_record(self.raw, stream.as_raw()) },
            "hipEventRecord",
        )
    }

    /// Returns whether this event has completed without blocking the host.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] for a HIP query error other than
    /// `hipErrorNotReady`.
    pub fn is_complete(&self) -> Result<bool, HipError> {
        // SAFETY: `self.raw` is a live event owned by this value and cannot be
        // destroyed concurrently because this method only borrows `self`.
        match unsafe { ffi::hip_event_query(self.raw) } {
            0 => Ok(true),
            HIP_ERROR_NOT_READY => Ok(false),
            code => Err(HipError::Hip {
                code,
                context: "hipEventQuery",
            }),
        }
    }

    /// Returns elapsed device time from `start` to this event, which the
    /// caller has observed complete.
    fn elapsed_since(&self, start: &Self) -> Result<Duration, HipError> {
        let mut milliseconds = 0.0_f32;
        // SAFETY: `milliseconds` is a writable f32 out-pointer. Both event
        // handles are live and timing-enabled. The caller has already queried
        // this stop event successfully, satisfying HIP's completion precondition.
        check_hip(
            unsafe { ffi::hip_event_elapsed_time(&mut milliseconds, start.raw, self.raw) },
            "hipEventElapsedTime",
        )?;
        Ok(Duration::from_secs_f64(f64::from(milliseconds) / 1_000.0))
    }
}

impl Drop for HipEvent {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            // SAFETY: `self.raw` came from `hipEventCreate` in `new`, remains
            // solely owned by this value, and Drop runs exactly once. HIP owns
            // any pending device work; the returned teardown error cannot be
            // recovered during Rust destruction.
            unsafe {
                let _ = ffi::hip_event_destroy(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

/// A stream-local start/stop pair measured on HIP's device clock.
///
/// [`elapsed`](Self::elapsed) returns `hipErrorNotReady` until the stop event
/// has completed; it never returns a partial duration.
pub struct HipEventSpan {
    start: HipEvent,
    stop: HipEvent,
}

impl HipEventSpan {
    /// Creates an unrecorded start/stop event pair.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if either event cannot be created.
    pub fn new() -> Result<Self, HipError> {
        let start = HipEvent::new()?;
        let stop = HipEvent::new()?;
        Ok(Self { start, stop })
    }

    /// Creates an unrecorded event pair in `device_id`'s HIP context.
    pub(crate) fn new_on_device(device_id: i32) -> Result<Self, HipError> {
        let start = HipEvent::new_on_device(device_id)?;
        let stop = HipEvent::new_on_device(device_id)?;
        Ok(Self { start, stop })
    }

    /// Records the start marker on the caller-supplied stream.
    pub fn record_start(&self, stream: &HipStream) -> Result<(), HipError> {
        self.start.record(stream)
    }

    /// Records the stop marker on the caller-supplied stream.
    pub fn record_stop(&self, stream: &HipStream) -> Result<(), HipError> {
        self.stop.record(stream)
    }

    /// Raw handle of the start event, valid while the span is alive.
    pub(crate) fn start_raw(&self) -> *mut c_void {
        self.start.raw
    }

    /// Returns whether the span's stop marker has completed.
    pub fn is_complete(&self) -> Result<bool, HipError> {
        self.stop.is_complete()
    }

    /// Returns elapsed device time between the recorded markers.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] with code `hipErrorNotReady` if the stop event
    /// is incomplete.
    pub fn elapsed(&self) -> Result<Duration, HipError> {
        if !self.stop.is_complete()? {
            return Err(HipError::Hip {
                code: HIP_ERROR_NOT_READY,
                context: "hipEventQuery(stop): elapsed time requested before completion",
            });
        }
        self.stop.elapsed_since(&self.start)
    }

    /// Returns elapsed device time from `marker` to this span's start marker.
    pub(crate) fn elapsed_before_start(&self, marker: &HipEvent) -> Result<Duration, HipError> {
        if !self.start.is_complete()? {
            return Err(HipError::Hip {
                code: HIP_ERROR_NOT_READY,
                context: "hipEventQuery(start): device marker requested before completion",
            });
        }
        self.start.elapsed_since(marker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stream can outlive the current-device selection that created it. Event
    /// creation must therefore explicitly target the stream's device before
    /// recording the event on that stream.
    #[test]
    #[ignore = "external: requires two HIP devices"]
    fn scoped_events_record_on_stream_after_restoring_another_device() {
        let mut device_count = 0;
        // SAFETY: `device_count` is a valid writable out-pointer for the HIP
        // runtime's device-count query.
        check_hip(
            unsafe { ffi::hip_device_get_count(&mut device_count) },
            "hipGetDeviceCount",
        )
        .expect("query HIP device count");
        assert!(
            device_count >= 2,
            "this focused context test requires HIP devices 0 and 1"
        );

        let original_device = select_device(0).expect("select ambient device 0");
        let result = (|| -> Result<i32, HipError> {
            let stream_previous = select_device(1)?;
            let stream_result = HipStream::new();
            let stream_restore = restore_device(stream_previous);
            let stream = stream_result?;
            if stream_restore != 0 {
                return Err(HipError::Hip {
                    code: stream_restore,
                    context: "hipSetDevice(restore stream setup)",
                });
            }

            let marker = HipEvent::new_on_device(stream.device_id())?;
            let span = HipEventSpan::new_on_device(stream.device_id())?;

            let mut current_device = -1;
            // SAFETY: `current_device` is a valid writable out-pointer; this
            // query verifies the scoped constructors restored ambient device 0.
            check_hip(
                unsafe { ffi::hip_get_device(&mut current_device) },
                "hipGetDevice",
            )?;

            marker.record(&stream)?;
            span.record_start(&stream)?;
            span.record_stop(&stream)?;
            stream.synchronize()?;
            span.elapsed()?;
            Ok(current_device)
        })();
        let final_restore = restore_device(original_device);

        assert_eq!(final_restore, 0, "restore the test's original HIP device");
        assert_eq!(
            result.expect("create and record device-scoped events"),
            0,
            "scoped event creation must restore the ambient current device"
        );
    }
}
