//! `benchmark-acceptance --bootstrap`: the canonical bootstrap of caller-given
//! pairs, the entry a sizing script reads instead of resampling on its own.

use std::io::Write;
use std::process::{Command, Stdio};
use tuning_campaign_support::abtest::{
    paired_bootstrap_speedup, BootstrapRequest, PairedObservation, SizedInterval,
};

fn request() -> BootstrapRequest {
    BootstrapRequest {
        pairs: (1..=12)
            .map(|index| PairedObservation {
                baseline_ns_per_call: 200.0 + f64::from(index * index) / 7.0,
                candidate_ns_per_call: 190.0 + f64::from(index) / 3.0,
            })
            .collect(),
        resamples: 1000,
        seed: u64::MAX - 5,
        alphas: vec![0.05, 0.0125],
    }
}

fn run(input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_benchmark-acceptance"))
        .arg("--bootstrap")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn request_intervals_are_the_canonical_bootstrap_at_each_alpha() {
    let request = request();
    let sized = request.intervals().unwrap();
    assert_eq!(sized.len(), request.alphas.len());
    for (found, alpha) in sized.iter().zip(&request.alphas) {
        let canonical =
            paired_bootstrap_speedup(&request.pairs, request.resamples, *alpha, request.seed)
                .unwrap();
        assert_eq!(found.interval, canonical);
        assert_eq!(found.relative_half_width, canonical.relative_half_width());
    }
    // One replicate set serves every alpha, so a stricter alpha nests the interval.
    assert!(sized[1].interval.lower <= sized[0].interval.lower);
    assert!(sized[0].interval.upper <= sized[1].interval.upper);
}

#[test]
fn relative_half_width_is_the_wider_side_over_the_estimate() {
    let mut interval = request().intervals().unwrap()[0].interval;
    (interval.estimate, interval.lower, interval.upper) = (2.0, 1.5, 2.25);
    assert_eq!(interval.relative_half_width(), 0.25);
    interval.upper = 3.5;
    assert_eq!(interval.relative_half_width(), 0.75);
}

#[test]
fn command_prints_the_library_intervals_bit_for_bit() {
    let request = request();
    let output = run(&serde_json::to_vec(&request).unwrap());
    assert!(output.status.success(), "{output:?}");
    let printed: Vec<SizedInterval> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(printed, request.intervals().unwrap());
}

#[test]
fn command_refuses_a_request_it_cannot_bootstrap() {
    let mut short = request();
    short.pairs.truncate(1);
    for input in [serde_json::to_vec(&short).unwrap(), b"{}".to_vec()] {
        let output = run(&input);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
