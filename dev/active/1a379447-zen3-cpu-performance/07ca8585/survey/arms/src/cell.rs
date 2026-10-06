//! The timed cell an isolated check-node arm runs.
//!
//! One function owns the arm lifecycle so the gf2 and AFF3CT kernel arms differ
//! only in how a worker builds its pass and runs it: pool start-up and setup
//! accounting, the canonical timing windows over pool dispatches, the untimed
//! dispatch probe, the placement checks, and the checksum check that both arms
//! read the same prepared messages and write the same results.
//!
//! It is the `3be770d5` steady-state driver without the per-frame decision
//! check. An isolated check-node pass decodes nothing, so no per-frame error
//! vector exists to check it against; the declared cell is `kernel-isolated`
//! with no decoder block, and the runner therefore asks for no quality. What
//! replaces the decision check is stronger for a matched kernel comparison: the
//! cell declares the checksum of the prepared input array and of the output
//! array, both in the canonical check-major edge order, and every worker of
//! every arm must reproduce both.

use ldpc_survey::arm::{self, Request, Window};
use ldpc_throughput::pool::{self, process_threads};
use ldpc_throughput::workload::{check_threads, median_u64, PlacementReport, Threads};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Empty dispatches the untimed probe times after the last window.
pub const DISPATCH_PROBES: usize = 101;

/// The request role that runs one untimed dispatch instead of timing windows.
pub const VALIDATION_ROLE: &str = "validation";

/// What a kernel cell measured.
pub struct KernelCellRun {
    pub windows: Vec<Window>,
    pub setup_ns: u64,
    pub dispatch_ns: u64,
    pub workers_observed: u32,
    pub threads: Threads,
}

/// Runs one isolated check-node cell on one pinned worker per requested CPU.
///
/// Setup runs from `setup_start` until every worker has built its state with
/// `init`. Each timed call dispatches `body` to every worker; the dispatch is
/// part of the call. After the windows, an untimed probe times
/// [`DISPATCH_PROBES`] dispatches of an empty body and reports their median.
///
/// A request with role [`VALIDATION_ROLE`] runs one untimed dispatch and no
/// probe, returns no windows, and applies every placement and checksum check,
/// so an arm can be exercised end to end without a timing run.
///
/// `expected` is the pair of canonical checksums the cell declares, as
/// `(input, output)` hex strings; an empty string skips that check, which the
/// untimed preparation tools use to record the checksums in the first place.
/// `observed` reports a finished worker's own `(input, output)` checksums.
///
/// # Errors
///
/// Returns an error when the pool fails or any worker violates its placement,
/// its call count or either checksum.
#[allow(clippy::too_many_arguments)]
pub fn run_kernel_cell<S, I, B, C>(
    arm_name: &str,
    request: &Request,
    setup_start: Instant,
    expected: (&str, &str),
    init: I,
    body: B,
    observed: C,
) -> Result<KernelCellRun, String>
where
    S: Send,
    I: Fn(usize) -> Result<S, String> + Sync,
    B: Fn(&mut S) + Sync,
    C: Fn(&S) -> (String, String),
{
    let empty = AtomicBool::new(false);
    let validation = request.role == VALIDATION_ROLE;
    let (measured, finished) = pool::run(
        &request.cpus,
        init,
        |_, state| {
            if !empty.load(Ordering::Acquire) {
                body(state);
            }
        },
        |dispatch| {
            let setup_ns = setup_start.elapsed().as_nanos() as u64;
            let ready = process_threads();
            let mut calls = 0u64;
            if validation {
                dispatch.call();
                calls += 1;
                let threads = Threads {
                    ready,
                    after: process_threads(),
                };
                return (Vec::new(), setup_ns, threads, calls, 0);
            }
            let windows = arm::timing_windows(request, &mut |_bank| {
                dispatch.call();
                calls += 1;
            });
            let threads = Threads {
                ready,
                after: process_threads(),
            };
            empty.store(true, Ordering::Release);
            let mut rounds = Vec::with_capacity(DISPATCH_PROBES);
            for _ in 0..DISPATCH_PROBES {
                let start = Instant::now();
                dispatch.call();
                rounds.push(start.elapsed().as_nanos() as u64);
                calls += 1;
            }
            (windows, setup_ns, threads, calls, median_u64(rounds))
        },
    )?;
    let (windows, setup_ns, threads, calls, dispatch_ns) = measured;
    let report = PlacementReport {
        kind: "worker-placement",
        arm: arm_name,
        threads,
        workers: finished.iter().map(|f| f.observation.clone()).collect(),
    };
    eprintln!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    check_threads(threads, finished.len())?;
    for done in &finished {
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
        let (input, output) = observed(&done.state);
        for (name, declared, got) in [
            ("input", expected.0, &input),
            ("output", expected.1, &output),
        ] {
            if !declared.is_empty() && declared != got {
                return Err(format!(
                    "worker {}: the {name} array checksums {got}; the cell declares {declared}",
                    observation.worker
                ));
            }
        }
    }
    Ok(KernelCellRun {
        windows,
        setup_ns,
        dispatch_ns,
        workers_observed: finished.len() as u32,
        threads,
    })
}
