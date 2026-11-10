//! Throughput of one decoder configuration over a recorded bundle, outside the
//! campaign lifecycle: the argument shape, pinned warm and timed call loop and
//! JSON line shared by `gf2-fast-bench` and `aff3ct-fast-bench`.

use crate::arm::DecoderCase;
use crate::pool::pin_current_thread;
use crate::workload::{median_u64, Workload};
use std::time::Instant;

/// Iteration cap of every fast-bench decoder.
pub const ITERATION_CAP: u32 = 50;
/// Normalized min-sum factor of every fast-bench decoder.
pub const NORMALIZATION: f32 = 0.75;
/// Untimed calls of each worker before its timed calls.
pub const WARM_CALLS: usize = 2;

/// `BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS`.
pub struct FastCell {
    pub bundle: String,
    pub code: String,
    pub kind: String,
    /// `none` or a vector backend name.
    pub simd: String,
    pub precision: String,
    pub workers: usize,
    pub batch: usize,
    pub calls: usize,
}

fn positive(text: &str, what: &str) -> Result<usize, String> {
    text.parse()
        .ok()
        .filter(|&value| value > 0)
        .ok_or(format!("{what} is a positive integer, not {text}"))
}

impl FastCell {
    /// Parses the eight positional arguments.
    ///
    /// # Errors
    ///
    /// Returns the usage on another argument count, and an error for a
    /// non-positive count.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let [bundle, code, kind, simd, precision, workers, batch, calls] = args else {
            return Err("usage: BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS".to_owned());
        };
        Ok(Self {
            bundle: bundle.clone(),
            code: code.clone(),
            kind: kind.clone(),
            simd: simd.clone(),
            precision: precision.clone(),
            workers: positive(workers, "WORKERS")?,
            batch: positive(batch, "BATCH")?,
            calls: positive(calls, "CALLS")?,
        })
    }

    /// Loads the bundle with the fast-bench decoder settings.
    ///
    /// # Errors
    ///
    /// As [`Workload::load`], and when the bundle holds fewer than `batch`
    /// frames.
    pub fn load(&self) -> Result<Workload, String> {
        let workload = Workload::load(&DecoderCase {
            bundle: self.bundle.clone(),
            code: self.code.clone(),
            iteration_cap: ITERATION_CAP,
            normalization_factor: NORMALIZATION,
            syndrome_stopping: true,
            batch_size: self.batch as u32,
            quality_frames: 0,
            decisions_out: None,
        })?;
        if self.batch > workload.manifest.frames {
            return Err(format!("the bundle holds fewer than {} frames", self.batch));
        }
        Ok(workload)
    }

    /// Runs `init` and then [`WARM_CALLS`] and `calls` calls of `call` on
    /// each of `workers` threads, worker `i` pinned to CPU `i`, and returns
    /// the median over workers of each worker's median call time. Timed calls
    /// start together.
    ///
    /// # Errors
    ///
    /// Returns an error when pinning or `init` fails or a worker panics.
    pub fn median_call_ns<W>(
        &self,
        init: impl Fn(usize) -> Result<W, String> + Sync,
        call: impl Fn(&mut W) + Sync,
    ) -> Result<u64, String> {
        let start = std::sync::Barrier::new(self.workers);
        let medians = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..self.workers)
                .map(|cpu| {
                    let (init, call, start) = (&init, &call, &start);
                    scope.spawn(move || -> Result<u64, String> {
                        // Every worker reaches the barrier, so one failure
                        // cannot leave the others waiting.
                        let worker = pin_current_thread(cpu as u32).and_then(|_| init(cpu));
                        start.wait();
                        let mut worker = worker?;
                        for _ in 0..WARM_CALLS {
                            call(&mut worker);
                        }
                        let nanoseconds = (0..self.calls)
                            .map(|_| {
                                let begin = Instant::now();
                                call(&mut worker);
                                begin.elapsed().as_nanos() as u64
                            })
                            .collect();
                        Ok(median_u64(nanoseconds))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().map_err(|_| "a worker panicked".to_owned())?)
                .collect::<Result<Vec<u64>, String>>()
        })?;
        Ok(median_u64(medians))
    }

    /// Prints the cell's JSON line. `frame_errors` counts the frames of the
    /// untimed quality pass over `quality_frames` frames that decode with an
    /// information-bit error.
    #[allow(clippy::too_many_arguments)]
    pub fn report(
        &self,
        decoder: &str,
        iteration_cap: usize,
        k: usize,
        median_call_ns: u64,
        quality_frames: usize,
        frame_errors: usize,
    ) {
        let frames_per_second = (self.workers * self.batch) as f64 / (median_call_ns as f64 * 1e-9);
        println!(
            "{}",
            serde_json::json!({
                "decoder": decoder,
                "iteration_cap": iteration_cap,
                "kind": self.kind,
                "simd": self.simd,
                "precision": self.precision,
                "code": self.code,
                "workers": self.workers,
                "batch": self.batch,
                "calls": self.calls,
                "median_call_ns": median_call_ns,
                "frames_per_second": frames_per_second,
                "info_mbps": frames_per_second * k as f64 / 1e6,
                "quality_frames": quality_frames,
                "frame_errors": frame_errors,
            })
        );
    }
}

/// Frames of `decisions` (`k` bytes per frame, from frame 0) with an
/// information-bit error.
#[must_use]
pub fn erroneous_frames(workload: &Workload, decisions: &[u8]) -> usize {
    workload
        .frame_errors(0, decisions)
        .iter()
        .filter(|&&errors| errors > 0)
        .count()
}

/// Runs `run` on the process arguments and maps an error to a failing exit
/// status with the program name.
pub fn main_with(
    program: &str,
    run: impl FnOnce(&[String]) -> Result<(), String>,
) -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{program}: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
