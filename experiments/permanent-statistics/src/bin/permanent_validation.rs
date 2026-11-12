//! Preregistered validation; status 10 means terminal anchors remain. A run
//! prints its executable digest first, as every multi-window harness does.
//! `--check PATH` only parses the configuration and reports it: no stream, device, or file.
mod support;
use experiments_common::{write_executable_digests, RepositoryRoot, RunRecord};
use permanent_statistics::{
    campaign::{
        validation::{run_validation, ValidationStatus},
        CampaignConfig, CampaignError,
    },
    sampler_rng,
};
use std::{path::Path, process::ExitCode};

fn check() -> ExitCode {
    let values = match support::arguments(&["--check"], &["--check"]) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let report = (|| {
        let config = CampaignConfig::load(Path::new(&values["--check"]))?;
        let cells = config.cells();
        let validation = &config.validation;
        println!("campaign_id {}", config.campaign_id);
        println!("configuration_sha256 {}", config.digest()?);
        println!("cells {}", cells.len());
        println!(
            "shards {}",
            cells.iter().map(|c| c.shard_count()).sum::<u64>()
        );
        println!("first_cell {:?}", config.execution.first_cell);
        println!("validation_root_seed {}", validation.root_seed);
        println!("validation_anchors {}", validation.anchors.len());
        println!("sample_matrix_count {}", validation.sample_matrix_count);
        println!(
            "regeneration_matrix_count {}",
            validation.regeneration_matrix_count
        );
        println!("level {}", validation.level);
        Ok::<_, CampaignError>(())
    })();
    match report {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    if std::env::args().any(|a| a == "--check") {
        return check();
    }
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
        write_executable_digests(&mut std::io::stdout(), std::iter::empty::<&Path>())?;
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
