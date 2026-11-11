//! Throughput of one decoder configuration over a recorded bundle, outside the
//! campaign lifecycle: the argument shape, pinned warm and timed call loop and
//! JSON line shared by `gf2-fast-bench` and `aff3ct-fast-bench`.

use crate::arm::DecoderCase;
use crate::pool::{physical_first_cpus, pin_current_thread};
use crate::workload::{median_u64, Workload};
use std::time::Instant;

/// Iteration cap of every fast-bench decoder.
pub const ITERATION_CAP: u32 = 50;
/// Normalized min-sum factor of every fast-bench decoder.
pub const NORMALIZATION: f32 = 0.75;
/// Untimed calls of each worker before its timed calls.
pub const WARM_CALLS: usize = 2;

/// Worker placement of a timed run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pinning {
    /// Worker `i` pinned to the `i`-th CPU of [`physical_first_cpus`].
    Pinned,
    /// Workers left to the scheduler.
    Unpinned,
}

impl Pinning {
    /// Parses `pinned` or `unpinned`.
    ///
    /// # Errors
    ///
    /// Returns an error for another word.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "pinned" => Ok(Self::Pinned),
            "unpinned" => Ok(Self::Unpinned),
            other => Err(format!("PINNING is pinned or unpinned, not {other}")),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Unpinned => "unpinned",
        }
    }
}

/// `BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS PINNING`.
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
    pub pinning: Pinning,
}

fn positive(text: &str, what: &str) -> Result<usize, String> {
    text.parse()
        .ok()
        .filter(|&value| value > 0)
        .ok_or(format!("{what} is a positive integer, not {text}"))
}

impl FastCell {
    /// Parses the nine positional arguments.
    ///
    /// # Errors
    ///
    /// Returns the usage on another argument count, and an error for a
    /// non-positive count or an unknown pinning.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let [bundle, code, kind, simd, precision, workers, batch, calls, pinning] = args else {
            return Err(
                "usage: BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS PINNING".to_owned(),
            );
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
            pinning: Pinning::parse(pinning)?,
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
    /// each of `workers` threads placed as `pinning` says, and returns the
    /// median over workers of each worker's median call time. Timed calls
    /// start together.
    ///
    /// # Errors
    ///
    /// Returns an error when the host has fewer CPUs than pinned workers,
    /// when pinning or `init` fails, or when a worker panics.
    pub fn median_call_ns<W>(
        &self,
        init: impl Fn(usize) -> Result<W, String> + Sync,
        call: impl Fn(&mut W) + Sync,
    ) -> Result<u64, String> {
        let cpus = match self.pinning {
            Pinning::Pinned => {
                let cpus = physical_first_cpus()?;
                if cpus.len() < self.workers {
                    return Err(format!(
                        "{} pinned workers on {} CPUs",
                        self.workers,
                        cpus.len()
                    ));
                }
                Some(cpus)
            }
            Pinning::Unpinned => None,
        };
        let start = std::sync::Barrier::new(self.workers);
        let timed = std::sync::Barrier::new(self.workers);
        let medians = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..self.workers)
                .map(|index| {
                    let (init, call, start, timed, cpus) = (&init, &call, &start, &timed, &cpus);
                    scope.spawn(move || -> Result<u64, String> {
                        // Every worker reaches both barriers, so one failure
                        // cannot leave the others waiting. The second one
                        // starts the timed calls of all workers together, so
                        // none is timed while another still warms up.
                        let worker = match cpus {
                            Some(cpus) => pin_current_thread(cpus[index]).map(drop),
                            None => Ok(()),
                        }
                        .and_then(|()| init(index));
                        start.wait();
                        let mut worker = match worker {
                            Ok(mut worker) => {
                                for _ in 0..WARM_CALLS {
                                    call(&mut worker);
                                }
                                timed.wait();
                                worker
                            }
                            Err(error) => {
                                timed.wait();
                                return Err(error);
                            }
                        };
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
                "pinning": self.pinning.as_str(),
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

#[cfg(test)]
mod tests {
    use super::{FastCell, Pinning};

    fn args(pinning: &str) -> Vec<String> {
        ["dir", "code", "qc", "avx2", "f32", "2", "16", "3", pinning]
            .map(str::to_owned)
            .to_vec()
    }

    #[test]
    fn the_last_argument_names_the_pinning() {
        assert_eq!(
            FastCell::parse(&args("pinned")).unwrap().pinning,
            Pinning::Pinned
        );
        assert_eq!(
            FastCell::parse(&args("unpinned")).unwrap().pinning,
            Pinning::Unpinned
        );
        assert!(FastCell::parse(&args("cpu0")).is_err());
        assert!(FastCell::parse(&args("pinned")[..8]).is_err());
    }

    #[test]
    fn no_call_is_timed_while_a_worker_still_warms_up() {
        use super::WARM_CALLS;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let mut cell = FastCell::parse(&args("unpinned")).unwrap();
        cell.workers = 3;
        let warmed = AtomicUsize::new(0);
        let early = AtomicBool::new(false);
        cell.median_call_ns(
            |index| Ok((index, 0usize)),
            |(index, calls)| {
                if *calls < WARM_CALLS {
                    // Worker 0 warms up slowest.
                    if *index == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    warmed.fetch_add(1, Ordering::SeqCst);
                } else if warmed.load(Ordering::SeqCst) < 3 * WARM_CALLS {
                    early.store(true, Ordering::SeqCst);
                }
                *calls += 1;
            },
        )
        .unwrap();
        assert!(!early.load(Ordering::SeqCst));
    }
}
