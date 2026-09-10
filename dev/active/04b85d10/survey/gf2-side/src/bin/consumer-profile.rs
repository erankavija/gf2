//! Allocation and profiler driver for the bit-storage consumer profile
//! (jit:04b85d10).
//!
//! It runs one workload route a fixed number of times under a counting global
//! allocator and prints one JSON record. The fixed call count is what makes
//! the run comparable under `perf stat` and `perf record`: the profiler
//! attributes cycles and cache traffic to the same work the allocator counters
//! describe. It measures the same routes as the protocol arm through the same
//! [`consumer_profile_gf2_side::prepare`], so an allocation figure and a
//! timing receipt describe one implementation.
//!
//! Usage:
//!
//! ```text
//! consumer-profile --case '<json>' --path <route> --cache-state <state> \
//!     [--calls <n> | --target-ms <ms>]
//! ```
//!
//! `--target-ms` derives the count that fills the target from a doubling
//! burst, which keeps a sweep over sizes spanning five orders of magnitude
//! bounded. A single probe call would not do: the clock read around one call
//! of a few nanoseconds costs more than the call, and the count derived from
//! it would fall short of the target by more than an order of magnitude. The
//! record reports the count the run actually used, so the derived value is as
//! auditable as a written one.
//!
//! `GF2_PROFILE_ALLOC_TRACE=<n>` prints a backtrace of the first `n`
//! allocations the timed calls make, which is how an allocation count becomes
//! an allocation site.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use consumer_profile_gf2_side::{bank_count, prepare, ArmPath, Case};
use tuning_campaign_support::timing::FIXTURE_BANKS;

/// Number of allocations served since the last reset.
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
/// Bytes requested since the last reset.
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
/// Bytes released since the last reset.
static FREED_BYTES: AtomicU64 = AtomicU64::new(0);
/// Largest live-byte excess observed since the last reset.
static PEAK_EXCESS: AtomicU64 = AtomicU64::new(0);
/// Remaining allocations whose backtrace is printed, when tracing is on.
static TRACE_REMAINING: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// Guards the tracer against itself: capturing a backtrace allocates, and
    /// an unguarded capture would re-enter the allocator without end.
    static TRACING: Cell<bool> = const { Cell::new(false) };
}

/// Prints one backtrace of the allocation now being served.
fn trace_allocation(size: usize) {
    if TRACE_REMAINING.load(Ordering::Relaxed) == 0 {
        return;
    }
    TRACING.with(|guard| {
        if guard.get() {
            return;
        }
        guard.set(true);
        if TRACE_REMAINING
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            eprintln!(
                "# allocation of {size} bytes\n{}",
                std::backtrace::Backtrace::force_capture()
            );
        }
        guard.set(false);
    });
}

/// Counting wrapper over the system allocator.
///
/// It records the counts the profile publishes and forwards every request
/// unchanged, so the measured route allocates exactly what it allocates
/// without the counter.
struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        let allocated = ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed)
            + layout.size() as u64;
        let freed = FREED_BYTES.load(Ordering::Relaxed);
        let live = allocated.saturating_sub(freed);
        PEAK_EXCESS.fetch_max(live, Ordering::Relaxed);
        trace_allocation(layout.size());
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.dealloc(pointer, layout);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// One snapshot of the allocator counters.
#[derive(Clone, Copy)]
struct Counters {
    allocations: u64,
    allocated_bytes: u64,
    freed_bytes: u64,
    peak_excess: u64,
}

impl Counters {
    fn read() -> Self {
        Self {
            allocations: ALLOCATIONS.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            freed_bytes: FREED_BYTES.load(Ordering::Relaxed),
            peak_excess: PEAK_EXCESS.load(Ordering::Relaxed),
        }
    }

    fn since(self, earlier: Self) -> Self {
        Self {
            allocations: self.allocations - earlier.allocations,
            allocated_bytes: self.allocated_bytes - earlier.allocated_bytes,
            freed_bytes: self.freed_bytes - earlier.freed_bytes,
            peak_excess: self.peak_excess,
        }
    }
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("consumer-profile: {}", message.as_ref());
    std::process::exit(2);
}

