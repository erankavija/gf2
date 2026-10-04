//! Runner for the independent importance-sampling cross-check. Its one argument
//! is the repository-relative path of a frozen configuration in the schema of
//! the preregistered design; the run is
//! [`gf2_sim::permanent_rare_event::runner`].
//!
//! # Environment
//!
//! `RAYON_NUM_THREADS` declares the worker count. `GF2_RARE_EVENT_BLOCK_BUDGET`
//! optionally bounds one invocation to that many checkpoint blocks; the next
//! invocation continues from the published prefix.
//!
//! # Exit status
//!
//! `0` the dataset is complete, `10` durable progress was published and blocks
//! remain, `2` the argument vector is not one configuration path, and `1` any
//! refusal.

use std::process::ExitCode;

use gf2_sim::permanent_rare_event::artifact::decode_configuration;
use gf2_sim::permanent_rare_event::runner::{execute_frozen_run, RunError, RunStatus};

const EXIT_INCOMPLETE: u8 = 10;
const EXIT_REFUSED: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [configuration_path] = arguments.as_slice() else {
        eprintln!("usage: permanent_rare_event <frozen-configuration-path>");
        return ExitCode::from(EXIT_USAGE);
    };
    match run(configuration_path) {
        Ok(RunStatus::Complete) => ExitCode::SUCCESS,
        Ok(RunStatus::Incomplete) => ExitCode::from(EXIT_INCOMPLETE),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(EXIT_REFUSED)
        }
    }
}

fn run(configuration_path: &str) -> Result<RunStatus, RunError> {
    let bytes = std::fs::read(configuration_path)?;
    let configuration = decode_configuration(&bytes)?;
    let outcome = execute_frozen_run(configuration_path, &configuration)?;
    println!(
        "dataset {} attempt {} published {} blocks, {} remaining",
        outcome.dataset_id(),
        outcome.attempt_id(),
        outcome.published_blocks(),
        outcome.remaining_blocks()
    );
    if let Some(digest) = outcome.final_receipt_sha256() {
        println!("final receipt {digest}");
    }
    Ok(outcome.status())
}
