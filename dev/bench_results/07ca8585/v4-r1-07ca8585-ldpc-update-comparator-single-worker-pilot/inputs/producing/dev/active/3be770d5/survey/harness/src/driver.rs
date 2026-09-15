//! The steady-state cell every arm binary runs.
//!
//! One function owns the arm lifecycle so the gf2 and AFF3CT arms differ only
//! in how a worker builds its decoder and decodes a batch: pool start-up and
//! setup accounting, the canonical timing windows over pool dispatches, the
//! untimed dispatch probe, and the placement and decision checks.

use crate::pool::{self, process_threads};
use crate::workload::{median_u64, verify_workers, Threads, Workload};
use ldpc_survey::arm::{self, ConversionCosts, Quality, Request, Window};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Empty dispatches the untimed probe times after the last window.
pub const DISPATCH_PROBES: usize = 101;

/// The request role that runs one untimed dispatch instead of timing windows.
pub const VALIDATION_ROLE: &str = "validation";

/// What a steady-state cell measured.
pub struct CellRun {
    pub windows: Vec<Window>,
    pub conversion: ConversionCosts,
    pub workers_observed: u32,
    pub threads: Threads,
}

/// Runs one steady-state cell on one pinned worker per requested CPU.
///
/// Setup runs from `setup_start` until every worker has built its state with
/// `init`. Each timed call dispatches `body` to every worker; the dispatch is
/// part of the call. After the windows, an untimed probe times
/// [`DISPATCH_PROBES`] dispatches of an empty body and reports their median
/// as the dispatch cost. `pack_ns` is the arm's untimed per-batch conversion
/// probe, or zero when the conversion happens inside the decoder call.
///
/// A request with role [`VALIDATION_ROLE`] runs one untimed dispatch and no
/// probe, returns no windows, and applies every placement and decision check,
/// so an arm can be exercised end to end without a timing run.
///
/// # Errors
///
/// Returns an error when the pool fails or any worker violates its placement
/// or decodes other than the prepared quality records.
#[allow(clippy::too_many_arguments)]
pub fn run_cell<S, I, B, D>(
    arm_name: &str,
    request: &Request,
    workload: &Workload,
    quality: &Quality,
    setup_start: Instant,
    pack_ns: u64,
    init: I,
    body: B,
    decisions: D,
) -> Result<CellRun, String>
where
    S: Send,
    I: Fn(usize) -> Result<S, String> + Sync,
    B: Fn(&mut S) + Sync,
    D: Fn(&S) -> &[u8],
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
    let workers_observed = verify_workers(
        arm_name, &finished, calls, threads, decisions, workload, quality,
    )?;
    Ok(CellRun {
        windows,
        conversion: ConversionCosts {
            setup_ns,
            pack_ns,
            // Decisions are extracted inside every timed call; there is no
            // separate unpack stage.
            unpack_ns: 0,
            // The batch is recorded data already resident in memory.
            batch_fill_ns: 0,
            dispatch_ns,
        },
        workers_observed,
        threads,
    })
}
