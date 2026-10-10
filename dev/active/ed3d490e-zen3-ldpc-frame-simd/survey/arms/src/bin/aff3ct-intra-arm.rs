//! AFF3CT steady-state arm of the float sum-product INTRA decoder.

use std::process::ExitCode;

fn main() -> ExitCode {
    match ldpc_batch_arms::aff3ct_intra::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aff3ct-intra-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
