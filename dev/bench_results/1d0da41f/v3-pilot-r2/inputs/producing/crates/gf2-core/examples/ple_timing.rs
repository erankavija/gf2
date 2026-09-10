//! Minimal PLE timing harness for the 40195c09 PLE Schur-update non-regression.
//!
//! Measures `FieldMatrix::ple` at n=256 for Fp<7>, Fp<251>, Fp<65521> using
//! a simple wall-clock loop (warmup=2, iters=20), then prints the median.
//!
//! Usage:
//! ```bash
//! cargo run -p gf2-core --example ple_timing --features rand --release
//! ```

use std::time::Instant;

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;

fn measure_ple<const P: u64>(n: usize, label: &str) {
    let seed = 0xDEAD_BEEF_0000_0001u64 ^ (P * 1000 + n as u64);
    let mat: FieldMatrix<Fp<P>> = fp_matrix_from_seed::<P>(n, n, seed);

    // warmup
    for _ in 0..2 {
        let _ = mat.ple();
    }

    // measure
    const ITERS: usize = 20;
    let mut timings_ns: Vec<u64> = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        let r = mat.ple();
        let elapsed = t0.elapsed().as_nanos() as u64;
        let _ = std::hint::black_box(r);
        timings_ns.push(elapsed);
    }

    timings_ns.sort_unstable();
    let median_ns = timings_ns[ITERS / 2];
    let median_ms = median_ns as f64 / 1_000_000.0;
    println!("ple/{label}/{n}  median={median_ms:.3}ms  (over {ITERS} iters)");
}

fn main() {
    println!("# PLE Schur-update non-regression timing (40195c09)");
    println!("# n=256 uniform matrices, 20 iters, median");

    measure_ple::<7>(256, "Fp_7");
    measure_ple::<251>(256, "Fp_251");
    measure_ple::<65521>(256, "Fp_65521");
}
