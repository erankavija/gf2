//! Inspect, checksum, conform, and verify a published permanent-zero-fraction dataset.
//!
//! This is the reader- and finalization-side tool for the dataset described in
//! `dev/simulation_results/permanent-zero-fraction/README.md`. It exists so the
//! source-identity guard and the integrity layer are runnable by hand, and so
//! runtime-observed provenance is carried by a real executable rather than
//! only by the library that any executable links.
//!
//! The campaign driver is the `permanent_campaign` binary. It enumerates the
//! frozen manifest's work items, derives stream addresses, draws matrices,
//! evaluates permanents and any manifest-requested determinant companions on
//! the same row-major samples, accumulates shard records, and writes shard
//! files and the field summary through the emission guard. This binary has no
//! sampler, no backend selection, and no
//! accumulator, and it never creates or mutates a dataset file: `checksums`
//! prints to standard output rather than writing `checksums.sha256`, so the
//! executable has no write path into a dataset at all.
//!
//! ```console
//! $ permanent_dataset revision
//! $ permanent_dataset emission-check <campaign-directory> [emitter-path]
//! $ permanent_dataset checksums <campaign-directory> > <campaign-directory>/checksums.sha256
//! $ permanent_dataset conform <campaign-directory>
//! $ permanent_dataset verify <campaign-directory>
//! ```
//!
//! `revision` prints the repository-wide revision observed at command start as
//! provenance context.
//! `emission-check` verifies every emission guard. Without an `emitter-path`,
//! it verifies everything except writer identity and reports the pinned emitter
//! digest; writer identity is asserted only by the writer's own guard at
//! emission time. With an `emitter-path`, it hashes that file and performs the
//! full check, including writer identity.
//! `checksums` renders the integrity file for a finished dataset on stdout.
//! `conform` checks the complete schema and all cross-document aggregates.
//! `verify` re-checks a dataset against that file and its recorded source.
//!
//! Exit status: `0` for success or a verified dataset, `1` for a refusal, a
//! failed dataset, or an error, `2` for a dataset whose provenance could not be
//! decided, and `64` for a usage error.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use gf2_sim::permanent_campaign::provenance::{
    approve_emission_with_binary_digest, generate_integrity_file, runtime_git_revision,
    verify_dataset, DatasetVerdict,
};
use gf2_sim::permanent_campaign::schema::{conform_dataset, read_manifest, Sha256Digest};
use sha2::{Digest, Sha256};

const USAGE: &str = "usage: permanent_dataset <revision | emission-check | checksums | conform | \
                    verify> [campaign-directory]\n       permanent_dataset emission-check <campaign-directory> [emitter-path]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match arguments.as_slice() {
        ["revision"] => match runtime_git_revision() {
            Ok(revision) => {
                println!("{revision}");
                ExitCode::SUCCESS
            }
            Err(error) => report(&error),
        },
        ["emission-check", root] => emission_check(Path::new(root), None),
        ["emission-check", root, emitter] => {
            emission_check(Path::new(root), Some(Path::new(emitter)))
        }
        ["checksums", root] => checksums(Path::new(root)),
        ["conform", root] => conform(Path::new(root)),
        ["verify", root] => verify(Path::new(root)),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}

fn emission_check(root: &Path, emitter_path: Option<&Path>) -> ExitCode {
    let binary_sha256 = match emitter_path {
        Some(path) => match emitter_digest(path) {
            Ok(digest) => Some(digest),
            Err(error) => return report(error.as_ref()),
        },
        None => None,
    };
    match approve_emission_with_binary_digest(binary_sha256, root) {
        Ok(approval) => match emitter_path {
            Some(_) => {
                println!("emission approved for binary {}", approval.binary_sha256());
                ExitCode::SUCCESS
            }
            None => {
                println!(
                    "everything except writer identity is verified; the pinned emitter digest is {}; writer identity is asserted only by the writer's own guard at emission time.",
                    approval.binary_sha256()
                );
                ExitCode::SUCCESS
            }
        },
        Err(refusal) => report(&refusal),
    }
}

fn emitter_digest(path: &Path) -> Result<Sha256Digest, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}").parse()?)
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

fn conform(root: &Path) -> ExitCode {
    match conform_dataset(root) {
        Ok(_) => {
            println!("conforms");
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

#[cfg(test)]
mod tests {
    use super::USAGE;

    #[test]
    fn usage_lists_canonical_conformance() {
        assert!(USAGE.contains("conform"));
    }

    #[test]
    fn usage_documents_optional_emitter_path() {
        assert!(USAGE.contains("emission-check <campaign-directory> [emitter-path]"));
    }
}
