//! Independent acceptance evaluation of a Zen 3 benchmark receipt directory:
//! `benchmark-acceptance <receipt-dir>` recomputes every digest, statistic and
//! decision from the receipt's raw parts, writes `acceptance-summary.json` and
//! `acceptance-summary.md` beside the receipt, and exits 0 when the receipt is
//! accepted, 1 when it is rejected, and 2 on a usage or I/O error.
//! `benchmark-acceptance --bootstrap` reads an `abtest::BootstrapRequest` from
//! standard input and prints its intervals, so a sizing script takes the
//! evaluator's bootstrap of the pairs it names. `benchmark-acceptance
//! --next-attempt <ledger.jsonl> <family-id>` prints the next confirmatory
//! attempt that family ledger admits at the frozen shared settings.

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use tuning_campaign_support::abtest::BootstrapRequest;
use tuning_campaign_support::protocol::SHARED_SETTINGS;
use tuning_campaign_support::receipt::{
    evaluate, render_markdown, Verdict, MIN_TAIL_DRAWS, SUMMARY_JSON_FILE, SUMMARY_MARKDOWN_FILE,
};
use tuning_campaign_support::trial_ledger;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("--bootstrap") {
        if args.len() != 2 {
            usage();
        }
        bootstrap();
    }
    if args.get(1).map(String::as_str) == Some("--next-attempt") {
        match args.as_slice() {
            [_, _, ledger, family] => next_attempt(ledger, family),
            _ => usage(),
        }
    }
    let mut receipt_dir: Option<PathBuf> = None;
    for arg in args.iter().skip(1) {
        if receipt_dir.is_none() {
            receipt_dir = Some(PathBuf::from(arg));
        } else {
            usage();
        }
    }
    let Some(receipt_dir) = receipt_dir else {
        usage()
    };
    let summary = match evaluate(&receipt_dir) {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("benchmark-acceptance: {error}");
            std::process::exit(2);
        }
    };
    let mut json = match serde_json::to_vec_pretty(&summary) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("benchmark-acceptance: {error}");
            std::process::exit(2);
        }
    };
    json.push(b'\n');
    if let Err(error) = fs::write(receipt_dir.join(SUMMARY_JSON_FILE), json).and_then(|()| {
        fs::write(
            receipt_dir.join(SUMMARY_MARKDOWN_FILE),
            render_markdown(&summary),
        )
    }) {
        eprintln!("benchmark-acceptance: {error}");
        std::process::exit(2);
    }
    println!(
        "GF2_BENCHMARK_ACCEPTANCE={:?} qualifies={} findings={}",
        summary.verdict,
        summary.qualifies,
        summary.findings.len()
    );
    std::process::exit(if summary.verdict == Verdict::Accepted {
        0
    } else {
        1
    });
}

fn bootstrap() -> ! {
    let printed = serde_json::from_reader::<_, BootstrapRequest>(io::stdin().lock())
        .map_err(|error| error.to_string())
        .and_then(|request| request.intervals().map_err(|error| error.to_string()))
        .and_then(|sized| serde_json::to_string_pretty(&sized).map_err(|error| error.to_string()));
    match printed {
        Ok(json) => {
            println!("{json}");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("benchmark-acceptance: {error}");
            std::process::exit(2);
        }
    }
}

fn next_attempt(ledger: &str, family: &str) -> ! {
    let printed = fs::read(ledger)
        .and_then(|bytes| trial_ledger::next_attempt(&bytes, family, &SHARED_SETTINGS))
        .map_err(|error| error.to_string())
        .and_then(|next| {
            serde_json::to_string_pretty(&serde_json::json!({
                "next_attempt": next,
                "family_alpha": SHARED_SETTINGS.family_alpha,
                "bootstrap_resamples": SHARED_SETTINGS.bootstrap_resamples,
                "min_tail_draws": MIN_TAIL_DRAWS,
            }))
            .map_err(|error| error.to_string())
        });
    match printed {
        Ok(json) => {
            println!("{json}");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("benchmark-acceptance: {error}");
            std::process::exit(2);
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: benchmark-acceptance <receipt-dir> | --bootstrap < request.json | \
         --next-attempt <ledger.jsonl> <family-id>"
    );
    std::process::exit(2);
}
