//! Production runner for the independent importance-sampling cross-check.
//!
//! The binary takes exactly one argument: the repository-relative path of the
//! frozen configuration. It decodes that configuration, delegates the whole run
//! to [`gf2_sim::permanent_rare_event::runner`], and maps the returned status
//! onto an exit code. Proposal, weighting, interval, effective-sample-size, and
//! artifact-lifecycle logic all live in the library.
//!
//! # Where a frozen configuration comes from
//!
//! A frozen configuration is authored to the schema the preregistered design
//! `dev/active/3f664839/design.md` fixes, and reaches this binary only through
//! [`decode_configuration`], which refuses a non-canonical encoding, an unknown
//! schema, or any field outside that closed grammar. The issue that executes a
//! campaign commits its frozen configuration before its first draw, so every
//! artifact the run publishes cites immutable preregistered bytes by path and
//! digest.
//!
//! This issue builds and exercises the estimator and its runner rather than
//! executing a campaign, so it deliberately commits no configuration of its
//! own. No path in this file names a file that exists in the repository.
//!
//! `RAYON_NUM_THREADS` declares the worker count. `GF2_RARE_EVENT_BLOCK_BUDGET`
//! optionally bounds one invocation to that many checkpoint blocks; the run is
//! resumable, so the next invocation continues from the published prefix.
//!
//! ```text
//! RAYON_NUM_THREADS=8 GF2_RARE_EVENT_BLOCK_BUDGET=64 \
//!     cargo run -p gf2-sim --release --bin permanent_rare_event -- \
//!     <repository-relative path of the frozen configuration>
//! ```
//!
//! Exit codes: `0` the dataset is complete, `10` durable progress was published
//! and blocks remain, `2` the argument vector is not one configuration path,
//! and `1` any refusal.

use std::process::ExitCode;

use gf2_sim::permanent_rare_event::artifact::decode_configuration;
use gf2_sim::permanent_rare_event::runner::{execute_frozen_run, RunError, RunStatus};

/// Exit code reporting that blocks remain for a following bounded run.
const EXIT_INCOMPLETE: u8 = 10;
/// Exit code reporting a refusal.
const EXIT_REFUSED: u8 = 1;
/// Exit code reporting an argument vector that is not one configuration path.
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

/// Decodes the frozen configuration and delegates the run to the library.
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
