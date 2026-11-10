//! `qc-route-sweep OUT_DIR` runs or resumes the route sweep of
//! [`aff3ct_ldpc_gaps::route_sweep`] over its issue grid, with its execution
//! log in `OUT_DIR`. It measures only under an inherited benchmark lock, as
//! `dev/scripts/ccx1-bench-flock.sh --full-host` grants in a benchmark window.

use aff3ct_ldpc_gaps::route_sweep::{run, SweepConfig};
use experiments_common::RepositoryRoot;
use std::path::Path;
use std::process::ExitCode;

const LOCK_PATH: &str = "/tmp/gf2-ccx1.lock";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [out_dir] = arguments.as_slice() else {
        eprintln!("usage: qc-route-sweep OUT_DIR");
        return ExitCode::from(2);
    };
    let outcome = tuning_campaign_support::host::inherited_lock(Path::new(LOCK_PATH))
        .map_err(|error| format!("{LOCK_PATH}: {error}"))
        .and_then(|_| RepositoryRoot::containing(Path::new(".")).map_err(|e| e.to_string()))
        .and_then(|root| run(&SweepConfig::issue_grid(), Path::new(out_dir), &root));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qc-route-sweep: {error}");
            ExitCode::FAILURE
        }
    }
}
