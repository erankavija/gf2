//! Reporter-free timed worker for the tuning calibration campaign.
//!
//! The shared harness module selects this role from the absence of the
//! `test-support` feature. The authoritative build compiles this normal binary
//! separately from the controller bench so production dispatchers contain no
//! test-support observation stores during timed windows.

#[allow(dead_code)]
#[path = "../../benches/tuning_calibration.rs"]
mod harness;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    harness::timed_worker_main()
}
