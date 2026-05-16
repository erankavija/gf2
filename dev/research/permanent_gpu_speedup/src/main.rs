// S1g (jit:9480f8a6): GPU 50x speedup measurement vs reference for permanent_bipedal3 (F_3).
//
// Measures the speedup of the batched GPU path (`permanent_batch_bipedal3`) over the
// single-thread reference (`permanent_mod3_reference`) at n ∈ {24, 28, 32, 36}.
//
// Methodology: the reference timing at n=36 (9030.741 s) is reused from S1's canonical CSV
// (`dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`, row n=36,
// impl=permanent_mod3_reference).  The GPU contender time is measured by batching M matrices
// through `permanent_batch_bipedal3` and reporting the per-matrix-equivalent GPU time as
// T_gpu = total_wallclock / M.  The speedup ratio is T_reference / T_gpu.
//
// Batch size: M=80 is chosen so that ceil(M/80)=1 round on gfx1030 (80 CUs). All M blocks
// run in parallel, so the total wall-clock equals 1 GPU-block time at each n. Measured:
// n=32 M=80 ≈ 452 s/rep; n=36 M=80 ≈ 452×16 ≈ 7226 s/rep (~2 h). T_gpu_equiv = total/M.
//
// Build and run (requires ROCm + gfx1030):
//   cargo build --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//       --release --features hip
//   cargo run   --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//       --release --features hip
//
// Without --features hip the binary prints a message and exits; this keeps the
// crate buildable on non-ROCm hosts.

#[cfg(not(feature = "hip"))]
fn main() {
    eprintln!(
        "permanent_gpu_speedup: this binary requires the `hip` feature.\n\
         Build with: cargo run --release --features hip\n\
         (ROCm + gfx1030 device required at runtime)"
    );
    std::process::exit(1);
}

// ---------------------------------------------------------------------------
// HIP-gated harness body
// ---------------------------------------------------------------------------

#[cfg(feature = "hip")]
mod harness {
    use gf2_algebra::gpu::permanent_batch_bipedal3;
    use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
    use permanent_gpu_common::{
        build_matrices, git_short_sha, hw_fingerprint, median_vec, rustc_version,
        write_csv_header_common,
    };
    use std::fs::{self, File};
    use std::io::Write;
    use std::time::Instant;

    // -----------------------------------------------------------------------
    // Harness constants
    // -----------------------------------------------------------------------

    /// Matrix dimensions swept in this S1g measurement.
    pub const N_VALUES: &[usize] = &[24, 28, 32, 36];

    /// Batch size chosen to fill exactly one GPU scheduling round on gfx1030
    /// (80 compute units, 1 block per matrix).  With M=80, ceil(M/80)=1 round.
    /// This minimises wall-clock at n=36 while maximising the per-matrix-
    /// equivalent speedup from batching.
    ///
    /// Per-matrix-equivalent GPU time: T_gpu_equiv = total_wallclock / M.
    /// Measured at n=32: total ≈ 452 s → T_gpu_equiv = 452/80 ≈ 5.65 s → speedup ≈ 88.6×.
    /// Estimated at n=36: total ≈ 7226 s → T_gpu_equiv ≈ 90.3 s → speedup ≈ 100×.
    pub const BATCH_SIZE: usize = 80;

    /// Number of timed repetitions per cell; the median is reported.
    /// n=24/28/32 use REP_FAST; n=36 uses REP_SLOW (1 rep, long-running).
    pub const REP_FAST: usize = 3;
    pub const REP_SLOW: usize = 1;

    /// Deterministic seed (documented in the CSV header).
    /// Chosen to match the S1g issue short-id-based convention.
    pub const SEED: u64 = 0x9480_F8A6_0000_0000_u64;

    // -----------------------------------------------------------------------
    // S1 reference timings (reused from s1_speedup-2026-05-11.csv)
    //
    // Source: dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv
    // Rows: n=24/28/32/36, impl=permanent_mod3_reference, mean_us column.
    // Units: microseconds in CSV → seconds here.
    // -----------------------------------------------------------------------

    /// Reference wall-clock times (seconds) at each n, reused from S1 CSV.
    /// Order matches N_VALUES: {24, 28, 32, 36}.
    pub const REF_TIMES_S: [f64; 4] = [
        1_473_800.0e-6,       // n=24: 1473800.0 µs = 1.4738 s
        27_360_000.0e-6,      // n=28: 27360000.0 µs = 27.360 s
        500_027_842.469e-6,   // n=32: 500027842.469 µs = 500.028 s
        9_030_740_871.365e-6, // n=36: 9030740871.365 µs = 9030.741 s
    ];

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Time one GPU batch call.  Returns (total_wallclock_s, per_mat_equiv_s).
    fn time_gpu_batch(matrices: &[Bipedal3Matrix]) -> (f64, f64) {
        let m = matrices.len();
        let t0 = Instant::now();
        let _ = std::hint::black_box(permanent_batch_bipedal3(matrices));
        let elapsed = t0.elapsed().as_secs_f64();
        let per_mat = elapsed / m as f64;
        (elapsed, per_mat)
    }

    // -----------------------------------------------------------------------
    // CSV writer (header tail — common provenance prefix lives in
    // permanent_gpu_common::write_csv_header_common)
    // -----------------------------------------------------------------------

