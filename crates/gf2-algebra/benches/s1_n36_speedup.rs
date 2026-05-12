//! S1 (jit:c98ed603) — dedicated single-thread speedup benchmark.
//!
//! Measures `permanent_bipedal3` (SIMD path, T13) against
//! `permanent_mod3_reference` (T8) at `n ∈ {24, 28, 32, 36}` on the dev host
//! (AMD Ryzen 9 5900X, Zen 3, AVX2-only).
//!
//! ## Structure
//!
//! - **Criterion cells** cover `n ∈ {24, 28}` in two benchmark groups
//!   (`s1_permanent_mod3_reference` and `s1_permanent_bipedal3`).
//!   `sample_size(10)` with a 25 s `measurement_time` keeps each cell under
//!   the criterion-4 60 s/cell budget on the dev host.
//!
//! - **Offline cells** cover `n ∈ {32, 36}`.  Activated by setting
//!   `S1_OFFLINE=1` in the environment.  Each cell takes a single wall-clock
//!   sample (Criterion's 10-sample minimum would require ~20 min for n=32 ref
//!   and ~100 hr for n=36 ref on this hardware).
//!
//! ## Usage
//!
//! ```bash
//! # Criterion sweep (n=24, n=28, ~4 min total):
//! cargo bench -p gf2-algebra --features "simd test-support" --bench s1_n36_speedup
//!
//! # Offline one-shot timing for n=32 (~20 min) and n=36 (~10 hr):
//! S1_OFFLINE=1 cargo bench -p gf2-algebra --features "simd test-support" \
//!   --bench s1_n36_speedup -- --nocapture
//!
//! # Offline for n=32 only (skip n=36):
//! S1_OFFLINE=1 S1_OFFLINE_MAX_N=32 cargo bench -p gf2-algebra \
//!   --features "simd test-support" --bench s1_n36_speedup -- --nocapture
//! ```
//!
//! ## CSV output
//!
//! `dev/benchmarks/gf2_algebra_permanent/s1_speedup-<DATE>.csv`
//! (overridable via `SA_DATE`).  Columns:
//!
//! `n,impl,mean_us,std_us,samples,ratio_vs_reference,hardware_fingerprint`
//!
//! The offline harness appends rows to the CSV if it already exists (so a
//! Criterion run for n=24/28 can be followed by an offline run for n=32/36).
//!
//! ## Reproducibility
//!
//! All inputs come from [`gf2_algebra::testutil::random_matrix`] seeded from
//! the JIT issue ID (`c98ed603`).  Both implementations at each `n` use the
//! same seed so the speedup ratio is over identical inputs.

use criterion::{black_box, criterion_group, BenchmarkId, Criterion};
use std::time::Duration;

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_mod3_reference};
use gf2_algebra::testutil::random_matrix;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Base seed derived from the JIT issue ID `c98ed603`.
const S1_SEED_BASE: u64 = 0xc98e_d603_0000_0000_u64;

/// Hardware fingerprint embedded in the CSV header (verified by `lscpu`).
const HW_MODEL: &str = "AMD Ryzen 9 5900X 12-Core Processor";
const HW_ARCH: &str = "Zen 3";
const HW_AVX2: &str = "yes";
const HW_AVX512: &str = "no";

// ---------------------------------------------------------------------------
// Criterion benchmark groups (n ∈ {24, 28})
// ---------------------------------------------------------------------------

/// Criterion group: `permanent_mod3_reference` at n ∈ {24, 28}.
///
/// n=32 and n=36 are excluded from Criterion: Criterion's minimum
/// `sample_size` of 10 puts n=32 at ~200 min/cell and n=36 at ~100 hr/cell.
/// Those cells are covered by the offline harness (`S1_OFFLINE=1`).
///
/// # Arguments
///
/// * `c` — Criterion context injected by the `criterion_group!` harness.
fn s1_bench_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("s1_permanent_mod3_reference");
    group.sample_size(10); // Criterion's hard minimum.
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(25));

    for n in [24usize, 28] {
        let seed = S1_SEED_BASE.wrapping_add(n as u64);
        let row_major = random_matrix::<3>(n, seed);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n_val| {
            b.iter(|| permanent_mod3_reference(black_box(&row_major), black_box(n_val)))
        });
    }
    group.finish();
}

