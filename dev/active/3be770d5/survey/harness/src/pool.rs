//! Pinned worker pool shared by every arm and profile binary.
//!
//! [`run`] starts one thread per listed CPU, pins it there, lets it build its
//! own state, and hands the caller a [`Dispatch`]: each [`Dispatch::call`]
//! releases every worker into one body invocation and returns when all have
//! finished. The two barrier crossings per call are the dispatch cost a timed
//! call includes. Workers observe their own placement; the pool never infers
//! it from the request.

use serde::Serialize;
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Barrier, Mutex};
use std::thread;

/// What one worker observed about its own placement and work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkerObservation {
    /// Index of the worker; worker `i` is assigned the `i`-th listed CPU.
    pub worker: usize,
    /// The CPU the worker was assigned.
    pub cpu: u32,
    /// The worker thread's affinity mask read back after pinning.
    pub affinity: Vec<u32>,
    /// Every CPU `sched_getcpu` reported at the start and end of a body call.
    pub cpus_seen: BTreeSet<u32>,
    /// Body invocations this worker completed.
    pub calls: u64,
}

impl WorkerObservation {
    /// Whether the worker ran only on its assigned CPU.
    #[must_use]
    pub fn pinned(&self) -> bool {
        self.affinity == [self.cpu] && self.cpus_seen.iter().all(|cpu| *cpu == self.cpu)
    }
}

/// A worker's final state with its observation.
pub struct Finished<S> {
    pub state: S,
    pub observation: WorkerObservation,
}

/// Releases the workers into one body invocation per call.
pub struct Dispatch<'a> {
    start: &'a Barrier,
    done: &'a Barrier,
}

impl Dispatch<'_> {
    /// Runs the body once on every worker and waits for all of them.
    pub fn call(&self) {
        self.start.wait();
        self.done.wait();
    }
}

/// Pins the calling thread to `cpu` and returns the mask read back.
///
/// # Errors
///
/// Returns an error when the kernel refuses the mask.
pub fn pin_current_thread(cpu: u32) -> Result<Vec<u32>, String> {
    let mut set = rustix::thread::CpuSet::new();
    set.set(cpu as usize);
    rustix::thread::sched_setaffinity(None, &set)
        .map_err(|error| format!("cannot pin to CPU {cpu}: {error}"))?;
    let observed = rustix::thread::sched_getaffinity(None)
        .map_err(|error| format!("cannot read the affinity back: {error}"))?;
    Ok((0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| observed.is_set(*cpu))
        .map(|cpu| cpu as u32)
        .collect())
}

/// The CPU the calling thread runs on now.
#[must_use]
pub fn current_cpu() -> u32 {
    rustix::thread::sched_getcpu() as u32
}

/// Threads of this process as the kernel counts them, from `/proc/self/status`.
///
/// Returns zero when the field is unavailable.
#[must_use]
pub fn process_threads() -> u32 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Threads:"))
                .and_then(|value| value.trim().parse().ok())
        })
        .unwrap_or(0)
}

