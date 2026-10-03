//! Prints the 95% Clopper-Pearson interval of each `(errors, frames)` row of a
//! campaign CSV read from standard input.
use std::io::BufRead;

use gf2_stats::intervals::clopper_pearson_interval;

fn main() {
    for line in std::io::stdin().lock().lines().skip(1) {
        let line = line.expect("stdin is readable");
        let cols: Vec<&str> = line.split(',').collect();
        let frames: u64 = cols[3].parse().expect("frames column");
        let errors: u64 = cols[4].parse().expect("errors column");
        let (lo, hi) = clopper_pearson_interval(errors, frames, 0.95);
        println!("{},{errors},{frames},{lo:.4},{hi:.4}", cols[0]);
    }
}
