//! gf2 steady-state arm of the LDPC batch decoder on the single-frame route.

use ldpc_batch_arms::{run, BatchRoute};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run("gf2-batch-single-frame", BatchRoute::SingleFrame) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-single-frame-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
