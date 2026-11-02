//! Bounded permanent campaign invocation; status 10 means progress remains.
mod support;
use experiments_common::{RepositoryRoot, RunRecord};
use permanent_statistics::{
    campaign::{
        runner::{run_cell, Invocation, OnUnavailable, RunStatus},
        CampaignConfig,
    },
    sampler_rng,
};
use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    let parsed = (|| {
        let values = support::arguments(
            &[
                "--config",
                "--output",
                "--q",
                "--n",
                "--workers",
                "--max-shards",
                "--on-unavailable",
            ],
            &["--config", "--output", "--q", "--n"],
        )?;
        let q = values["--q"]
            .parse::<u8>()
            .map_err(|_| "invalid --q".to_owned())?;
        let n = values["--n"]
            .parse::<u16>()
            .map_err(|_| "invalid --n".to_owned())?;
        if !matches!(q, 3 | 5 | 7) || n == 0 {
            return Err("invalid cell selector".into());
        }
        let invocation = Invocation {
            workers: support::count(&values, "--workers")?.unwrap_or(1),
            max_shards: support::count(&values, "--max-shards")?,
            on_unavailable: values
                .get("--on-unavailable")
                .map(|s| s.parse::<OnUnavailable>().map_err(|e| e.to_string()))
                .transpose()?
                .unwrap_or(OnUnavailable::Refuse),
            argv: std::env::args().collect(),
        };
        Ok::<_, String>((values, q, n, invocation))
    })();
    let (values, q, n, invocation) = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let config = match CampaignConfig::load(Path::new(&values["--config"])) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if !config.cells().iter().any(|c| (c.q, c.n) == (q, n)) {
        eprintln!("unknown cell selector");
        return ExitCode::from(2);
    }
    let run = (|| {
        let root = RepositoryRoot::containing(&std::env::current_dir()?)?;
        let record = RunRecord::observe(&root, config.digest()?, config.root_seed, sampler_rng())?;
        run_cell(
            &config,
            &record,
            Path::new(&values["--output"]),
            q,
            n,
            &invocation,
        )
    })();
    match run {
        Ok(RunStatus::Terminal) => ExitCode::SUCCESS,
        Ok(RunStatus::Incomplete) => ExitCode::from(10),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
