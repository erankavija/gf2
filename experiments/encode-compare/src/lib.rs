//! Throughput harness of the encoder comparisons (@/issue/be6d6f45,
//! @/issue/040bc2b8): a cell's arms encode the same seeded messages, each in
//! its own input format, and report frames per second at a worker count and
//! message pool size.
//!
//! Every worker builds its own arm and encodes the shared messages in turn
//! for [`REP_SECONDS`]; a cell's rate is the median over [`REPS`] such
//! repetitions of the frames all workers finished divided by the wall time.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Barrier;
use std::time::{Duration, Instant};

/// Wall time of one timed repetition.
pub const REP_SECONDS: f64 = 0.5;
/// Timed repetitions per cell; the cell reports their median.
pub const REPS: usize = 5;
/// Untimed encoding before the first repetition.
pub const WARMUP_SECONDS: f64 = 0.2;
/// Distinct messages an NR cell cycles through.
pub const MESSAGES: usize = 64;

/// One implementation's encoder, owned by one worker.
pub trait Arm {
    /// Encodes message `index` of the cell's messages into the arm's own
    /// output buffer.
    fn encode(&mut self, index: usize);
}

/// Median frames per second of `workers` workers, each running the arm
/// `make` builds over messages `0..pool` in turn.
pub fn frames_per_second<F>(workers: usize, pool: usize, make: F) -> f64
where
    F: Fn() -> Box<dyn Arm> + Sync,
{
    let barrier = Barrier::new(workers + 1);
    let stop = AtomicBool::new(false);
    let frames = AtomicU64::new(0);
    let mut rates = Vec::with_capacity(REPS);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                let mut arm = make();
                let mut index = 0usize;
                let mut run = |arm: &mut Box<dyn Arm>| -> u64 {
                    let mut done = 0;
                    while !stop.load(Ordering::Relaxed) {
                        for _ in 0..8 {
                            arm.encode(black_box(index % pool));
                            index += 1;
                        }
                        done += 8;
                    }
                    done
                };
                for _ in 0..=REPS {
                    barrier.wait();
                    let done = run(&mut arm);
                    frames.fetch_add(done, Ordering::Relaxed);
                    barrier.wait();
                }
            });
        }
        for rep in 0..=REPS {
            stop.store(false, Ordering::Relaxed);
            frames.store(0, Ordering::Relaxed);
            let seconds = if rep == 0 {
                WARMUP_SECONDS
            } else {
                REP_SECONDS
            };
            barrier.wait();
            let start = Instant::now();
            std::thread::sleep(Duration::from_secs_f64(seconds));
            stop.store(true, Ordering::Relaxed);
            barrier.wait();
            let elapsed = start.elapsed().as_secs_f64();
            if rep > 0 {
                rates.push(frames.load(Ordering::Relaxed) as f64 / elapsed);
            }
        }
    });
    rates.sort_by(f64::total_cmp);
    rates[REPS / 2]
}

/// The kernel host name and the first CPU model name `/proc/cpuinfo` lists.
pub fn host_description() -> String {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_owned())
        .unwrap_or_else(|_| "unknown-host".to_owned());
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|info| {
            info.lines()
                .find_map(|line| line.strip_prefix("model name"))
                .map(|rest| rest.trim_start_matches([' ', '\t', ':']).to_owned())
        })
        .unwrap_or_else(|| "unknown CPU".to_owned());
    format!("{host} ({cpu})")
}

/// `count` seeded messages of `k` bits each, the inputs every arm of a cell
/// encodes.
pub fn messages(k: usize, count: usize, seed: u64) -> Vec<gf2_core::BitVec> {
    (0..count as u64)
        .map(|i| gf2_core::BitVec::random_seeded(k, seed ^ (i << 32)))
        .collect()
}

pub mod bch;
pub mod cutover;
pub mod nr;
