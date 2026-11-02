//! Preregistered validation; status 10 means terminal anchors remain.
mod support;
use experiments_common::{RepositoryRoot, RunRecord};
use permanent_statistics::{
    campaign::{
        validation::{run_validation, ValidationStatus},
        CampaignConfig,
    },
    sampler_rng,
};
use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    let parsed = (|| {
        let values = support::arguments(
            &["--config", "--output", "--workers", "--max-anchors"],
            &["--config", "--output"],
        )?;
        let workers = support::count(&values, "--workers")?.unwrap_or(1);
        let budget = support::count(&values, "--max-anchors")?;
        Ok::<_, String>((values, workers, budget))
    })();
    let (values, workers, budget) = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let run = (|| {
        let config = CampaignConfig::load(Path::new(&values["--config"]))?;
        let root = RepositoryRoot::containing(&std::env::current_dir()?)?;
        let record = RunRecord::observe(
            &root,
            config.digest()?,
            config.validation.root_seed,
            sampler_rng(),
        )?;
        run_validation(
            &config,
            &record,
            Path::new(&values["--output"]),
            workers,
            budget,
        )
    })();
    match run {
        Ok(ValidationStatus::Passed) => ExitCode::SUCCESS,
        Ok(ValidationStatus::Incomplete) => ExitCode::from(10),
        Ok(ValidationStatus::Failed) => ExitCode::from(1),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
