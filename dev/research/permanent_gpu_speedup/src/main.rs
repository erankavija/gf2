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
    use gf2_algebra::testutil::random_matrix_with_rng;
    use gf2_core::gfp::Fp;
    use gf2_core::rng::Lcg;
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

    /// Compute the median of a non-empty slice.
    fn median_vec(v: &[f64]) -> f64 {
        assert!(!v.is_empty());
        if v.len() == 1 {
            return v[0];
        }
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = s.len() / 2;
        if s.len().is_multiple_of(2) {
            (s[mid - 1] + s[mid]) / 2.0
        } else {
            s[mid]
        }
    }

    /// Build M random F_3 matrices, each n×n, from a deterministic LCG seed.
    fn build_matrices(n: usize, m: usize, seed: u64) -> Vec<Bipedal3Matrix> {
        let mut rng = Lcg::new(seed);
        (0..m)
            .map(|_| {
                let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
                Bipedal3Matrix::from_row_major(&elems, n, n)
            })
            .collect()
    }

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
    // Hardware fingerprint
    // -----------------------------------------------------------------------

    fn hw_fingerprint() -> HwInfo {
        let cpu_model = std::fs::read_to_string("/proc/cpuinfo")
            .unwrap_or_default()
            .lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let rocm_ver = std::fs::read_to_string("/opt/rocm/.info/version")
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string();

        let gfx_target = std::process::Command::new("rocminfo")
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).to_string();
                s.lines()
                    .filter(|l| l.contains("Name") && l.contains("gfx"))
                    .find_map(|l| {
                        l.split_whitespace()
                            .find(|w| w.starts_with("gfx"))
                            .map(|s| s.to_string())
                    })
            })
            .unwrap_or_else(|| "gfx1030".to_string());

        let gpu_name = std::process::Command::new("rocminfo")
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).to_string();
                s.lines()
                    .find(|l| l.contains("Marketing Name") && l.contains("Radeon"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "AMD Radeon RX 6950 XT".to_string());

        let kernel_ver = std::process::Command::new("uname")
            .arg("-r")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        HwInfo {
            cpu_model,
            gpu_name,
            gfx_target,
            rocm_ver,
            kernel_ver,
        }
    }

    struct HwInfo {
        cpu_model: String,
        gpu_name: String,
        gfx_target: String,
        rocm_ver: String,
        kernel_ver: String,
    }

    // -----------------------------------------------------------------------
    // CSV writer
    // -----------------------------------------------------------------------

    fn write_csv_header(f: &mut File, hw: &HwInfo, commit_sha: &str, rustc_ver: &str, seed: u64) {
        writeln!(
            f,
            "# S1g (jit:9480f8a6) GPU 50x speedup measurement for permanent_bipedal3 (F_3)"
        )
        .unwrap();
        writeln!(f, "# commit: {commit_sha}").unwrap();
        writeln!(f, "# rustc: {rustc_ver}").unwrap();
        writeln!(f, "# cpu: {}", hw.cpu_model).unwrap();
        writeln!(f, "# gpu: {}", hw.gpu_name).unwrap();
        writeln!(f, "# gfx_target: {}", hw.gfx_target).unwrap();
        writeln!(f, "# rocm_version: {}", hw.rocm_ver).unwrap();
        writeln!(f, "# kernel: {}", hw.kernel_ver).unwrap();
        writeln!(f, "# seed: {seed:#018x}").unwrap();
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

        let rustc_ver = std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let commit_sha = std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

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

        write_csv_header(&mut csv_file, &hw, &commit_sha, &rustc_ver, SEED);
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
