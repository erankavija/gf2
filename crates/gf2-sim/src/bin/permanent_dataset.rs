//! Inspect, checksum, and verify a published permanent-zero-fraction dataset.
//!
//! This is the reader- and finalization-side tool for the dataset described in
//! `dev/simulation_results/permanent-zero-fraction/README.md`. It is not the
//! campaign driver: it draws no matrices and writes no dataset file. It exists
//! so the source-identity guard and the integrity layer are runnable by hand,
//! and so the revision `gf2-sim` embeds at build time is carried by a real
//! binary rather than only by the library.
//!
//! ```console
//! $ permanent_dataset revision
//! $ permanent_dataset emission-check <campaign-directory>
//! $ permanent_dataset checksums <campaign-directory> > <campaign-directory>/checksums.sha256
//! $ permanent_dataset verify <campaign-directory>
//! ```
//!
//! `revision` prints the embedded source revision, which equals
//! `git rev-parse HEAD` exactly when this binary is current with the checkout.
//! `emission-check` runs the guard a campaign driver must pass before writing.
//! `checksums` renders the integrity file for a finished dataset on stdout.
//! `verify` re-checks a dataset against that file and its recorded source.
//!
//! Exit status: `0` for success or a verified dataset, `1` for a refusal, a
//! failed dataset, or an error, `2` for a dataset whose provenance could not be
//! decided, and `64` for a usage error.

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use gf2_sim::permanent_campaign::provenance::{
    approve_emission, build_revision, generate_integrity_file, verify_dataset, DatasetVerdict,
};
use gf2_sim::permanent_campaign::schema::read_manifest;

const USAGE: &str =
    "usage: permanent_dataset <revision | emission-check | checksums | verify> [campaign-directory]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match arguments.as_slice() {
        ["revision"] => {
            println!("{}", build_revision());
            ExitCode::SUCCESS
        }
        ["emission-check", root] => emission_check(Path::new(root)),
        ["checksums", root] => checksums(Path::new(root)),
        ["verify", root] => verify(Path::new(root)),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}

fn emission_check(root: &Path) -> ExitCode {
    match approve_emission(root) {
        Ok(approval) => {
            println!("emission approved at revision {}", approval.revision());
            ExitCode::SUCCESS
        }
        Err(refusal) => report(&refusal),
    }
}

fn checksums(root: &Path) -> ExitCode {
    let manifest = match read_manifest(root) {
        Ok(manifest) => manifest,
        Err(error) => return report(&error),
    };
    match generate_integrity_file(root, &manifest) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => report(&error),
    }
}

fn verify(root: &Path) -> ExitCode {
    match verify_dataset(root) {
        Ok(DatasetVerdict::Verified) => {
            println!("verified");
            ExitCode::SUCCESS
        }
        Ok(verdict @ DatasetVerdict::Failed { .. }) => {
            eprintln!("{verdict}");
            ExitCode::FAILURE
        }
        Ok(verdict @ DatasetVerdict::Unverifiable { .. }) => {
            eprintln!("{verdict}");
            ExitCode::from(2)
        }
        Err(error) => report(&error),
    }
}

fn report(error: &dyn Error) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}