/// Runs one pinned worker per CPU of `cpus` around `main`.
///
/// Worker `i` pins itself to `cpus[i]`, then builds its state with
/// `init(i)`. Once every worker is ready, `main` receives the dispatch handle;
/// each call runs `body(i, &mut state)` on every worker. When `main` returns,
/// the workers stop and their final states come back in worker order.
///
/// # Errors
///
/// Returns an error naming every worker that could not pin itself, could not
/// build its state, or panicked inside `body`. `main` does not run when a
/// worker fails before it is ready.
pub fn run<S, I, B, M, R>(
    cpus: &[u32],
    init: I,
    body: B,
    main: M,
) -> Result<(R, Vec<Finished<S>>), String>
where
    S: Send,
    I: Fn(usize) -> Result<S, String> + Sync,
    B: Fn(usize, &mut S) + Sync,
    M: FnOnce(&Dispatch<'_>) -> R,
{
    if cpus.is_empty() {
        return Err("a pool needs at least one CPU".to_owned());
    }
    let workers = cpus.len();
    let ready = Barrier::new(workers + 1);
    let start = Barrier::new(workers + 1);
    let done = Barrier::new(workers + 1);
    let stop = AtomicBool::new(false);
    let failures = Mutex::new(Vec::<String>::new());
    thread::scope(|scope| {
        let handles: Vec<_> = cpus
            .iter()
            .copied()
            .enumerate()
            .map(|(worker, cpu)| {
                let (ready, start, done, stop) = (&ready, &start, &done, &stop);
                let (failures, init, body) = (&failures, &init, &body);
                scope.spawn(move || {
                    let mut observation = WorkerObservation {
                        worker,
                        cpu,
                        affinity: Vec::new(),
                        cpus_seen: BTreeSet::new(),
                        calls: 0,
                    };
                    let built = pin_current_thread(cpu).and_then(|affinity| {
                        observation.affinity = affinity;
                        init(worker)
                    });
                    let mut state = match built {
                        Ok(state) => Some(state),
                        Err(error) => {
                            failures
                                .lock()
                                .expect("failure list")
                                .push(format!("worker {worker} on CPU {cpu}: {error}"));
                            None
                        }
                    };
                    ready.wait();
                    loop {
                        start.wait();
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        if let Some(current) = state.as_mut() {
                            observation.cpus_seen.insert(current_cpu());
                            let outcome = catch_unwind(AssertUnwindSafe(|| body(worker, current)));
                            observation.cpus_seen.insert(current_cpu());
                            match outcome {
                                Ok(()) => observation.calls += 1,
                                Err(_) => {
                                    failures
                                        .lock()
                                        .expect("failure list")
                                        .push(format!("worker {worker} panicked in its body"));
                                    state = None;
                                }
                            }
                        }
                        done.wait();
                    }
                    state.map(|state| Finished { state, observation })
                })
            })
            .collect();
        ready.wait();
        let ready_failures = failures.lock().expect("failure list").len();
        let result = (ready_failures == 0).then(|| {
            main(&Dispatch {
                start: &start,
                done: &done,
            })
        });
        stop.store(true, Ordering::Release);
        start.wait();
        let finished: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("a worker outside its body panicked"))
            .collect();
        let failures = std::mem::take(&mut *failures.lock().expect("failure list"));
        match result {
            Some(result) if failures.is_empty() => {
                Ok((result, finished.into_iter().map(Option::unwrap).collect()))
            }
            _ => Err(failures.join("; ")),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{current_cpu, run};

    fn own_cpus() -> Vec<u32> {
        tuning_campaign_support::host::CpuAffinity::observe()
            .expect("affinity")
            .cpus()
            .to_vec()
    }

    #[test]
    fn every_worker_runs_every_call_on_its_own_cpu() {
        let cpus: Vec<u32> = own_cpus().into_iter().take(3).collect();
        let (calls, finished) = run(
            &cpus,
            |worker| Ok(vec![worker]),
            |worker, seen: &mut Vec<usize>| {
                seen.push(worker);
                assert_eq!(current_cpu(), cpus[worker]);
            },
            |dispatch| {
                for _ in 0..5 {
                    dispatch.call();
                }
                5
            },
        )
        .expect("pool");
        assert_eq!(finished.len(), cpus.len());
        for (worker, done) in finished.iter().enumerate() {
            assert!(done.observation.pinned());
            assert_eq!(done.observation.calls, calls);
            assert_eq!(done.state.len(), 1 + calls as usize);
            assert!(done.state.iter().all(|seen| *seen == worker));
        }
    }

    #[test]
    fn a_failing_worker_stops_the_pool_before_main() {
        let cpus: Vec<u32> = own_cpus().into_iter().take(2).collect();
        let outcome = run(
            &cpus,
            |worker| {
                if worker == 1 {
                    Err("refused".to_owned())
                } else {
                    Ok(())
                }
            },
            |_, _: &mut ()| {},
            |_| panic!("main must not run"),
        );
        let error = outcome.err().expect("the pool reports the failure");
        assert!(error.contains("worker 1") && error.contains("refused"));
    }

    #[test]
    fn a_panicking_body_is_reported_without_deadlock() {
        let cpus: Vec<u32> = own_cpus().into_iter().take(2).collect();
        let outcome = run(
            &cpus,
            |_| Ok(()),
            |worker, _: &mut ()| assert_ne!(worker, 0, "worker 0 fails"),
            |dispatch| {
                dispatch.call();
                dispatch.call();
            },
        );
        assert!(outcome.err().expect("reported").contains("worker 0 panicked"));
    }
}
