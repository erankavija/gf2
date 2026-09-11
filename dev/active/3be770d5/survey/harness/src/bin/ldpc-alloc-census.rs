//! Allocation census of the gf2 steady-state decode (jit:3be770d5).
//!
//! Decodes the first `--batch` recorded frames through one reused
//! [`Gf2Worker`], exactly as a throughput worker does, under a counting
//! global allocator enabled only around each frame's decode. It prints one
//! JSON record per frame: allocations, reallocations, deallocations, bytes
//! requested, the frame's iteration count, and a histogram of requested sizes.
//! Counts are deterministic for a fixed decoder and input, so the record is
//! exact evidence of which allocation sizes a frame and an iteration request,
//! not a timing.
//!
//! Usage:
//!   ldpc-alloc-census --bundle DIR --code NAME [--batch 16]
//!       [--iteration-cap 50] [--norm 0.75]

use ldpc_survey::arm::DecoderCase;
use ldpc_throughput::gf2::{config, Gf2Worker};
use ldpc_throughput::workload::Workload;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Sizes up to this many bytes are counted individually; larger requests
/// share the final bucket.
const EXACT_SIZES: usize = 16384;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static SIZES: [AtomicU64; EXACT_SIZES + 1] = [const { AtomicU64::new(0) }; EXACT_SIZES + 1];

fn record(size: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(size as u64, Ordering::Relaxed);
    SIZES[size.min(EXACT_SIZES)].fetch_add(1, Ordering::Relaxed);
}

/// The system allocator with request counters that record only while
/// [`COUNTING`] is set.
struct Counting;

// SAFETY: every method forwards to `System` with the caller's arguments; the
// counters are atomics and allocate nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            record(layout.size());
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            record(layout.size());
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.load(Ordering::Relaxed) {
            DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        // SAFETY: forwarded unchanged.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn reset() {
    for counter in [&ALLOCATIONS, &REALLOCATIONS, &DEALLOCATIONS, &BYTES] {
        counter.store(0, Ordering::Relaxed);
    }
    for size in &SIZES {
        size.store(0, Ordering::Relaxed);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut bundle, mut code, mut batch, mut cap, mut norm) =
        (String::new(), String::new(), 16usize, 50u32, 0.75f32);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--bundle" => bundle = value,
            "--code" => code = value,
            "--batch" => batch = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--iteration-cap" => cap = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--norm" => norm = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let case = DecoderCase {
        bundle,
        code: code.clone(),
        iteration_cap: cap,
        normalization_factor: norm,
        syndrome_stopping: true,
        batch_size: batch as u32,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&case)?;
    if batch == 0 || batch > workload.manifest.frames {
        return Err(format!("--batch must lie in 1..={}", workload.manifest.frames));
    }
    let decoder_code =
        ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    let mut worker = Gf2Worker::new(&decoder_code, config(norm, true), 1, cap as usize);
    // One untimed, uncounted frame first, as a throughput worker's warm pass.
    worker.decode_batch(workload.frames(0, 1));
    for frame in 0..batch {
        let llrs = workload.frames(frame, 1);
        reset();
        COUNTING.store(true, Ordering::SeqCst);
        worker.decode_batch(llrs);
        COUNTING.store(false, Ordering::SeqCst);
        let sizes: Vec<_> = SIZES
            .iter()
            .enumerate()
            .filter_map(|(size, count)| {
                let count = count.load(Ordering::Relaxed);
                (count > 0).then(|| json!([size, count]))
            })
            .collect();
        let errors = workload.frame_errors(frame, &worker.decisions);
        println!(
            "{}",
            json!({
                "schema": "ldpc-alloc-census-v1",
                "code": code,
                "frame": frame,
                "iterations": worker.iterations[0],
                "bit_errors": errors[0],
                "allocations": ALLOCATIONS.load(Ordering::Relaxed),
                "reallocations": REALLOCATIONS.load(Ordering::Relaxed),
                "deallocations": DEALLOCATIONS.load(Ordering::Relaxed),
                "bytes_requested": BYTES.load(Ordering::Relaxed),
                "sizes": sizes,
                "size_bucket_limit": EXACT_SIZES,
            })
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-alloc-census: {error}");
            ExitCode::FAILURE
        }
    }
}