/// Reads one `--name value` argument.
fn argument(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let case_text = argument(&arguments, "--case").unwrap_or_else(|| fail("--case is required"));
    let path_text = argument(&arguments, "--path").unwrap_or_else(|| fail("--path is required"));
    let cache_state =
        argument(&arguments, "--cache-state").unwrap_or_else(|| fail("--cache-state is required"));
    let declared_calls: Option<u64> = argument(&arguments, "--calls").map(|value| {
        value
            .parse()
            .unwrap_or_else(|error| fail(format!("--calls does not parse: {error}")))
    });
    let target_ms: Option<u64> = argument(&arguments, "--target-ms").map(|value| {
        value
            .parse()
            .unwrap_or_else(|error| fail(format!("--target-ms does not parse: {error}")))
    });
    if declared_calls.is_none() && target_ms.is_none() {
        fail("one of --calls and --target-ms is required");
    }

    let case: Case = serde_json::from_str(&case_text)
        .unwrap_or_else(|error| fail(format!("--case does not decode: {error}")));
    let path =
        ArmPath::parse(&path_text).unwrap_or_else(|error| fail(format!("--path is invalid: {error}")));

    let before_prepare = Counters::read();
    let mut prepared = prepare(&case, path, &cache_state, FIXTURE_BANKS)
        .unwrap_or_else(|error| fail(format!("prepare failed: {error}")));
    let after_prepare = Counters::read();
    let banks = bank_count(&cache_state, FIXTURE_BANKS);

    if cache_state == "warm" {
        prepared.run(0);
    }

    // A target time is filled by the count a doubling burst implies. The burst
    // grows until it spans a millisecond, so the clock read that bounds it is
    // negligible beside the calls it times; a single-call probe would charge
    // its own clock reads to a nanosecond-scale route and undershoot the
    // target by more than an order of magnitude. At least one call always
    // runs, so a route slower than the target still measures once.
    let calls = match declared_calls {
        Some(calls) => calls.max(1),
        None => {
            let target_ns = u128::from(target_ms.expect("one of the two is present")) * 1_000_000;
            let mut burst = 1u64;
            let mut burst_ns;
            loop {
                let probe = Instant::now();
                for call in 0..burst {
                    prepared.run((call as usize) % banks);
                }
                burst_ns = probe.elapsed().as_nanos().max(1);
                if burst_ns >= 1_000_000 || burst >= 1 << 30 {
                    break;
                }
                burst = burst.saturating_mul(2);
            }
            let per_call_ns = burst_ns / u128::from(burst);
            u64::try_from(target_ns / per_call_ns.max(1))
                .unwrap_or(u64::MAX)
                .max(1)
        }
    };

    // The counters reset here so the reported figures describe the timed
    // calls alone; preparation is reported separately.
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    FREED_BYTES.store(0, Ordering::Relaxed);
    PEAK_EXCESS.store(0, Ordering::Relaxed);
    if let Ok(value) = std::env::var("GF2_PROFILE_ALLOC_TRACE") {
        let count: u64 = value
            .parse()
            .unwrap_or_else(|error| fail(format!("GF2_PROFILE_ALLOC_TRACE: {error}")));
        TRACE_REMAINING.store(count, Ordering::Relaxed);
    }
    let before_calls = Counters::read();

    let start = Instant::now();
    for call in 0..calls {
        prepared.run((call as usize) % banks);
    }
    let elapsed_ns = start.elapsed().as_nanos();
    let after_calls = Counters::read();
    black_box(prepared.sink());

    let prepare_counters = after_prepare.since(before_prepare);
    let call_counters = after_calls.since(before_calls);
    let conversion = prepared
        .conversion
        .map(|conversion| serde_json::to_value(conversion).expect("conversion costs serialize"))
        .unwrap_or(serde_json::Value::Null);

    let record = serde_json::json!({
        "schema": "consumer-profile-record-v1",
        "workload": case.workload,
        "size": case.size,
        "seed": case.seed,
        "path": path_text,
        "cache_state": cache_state,
        "banks": banks,
        "calls": calls,
        "call_count_source": if declared_calls.is_some() { "declared" } else { "target-ms" },
        "target_ms": target_ms,
        "elapsed_ns": elapsed_ns.to_string(),
        "ns_per_call": elapsed_ns as f64 / calls as f64,
        "selected_path": prepared.selected_path,
        "workers_observed": prepared.workers_observed,
        "conversion": conversion,
        "prepare": {
            "allocations": prepare_counters.allocations,
            "allocated_bytes": prepare_counters.allocated_bytes,
            "freed_bytes": prepare_counters.freed_bytes,
        },
        "timed": {
            "allocations": call_counters.allocations,
            "allocated_bytes": call_counters.allocated_bytes,
            "freed_bytes": call_counters.freed_bytes,
            "peak_live_bytes": call_counters.peak_excess,
            "allocations_per_call": call_counters.allocations as f64 / calls as f64,
            "allocated_bytes_per_call": call_counters.allocated_bytes as f64 / calls as f64,
        },
    });
    println!(
        "{}",
        serde_json::to_string(&record).expect("the profile record serializes")
    );
}
