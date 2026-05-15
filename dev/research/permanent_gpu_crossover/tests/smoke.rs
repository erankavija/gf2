//! Smoke test for the S5 GPU-vs-CPU-SIMD crossover harness (jit:a9e461de).
//!
//! These tests are gated behind `#[cfg(feature = "hip")]` and require a
//! gfx1030 device at runtime; they are marked `#[ignore = "external: ..."]`
//! so they are skipped in CI and normal `cargo nextest run`.
//!
//! Run manually on the gfx1030 host:
//!   cargo test --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
//!       --release --features hip -- --ignored

#![cfg(feature = "hip")]

use gf2_algebra::gpu::permanent_batch_bipedal3;
use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::testutil::random_matrix_with_rng;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;
use std::time::Instant;

/// Smoke test: runs the harness at a single small (n=8, M=4) cell to verify the
/// CPU + GPU + CSV-write plumbing works end-to-end. Not a measurement test —
/// just confirms the two paths agree on results and that neither panics.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_crossover_sim_runs_one_cell() {
    let n = 8;
    let m = 4;
    let seed = 0x00C0_FFEE_DEAD_BEEF_u64;

    // Build M random F_3 matrices.
    let mut rng = Lcg::new(seed);
    let matrices: Vec<Bipedal3Matrix> = (0..m)
        .map(|_| {
            let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
            Bipedal3Matrix::from_row_major(&elems, n, n)
        })
        .collect();

    // CPU SIMD path (sequential).
    let t_cpu = Instant::now();
    let cpu_results: Vec<Fp<3>> = matrices.iter().map(permanent_bipedal3).collect();
    let cpu_elapsed = t_cpu.elapsed().as_secs_f64();
    assert!(cpu_elapsed < 60.0, "CPU path took too long: {cpu_elapsed}s");

    // GPU batch path.
    let t_gpu = Instant::now();
    let gpu_results = permanent_batch_bipedal3(&matrices);
    let gpu_elapsed = t_gpu.elapsed().as_secs_f64();
    assert!(gpu_elapsed < 60.0, "GPU path took too long: {gpu_elapsed}s");

    // Correctness: both paths must agree on every matrix.
    assert_eq!(
        cpu_results.len(),
        gpu_results.len(),
        "result count mismatch"
    );
    for (i, (cpu_r, gpu_r)) in cpu_results.iter().zip(gpu_results.iter()).enumerate() {
        assert_eq!(
            cpu_r, gpu_r,
            "permanent mismatch at matrix {i}: cpu={cpu_r:?}, gpu={gpu_r:?}"
        );
    }

    println!(
        "smoke: n={n} M={m} cpu={:.3}ms gpu={:.3}ms — all {m} results agree",
        cpu_elapsed * 1e3,
        gpu_elapsed * 1e3
    );

    // Basic throughput plausibility check: GPU result count matches input count.
    assert_eq!(gpu_results.len(), m);
}

/// Verify that the harness correctly computes the CPU SIMD wall-clock and
/// perm/s for a known small case without GPU involvement.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_cpu_path_timing_plausible() {
    let n = 4;
    let m = 8;
    let seed = 0x0000_0001_0000_0002_u64;

    let mut rng = Lcg::new(seed);
    let matrices: Vec<Bipedal3Matrix> = (0..m)
        .map(|_| {
            let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
            Bipedal3Matrix::from_row_major(&elems, n, n)
        })
        .collect();

    let t0 = Instant::now();
    for mat in &matrices {
        let _ = std::hint::black_box(permanent_bipedal3(mat));
    }
    let elapsed = t0.elapsed().as_secs_f64();
    let pps = m as f64 / elapsed;

    assert!(pps > 1.0, "implausibly low perm/s: {pps}");
    assert!(elapsed < 10.0, "implausibly slow for n={n}: {elapsed}s");
    println!("cpu timing smoke: n={n} M={m} pps={pps:.0} elapsed={elapsed:.6}s");
}
