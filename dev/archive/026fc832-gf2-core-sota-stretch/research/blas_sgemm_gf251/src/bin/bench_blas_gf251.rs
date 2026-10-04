//! BLAS-backed GF(251) cascade bench driver — single trial of the
//! 5-trial route-B protocol for JIT issue 91429c1c.
//!
//! Runs `n_inner` GEMM iterations per cell at the requested sizes
//! and emits CSV-style lines to stdout. There are two modes per
//! cell, both reported on separate lines:
//!
//!   * `route_b_full` — full pipeline: pack
//!     `FieldMatrix<Fp<251>>` → sgemm cascade → unpack
//!     `FieldMatrix<Fp<251>>`. This is the apples-to-apples
//!     comparison against `gf2_core::field::matrix::gemm` (the
//!     Candidate-C default dispatch).
//!   * `route_b_canon` — canonical-byte pipeline: input/output are
//!     `Vec<u8>` of canonical residues in `[0, 251)`. This skips
//!     the Montgomery REDC tax on the I/O boundary and is the
//!     apples-to-apples comparison against fflas-ffpack's
//!     `Modular<float>` route, which stores values in canonical
//!     f32 (no Montgomery encoding).
//!
//! Per-cell line format:
//!   trial,route,prime,n,median_ns,min_ns,max_ns,iters,gop_s
//!
//! Pinning / nicing is the responsibility of the launcher shell
//! (`taskset -c 6-11 nice -n -5 ...`).

use std::env;
use std::time::Instant;

use blas_sgemm_gf251::{
    blas_gf251_gemm, blas_gf251_gemm_canonical_bytes, matrix_to_canonical_bytes,
    openblas_get_num_threads, openblas_set_num_threads,
};
use gf2_core::bench_seed::fp_matrix_from_seed;

const P: u64 = 251;

fn main() {
    let mut args = env::args().skip(1);
    let mut trial: u32 = 0;
    let mut sizes: Vec<usize> = vec![64, 256, 1024];
    let mut n_inner: usize = 30;
    let mut warmup: usize = 3;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--trial" => {
                trial = args.next().expect("--trial N").parse().expect("trial num");
            }
            "--sizes" => {
                let csv = args.next().expect("--sizes 64,256,...");
                sizes = csv
                    .split(',')
                    .map(|s| s.trim().parse().expect("size as usize"))
                    .collect();
            }
            "--inner" => {
                n_inner = args.next().expect("--inner N").parse().expect("inner");
            }
            "--warmup" => {
                warmup = args.next().expect("--warmup N").parse().expect("warmup");
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }

    // Force single-threaded BLAS up front so the warmup matches the
    // measured configuration.
    // SAFETY: openblas_set_num_threads is safe to call.
    unsafe { openblas_set_num_threads(1) };
    // SAFETY: openblas_get_num_threads is safe to call.
    let nt = unsafe { openblas_get_num_threads() };
    eprintln!(
        "[bench_blas_gf251] trial={trial} sizes={sizes:?} inner={n_inner} warmup={warmup} \
         openblas_num_threads={nt}"
    );

    println!("trial,route,prime,n,median_ns,min_ns,max_ns,iters,gop_s");

    for &n in &sizes {
        let seed_a = 0x9142_9c1c_0000_0000_u64 ^ ((n as u64) << 32) ^ (trial as u64);
        let seed_b = 0x9142_9c1c_0000_0001_u64 ^ ((n as u64) << 32) ^ (trial as u64);
        let a = fp_matrix_from_seed::<P>(n, n, seed_a);
        let b = fp_matrix_from_seed::<P>(n, n, seed_b);
        let a_bytes = matrix_to_canonical_bytes(&a);
        let b_bytes = matrix_to_canonical_bytes(&b);

        // ─── route_b_full: time the FieldMatrix-in/FieldMatrix-out pipeline.
        for _ in 0..warmup {
            let c = blas_gf251_gemm(&a, &b);
            std::hint::black_box(&c);
        }
        let mut samples_full: Vec<u128> = Vec::with_capacity(n_inner);
        for _ in 0..n_inner {
            let t0 = Instant::now();
            let c = blas_gf251_gemm(&a, &b);
            let dt = t0.elapsed().as_nanos();
            std::hint::black_box(&c);
            samples_full.push(dt);
        }
        samples_full.sort();
        let med = samples_full[n_inner / 2];
        let min = samples_full[0];
        let max = samples_full[n_inner - 1];
        let gop_s = 2.0 * (n as f64).powi(3) / (med as f64);
        println!(
            "{trial},route_b_full,{P},{n},{med},{min},{max},{n_inner},{:.4}",
            gop_s
        );

        // ─── route_b_canon: canonical-byte pipeline (apples-to-apples vs fflas).
        for _ in 0..warmup {
            let c = blas_gf251_gemm_canonical_bytes(&a_bytes, n, n, &b_bytes, n);
            std::hint::black_box(&c);
        }
        let mut samples_canon: Vec<u128> = Vec::with_capacity(n_inner);
        for _ in 0..n_inner {
            let t0 = Instant::now();
            let c = blas_gf251_gemm_canonical_bytes(&a_bytes, n, n, &b_bytes, n);
            let dt = t0.elapsed().as_nanos();
            std::hint::black_box(&c);
            samples_canon.push(dt);
        }
        samples_canon.sort();
        let med = samples_canon[n_inner / 2];
        let min = samples_canon[0];
        let max = samples_canon[n_inner - 1];
        let gop_s = 2.0 * (n as f64).powi(3) / (med as f64);
        println!(
            "{trial},route_b_canon,{P},{n},{med},{min},{max},{n_inner},{:.4}",
            gop_s
        );
    }
}
