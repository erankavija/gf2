// S1g (jit:9480f8a6): determinism check — GPU vs CPU SIMD at n=36, M=1.
//
// Verifies that `permanent_batch_bipedal3` and `permanent_bipedal3` (SIMD path)
// return the same Fp<3> value for the same seeded matrix at n=36.
//
// The CPU permanent (permanent_bipedal3) is launched in a background thread
// while the GPU kernel runs, so the wall-clock is max(GPU_time, CPU_time)
// rather than their sum. At n=36: GPU ≈ 7200 s, CPU ≈ 848 s. Wall-clock ≈ 7200 s.
//
// Build and run (requires ROCm + gfx1030):
//   cargo build --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//       --release --features hip --bin det_check
//   cargo run --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//       --release --features hip --bin det_check
//
// Expected output: "PASS: GPU == SIMD == Fp<3>(v)" where v is the computed permanent.
// Expected wall-clock: ~7200 s (~2 h) at n=36 (GPU-bound; CPU overlapped via thread).

#[cfg(not(feature = "hip"))]
fn main() {
    eprintln!(
        "det_check: this binary requires the `hip` feature.\n\
         Build with: cargo run --release --features hip --bin det_check\n\
         (ROCm + gfx1030 device required at runtime)"
    );
    std::process::exit(1);
}

#[cfg(feature = "hip")]
fn main() {
    use gf2_algebra::gpu::permanent_batch_bipedal3;
    use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
    use gf2_algebra::permanent::permanent_bipedal3;
    use gf2_algebra::testutil::random_matrix_with_rng;
    use gf2_core::gfp::Fp;
    use gf2_core::rng::Lcg;
    use std::time::Instant;

    // Use the same seed as the main sweep for n=36.
    let n = 36_usize;
    let seed = 0x9480_F8A6_0000_0000_u64 ^ (n as u64);

    println!("S1g det_check: n={n}  seed={seed:#018x}");
    println!(
        "  Building 1 random {n}x{n} F_3 matrix (same seed as main sweep matrix 0 at n={n})..."
    );

    let mut rng = Lcg::new(seed);
    let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
    let mat = Bipedal3Matrix::from_row_major(&elems, n, n);

    // Clone the matrix for the CPU thread (Bipedal3Matrix derives Clone).
    let mat_for_cpu = mat.clone();

    // Launch CPU SIMD computation in a background thread so it overlaps with
    // the GPU kernel. The GPU kernel takes ~7200 s; the CPU takes ~848 s.
    // Using a thread cuts the wall-clock to max(GPU, CPU) ≈ 7200 s rather
    // than GPU + CPU ≈ 8048 s.
    println!(
        "  Starting CPU SIMD thread (permanent_bipedal3, expected ~848 s) and GPU concurrently..."
    );
    let t_wall = Instant::now();

    let cpu_handle = std::thread::spawn(move || {
        let t_cpu = Instant::now();
        let result = permanent_bipedal3(&mat_for_cpu);
        let elapsed = t_cpu.elapsed().as_secs_f64();
        (result, elapsed)
    });

    // GPU batch path (M=1). Blocks on hipDeviceSynchronize — the CPU thread
    // runs concurrently during this call.
    println!("  Running GPU path (permanent_batch_bipedal3, M=1, expected ~7200 s)...");
    let t_gpu = Instant::now();
    let gpu_results = permanent_batch_bipedal3(std::slice::from_ref(&mat));
    let gpu_s = t_gpu.elapsed().as_secs_f64();
    let gpu_result = gpu_results[0];

    // Collect the CPU thread result.
    let (cpu_result, cpu_s) = cpu_handle.join().expect("CPU thread panicked");

    let wall_s = t_wall.elapsed().as_secs_f64();

    println!("  CPU SIMD: {cpu_result:?} in {cpu_s:.1} s");
    println!("  GPU:      {gpu_result:?} in {gpu_s:.1} s");
    println!("  Total wall-clock: {wall_s:.1} s (GPU-bound, CPU overlapped)");

    if cpu_result == gpu_result {
        println!("PASS: GPU == SIMD == {cpu_result:?}  (n={n}, seed={seed:#018x})");
    } else {
        eprintln!(
            "FAIL: GPU ({gpu_result:?}) != SIMD ({cpu_result:?}) at n={n}, seed={seed:#018x}"
        );
        std::process::exit(1);
    }
}
