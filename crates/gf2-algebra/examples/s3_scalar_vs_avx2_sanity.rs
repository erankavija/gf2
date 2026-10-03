//! Scalar-vs-AVX2 sanity sweep for the single-word F_3 permanent kernels.
//!
//! Times `permanent_bipedal3_singleword` and
//! `permanent_bipedal3_singleword_simd` on the same seeded matrices at
//! n ∈ {16, 20, 24}, panics unless their `Fp<3>` results are bit-identical, and
//! prints CSV rows to stdout. When AVX2 is not detected, the AVX2 row times the
//! scalar path again.

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::bipedal3::{
    permanent_bipedal3_singleword, permanent_bipedal3_singleword_simd,
};
use gf2_algebra::testutil::{random_matrix, today_yyyy_mm_dd};
use std::io::Write;
use std::time::Instant;

const N_VALUES: &[usize] = &[16, 20, 24];

const SAMPLES: usize = 5;

const SEED_BASE: u64 = 0x363556e600000000;

// Per-row fingerprint; AVX-512 status is recorded in the CSV header.
const HW_FINGERPRINT: &str = "AMD Ryzen 9 5900X 12-Core Processor/Zen 3/AVX2=yes";

fn main() {
    let date = today_yyyy_mm_dd();

    #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
    let avx2_fns = gf2_kernels_simd::bipedal::detect_avx2();

    #[cfg(not(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64"))))]
    let avx2_fns: Option<()> = None;

    println!("S3 (jit:363556e6) — direct scalar-vs-AVX2 single-word sanity sweep");
    println!("Host: {HW_FINGERPRINT}");
    println!("date: {date}");
    println!("seed_base: {SEED_BASE:#018x}");
    println!("samples per cell: {SAMPLES}");
    println!();

    #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
    {
        if avx2_fns.is_none() {
            eprintln!(
                "WARNING: AVX2 not detected at runtime — direct kernel will not be measured."
            );
            eprintln!("         Sanity ratio will be 1.000 (scalar-vs-scalar).");
        }
    }

    println!(
        "# S3 (jit:363556e6) direct scalar-vs-AVX2 single-word sanity sweep — fresh measurements"
    );
    println!("# date: {date}");
    println!("# host: AMD Ryzen 9 5900X 12-Core Processor");
    println!("# arch: Zen 3");
    println!("# avx2: yes, avx512: no");
    println!("# seed_base: {SEED_BASE:#018x}");
    println!("# scope: direct scalar and single-matrix AVX2 kernels at n in {{16, 20, 24}}; public dispatch and four-matrix batching are not timed");
    println!("# samples: {SAMPLES} per cell");
    println!("n,impl,mean_us,std_us,samples,ratio_vs_avx2,hardware_fingerprint");

    let stderr = std::io::stderr();
    let mut progress = stderr.lock();

    for &n in N_VALUES {
        writeln!(
            progress,
            "=== n={n}: measuring direct scalar and AVX2 ({SAMPLES} samples each) ==="
        )
        .unwrap();

        let matrices: Vec<Bipedal3Matrix> = (0..SAMPLES)
            .map(|s| {
                let seed = SEED_BASE ^ (n as u64) ^ (s as u64);
                let row_major = random_matrix::<3>(n, seed);
                Bipedal3Matrix::from_row_major(&row_major, n, n)
            })
            .collect();

        let mut scalar_timings_us: Vec<f64> = Vec::with_capacity(SAMPLES);
        let mut scalar_results: Vec<u64> = Vec::with_capacity(SAMPLES);

        for mat in &matrices {
            let t0 = Instant::now();
            let result = std::hint::black_box(permanent_bipedal3_singleword(mat));
            let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
            scalar_timings_us.push(elapsed_us);
            scalar_results.push(result.value());
        }

        let scalar_mean = scalar_timings_us.iter().sum::<f64>() / SAMPLES as f64;
        let scalar_std = stddev(&scalar_timings_us, scalar_mean);

        writeln!(
            progress,
            "  scalar: mean={scalar_mean:.1} us, std={scalar_std:.1} us"
        )
        .unwrap();

        let (avx2_mean, avx2_std) = measure_avx2(
            &matrices,
            &scalar_results,
            n,
            #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
            &avx2_fns,
            &mut progress,
        );

        writeln!(
            progress,
            "  direct AVX2: mean={avx2_mean:.1} us, std={avx2_std:.1} us"
        )
        .unwrap();

        let ratio = if avx2_mean > 0.0 {
            scalar_mean / avx2_mean
        } else {
            1.0
        };

        writeln!(progress, "  ratio (scalar/AVX2) = {ratio:.4}").unwrap();
        writeln!(progress).unwrap();

        // Scalar row: ratio_vs_avx2 = scalar_mean / avx2_mean.
        println!(
            "{n},permanent_bipedal3_scalar,{scalar_mean:.3},{scalar_std:.3},{SAMPLES},{ratio:.4},{HW_FINGERPRINT}"
        );
        // AVX2 sanity row: ratio = 1.000 by definition.
        println!(
            "{n},permanent_bipedal3_avx2_sanity,{avx2_mean:.3},{avx2_std:.3},{SAMPLES},1.0000,{HW_FINGERPRINT}"
        );
    }

    writeln!(
        progress,
        "Done. Rows compare direct scalar and single-matrix AVX2 kernels; S1 separately measures the public scalar dispatcher."
    )
    .unwrap();
}

