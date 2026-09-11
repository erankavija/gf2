//! The recorded workload every arm decodes and the checks on its decisions.

use crate::pool::{process_threads, Finished, WorkerObservation};
use ldpc_survey::arm::{DecoderCase, Quality, Request};
use ldpc_survey::BundleManifest;
use serde::Serialize;
use std::path::PathBuf;

/// A digest-verified recorded bundle held in memory.
pub struct Workload {
    pub manifest: BundleManifest,
    pub dir: PathBuf,
    /// `frames * n` recorded little-endian f32 LLRs.
    pub llrs: Vec<f32>,
    /// `frames * n` transmitted bits, one byte per bit.
    pub codewords: Vec<u8>,
}

impl Workload {
    /// Loads the bundle a case names and verifies every recorded digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the bundle is unreadable, a digest differs, or
    /// the bundle holds another code than the case declares.
    pub fn load(case: &DecoderCase) -> Result<Self, String> {
        let dir = PathBuf::from(&case.bundle);
        let manifest = ldpc_survey::load_manifest(&dir).map_err(|e| e.to_string())?;
        ldpc_survey::verify_digests(&dir, &manifest).map_err(|e| e.to_string())?;
        if manifest.code != case.code {
            return Err(format!(
                "bundle holds {} but the case declares {}",
                manifest.code, case.code
            ));
        }
        let llrs = ldpc_survey::read_llrs(&dir, manifest.frames, manifest.n)
            .map_err(|e| e.to_string())?;
        let codewords = ldpc_survey::read_codewords(&dir, manifest.frames, manifest.n)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            manifest,
            dir,
            llrs,
            codewords,
        })
    }

    /// Path of the recorded AList.
    #[must_use]
    pub fn alist(&self) -> PathBuf {
        self.dir.join(ldpc_survey::ALIST_FILE)
    }

    /// LLRs of the `frames` consecutive recorded frames starting at `first`.
    ///
    /// # Panics
    ///
    /// Panics when the range leaves the bundle.
    #[must_use]
    pub fn frames(&self, first: usize, frames: usize) -> &[f32] {
        let n = self.manifest.n;
        &self.llrs[first * n..(first + frames) * n]
    }

    /// Information-window bit errors of each decoded frame, in frame order.
    ///
    /// `decisions` holds `k` 0/1 bytes per frame for the frames starting at
    /// `first`.
    #[must_use]
    pub fn frame_errors(&self, first: usize, decisions: &[u8]) -> Vec<u64> {
        let (n, k) = (self.manifest.n, self.manifest.k);
        decisions
            .chunks_exact(k)
            .enumerate()
            .map(|(offset, decided)| {
                let sent = &self.codewords[(first + offset) * n..(first + offset) * n + k];
                decided.iter().zip(sent).filter(|(a, b)| a != b).count() as u64
            })
            .collect()
    }
}

/// Checks decisions for the frames starting at `first` against the frozen
/// per-frame error vector of the prepared quality evidence.
///
/// # Errors
///
/// Returns an error naming the first frame whose error count differs.
pub fn check_prepared(errors: &[u64], quality: &Quality, first: usize) -> Result<(), String> {
    let frozen = quality
        .frame_bit_errors
        .get(first..first + errors.len())
        .ok_or("the prepared quality holds fewer frames than the batch")?;
    match errors.iter().zip(frozen).position(|(a, b)| a != b) {
        None => Ok(()),
        Some(offset) => Err(format!(
            "frame {} decoded with {} bit errors; the prepared quality records {}",
            first + offset,
            errors[offset],
            frozen[offset]
        )),
    }
}

/// The request fields a steady-state cell must satisfy, checked once.
///
/// # Errors
///
/// Returns an error when the cell is not warm, when the worker count differs
/// from the resolved CPU count, or when the batch leaves the bundle.
pub fn check_request(request: &Request, case: &DecoderCase, frames: usize) -> Result<(), String> {
    if request.cache_state != "warm" {
        return Err(format!(
            "steady-state cells declare warm cache state, not {}",
            request.cache_state
        ));
    }
    if request.workers_declared as usize != request.cpus.len() {
        return Err(format!(
            "{} workers declared for {} resolved CPUs; this harness runs one worker per CPU",
            request.workers_declared,
            request.cpus.len()
        ));
    }
    if case.batch_size == 0 || case.batch_size as usize > frames {
        return Err(format!(
            "per-worker batch {} must lie in 1..={frames}",
            case.batch_size
        ));
    }
    Ok(())
}

/// Placement evidence every arm writes to standard error, which the runner
/// journals as a child diagnostic.
#[derive(Debug, Serialize)]
pub struct PlacementReport<'a> {
    pub kind: &'static str,
    pub arm: &'a str,
    /// Process threads counted after the pool was ready: workers plus the
    /// dispatching thread when no other pool exists.
    pub threads_ready: u32,
    /// Process threads counted after the last timed call.
    pub threads_after: u32,
    pub workers: Vec<WorkerObservation>,
}

/// Verifies placement and decisions of every finished worker, prints the
/// placement report and returns the number of verified workers.
///
/// Each worker must have run only on its assigned CPU, completed `calls` body
/// invocations, and decoded exactly the prepared per-frame error vector for
/// the batch. The process must hold no thread beyond the workers and the
/// dispatching thread, since the cells declare no nested pool.
///
/// # Errors
///
/// Returns an error for the first violated condition.
pub fn verify_workers<S>(
    arm: &str,
    finished: &[Finished<S>],
    calls: u64,
    threads_ready: u32,
    decisions: impl Fn(&S) -> &[u8],
    workload: &Workload,
    quality: &Quality,
) -> Result<u32, String> {
    let report = PlacementReport {
        kind: "worker-placement",
        arm,
        threads_ready,
        threads_after: process_threads(),
        workers: finished.iter().map(|f| f.observation.clone()).collect(),
    };
    eprintln!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    let expected_threads = finished.len() as u32 + 1;
    if report.threads_ready != expected_threads || report.threads_after != expected_threads {
        return Err(format!(
            "{} and {} process threads observed around a {}-worker pool; a nested pool is not declared",
            report.threads_ready,
            report.threads_after,
            finished.len()
        ));
    }
    for done in finished {
        let observation = &done.observation;
        if !observation.pinned() {
            return Err(format!(
                "worker {} was assigned CPU {} but observed affinity {:?} and CPUs {:?}",
                observation.worker, observation.cpu, observation.affinity, observation.cpus_seen
            ));
        }
        if observation.calls != calls {
            return Err(format!(
                "worker {} completed {} of {calls} calls",
                observation.worker, observation.calls
            ));
        }
        let errors = workload.frame_errors(0, decisions(&done.state));
        check_prepared(&errors, quality, 0)
            .map_err(|error| format!("worker {}: {error}", observation.worker))?;
    }
    Ok(finished.len() as u32)
}

/// Median of `values`, which must be nonempty.
///
/// # Panics
///
/// Panics when `values` is empty.
#[must_use]
pub fn median_u64(mut values: Vec<u64>) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}
