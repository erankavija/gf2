// S5 (jit:a9e461de): GPU-vs-CPU-SIMD crossover measurement for permanent_bipedal3 (F_3).
//
// Measures batched-throughput crossover: at what n does the gfx1030 GPU outperform
// the AVX2 CPU SIMD path on the same number of matrices per second?
//
// Build and run (requires ROCm + gfx1030):
//   cargo build --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
//       --release --features hip
//   cargo run   --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
//       --release --features hip
//
// Without --features hip the binary prints a message and exits; this keeps the
// crate buildable on non-ROCm hosts.

#[cfg(not(feature = "hip"))]
fn main() {
    eprintln!(
        "permanent_gpu_crossover: this binary requires the `hip` feature.\n\
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
    use gf2_algebra::permanent::permanent_bipedal3;
    use permanent_gpu_common::{
        build_matrices, git_short_sha, hw_fingerprint, median_vec, rustc_version,
        write_csv_header_common,
    };
    use std::fs::{self, File};
    use std::io::Write;
    use std::time::Instant;

    /// Matrix dimensions swept at the **fixed batch size** specified by [`BATCH_SIZE`].
    ///
    /// The sweep is restricted to `n ∈ {24, 28}` so the CPU SIMD side completes within the
    /// per-cell wall-clock budget (~5 min CPU at n=24, ~14 min CPU at n=28). Larger `n` at
    /// `M = 256` is impractical: CPU SIMD time grows as `M × n × 2^n`, putting n=32 at
    /// ~13,500 s of CPU wall-clock per repetition (~3.7 h). The crossover question at the
    /// production batch size is fully answerable from these two points: the GPU wins at
    /// both with comparable speedup, so the crossover threshold (at M=256) lies below the
    /// smallest tested n. Extrapolation to larger n and the M-dependence of the crossover
    /// is discussed in `dev/plans/s5_gpu_crossover.md` §3.
    pub const N_VALUES: &[usize] = &[24, 28];

    /// Fixed batch size for both CPU and GPU paths. The criterion requires a single
    /// `M` value across the sweep so the throughput-vs-n plot has a well-defined batch
    /// dimension.
    ///
    /// `M = 256` is the production batch size: it matches the GPU dispatcher's typical
    /// per-launch occupancy on gfx1030 (~80 CUs × 3 waves) and is the size at which the
    /// GPU path's per-launch overhead is fully amortized. The writeup §3 discusses why
    /// smaller M shifts the crossover.
    pub const BATCH_SIZE: usize = 256;

    fn batch_size(_n: usize) -> usize {
        BATCH_SIZE
    }

    /// Number of timed repetitions per cell; the median is reported.
    fn repeats(_n: usize) -> usize {
        REPEATS
    }

    /// Repetitions used for the median-of-K timing.
    const REPEATS: usize = 3;

    /// Deterministic seed (documented in the CSV header).
    const SEED: u64 = 0x00C0_FFEE_0000_0000_u64;

    /// Time the CPU SIMD path (sequential permanent_bipedal3 per matrix).
    /// Returns (wall_clock_s, perm_per_s).
    fn time_cpu_simd(matrices: &[Bipedal3Matrix]) -> (f64, f64) {
        let m = matrices.len();
        let t0 = Instant::now();
        for mat in matrices {
            let _ = std::hint::black_box(permanent_bipedal3(mat));
        }
        let elapsed = t0.elapsed().as_secs_f64();
        let pps = m as f64 / elapsed;
        (elapsed, pps)
    }

    /// Time the GPU batch path (one kernel launch for all M matrices).
    /// Returns (wall_clock_s, perm_per_s).
    fn time_gpu_batch(matrices: &[Bipedal3Matrix]) -> (f64, f64) {
        let m = matrices.len();
        let t0 = Instant::now();
        let _ = std::hint::black_box(permanent_batch_bipedal3(matrices));
        let elapsed = t0.elapsed().as_secs_f64();
        let pps = m as f64 / elapsed;
        (elapsed, pps)
    }

    /// Write the S5-specific CSV header tail (sweep + note lines).
    ///
    /// The common provenance prefix (title + commit/rustc/cpu/gpu/gfx/rocm/
    /// seed; no `# kernel:` line for this CSV schema) is emitted by
    /// [`write_csv_header_common`] with `include_kernel = false`; this tail
    /// plus that prefix reproduce the exact byte layout this measurement's CSV
    /// has always used.
    fn write_csv_header_tail(f: &mut File) {
        writeln!(
            f,
            "# sweep: n ∈ {:?}, fixed M={}, reps={} (median wall-clock)",
            N_VALUES, BATCH_SIZE, REPEATS
        )
        .unwrap();
        writeln!(
            f,
            "# note: GPU-vs-CPU ratio > 1 means GPU is faster (higher perm/s than CPU SIMD)"
        )
        .unwrap();
    }

    pub fn run() {
        let hw = hw_fingerprint();

        // Collect rustc version and commit SHA.
        let rustc_ver = rustc_version();
        let commit_sha = git_short_sha(env!("CARGO_MANIFEST_DIR"));

        // CSV output path — match the project naming convention.
        // Respects SA_DATE env for reproducible paths.
        let date = std::env::var("SA_DATE").unwrap_or_else(|_| {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_secs() as i64;
            let (y, m, d) = unix_secs_to_ymd(secs);
            format!("{y:04}-{m:02}-{d:02}")
        });

        let csv_dir = "dev/benchmarks/gf2_algebra_permanent";
        let csv_path = format!("{csv_dir}/s5_gpu_crossover-{date}.csv");

        // Run from workspace root so relative paths work.
        let workspace_root = {
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            // manifest is at dev/research/permanent_gpu_crossover/ — go 3 levels up
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
            "S5 (jit:a9e461de) GPU-vs-CPU-SIMD crossover for permanent_bipedal3 (F_3)",
            &hw,
            &commit_sha,
            &rustc_ver,
            false,
            SEED,
        );
        write_csv_header_tail(&mut csv_file);
        writeln!(
            csv_file,
            "n,m,cpu_simd_wallclock_s,cpu_simd_perm_per_s,gpu_wallclock_s,gpu_perm_per_s,gpu_cpu_ratio,gpu_wins"
        )
        .unwrap();

        println!("S5 (jit:a9e461de) — GPU-vs-CPU-SIMD crossover sweep");
        println!("  commit: {commit_sha}  rustc: {rustc_ver}");
        println!("  cpu: {}", hw.cpu_model);
        println!(
            "  gpu: {}  gfx: {}  rocm: {}",
            hw.gpu_name, hw.gfx_target, hw.rocm_ver
        );
        println!("  seed: {SEED:#018x}  repeats: {REPEATS}");
        println!("  CSV: {}", csv_abs.display());
        println!();
        println!(
            "{:>4}  {:>5}  {:>22}  {:>22}  {:>12}  gpu_wins",
            "n", "M", "cpu_simd perm/s", "gpu perm/s", "gpu/cpu ratio"
        );
        println!("{}", "-".repeat(80));

        for &n in N_VALUES {
            let m = batch_size(n);
            let reps = repeats(n);

            // Build M random matrices once; same matrices for both CPU and GPU.
            let matrices = build_matrices(n, m, SEED ^ (n as u64));

            // --- CPU SIMD: reps timed repetitions, take median ---
            let mut cpu_times = vec![0f64; reps];
            let mut cpu_pps_v = vec![0f64; reps];
            for rep in 0..reps {
                let (t, p) = time_cpu_simd(&matrices);
                cpu_times[rep] = t;
                cpu_pps_v[rep] = p;
            }
            let cpu_med_t = median_vec(&cpu_times);
            let cpu_med_pps = median_vec(&cpu_pps_v);

            // --- GPU batch: reps timed repetitions (includes H2D + kernel + D2H), take median ---
            let mut gpu_times = vec![0f64; reps];
            let mut gpu_pps_v = vec![0f64; reps];
            for rep in 0..reps {
                let (t, p) = time_gpu_batch(&matrices);
                gpu_times[rep] = t;
                gpu_pps_v[rep] = p;
            }
            let gpu_med_t = median_vec(&gpu_times);
            let gpu_med_pps = median_vec(&gpu_pps_v);

            let ratio = gpu_med_pps / cpu_med_pps;
            let gpu_wins = ratio > 1.0;

            println!(
                "{:>4}  {:>5}  {:>22.3}  {:>22.3}  {:>12.4}  {}",
                n,
                m,
                cpu_med_pps,
                gpu_med_pps,
                ratio,
                if gpu_wins { "YES" } else { "no" }
            );

            writeln!(
                csv_file,
                "{n},{m},{cpu_med_t:.6},{cpu_med_pps:.3},{gpu_med_t:.6},{gpu_med_pps:.3},{ratio:.6},{gpu_wins}"
            )
            .unwrap();
        }

        println!();
        println!("CSV written: {}", csv_abs.display());
    }

    /// Convert Unix epoch seconds to `(year, month, day)` UTC.
    /// Howard Hinnant civil_from_days algorithm — avoids pulling in chrono/time.
    fn unix_secs_to_ymd(secs: i64) -> (i32, u32, u32) {
        let days = secs.div_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = (z - era * 146_097) as u32;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i32 + era as i32 * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y_final = if m <= 2 { y + 1 } else { y };
        (y_final, m, d)
    }
}

#[cfg(feature = "hip")]
fn main() {
    harness::run();
}
