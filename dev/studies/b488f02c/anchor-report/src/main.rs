//! Prints the observed side of the order-3 sampling anchor.
//!
//! The harness asserts that its sampler and kernels recover the exact order-3
//! zero fraction within 4 sigma over 400 000 draws per field, but a passing
//! assertion emits nothing. This binary reproduces that draw against the same
//! library code and prints what was observed, so the study's anchor claim has a
//! receipt with numbers in it rather than a pass/fail verdict.
//!
//! The draw is reproduced, not re-implemented: `MatrixSampler` and the packed
//! kernels come from the harness and `gf2-algebra` themselves, so a defect in
//! either shows up here. The three parameters below are transcribed from the
//! harness test, which does not expose them as constants; the receipt script
//! parses the test source and checks these values against it.

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::packed::packed7::Packed7Matrix;
use permanent_sampling_feas::equivalence::exact_zero_count_order3;
use permanent_sampling_feas::sampler::MatrixSampler;
use permanent_sampling_feas::stats::{wilson_interval, Z_95};

/// Transcribed from `equivalence.rs`'s anchor test. Not exported by the
/// harness; the receipt script cross-checks them against the test source.
const SEED_ROOT: u64 = 0xB488_F02C;
const STREAM: u64 = 12_345;
const DRAWS: usize = 400_000;
const N: usize = 3;

fn permanent_value(q: u64, sampler: &mut MatrixSampler) -> u64 {
    match q {
        3 => {
            let d = sampler.next_matrix::<3>(N);
            gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword(
                &Bipedal3Matrix::from_row_major(&d, N, N),
            )
            .value()
        }
        5 => {
            let d = sampler.next_matrix::<5>(N);
            gf2_algebra::permanent::bipedal5::permanent_bipedal5(&Packed5Matrix::from_row_major(
                &d, N, N,
            ))
            .value()
        }
        7 => {
            let d = sampler.next_matrix::<7>(N);
            gf2_algebra::permanent::bipedal7::permanent_bipedal7(&Packed7Matrix::from_row_major(
                &d, N, N,
            ))
            .value()
        }
        _ => panic!("unsupported q = {q}"),
    }
}

fn main() {
    println!("anchor_parameters_used: seed_root=0x{SEED_ROOT:X} stream={STREAM} draws_per_field={DRAWS} n={N}");
    println!("threshold_sigma: 4.0");
    println!();
    println!(
        "q  draws     zeros    p_hat      exact       wilson_lo  wilson_hi  z       within_4_sigma"
    );

    let mut all_pass = true;
    for q in [3u64, 5, 7] {
        let (_, exact_zeros, total) = exact_zero_count_order3(q);
        let truth = exact_zeros as f64 / total as f64;

        let mut sampler = MatrixSampler::new(SEED_ROOT, q, N, STREAM);
        let mut zeros = 0u64;
        for _ in 0..DRAWS {
            if permanent_value(q, &mut sampler) == 0 {
                zeros += 1;
            }
        }

        let p_hat = zeros as f64 / DRAWS as f64;
        let (lo, hi) = wilson_interval(zeros, DRAWS as u64, Z_95);
        let se = (truth * (1.0 - truth) / DRAWS as f64).sqrt();
        let z = (p_hat - truth) / se;
        let ok = z.abs() < 4.0;
        all_pass &= ok;

        println!(
            "{q}  {DRAWS}    {zeros:<7}  {p_hat:.6}   {truth:.6}    {lo:.6}   {hi:.6}   {z:+.3}  {}",
            if ok { "yes" } else { "NO" }
        );
    }
    println!();
    println!(
        "all_fields_within_4_sigma: {}",
        if all_pass { "yes" } else { "NO" }
    );
    if !all_pass {
        std::process::exit(1);
    }
}
