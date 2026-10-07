//! Bootstrap width and endpoint stability of a receipt at a chosen alpha.
//!
//! Usage: `bootstrap-resolution <receipt.json> <alpha>`
//!
//! For every measured cell this recomputes the protocol's paired percentile
//! bootstrap from the receipt's raw pairs with the canonical
//! `tuning_campaign_support::abtest` implementation, the frozen 10 000
//! resamples and the receipt's own cell seed. It prints the interval, the
//! conservative relative half-width `max(estimate - lower, upper - estimate) /
//! estimate`, and the P-20 endpoint shift: the largest endpoint movement,
//! relative to the estimate, when the same interval is recomputed from the
//! independent seed stream `cell_seed ^ 0xd1b54a32d192ed03`.
//!
//! Evaluated at the receipt's own corrected alpha it reproduces the acceptance
//! tool's intervals and P-20 endpoint test. Evaluated on a pilot at a
//! confirmation's corrected alpha it shows how wide and how stable that
//! confirmation's intervals would be on the pilot's data.

use serde_json::Value;
use tuning_campaign_support::abtest::{
    bootstrap_seed, paired_bootstrap_speedup, PairedObservation,
};

/// The protocol's frozen `bootstrap_resamples`.
const RESAMPLES: u32 = 10_000;
/// The protocol's independent P-20 seed stream.
const CHECK_STREAM: u64 = 0xd1b54a32d192ed03;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, path, alpha] = args.as_slice() else {
        eprintln!("usage: bootstrap-resolution <receipt.json> <alpha>");
        std::process::exit(2);
    };
    let alpha: f64 = alpha.parse().expect("alpha is a number");
    let receipt: Value = serde_json::from_slice(&std::fs::read(path).expect("receipt is readable"))
        .expect("receipt is JSON");
    let campaign_seed = receipt["campaign_seed"]
        .as_u64()
        .expect("receipt has a campaign seed");
    println!(
        "receipt {path}\ncampaign {} seed {campaign_seed}\nalpha {alpha} confidence {:.6} resamples {RESAMPLES}\n",
        receipt["campaign_id"].as_str().unwrap_or_default(),
        1.0 - alpha
    );
    println!(
        "{:<52}{:>9}{:>9}{:>9}{:>10}{:>10}",
        "cell", "estimate", "lower", "upper", "rel_half", "shift"
    );
    let mut widest = (0.0_f64, String::new());
    let mut unstable = (0.0_f64, String::new());
    for cell in receipt["cells"].as_array().expect("receipt has cells") {
        let Some(pairs) = cell["pairs"].as_array().filter(|pairs| !pairs.is_empty()) else {
            continue;
        };
        let pairs: Vec<PairedObservation> = pairs
            .iter()
            .map(|pair| PairedObservation {
                baseline_ns_per_call: pair["baseline"]["ns_per_call"]
                    .as_f64()
                    .expect("baseline value"),
                candidate_ns_per_call: pair["candidate"]["ns_per_call"]
                    .as_f64()
                    .expect("candidate value"),
            })
            .collect();
        let key = cell["key"].as_str().expect("cell key");
        let seed = bootstrap_seed(campaign_seed, key);
        let interval = paired_bootstrap_speedup(&pairs, RESAMPLES, alpha, seed).expect("bootstrap");
        let check = paired_bootstrap_speedup(&pairs, RESAMPLES, alpha, seed ^ CHECK_STREAM)
            .expect("bootstrap");
        let half = (interval.estimate - interval.lower).max(interval.upper - interval.estimate)
            / interval.estimate;
        let shift = (check.lower - interval.lower)
            .abs()
            .max((check.upper - interval.upper).abs())
            / interval.estimate;
        println!(
            "{key:<52}{:>9.4}{:>9.4}{:>9.4}{half:>10.4}{shift:>10.4}",
            interval.estimate, interval.lower, interval.upper
        );
        if half > widest.0 {
            widest = (half, key.to_owned());
        }
        if shift > unstable.0 {
            unstable = (shift, key.to_owned());
        }
    }
    println!();
    println!("widest relative half-width: {:.6} ({})", widest.0, widest.1);
    println!(
        "largest endpoint shift:     {:.6} ({})",
        unstable.0, unstable.1
    );
    println!(
        "larger of the two rounded up to two decimals: {:.2}",
        (widest.0.max(unstable.0) * 100.0).ceil() / 100.0
    );
}
