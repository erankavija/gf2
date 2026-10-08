//! gf2 steady-state arm of the LDPC batch decoder on the lane route. The plan
//! arm declares the lane backend in [`ldpc_batch_arms::LANE_BACKEND_VAR`].

use ldpc_batch_arms::{declared_lane_backend, run, BatchRoute};
use std::process::ExitCode;

fn main() -> ExitCode {
    match declared_lane_backend()
        .and_then(|backend| run("gf2-batch-lane", BatchRoute::Lane(backend)))
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-lane-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