fn stddev(samples: &[f64], mean: f64) -> f64 {
    if samples.len() <= 1 {
        return 0.0;
    }
    let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (samples.len() - 1) as f64;
    var.sqrt()
}

/// Measure the direct AVX2 path on `matrices`, asserting bit-identical results
/// against `scalar_results`. Returns `(mean_us, std_us)`.
///
/// When AVX2 is not detected, times the scalar path again.
#[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
fn measure_avx2(
    matrices: &[Bipedal3Matrix],
    scalar_results: &[u64],
    n: usize,
    avx2_fns: &Option<gf2_kernels_simd::bipedal::BipedalAvx2Fns>,
    progress: &mut impl Write,
) -> (f64, f64) {
    match avx2_fns {
        Some(fns) => {
            let mut timings_us: Vec<f64> = Vec::with_capacity(matrices.len());
            for (i, mat) in matrices.iter().enumerate() {
                let t0 = Instant::now();
                let result = std::hint::black_box(permanent_bipedal3_singleword_simd(mat, fns));
                let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
                timings_us.push(elapsed_us);
                assert_eq!(
                    result.value(),
                    scalar_results[i],
                    "S3 CORRECTNESS FAIL: AVX2 != scalar at n={n}, sample={i}: avx2={}, scalar={}",
                    result.value(),
                    scalar_results[i]
                );
            }
            let mean = timings_us.iter().sum::<f64>() / timings_us.len() as f64;
            let std = stddev(&timings_us, mean);
            (mean, std)
        }
        None => {
            writeln!(
                progress,
                "  [no AVX2] falling back to scalar for AVX2 row (ratio will be 1.000)"
            )
            .unwrap();
            let mut timings_us: Vec<f64> = Vec::with_capacity(matrices.len());
            for mat in matrices {
                let t0 = Instant::now();
                let _ = std::hint::black_box(permanent_bipedal3_singleword(mat));
                let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
                timings_us.push(elapsed_us);
            }
            let mean = timings_us.iter().sum::<f64>() / timings_us.len() as f64;
            let std = stddev(&timings_us, mean);
            (mean, std)
        }
    }
}

#[cfg(not(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64"))))]
fn measure_avx2(
    matrices: &[Bipedal3Matrix],
    _scalar_results: &[u64],
    _n: usize,
    _progress: &mut impl Write,
) -> (f64, f64) {
    // No `simd` feature or non-x86: scalar-vs-scalar, ratio = 1.000.
    let mut timings_us: Vec<f64> = Vec::with_capacity(matrices.len());
    for mat in matrices {
        let t0 = Instant::now();
        let _ = std::hint::black_box(permanent_bipedal3_singleword(mat));
        let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
        timings_us.push(elapsed_us);
    }
    let mean = timings_us.iter().sum::<f64>() / timings_us.len() as f64;
    let std = stddev(&timings_us, mean);
    (mean, std)
}
