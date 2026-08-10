//! Device-gated event-timing coverage for the permanent launch boundary.
//!
//! Run on the ROCm gfx1030 host with:
//!
//! ```text
//! cargo test --manifest-path crates/gf2-kernels-hip/Cargo.toml --release \
//!   --features hip -- --ignored permanent_event_timing
//! ```

#![cfg(feature = "hip")]

use std::time::{Duration, Instant};

use gf2_kernels_hip::host::HipStream;
use gf2_kernels_hip::permanent::{dispatch_permanent_batch_instrumented, PermanentField};

/// One full event-instrumented F_3 dispatch yields positive H2D, kernel, and
/// D2H durations. Their device-clock sum fits within the host wall clock of
/// this synchronous (finish-and-read) dispatch, while the kernel-only span is
/// strictly smaller than that enclosing end-to-end time. The separately
/// reported launch marker span starts before host submission and ends at the
/// C++ wrapper's event record immediately before kernel submission.
#[test]
#[ignore = "external: gfx1030 HIP device required for permanent event timing"]
fn permanent_event_timing_reports_positive_phase_spans() {
    // n=16 has a substantial, known 2^16-step Gray walk per matrix without
    // making this device-only timing test impractically long. The batch also
    // makes the transfer spans comfortably above HIP event clock granularity.
    let n = 16usize;
    let m = 1_024usize;
    let matrices: Vec<u8> = (0..m * n * n).map(|index| (index % 3) as u8).collect();
    let stream = HipStream::new().expect("create timing stream");

    let wall_started = Instant::now();
    let dispatch =
        dispatch_permanent_batch_instrumented(PermanentField::F3, &matrices, n, m, &stream)
            .expect("enqueue timed permanent dispatch");
    let (output, timing) = dispatch.finish().expect("finish timed permanent dispatch");
    let wall = wall_started.elapsed();

    assert_eq!(output.len(), m, "one permanent result per input matrix");

    let h2d = timing.h2d.expect("this boundary submitted H2D");
    let kernel = timing.kernel.expect("this boundary submitted a kernel");
    let d2h = timing.d2h.expect("this boundary submitted D2H");
    assert!(h2d > Duration::ZERO, "H2D event span must be positive");
    assert!(
        kernel > Duration::ZERO,
        "kernel event span must be positive"
    );
    assert!(d2h > Duration::ZERO, "D2H event span must be positive");
    assert!(
        kernel < wall,
        "kernel-only device span must be smaller than the enclosing synchronous dispatch"
    );
    assert!(
        h2d + kernel + d2h <= wall,
        "serialized device phase spans must fit within the enclosing wall clock"
    );

    assert!(
        timing.device_submission_to_kernel.is_some(),
        "the wrapper-recorded pre-submit-to-kernel-start device span is reported independently"
    );
}
