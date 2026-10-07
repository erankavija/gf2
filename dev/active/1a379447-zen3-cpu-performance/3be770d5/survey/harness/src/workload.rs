//! The recorded workload every arm decodes and the checks on its decisions.

use crate::pool::{Finished, WorkerObservation};
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
        let llrs =
            ldpc_survey::read_llrs(&dir, manifest.frames, manifest.n).map_err(|e| e.to_string())?;
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

/// Process thread counts observed while the pool was alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Threads {
    /// Counted once every worker was ready, before the first dispatch.
    pub ready: u32,
    /// Counted after the last timed call, before the workers stop.
    pub after: u32,
}

/// Placement evidence every arm writes to standard error, which the runner
/// journals as a child diagnostic.
#[derive(Debug, Serialize)]
pub struct PlacementReport<'a> {
    pub kind: &'static str,
    pub arm: &'a str,
    /// Process threads around the measured calls: the workers plus the
    /// dispatching thread when no other pool exists.
    pub threads: Threads,
    pub workers: Vec<WorkerObservation>,
}

/// Verifies placement and decisions of every finished worker, prints the
/// placement report and returns the number of verified workers.
///
/// Each worker must have run only on its assigned CPU, completed `calls` body
/// invocations, and decoded exactly the prepared per-frame error vector for
/// the batch. The process must have held no thread beyond the workers and the
/// dispatching thread, since the cells declare no nested pool.
///
/// # Errors
///
/// Returns an error for the first violated condition.
pub fn verify_workers<S>(
    arm: &str,
    finished: &[Finished<S>],
    calls: u64,
    threads: Threads,
    decisions: impl Fn(&S) -> &[u8],
    workload: &Workload,
    quality: &Quality,
) -> Result<u32, String> {
    let report = PlacementReport {
        kind: "worker-placement",
        arm,
        threads,
        workers: finished.iter().map(|f| f.observation.clone()).collect(),
    };
    eprintln!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    check_threads(threads, finished.len())?;
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

/// Checks that a `workers`-thread pool ran with no other thread than the
/// dispatching one.
///
/// # Errors
///
/// Returns an error naming both counts when either differs.
pub fn check_threads(threads: Threads, workers: usize) -> Result<(), String> {
    let expected = workers as u32 + 1;
    if threads.ready == expected && threads.after == expected {
        Ok(())
    } else {
        Err(format!(
            "{} and {} process threads observed around a {workers}-worker pool; \
             a nested pool is not declared",
            threads.ready, threads.after
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::{check_prepared, check_threads, Threads, Workload};
    use crate::pool::{process_threads, run};
    use ldpc_survey::arm::{
        ArmSettings, IterationDistribution, Normalization, NormalizationKind, Precision, Quality,
        Schedule, Stopping, StoppingKind,
    };
    use ldpc_survey::{BundleManifest, CodewordSource};

    fn quality(frame_bit_errors: Vec<u64>) -> Quality {
        Quality {
            frames: frame_bit_errors.len() as u64,
            frame_errors: frame_bit_errors.iter().filter(|e| **e > 0).count() as u64,
            bits: 0,
            bit_errors: frame_bit_errors.iter().sum(),
            frame_bit_errors,
            fer: 0.0,
            fer_interval: [0.0, 1.0],
            ber: 0.0,
            ber_interval: [0.0, 1.0],
            interval_method: "frame-hoeffding-95+fer-wilson-95".to_owned(),
            iterations: IterationDistribution {
                mean: 1.0,
                p50: 1,
                p90: 1,
                max: 1,
            },
            memory_bytes: 0,
            latency_ns_p50: 0,
            settings: ArmSettings {
                precision: Precision::F32,
                schedule: Schedule::Flooding,
                normalization: Normalization {
                    kind: NormalizationKind::NormalizedMinSum,
                    factor: Some(0.75),
                },
                iteration_cap: 1,
                stopping: Stopping {
                    kind: StoppingKind::Syndrome,
                    crc: None,
                },
                batch_size: 1,
            },
        }
    }

    /// Two frames of n = 4 with the information window k = 2.
    fn workload() -> Workload {
        Workload {
            manifest: BundleManifest {
                schema: ldpc_survey::BUNDLE_SCHEMA.to_owned(),
                code: "fixture".to_owned(),
                n: 4,
                k: 2,
                m: 2,
                nnz: 0,
                h_sha256: String::new(),
                codewords_sha256: String::new(),
                llrs_sha256: String::new(),
                frames: 2,
                seed: 0,
                esn0_db: 0.0,
                sigma: 1.0,
                codeword_source: CodewordSource::Both,
                punctured_prefix: 0,
            },
            dir: std::path::PathBuf::new(),
            llrs: vec![0.0; 8],
            codewords: vec![0, 1, 1, 1, 1, 0, 0, 0],
        }
    }

    #[test]
    fn frame_errors_score_only_the_information_window() {
        let fixture = workload();
        assert_eq!(fixture.frame_errors(0, &[0, 1, 1, 0]), vec![0, 0]);
        assert_eq!(fixture.frame_errors(0, &[1, 0, 0, 1]), vec![2, 2]);
        assert_eq!(fixture.frame_errors(1, &[1, 1]), vec![1]);
    }

    #[test]
    fn prepared_quality_check_names_the_first_differing_frame() {
        let frozen = quality(vec![0, 3, 0, 1]);
        assert!(check_prepared(&[3, 0], &frozen, 1).is_ok());
        let error = check_prepared(&[0, 2], &frozen, 0).unwrap_err();
        assert!(error.contains("frame 1") && error.contains("records 3"));
        assert!(check_prepared(&[0, 0], &frozen, 3).is_err());
    }

    #[test]
    fn thread_counts_are_observed_while_the_pool_is_alive() {
        let cpus: Vec<u32> = tuning_campaign_support::host::CpuAffinity::observe()
            .expect("affinity")
            .cpus()
            .iter()
            .copied()
            .take(2)
            .collect();
        let (threads, _) = run(
            &cpus,
            |_| Ok(()),
            |_, _: &mut ()| {},
            |dispatch| {
                let ready = process_threads();
                dispatch.call();
                Threads {
                    ready,
                    after: process_threads(),
                }
            },
        )
        .expect("pool");
        let workers = cpus.len();
        // The test harness may run other tests' threads concurrently, so the
        // check is exercised on the relation it enforces.
        let observed = check_threads(threads, workers);
        assert_eq!(
            observed.is_ok(),
            threads.ready == workers as u32 + 1 && threads.after == workers as u32 + 1
        );
        assert!(threads.ready > workers as u32);
        assert!(check_threads(Threads { ready: 3, after: 4 }, 2).is_err());
        assert!(check_threads(Threads { ready: 3, after: 3 }, 2).is_ok());
    }
}
