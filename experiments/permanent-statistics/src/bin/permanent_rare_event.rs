//! Runs a JSON configuration in one output directory, optionally bounded by a
//! prefix count. Exit status 10 means durable progress remains, 2 means invalid
//! arguments, and 1 means execution failed.

use std::path::Path;
use std::process::ExitCode;

use experiments_common::{RepositoryRoot, RngAlgorithm, RngIdentity, RunRecord};
use permanent_statistics::rare_event::runner::{
    execute_frozen_run, Configuration, RunError, RunStatus,
};

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&arguments.len()) {
        eprintln!(
            "usage: permanent_rare_event <configuration.json> <output-directory> [prefix-budget]"
        );
        return ExitCode::from(2);
    }
    let budget = match arguments
        .get(2)
        .map(|value| value.to_string_lossy().parse::<usize>())
        .transpose()
    {
        Ok(budget) => budget,
        Err(error) => {
            eprintln!("invalid prefix budget: {error}");
            return ExitCode::from(2);
        }
    };
    match run(Path::new(&arguments[0]), Path::new(&arguments[1]), budget) {
        Ok(RunStatus::Complete) => ExitCode::SUCCESS,
        Ok(RunStatus::Incomplete) => ExitCode::from(10),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run(
    configuration: &Path,
    directory: &Path,
    budget: Option<usize>,
) -> Result<RunStatus, RunError> {
    let configuration: Configuration = serde_json::from_slice(&std::fs::read(configuration)?)?;
    configuration.validate()?;
    let root = RepositoryRoot::containing(&std::env::current_dir()?)?;
    let record = RunRecord::observe(
        &root,
        configuration.digest()?,
        configuration.root_seed,
        RngIdentity {
            algorithm: RngAlgorithm::ChaCha20,
            version: "rand_chacha 0.9.0".into(),
        },
    )?;
    let outcome = execute_frozen_run(&configuration, &record, directory, budget)?;
    println!(
        "{:?}: {} trajectories, prefix {}",
        outcome.status, outcome.completed_trajectories, outcome.completed_prefix
    );
    if let Some(summary) = outcome.summary {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    }
    Ok(outcome.status)
}