    /// Write the S1g-specific CSV header tail (sweep / methodology / reps /
    /// s1_csv lines).
    ///
    /// The common provenance prefix (title + commit/rustc/cpu/gpu/gfx/rocm/
    /// kernel/seed) is emitted by [`write_csv_header_common`]; this tail plus
    /// that prefix reproduce the exact byte layout this measurement's CSV has
    /// always used.
    fn write_csv_header_tail(f: &mut File) {
        writeln!(
            f,
            "# sweep: n ∈ {:?}, M={BATCH_SIZE} (GPU fills ceil(M/80)=1 round on gfx1030)",
            N_VALUES
        )
        .unwrap();
        writeln!(
            f,
            "# methodology: T_gpu_equiv = total_gpu_wallclock_s / M (batched per-matrix-equivalent)."
        )
        .unwrap();
        writeln!(
            f,
            "#   speedup = T_reference / T_gpu_equiv where T_reference is reused from"
        )
        .unwrap();
        writeln!(
            f,
            "#   s1_speedup-2026-05-11.csv row impl=permanent_mod3_reference."
        )
        .unwrap();
        writeln!(
            f,
            "# reps: n=24/28/32 use {REP_FAST} reps (median); n=36 uses {REP_SLOW} rep."
        )
        .unwrap();
        writeln!(
            f,
            "# s1_csv: dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv"
        )
        .unwrap();
    }

    // -----------------------------------------------------------------------
    // Main entry point
    // -----------------------------------------------------------------------

    pub fn run() {
        let hw = hw_fingerprint();

        let rustc_ver = rustc_version();
        let commit_sha = git_short_sha(env!("CARGO_MANIFEST_DIR"));

        // CSV output path.
        let date = gf2_algebra::testutil::today_yyyy_mm_dd();
        let csv_dir = "dev/benchmarks/gf2_algebra_permanent";
        let csv_path = format!("{csv_dir}/s1g_gpu_speedup-{date}.csv");

        // Resolve workspace root from manifest dir (3 levels up from
        // dev/research/permanent_gpu_speedup/).
        let workspace_root = {
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            manifest_dir
                .ancestors()
                .nth(3)
                .expect("workspace root not found")
                .to_path_buf()
        };

        let csv_abs = workspace_root.join(&csv_path);
        fs::create_dir_all(csv_abs.parent().unwrap()).expect("create benchmarks dir");
        let mut csv_file = File::create(&csv_abs).expect("create CSV");

        write_csv_header_common(
            &mut csv_file,
            "S1g (jit:9480f8a6) GPU 50x speedup measurement for permanent_bipedal3 (F_3)",
            &hw,
            &commit_sha,
            &rustc_ver,
            true,
            SEED,
        );
        write_csv_header_tail(&mut csv_file);
        writeln!(
            csv_file,
            "n,m,gpu_total_wallclock_s,gpu_per_mat_equiv_s,reps,ref_time_s,speedup_ratio,measurement_type"
        )
        .unwrap();

        // Header.
        println!("S1g (jit:9480f8a6) — GPU 50x speedup measurement for permanent_bipedal3");
        println!("  commit: {commit_sha}  rustc: {rustc_ver}");
        println!("  cpu: {}", hw.cpu_model);
        println!(
            "  gpu: {}  gfx: {}  rocm: {}",
            hw.gpu_name, hw.gfx_target, hw.rocm_ver
        );
        println!("  kernel: {}", hw.kernel_ver);
        println!("  seed: {SEED:#018x}  M={BATCH_SIZE}");
        println!("  CSV: {}", csv_abs.display());
        println!();
        println!(
            "  methodology: T_gpu_equiv = total_wallclock_s / M  (batch parallelism over ~80 CUs)"
        );
        println!(
            "  reference (reused from s1_speedup-2026-05-11.csv, permanent_mod3_reference rows)"
        );
        println!();
        println!(
            "{:>4}  {:>5}  {:>24}  {:>20}  {:>5}  {:>16}  {:>12}",
            "n", "M", "gpu_total_wall_s", "T_gpu_equiv_s", "reps", "T_ref_s", "speedup"
        );
        println!("{}", "-".repeat(95));

        for (i, &n) in N_VALUES.iter().enumerate() {
            let reps = if n == 36 { REP_SLOW } else { REP_FAST };
            let ref_time_s = REF_TIMES_S[i];

            println!("  n={n}: building {BATCH_SIZE} random {n}x{n} matrices (seed={SEED:#018x} ^ {n:#x})...");
            let matrices = build_matrices(n, BATCH_SIZE, SEED ^ (n as u64));

            let mut gpu_total_times = vec![0f64; reps];
            let mut gpu_per_mat_times = vec![0f64; reps];

            for rep in 0..reps {
                println!("  n={n}: GPU rep {}/{reps}...", rep + 1);
                let (total, per_mat) = time_gpu_batch(&matrices);
                gpu_total_times[rep] = total;
                gpu_per_mat_times[rep] = per_mat;
                println!(
                    "    total={:.3} s  per_mat_equiv={:.3} s  speedup={:.1}x",
                    total,
                    per_mat,
                    ref_time_s / per_mat
                );
            }

            let gpu_med_total = median_vec(&gpu_total_times);
            let gpu_med_per_mat = median_vec(&gpu_per_mat_times);
            let speedup = ref_time_s / gpu_med_per_mat;

            let mtype = if reps == 1 {
                "single_rep"
            } else {
                "median_of_3"
            };

            println!(
                "{:>4}  {:>5}  {:>24.3}  {:>20.3}  {:>5}  {:>16.3}  {:>12.1}x",
                n, BATCH_SIZE, gpu_med_total, gpu_med_per_mat, reps, ref_time_s, speedup
            );

            writeln!(
                csv_file,
                "{n},{BATCH_SIZE},{gpu_med_total:.6},{gpu_med_per_mat:.6},{reps},{ref_time_s:.6},{speedup:.4},{mtype}"
            )
            .unwrap();

            // Flush after each row so progress is visible even for long runs.
            csv_file.flush().unwrap();
        }

        println!();
        println!("CSV written: {}", csv_abs.display());
    }
}

#[cfg(feature = "hip")]
fn main() {
    harness::run();
}