/// Criterion group: `permanent_bipedal3` (SIMD path) at n ∈ {24, 28}.
///
/// Uses the same seed as the reference group (same base + n-offset) so both
/// implementations receive bit-identical inputs and the speedup ratio is
/// over the same matrices.
///
/// # Arguments
///
/// * `c` — Criterion context injected by the `criterion_group!` harness.
fn s1_bench_bipedal3(c: &mut Criterion) {
    let mut group = c.benchmark_group("s1_permanent_bipedal3");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(25));

    for n in [24usize, 28] {
        // Same seed as s1_bench_reference so inputs are identical.
        let seed = S1_SEED_BASE.wrapping_add(n as u64);
        let row_major = random_matrix::<3>(n, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| permanent_bipedal3(black_box(&mat)))
        });
    }
    group.finish();
}

criterion_group!(s1_benches, s1_bench_reference, s1_bench_bipedal3);

// ---------------------------------------------------------------------------
// Offline one-shot harness (n ∈ {32, 36})
// ---------------------------------------------------------------------------

/// Offline timing harness for n=32 and n=36.
///
/// Called from `main` when `S1_OFFLINE=1` is set.  Takes a single wall-clock
/// sample per (n, impl) cell, writes CSV rows, and prints progress to stdout.
///
/// # Arguments
///
/// * `csv`   — open writer for the CSV file (must already have a header).
/// * `max_n` — skip any `n` above this value (e.g. 32 to skip n=36).
/// * `date`  — date string for progress output.
///
/// # Panics
///
/// Panics if `permanent_mod3_reference` and `permanent_bipedal3` disagree on
/// the same input, indicating a correctness regression in the SIMD path.
fn run_offline_cells(csv: &mut (impl std::io::Write + ?Sized), max_n: usize, date: &str) {
    use std::time::Instant;

    println!("S1 offline harness: single-sample wall-clock timing for n ∈ {{32, 36}}.");
    println!("  date    : {date}");
    println!("  max_n   : {max_n}");
    println!("  host    : {HW_MODEL}");
    println!("  arch    : {HW_ARCH}");
    println!("  avx2    : {HW_AVX2}   avx512: {HW_AVX512}");
    println!("  seed    : {S1_SEED_BASE:#018x}");
    println!();

    let hw_tag = format!("{HW_MODEL}/{HW_ARCH}/AVX2={HW_AVX2}");

    for n in [32usize, 36] {
        if n > max_n {
            println!("n={n}: skipped (> max_n={max_n}).");
            continue;
        }

        let seed = S1_SEED_BASE.wrapping_add(n as u64);
        let row_major = random_matrix::<3>(n, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, n, n);
        let total_subsets: u128 = (1u128 << n) - 1;

        println!("n={n}: measuring permanent_mod3_reference ({total_subsets} subsets) ...");
        let t0 = Instant::now();
        let ref_result = std::hint::black_box(permanent_mod3_reference(&row_major, n));
        let ref_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
        println!(
            "  permanent_mod3_reference: {:.3} s  (result={})",
            ref_us / 1_000_000.0,
            ref_result.value()
        );

        println!("n={n}: measuring permanent_bipedal3 (SIMD) ...");
        let t1 = Instant::now();
        let bip_result = std::hint::black_box(permanent_bipedal3(&mat));
        let bip_us = t1.elapsed().as_secs_f64() * 1_000_000.0;
        println!(
            "  permanent_bipedal3-simd  : {:.3} s  (result={})",
            bip_us / 1_000_000.0,
            bip_result.value()
        );

        assert_eq!(
            ref_result,
            bip_result,
            "S1 offline: correctness failure at n={n}: ref={} bip={}",
            ref_result.value(),
            bip_result.value()
        );

        let ratio = ref_us / bip_us;
        let cpu_verdict = if ratio >= 10.0 {
            "PASS (>= 10x CPU SIMD)"
        } else {
            "FAIL (< 10x CPU SIMD)"
        };
        let aspirational = if ratio >= 50.0 {
            " [also >= 50x — exceeds the GPU-target aspiration]"
        } else {
            ""
        };
        println!("  speedup n={n}: {ratio:.2}x  {cpu_verdict}{aspirational}");
        println!();

        writeln!(
            csv,
            "{n},permanent_mod3_reference,{ref_us:.3},N/A,1,1.000,{hw_tag}"
        )
        .expect("write reference CSV row");
        writeln!(
            csv,
            "{n},permanent_bipedal3_simd,{bip_us:.3},N/A,1,{ratio:.4},{hw_tag}"
        )
        .expect("write bipedal3 CSV row");
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Bench entry point.
///
/// When `S1_OFFLINE=1` is set: runs the offline single-sample harness for
/// `n ∈ {32, 36}` and writes the CSV, then exits without invoking Criterion.
///
/// Otherwise: runs the Criterion groups for `n ∈ {24, 28}` — equivalent to
/// the `criterion_main!(s1_benches)` expansion.
///
/// This manual main replaces the `criterion_main!` macro so that a single
/// bench binary can serve both the CI Criterion path and the long-running
/// offline timing path without a separate example binary.
///
/// # Examples
///
/// ```bash
/// # Criterion (n=24, 28):
/// cargo bench -p gf2-algebra --features "simd test-support" --bench s1_n36_speedup
///
/// # Offline (n=32, 36):
/// S1_OFFLINE=1 cargo bench -p gf2-algebra --features "simd test-support" \
///   --bench s1_n36_speedup -- --nocapture
/// ```
fn main() {
    use gf2_algebra::testutil::today_yyyy_mm_dd;
    use std::fs::{self, File, OpenOptions};
    use std::io::Write as IoWrite;

    let offline = std::env::var("S1_OFFLINE").unwrap_or_default() == "1";

    if offline {
        let date = today_yyyy_mm_dd();
        // `cargo bench` sets the binary's working directory to the package
        // directory (`crates/gf2-algebra/`), not the workspace root.
        // Navigate to the workspace root at runtime using the compile-time
        // CARGO_MANIFEST_DIR constant (crates/gf2-algebra → ../../ → workspace).
        let workspace_root = {
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            std::path::Path::new(manifest_dir)
                .parent() // → crates/
                .and_then(|p| p.parent()) // → workspace root
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        };
        let csv_dir = workspace_root.join("dev/benchmarks/gf2_algebra_permanent");
        let csv_path = csv_dir.join(format!("s1_speedup-{date}.csv"));
        fs::create_dir_all(&csv_dir).expect("create CSV directory");

        let csv_exists = csv_path.exists();
        let mut csv: Box<dyn IoWrite> = if csv_exists {
            // Append to an existing file created by a prior Criterion run.
            Box::new(
                OpenOptions::new()
                    .append(true)
                    .open(&csv_path)
                    .expect("open CSV for append"),
            )
        } else {
            // Fresh file: write the header block first.
            let mut f = File::create(&csv_path).expect("create CSV");
            writeln!(
                f,
                "# S1 (jit:c98ed603) single-thread speedup: permanent_bipedal3-simd vs permanent_mod3_reference"
            )
            .unwrap();
            writeln!(f, "# date: {date}").unwrap();
            writeln!(f, "# host: {HW_MODEL}").unwrap();
            writeln!(f, "# arch: {HW_ARCH}").unwrap();
            writeln!(f, "# avx2: {HW_AVX2}, avx512: {HW_AVX512}").unwrap();
            writeln!(f, "# seed_base: {S1_SEED_BASE:#018x}").unwrap();
            writeln!(
                f,
                "# method: n=24/28 from Criterion (sample_size=10, 25s measurement_time); \
                 n=32/36 offline (1 sample each)"
            )
            .unwrap();
            writeln!(
                f,
                "n,impl,mean_us,std_us,samples,ratio_vs_reference,hardware_fingerprint"
            )
            .unwrap();
            Box::new(f)
        };

        let max_n: usize = std::env::var("S1_OFFLINE_MAX_N")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(36);

        run_offline_cells(&mut *csv, max_n, &date);
        let csv_path_display = csv_path.display();
        println!("CSV written/appended to: {csv_path_display}");
        return;
    }

    // Normal path: run Criterion groups, then print the final summary.
    // This replicates the `criterion_main!(s1_benches)` expansion manually.
    s1_benches();
    criterion::Criterion::default()
        .configure_from_args()
        .final_summary();
}
