//! Independent acceptance evaluation of a Zen 3 benchmark receipt directory.
//!
//! `benchmark-acceptance <receipt-dir> [--repo <root>]` recomputes every
//! digest, statistic and decision from the receipt's raw parts, writes
//! `acceptance-summary.json` and `acceptance-summary.md` beside the receipt,
//! and exits 0 when the receipt is accepted, 1 when it is rejected, and 2 on
//! a usage or I/O error.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tuning_campaign_support::receipt::{
    evaluate, render_markdown, Verdict, SUMMARY_JSON_FILE, SUMMARY_MARKDOWN_FILE,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut receipt_dir: Option<PathBuf> = None;
    let mut repo: Option<PathBuf> = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--repo" {
            repo = iter.next().map(PathBuf::from);
            if repo.is_none() {
                usage();
            }
        } else if receipt_dir.is_none() {
            receipt_dir = Some(PathBuf::from(arg));
        } else {
            usage();
        }
    }
    let Some(receipt_dir) = receipt_dir else {
        usage()
    };
    let summary = match evaluate(&receipt_dir, repo.as_deref().map(Path::new)) {
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

fn usage() -> ! {
    eprintln!("usage: benchmark-acceptance <receipt-dir> [--repo <repository-root>]");
    std::process::exit(2);
}
