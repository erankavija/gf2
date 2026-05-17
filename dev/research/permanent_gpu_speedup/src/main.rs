// S1g (jit:9480f8a6): GPU 50x speedup measurement vs reference for permanent_bipedal3 (F_3).
//
// Measures the speedup of the batched GPU path (`permanent_batch_bipedal3`) over the
// single-thread reference (`permanent_mod3_reference`) at n ∈ {24, 28, 32, 36}.
//
// Methodology: the reference timings at every n are LOADED AT RUNTIME from the S1
// speedup CSV produced by the same one-command repro run
// (`dev/benchmarks/gf2_algebra_permanent/s1_speedup-<SA_DATE|today>.csv`, rows
// impl=permanent_mod3_reference; falls back to the newest s1_speedup-*.csv). They
// are NOT hard-coded — this keeps the one-command repro SSOT/end-to-end (S1g
// speedup derives from THIS run's S1 measurement). Given the committed
// s1_speedup-2026-05-11.csv the loaded values reproduce the committed
// s1g_gpu_speedup CSV's ratios byte-for-byte. The GPU contender time is measured
// by batching M matrices through `permanent_batch_bipedal3` and reporting the
// per-matrix-equivalent GPU time as T_gpu = total_wallclock / M. The speedup
// ratio is T_reference / T_gpu.
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
    // S1 reference timings — loaded at RUNTIME from the S1 CSV (SSOT).
    //
    // The reference wall-clock times are NOT hard-coded. They are read at
    // runtime from the S1 speedup CSV produced by the same one-command repro
    // run (dev/benchmarks/gf2_algebra_permanent/s1_speedup-<DATE>.csv, rows
    // impl=permanent_mod3_reference, mean_us column, µs → s). This mirrors
    // the S3 Part-A precedent in scripts/permanent-repro.sh (read the S1 CSV
    // at runtime instead of embedding copied numbers) so the one-command
    // repro is genuinely SSOT / end-to-end: S1g speedup ratios derive from
    // THIS run's S1 measurement, not a stale snapshot.
    //
    // CSV resolution (same approach as the S3 Part-A reader):
    //   1. dev/benchmarks/gf2_algebra_permanent/s1_speedup-<SA_DATE|today>.csv
    //   2. fall back to the newest s1_speedup-*.csv in that directory.
    // Given the committed s1_speedup-2026-05-11.csv the loaded values are
    // {1.473800, 27.360000, 500.027842, 9030.740871} s, identical to the
    // numbers previously embedded, so the computed ratios are unchanged and
    // still match the committed s1g_gpu_speedup CSV.
    // -----------------------------------------------------------------------

    /// Load the S1 reference wall-clock times (seconds) for `N_VALUES` from
    /// the S1 speedup CSV under `workspace_root`.
    ///
    /// Resolves `s1_speedup-<SA_DATE|today>.csv`, falling back to the newest
    /// `s1_speedup-*.csv` in the benchmark directory. Parses rows
    /// `impl=permanent_mod3_reference`, converting the `mean_us` column from
    /// microseconds to seconds.
    ///
    /// Returns `(ref_times_s, s1_csv_basename)`: one timing per `N_VALUES`
    /// entry in order, plus the basename of the S1 CSV actually read (so the
    /// CSV header / console can record the true SSOT source instead of a
    /// hard-coded historical filename).
    ///
    /// # Panics
    ///
    /// Panics if the CSV cannot be located, cannot be read, or does not
    /// contain a `permanent_mod3_reference` row for every `n` in `N_VALUES`.
    /// A missing reference row is a hard SSOT failure — the harness must not
    /// silently substitute a stale or guessed value.
    fn load_ref_times_s(workspace_root: &std::path::Path) -> (Vec<f64>, String) {
        let bench_dir = workspace_root.join("dev/benchmarks/gf2_algebra_permanent");

        // 1. Prefer the SA_DATE/today dated file (same env var the rest of
        //    the repro uses via testutil::today_yyyy_mm_dd()).
        let date = gf2_algebra::testutil::today_yyyy_mm_dd();
        let dated = bench_dir.join(format!("s1_speedup-{date}.csv"));
        let csv_path = if dated.is_file() {
            dated
        } else {
            // 2. Newest s1_speedup-*.csv (ISO dates sort lexically).
            let mut candidates: Vec<std::path::PathBuf> = fs::read_dir(&bench_dir)
                .unwrap_or_else(|e| panic!("read S1 bench dir {}: {e}", bench_dir.display()))
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.starts_with("s1_speedup-") && s.ends_with(".csv"))
                })
                .collect();
            candidates.sort();
            candidates.pop().unwrap_or_else(|| {
                panic!(
                    "no s1_speedup-*.csv found in {} — run the S1 bench (repro step 2) first",
                    bench_dir.display()
                )
            })
        };

        let content = fs::read_to_string(&csv_path)
            .unwrap_or_else(|e| panic!("read S1 CSV {}: {e}", csv_path.display()));

        // Parse: n,impl,mean_us,std_us,samples,ratio_vs_reference,hw_fingerprint
        let mut ref_us: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
        for line in content.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 3 || cols[1] != "permanent_mod3_reference" {
                continue;
            }
            if let (Ok(n), Ok(mean_us)) = (cols[0].parse::<usize>(), cols[2].parse::<f64>()) {
                ref_us.insert(n, mean_us);
            }
        }

        let times: Vec<f64> = N_VALUES
            .iter()
            .map(|&n| {
                let us = ref_us.get(&n).unwrap_or_else(|| {
                    panic!(
                        "S1 CSV {} has no permanent_mod3_reference row for n={n} \
                         (SSOT failure — S1g cannot derive its speedup)",
                        csv_path.display()
                    )
                });
                us * 1e-6
            })
            .collect();

        let basename = csv_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("s1_speedup.csv")
            .to_string();

        (times, basename)
    }

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
    fn write_csv_header_tail(f: &mut File, s1_csv_basename: &str) {
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
            "#   speedup = T_reference / T_gpu_equiv where T_reference is loaded at runtime from"
        )
        .unwrap();
        writeln!(
            f,
            "#   {s1_csv_basename} row impl=permanent_mod3_reference."
        )
        .unwrap();
        writeln!(
            f,
            "# reps: n=24/28/32 use {REP_FAST} reps (median); n=36 uses {REP_SLOW} rep."
        )
        .unwrap();
        writeln!(
            f,
            "# s1_csv: dev/benchmarks/gf2_algebra_permanent/{s1_csv_basename}"
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

        // Load S1 reference timings at RUNTIME from the S1 CSV produced by
        // this same repro run (SSOT — no hard-coded copied numbers).
        let (ref_times_s, s1_csv_basename) = load_ref_times_s(&workspace_root);

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
        write_csv_header_tail(&mut csv_file, &s1_csv_basename);
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
            "  reference (loaded at runtime from {s1_csv_basename}, permanent_mod3_reference rows)"
        );
        println!();
        println!(
            "{:>4}  {:>5}  {:>24}  {:>20}  {:>5}  {:>16}  {:>12}",
            "n", "M", "gpu_total_wall_s", "T_gpu_equiv_s", "reps", "T_ref_s", "speedup"
        );
        println!("{}", "-".repeat(95));

        for (i, &n) in N_VALUES.iter().enumerate() {
            let reps = if n == 36 { REP_SLOW } else { REP_FAST };
            let ref_time_s = ref_times_s[i];

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
