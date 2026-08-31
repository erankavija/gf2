//! Execute the frozen pre-draw validation phase of a permanent-zero-fraction
//! campaign.
//!
//! This CLI is a thin consumer. It supplies only this repository's frozen
//! constants — the committed preregistration path, the durable journal
//! directory, the receipt destination, and the worker count — and delegates
//! every exhaustive comparison, regeneration cross-check, exact decision, and
//! provenance observation to `gf2_sim::permanent_campaign::validation`. It
//! proposes no anchor, decides no verdict, and authors no provenance.
//!
//! ```console
//! $ permanent_validation \
//!     --preregistration dev/active/02b8137c/pre-draw-validation-v1-preregistration.json \
//!     --state-dir dev/active/02b8137c/validation-journal \
//!     --receipt dev/active/02b8137c/pre-draw-validation-v2-receipt.json \
//!     --workers 32
//! ```
//!
//! Paths are interpreted relative to the repository root, which is resolved
//! from the working directory. The preregistration must be committed before
//! the first validation draw: the run binds its content identity into the
//! receipt and refuses a plan whose recorded authority digests disagree with
//! the committed protocol, manifest, and mechanical evidence.
//!
//! The journal makes the run resumable and enforces the protocol's no-redraw
//! rule. An anchor whose durable start marker exists without a terminal record
//! is preserved as an interruption failure; its address is never reopened. A
//! resumed run repeats the identical command from the identical source closure:
//! the journal binds itself to the producer identity it observed, argument
//! tokens included, and refuses a journal recorded under a different one.
//! The sole exception is explicit `--continue-producer-segment PATH`: `PATH`
//! must be the committed schema-v2 owner authorization for one immutable
//! ordered boundary. The library rehashes the complete authorized prefix and
//! durably publishes the observed second-producer state before opening the
//! next address. No third producer is admitted.
//!
//! The frozen plan pins its producing toolchain and requires every backend the
//! frozen manifest selects. The runner refuses a build from another compiler,
//! and a build or host on which a required backend cannot execute, before it
//! opens the first address.
//!
//! ```console
//! $ permanent_validation --verify-receipt dev/active/02b8137c/pre-draw-validation-v2-receipt.json
//! ```
//!
//! `--verify-receipt` re-reads a committed receipt, revalidates it against the
//! committed frozen preregistration, and reports its verdict without opening a
//! sampler. This is the launch check consumed before campaign arms unblock.
//!
//! Exit status is `0` when validation passes, `2` when the preserved evidence
//! records a failed anchor or a changed frozen artifact, and `1` for a usage or
//! runtime error that produced no verdict.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf2_sim::permanent_campaign::provenance::repository_top_level;
use gf2_sim::permanent_campaign::validation::{
    load_frozen_campaign_validation_preregistration, load_validation_continuation_authorization,
    publish_validation_receipt_atomic, read_frozen_validation_receipt,
    run_frozen_campaign_validation_with_mode, ValidationReceipt, ValidationRunMode,
    FROZEN_VALIDATION_RECEIPT_PATH,
};

const USAGE: &str = "usage: permanent_validation --preregistration PATH --state-dir PATH --receipt PATH [--workers N] [--continue-producer-segment PATH]

       permanent_validation --verify-receipt PATH

  --preregistration PATH  committed frozen ten-anchor preregistration, repository-relative
  --state-dir PATH        durable no-redraw journal directory
  --receipt PATH          canonical immutable schema-v2 receipt destination
  --workers N             worker count for production evaluation (default: 1)
  --continue-producer-segment PATH
                           committed schema-v2 owner authorization for one second producer
  --verify-receipt PATH   revalidate a committed receipt against the frozen plan
";

const FAILED_VALIDATION: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut preregistration: Option<PathBuf> = None;
    let mut state_directory: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut verify_receipt: Option<PathBuf> = None;
    let mut continuation: Option<PathBuf> = None;
    let mut workers = 1_usize;
    let mut index = 0;
    while index < arguments.len() {
        let name = arguments[index].as_str();
        let value = arguments.get(index + 1).map(String::as_str);
        match (name, value) {
            ("--preregistration", Some(path)) => {
                preregistration = Some(PathBuf::from(path));
                index += 2;
            }
            ("--state-dir", Some(path)) => {
                state_directory = Some(PathBuf::from(path));
                index += 2;
            }
            ("--receipt", Some(path)) => {
                receipt = Some(PathBuf::from(path));
                index += 2;
            }
            ("--verify-receipt", Some(path)) => {
                verify_receipt = Some(PathBuf::from(path));
                index += 2;
            }
            ("--continue-producer-segment", Some(path)) => {
                continuation = Some(PathBuf::from(path));
                index += 2;
            }
            ("--workers", Some(value)) => match value.parse::<usize>() {
                Ok(parsed) if parsed >= 1 => {
                    workers = parsed;
                    index += 2;
                }
                _ => return usage("--workers must be an integer of at least 1"),
            },
            _ => return usage("unrecognized or incomplete argument"),
        }
    }

    let repository = match repository_top_level(Path::new(".")) {
        Ok(repository) => repository,
        Err(error) => return failure(&error),
    };

    if let Some(path) = verify_receipt {
        if preregistration.is_some()
            || state_directory.is_some()
            || receipt.is_some()
            || continuation.is_some()
        {
            return usage("--verify-receipt cannot be combined with execution options");
        }
        return match read_frozen_validation_receipt(&repository, &repository.join(path)) {
            Ok(receipt) => report(&receipt),
            Err(error) => failure(&error),
        };
    }

    let (Some(preregistration), Some(state_directory), Some(receipt_path)) =
        (preregistration, state_directory, receipt)
    else {
        return usage("execution needs --preregistration, --state-dir, and --receipt");
    };
    if receipt_path != Path::new(FROZEN_VALIDATION_RECEIPT_PATH) {
        return usage("--receipt must name the canonical schema-v2 validation receipt");
    }

    let (plan, identity) =
        match load_frozen_campaign_validation_preregistration(&repository, &preregistration) {
            Ok(loaded) => loaded,
            Err(error) => return failure(&error),
        };
    let mode = match continuation {
        Some(path) => match load_validation_continuation_authorization(&repository, &path) {
            Ok(authorization) => ValidationRunMode::ContinueWith(Box::new(authorization)),
            Err(error) => return failure(&error),
        },
        None => ValidationRunMode::ExactProducer,
    };
    let receipt = match run_frozen_campaign_validation_with_mode(
        &repository,
        &plan,
        identity,
        workers,
        &repository.join(state_directory),
        mode,
    ) {
        Ok(receipt) => receipt,
        Err(error) => return failure(&error),
    };
    if let Err(error) = publish_validation_receipt_atomic(&repository.join(receipt_path), &receipt)
    {
        return failure(&error);
    }
    report(&receipt)
}

fn report(receipt: &ValidationReceipt) -> ExitCode {
    match serde_json::to_string_pretty(receipt) {
        Ok(rendered) => println!("{rendered}"),
        Err(error) => return failure(&error),
    }
    if receipt.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILED_VALIDATION)
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("error: {message}\n\n{USAGE}");
    ExitCode::FAILURE
}

fn failure(error: &dyn std::error::Error) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::FAILURE
}
