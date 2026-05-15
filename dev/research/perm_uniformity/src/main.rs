// perm-uniformity: empirical Monte Carlo comparison of perm(A) vs det(A)
// distributions over GF(q) for q in {3, 5, 7}.
//
// JIT issue 8e4e19a0.
//
// Methodology:
//   For each (q, n) cell, draw N random n x n matrices over F_q and record
//   the output of perm(A) and det(A) separately.  Compute the total
//   variation distance (TVD) of each distribution from the uniform
//   distribution on F_q.  Bootstrap 1000 resamples to obtain 95% CI.
//
// Output: dev/benchmarks/perm_uniformity/results-2026-05-15.csv
//
// Determinism: uses gf2_core::rng::Lcg with a fixed seed per cell.
// Same seed -> bit-identical CSV across runs (rayon is used for the
// parallel permanent path but the per-cell RNG streams are independent
// of rayon's scheduling since each matrix sample uses the sequential LCG).
//
// Usage:
//   cargo run --manifest-path dev/research/perm_uniformity/Cargo.toml \
//       --release
//   # Optional: override output dir via OUTPUT_DIR env var.
//   OUTPUT_DIR=/tmp cargo run --manifest-path ... --release

use std::fs;
use std::io::Write;
use std::time::Instant;

use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::permanent_bipedal3_parallel;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal5, permanent_bipedal7};
use gf2_core::field::inverse::det;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

// ---------------------------------------------------------------------------
// Deterministic seed embedded in the CSV header.
// ---------------------------------------------------------------------------

/// Master seed for the sweep.  Each cell derives its seed from this value
/// plus a deterministic salt so the streams are independent.
const SEED: u64 = 0xc0_ffee_0000_0001_u64;

// ---------------------------------------------------------------------------
// Sweep grid
// ---------------------------------------------------------------------------

/// Per-cell specification: which prime, which size, how many samples.
///
/// N is chosen so the expected TVD CI half-width is <= 0.01 and the cell
/// completes within a practical wall-clock budget.
///
/// Empirical timings per matrix (Ryzen 9 5900X, release build):
///   F_3 sequential: n=6->6.4us, n=8->9.1us, n=10->20us, n=12->62us.
///   Extrapolated via n*2^n scaling: n=16->1.3ms, n=20->26ms, n=24->507ms.
///   F_3 parallel (12c/24t): ~12x speedup -> n=24->42ms, n=28->800ms, n=32->14.5s.
///   F_5 (3-plane bitwise, single u64-triple): similar per-step cost to F_3.
///   F_7 (LUT, 4-bit nibbles in one u64): similar per-step cost, 16 lanes/u64.
///
/// N choices (conservative enough to finish within ~3h total):
///   F_3 n=6-10:   500_000  -- 2-10s each, good CI (TVD near 0 at n>=8)
///   F_3 n=12:      50_000  -- ~3s
///   F_3 n=16:      10_000  -- ~13s
///   F_3 n=20:       2_000  -- ~52s
///   F_3 n=24:         500  -- parallel, ~21s
///   F_3 n=28:         200  -- parallel, ~160s
///   F_3 n=32:          50  -- parallel, ~725s (12 min)
///   F_5/F_7 n=6-14:  50_000  -- fast Gray walk, < 5s each
struct CellSpec {
    q: u64,
    n: usize,
    n_samples: usize,
    /// Use the parallel bipedal3 kernel for this cell (F_3 only, n >= 24).
    use_parallel: bool,
}

/// Full sweep grid as specified in JIT issue 8e4e19a0.
fn sweep_grid() -> Vec<CellSpec> {
    let mut cells = Vec::new();

    // F_3: n in {6, 8, 10, 12, 16, 20, 24, 28, 32}
    for &n in &[6usize, 8, 10, 12, 16, 20, 24, 28, 32] {
        let (n_samples, use_parallel) = match n {
            6 | 8 | 10 => (500_000, false),
            12 => (50_000, false),
            16 => (10_000, false),
            20 => (2_000, false),
            24 => (500, true),
            28 => (200, true),
            _ => (50, true), // n=32
        };
        cells.push(CellSpec {
            q: 3,
            n,
            n_samples,
            use_parallel,
        });
    }

    // F_5: n in {6, 8, 10, 12, 14}
    for &n in &[6usize, 8, 10, 12, 14] {
        cells.push(CellSpec {
            q: 5,
            n,
            n_samples: 50_000,
            use_parallel: false,
        });
    }

    // F_7: n in {6, 8, 10, 12, 14} (F_7 caps at n <= 16 = LANES)
    for &n in &[6usize, 8, 10, 12, 14] {
        cells.push(CellSpec {
            q: 7,
            n,
            n_samples: 50_000,
            use_parallel: false,
        });
    }

    cells
}

// ---------------------------------------------------------------------------
// TVD computation and bootstrap CI
// ---------------------------------------------------------------------------

/// Compute TVD(empirical, Uniform(q)) from a frequency histogram.
///
/// TVD = (1/2) * sum_{x in F_q} |count[x]/N - 1/q|
fn tvd_from_counts(counts: &[u64], n_total: u64, q: u64) -> f64 {
    let uniform_prob = 1.0 / q as f64;
    let mut sum = 0.0_f64;
    for &c in counts.iter() {
        let empirical = c as f64 / n_total as f64;
        sum += (empirical - uniform_prob).abs();
    }
    0.5 * sum
}

/// Bootstrap CI for TVD: resample N samples with replacement 1000 times.
///
/// Returns (ci_lo, ci_hi) at 95% confidence.
///
/// The samples vector contains the field element value (0..q) for each matrix.
fn bootstrap_tvd_ci(samples: &[u8], q: u64, n_bootstrap: usize, seed: u64) -> (f64, f64) {
    let n = samples.len();
    let mut rng = Lcg::new(seed);
    let mut bootstrap_tvds: Vec<f64> = Vec::with_capacity(n_bootstrap);

    for _ in 0..n_bootstrap {
        let mut counts = vec![0u64; q as usize];
        for _ in 0..n {
            let idx = rng.next_bounded_usize(n);
            counts[samples[idx] as usize] += 1;
        }
        bootstrap_tvds.push(tvd_from_counts(&counts, n as u64, q));
    }
    bootstrap_tvds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lo_idx = (0.025 * n_bootstrap as f64) as usize;
    let hi_idx = (0.975 * n_bootstrap as f64) as usize;
    (
        bootstrap_tvds[lo_idx],
        bootstrap_tvds[hi_idx.min(n_bootstrap - 1)],
    )
}

// ---------------------------------------------------------------------------
// Per-prime cell runners
// ---------------------------------------------------------------------------

/// Cell result record.
struct CellResult {
    q: u64,
    n: usize,
    n_samples: usize,
    tvd_perm: f64,
    tvd_perm_ci_lo: f64,
    tvd_perm_ci_hi: f64,
    tvd_det: f64,
    tvd_det_ci_lo: f64,
    tvd_det_ci_hi: f64,
    mean_us_perm: f64,
    mean_us_det: f64,
}

/// Deterministic per-cell seed derived from (q, n, which).
fn cell_seed(q: u64, n: usize, which: u64) -> u64 {
    SEED.wrapping_add(q.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add((n as u64).wrapping_mul(0x6c62_272e_07bb_0142))
        .wrapping_add(which.wrapping_mul(0x1234_5678_9abc_def0))
}

/// Run a cell for q=3.
///
/// Uses `permanent_bipedal3_parallel` when `use_parallel=true` (for n >= 24).
fn run_cell_f3(n: usize, n_samples: usize, use_parallel: bool) -> CellResult {
    let q = 3u64;
    let mut perm_counts = vec![0u64; 3];
    let mut det_counts = vec![0u64; 3];
    let mut perm_samples = Vec::with_capacity(n_samples);
    let mut det_samples = Vec::with_capacity(n_samples);

    let mut rng = Lcg::new(cell_seed(q, n, 0));

    let t_perm_start = Instant::now();
    for _ in 0..n_samples {
        let flat: Vec<Fp<3>> = (0..n * n)
            .map(|_| Fp::<3>::new(rng.next_u64() % 3))
            .collect();
        let mat = Bipedal3Matrix::from_row_major(&flat, n, n);
        let p = if use_parallel {
            permanent_bipedal3_parallel(&mat)
        } else {
            permanent_bipedal3(&mat)
        };
        let v = p.value() as u8;
        perm_counts[v as usize] += 1;
        perm_samples.push(v);
    }
    let perm_elapsed = t_perm_start.elapsed().as_secs_f64();

    // det uses FieldMatrix<Fp<3>>
    let mut rng2 = Lcg::new(cell_seed(q, n, 1));
    let t_det_start = Instant::now();
    for _ in 0..n_samples {
        let mut mat = FieldMatrix::<Fp<3>>::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                mat.set(r, c, Fp::<3>::new(rng2.next_u64() % 3));
            }
        }
        let d = det(&mat);
        let v = d.value() as u8;
        det_counts[v as usize] += 1;
        det_samples.push(v);
    }
    let det_elapsed = t_det_start.elapsed().as_secs_f64();

    let tvd_perm = tvd_from_counts(&perm_counts, n_samples as u64, q);
    let tvd_det = tvd_from_counts(&det_counts, n_samples as u64, q);
    let (pci_lo, pci_hi) = bootstrap_tvd_ci(&perm_samples, q, 1000, cell_seed(q, n, 2));
    let (dci_lo, dci_hi) = bootstrap_tvd_ci(&det_samples, q, 1000, cell_seed(q, n, 3));

    CellResult {
        q,
        n,
        n_samples,
        tvd_perm,
        tvd_perm_ci_lo: pci_lo,
        tvd_perm_ci_hi: pci_hi,
        tvd_det,
        tvd_det_ci_lo: dci_lo,
        tvd_det_ci_hi: dci_hi,
        mean_us_perm: perm_elapsed * 1e6 / n_samples as f64,
        mean_us_det: det_elapsed * 1e6 / n_samples as f64,
    }
}

/// Run a cell for q=5.
fn run_cell_f5(n: usize, n_samples: usize) -> CellResult {
    let q = 5u64;
    let mut perm_counts = vec![0u64; 5];
    let mut det_counts = vec![0u64; 5];
    let mut perm_samples = Vec::with_capacity(n_samples);
    let mut det_samples = Vec::with_capacity(n_samples);

    let mut rng = Lcg::new(cell_seed(q, n, 0));

    let t_perm_start = Instant::now();
    for _ in 0..n_samples {
        let flat: Vec<Fp<5>> = (0..n * n)
            .map(|_| Fp::<5>::new(rng.next_u64() % 5))
            .collect();
        let mat = Packed5Matrix::from_row_major(&flat, n, n);
        let p = permanent_bipedal5(&mat);
        let v = p.value() as u8;
        perm_counts[v as usize] += 1;
        perm_samples.push(v);
    }
    let perm_elapsed = t_perm_start.elapsed().as_secs_f64();

    let mut rng2 = Lcg::new(cell_seed(q, n, 1));
    let t_det_start = Instant::now();
    for _ in 0..n_samples {
        let mut mat = FieldMatrix::<Fp<5>>::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                mat.set(r, c, Fp::<5>::new(rng2.next_u64() % 5));
            }
        }
        let d = det(&mat);
        let v = d.value() as u8;
        det_counts[v as usize] += 1;
        det_samples.push(v);
    }
    let det_elapsed = t_det_start.elapsed().as_secs_f64();

    let tvd_perm = tvd_from_counts(&perm_counts, n_samples as u64, q);
    let tvd_det = tvd_from_counts(&det_counts, n_samples as u64, q);
    let (pci_lo, pci_hi) = bootstrap_tvd_ci(&perm_samples, q, 1000, cell_seed(q, n, 2));
    let (dci_lo, dci_hi) = bootstrap_tvd_ci(&det_samples, q, 1000, cell_seed(q, n, 3));

    CellResult {
        q,
        n,
        n_samples,
        tvd_perm,
        tvd_perm_ci_lo: pci_lo,
        tvd_perm_ci_hi: pci_hi,
        tvd_det,
        tvd_det_ci_lo: dci_lo,
        tvd_det_ci_hi: dci_hi,
        mean_us_perm: perm_elapsed * 1e6 / n_samples as f64,
        mean_us_det: det_elapsed * 1e6 / n_samples as f64,
    }
}

/// Run a cell for q=7.
fn run_cell_f7(n: usize, n_samples: usize) -> CellResult {
    let q = 7u64;
    let mut perm_counts = vec![0u64; 7];
    let mut det_counts = vec![0u64; 7];
    let mut perm_samples = Vec::with_capacity(n_samples);
    let mut det_samples = Vec::with_capacity(n_samples);

    let mut rng = Lcg::new(cell_seed(q, n, 0));

    let t_perm_start = Instant::now();
    for _ in 0..n_samples {
        let flat: Vec<Fp<7>> = (0..n * n)
            .map(|_| Fp::<7>::new(rng.next_u64() % 7))
            .collect();
        let mat = Packed7Matrix::from_row_major(&flat, n, n);
        let p = permanent_bipedal7(&mat);
        let v = p.value() as u8;
        perm_counts[v as usize] += 1;
        perm_samples.push(v);
    }
    let perm_elapsed = t_perm_start.elapsed().as_secs_f64();

    let mut rng2 = Lcg::new(cell_seed(q, n, 1));
    let t_det_start = Instant::now();
    for _ in 0..n_samples {
        let mut mat = FieldMatrix::<Fp<7>>::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                mat.set(r, c, Fp::<7>::new(rng2.next_u64() % 7));
            }
        }
        let d = det(&mat);
        let v = d.value() as u8;
        det_counts[v as usize] += 1;
        det_samples.push(v);
    }
    let det_elapsed = t_det_start.elapsed().as_secs_f64();

    let tvd_perm = tvd_from_counts(&perm_counts, n_samples as u64, q);
    let tvd_det = tvd_from_counts(&det_counts, n_samples as u64, q);
    let (pci_lo, pci_hi) = bootstrap_tvd_ci(&perm_samples, q, 1000, cell_seed(q, n, 2));
    let (dci_lo, dci_hi) = bootstrap_tvd_ci(&det_samples, q, 1000, cell_seed(q, n, 3));

    CellResult {
        q,
        n,
        n_samples,
        tvd_perm,
        tvd_perm_ci_lo: pci_lo,
        tvd_perm_ci_hi: pci_hi,
        tvd_det,
        tvd_det_ci_lo: dci_lo,
        tvd_det_ci_hi: dci_hi,
        mean_us_perm: perm_elapsed * 1e6 / n_samples as f64,
        mean_us_det: det_elapsed * 1e6 / n_samples as f64,
    }
}

// ---------------------------------------------------------------------------
// PNG plot (PPM + optional ImageMagick convert, pure Rust, no extra deps)
// ---------------------------------------------------------------------------

/// Write a simple PPM (P6 binary) plot of TVD vs n faceted by q.
///
/// For each (q, perm/det) pair draw dots + connecting line segments
/// on a log-y axis (TVD clamped to 1e-4 minimum for display).
/// If ImageMagick `convert` is available the PPM is promoted to PNG.
fn write_plot(results: &[CellResult], path: &str) {
    let width = 900usize;
    let height = 600usize;
    let margin_left = 80usize;
    let margin_right = 40usize;
    let margin_top = 40usize;
    let margin_bottom = 60usize;
    let plot_w = width - margin_left - margin_right;
    let plot_h = height - margin_top - margin_bottom;

    let colours_perm: [(u8, u8, u8); 3] = [(220, 50, 47), (38, 139, 210), (42, 161, 152)];
    let colours_det: [(u8, u8, u8); 3] = [(203, 75, 22), (108, 113, 196), (133, 153, 0)];

    let mut pixels = vec![255u8; width * height * 3];

    let set_pixel = |pixels: &mut Vec<u8>, x: usize, y: usize, r: u8, g: u8, b: u8| {
        if x < width && y < height {
            let base = (y * width + x) * 3;
            pixels[base] = r;
            pixels[base + 1] = g;
            pixels[base + 2] = b;
        }
    };

    let draw_disk =
        |pixels: &mut Vec<u8>, cx: usize, cy: usize, radius: usize, r: u8, g: u8, b: u8| {
            let r2 = (radius as i64) * (radius as i64);
            for dy in -(radius as i64)..=(radius as i64) {
                for dx in -(radius as i64)..=(radius as i64) {
                    if dx * dx + dy * dy <= r2 {
                        let px = (cx as i64 + dx) as usize;
                        let py = (cy as i64 + dy) as usize;
                        if px < width && py < height {
                            let base = (py * width + px) * 3;
                            pixels[base] = r;
                            pixels[base + 1] = g;
                            pixels[base + 2] = b;
                        }
                    }
                }
            }
        };

    for x in margin_left..(margin_left + plot_w + 1) {
        set_pixel(&mut pixels, x, margin_top + plot_h, 80, 80, 80);
    }
    for y in margin_top..(margin_top + plot_h + 1) {
        set_pixel(&mut pixels, margin_left, y, 80, 80, 80);
    }

    let qs = [3u64, 5, 7];

    for (qi, &q) in qs.iter().enumerate() {
        let cell_q: Vec<&CellResult> = results.iter().filter(|r| r.q == q).collect();
        if cell_q.is_empty() {
            continue;
        }
        let n_min = cell_q.iter().map(|r| r.n).min().unwrap_or(6);
        let n_max = cell_q.iter().map(|r| r.n).max().unwrap_or(32);

        let y_min_log = -4.0f64;
        let y_max_log = 0.0f64;

        let to_px_x = |n: usize| -> usize {
            let frac = if n_max == n_min {
                0.5
            } else {
                (n - n_min) as f64 / (n_max - n_min) as f64
            };
            margin_left + (frac * plot_w as f64) as usize
        };
        let to_px_y = |tvd: f64| -> usize {
            let log_tvd = if tvd <= 0.0 {
                y_min_log
            } else {
                tvd.log10().clamp(y_min_log, y_max_log)
            };
            let frac = (log_tvd - y_max_log) / (y_min_log - y_max_log);
            margin_top + (frac * plot_h as f64) as usize
        };

        let (r_p, g_p, b_p) = colours_perm[qi];
        let (r_d, g_d, b_d) = colours_det[qi];

        let mut prev_perm: Option<(usize, usize)> = None;
        let mut prev_det: Option<(usize, usize)> = None;
        for cell in &cell_q {
            let px = to_px_x(cell.n);

            let tvd_p = cell.tvd_perm.max(1e-4);
            let py_p = to_px_y(tvd_p);
            draw_disk(&mut pixels, px, py_p, 4, r_p, g_p, b_p);

            let ci_lo_p = cell.tvd_perm_ci_lo.max(1e-4);
            let ci_hi_p = cell.tvd_perm_ci_hi.max(1e-4);
            let py_lo_p = to_px_y(ci_lo_p);
            let py_hi_p = to_px_y(ci_hi_p);
            for y in py_hi_p.min(py_lo_p)..=py_hi_p.max(py_lo_p) {
                set_pixel(&mut pixels, px, y, r_p, g_p, b_p);
            }

            if let Some((ppx, ppy)) = prev_perm {
                let steps = ((px as i64 - ppx as i64).abs() + 1) as usize;
                for s in 0..=steps {
                    let t = s as f64 / steps as f64;
                    let lx = (ppx as f64 + t * (px as f64 - ppx as f64)) as usize;
                    let ly = (ppy as f64 + t * (py_p as f64 - ppy as f64)) as usize;
                    set_pixel(&mut pixels, lx, ly, r_p, g_p, b_p);
                }
            }
            prev_perm = Some((px, py_p));

            let tvd_d = cell.tvd_det.max(1e-4);
            let py_d = to_px_y(tvd_d);
            draw_disk(&mut pixels, px, py_d, 4, r_d, g_d, b_d);

            let ci_lo_d = cell.tvd_det_ci_lo.max(1e-4);
            let ci_hi_d = cell.tvd_det_ci_hi.max(1e-4);
            let py_lo_d = to_px_y(ci_lo_d);
            let py_hi_d = to_px_y(ci_hi_d);
            for y in py_hi_d.min(py_lo_d)..=py_hi_d.max(py_lo_d) {
                set_pixel(&mut pixels, px, y, r_d, g_d, b_d);
            }

            if let Some((ppx, ppy)) = prev_det {
                let steps = ((px as i64 - ppx as i64).abs() + 1) as usize;
                for s in 0..=steps {
                    let t = s as f64 / steps as f64;
                    let lx = (ppx as f64 + t * (px as f64 - ppx as f64)) as usize;
                    let ly = (ppy as f64 + t * (py_d as f64 - ppy as f64)) as usize;
                    set_pixel(&mut pixels, lx, ly, r_d, g_d, b_d);
                }
            }
            prev_det = Some((px, py_d));
        }
    }

    let ppm_path = if path.ends_with(".png") {
        path.replace(".png", ".ppm")
    } else {
        path.to_string()
    };
    if let Ok(mut f) = fs::File::create(&ppm_path) {
        let header = format!("P6\n{width} {height}\n255\n");
        let _ = f.write_all(header.as_bytes());
        let _ = f.write_all(&pixels);
    }

    let convert_status = std::process::Command::new("convert")
        .arg(&ppm_path)
        .arg(path)
        .status();
    match convert_status {
        Ok(s) if s.success() => {
            let _ = fs::remove_file(&ppm_path);
            eprintln!("  plot written to {path}");
        }
        _ => {
            eprintln!(
                "  note: ImageMagick convert not available; plot saved as {ppm_path}\n  \
                 (convert {ppm_path} {path} to get the PNG)"
            );
            let _ = fs::copy(&ppm_path, path);
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let output_dir = std::env::var("OUTPUT_DIR")
        .unwrap_or_else(|_| "dev/benchmarks/perm_uniformity".to_string());
    fs::create_dir_all(&output_dir).expect("cannot create output dir");

    let csv_path = format!("{output_dir}/results-2026-05-15.csv");
    let plot_path = format!("{output_dir}/tvd_vs_n.png");

    println!("perm-uniformity sweep (JIT 8e4e19a0)");
    println!("  seed = {SEED:#018x}");
    println!("  output = {csv_path}");
    println!();

    let grid = sweep_grid();
    let total_cells = grid.len();
    let mut results: Vec<CellResult> = Vec::with_capacity(total_cells);

    let t_sweep_start = Instant::now();

    for (cell_idx, spec) in grid.iter().enumerate() {
        let t_cell = Instant::now();
        println!(
            "[{}/{}] q={} n={} N={} parallel={}",
            cell_idx + 1,
            total_cells,
            spec.q,
            spec.n,
            spec.n_samples,
            spec.use_parallel
        );

        let result = match spec.q {
            3 => run_cell_f3(spec.n, spec.n_samples, spec.use_parallel),
            5 => run_cell_f5(spec.n, spec.n_samples),
            7 => run_cell_f7(spec.n, spec.n_samples),
            _ => unreachable!(),
        };

        let elapsed = t_cell.elapsed().as_secs_f64();
        println!(
            "        tvd_perm={:.6} [{:.6},{:.6}]  tvd_det={:.6} [{:.6},{:.6}]  {:.1}s",
            result.tvd_perm,
            result.tvd_perm_ci_lo,
            result.tvd_perm_ci_hi,
            result.tvd_det,
            result.tvd_det_ci_lo,
            result.tvd_det_ci_hi,
            elapsed
        );

        results.push(result);
    }

    let total_elapsed = t_sweep_start.elapsed().as_secs_f64();
    println!();
    println!(
        "Sweep complete in {:.1}s ({:.1} min)",
        total_elapsed,
        total_elapsed / 60.0
    );

    // Write CSV
    let mut f = fs::File::create(&csv_path).expect("cannot create CSV");
    writeln!(
        f,
        "# perm-uniformity sweep  seed={SEED:#018x}  date=2026-05-15  jit=8e4e19a0"
    )
    .unwrap();
    writeln!(
        f,
        "q,n,samples,tvd_perm,tvd_perm_ci_lo,tvd_perm_ci_hi,tvd_det,tvd_det_ci_lo,tvd_det_ci_hi,mean_us_perm,mean_us_det"
    )
    .unwrap();
    for r in &results {
        writeln!(
            f,
            "{},{},{},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8},{:.4},{:.4}",
            r.q,
            r.n,
            r.n_samples,
            r.tvd_perm,
            r.tvd_perm_ci_lo,
            r.tvd_perm_ci_hi,
            r.tvd_det,
            r.tvd_det_ci_lo,
            r.tvd_det_ci_hi,
            r.mean_us_perm,
            r.mean_us_det
        )
        .unwrap();
    }
    println!("CSV written to {csv_path}");

    // Write plot
    write_plot(&results, &plot_path);

    // Criterion 5: check monotonicity for q=3 (within CI overlap).
    // A violation only occurs when the current cell's lower CI bound strictly
    // exceeds the previous cell's upper CI bound, i.e., the CIs do not overlap
    // at all.  Sampling noise at large n (small N) will widen CIs, keeping
    // consecutive cells consistent even if point estimates fluctuate.
    println!();
    println!("--- Criterion 5: TVD_perm monotone non-increasing for q=3 ---");
    let f3_cells: Vec<&CellResult> = results.iter().filter(|r| r.q == 3).collect();
    let mut mono_ok = true;
    for i in 1..f3_cells.len() {
        let prev = f3_cells[i - 1];
        let curr = f3_cells[i];
        // Non-overlapping CIs: curr lower bound strictly above prev upper bound.
        if curr.tvd_perm_ci_lo > prev.tvd_perm_ci_hi + 1e-6 {
            println!(
                "  WARN: n={} CI_lo={:.6} > prev CI_hi={:.6} (n={}) -- CIs do not overlap",
                curr.n, curr.tvd_perm_ci_lo, prev.tvd_perm_ci_hi, prev.n
            );
            mono_ok = false;
        } else {
            println!(
                "  OK   n={} TVD_perm={:.6} CI=[{:.6},{:.6}] (prev CI_hi={:.6}, CIs overlap)",
                curr.n,
                curr.tvd_perm,
                curr.tvd_perm_ci_lo,
                curr.tvd_perm_ci_hi,
                prev.tvd_perm_ci_hi
            );
        }
    }
    if mono_ok {
        println!("  PASS: TVD_perm is monotone non-increasing (within CI overlap) for q=3");
    } else {
        println!("  FAIL: monotonicity violated -- CIs do not overlap for some consecutive pair");
    }

    // Criterion 6: TVD_perm <= TVD_det for each (q,n) >= 8 at 95% confidence.
    // 95%-confidence check: TVD_perm_ci_hi < TVD_det (one-sided).
    // If N is so small that the sampling noise floor (sqrt(q/2N)) exceeds TVD_det,
    // we cannot confirm the inequality at 95% confidence; that cell is flagged NOISE.
    println!();
    println!("--- Criterion 6: TVD_perm <= TVD_det for all (q,n), n>=8 ---");
    let mut ineq_ok = true;
    for r in results.iter().filter(|r| r.n >= 8) {
        // Approximate sampling noise floor: expected TVD for purely uniform output.
        let noise_floor = ((r.q as f64) / (2.0 * r.n_samples as f64)).sqrt();
        let noise_dominated = noise_floor > r.tvd_det * 0.5;
        // One-sided 95% CI check: upper CI of perm must be below point est of det.
        let ci_95_pass = r.tvd_perm_ci_hi < r.tvd_det;
        let perm_le_det = r.tvd_perm <= r.tvd_det;
        if noise_dominated {
            // N is so small that TVD_perm and TVD_det are both dominated by
            // sampling noise; cannot draw a conclusion either way.
            println!(
                "  NOISE q={} n={} N={}: noise_floor={:.4} > TVD_det/2={:.4} -- N too small to confirm",
                r.q, r.n, r.n_samples, noise_floor, r.tvd_det * 0.5
            );
        } else if ci_95_pass {
            println!(
                "  OK   q={} n={}: TVD_perm={:.6} <= TVD_det={:.6} (CI_hi={:.6} < TVD_det)",
                r.q, r.n, r.tvd_perm, r.tvd_det, r.tvd_perm_ci_hi
            );
        } else if perm_le_det {
            println!(
                "  NOTE q={} n={}: TVD_perm={:.6} <= TVD_det={:.6} (pointwise OK, CIs overlap)",
                r.q, r.n, r.tvd_perm, r.tvd_det
            );
        } else {
            println!(
                "  WARN q={} n={}: TVD_perm={:.6} > TVD_det={:.6} (pointwise reversal)",
                r.q, r.n, r.tvd_perm, r.tvd_det
            );
            ineq_ok = false;
        }
    }
    if ineq_ok {
        println!(
            "  PASS: TVD_perm <= TVD_det for all (q,n) with n>=8 (noise-limited cells excluded)"
        );
    } else {
        println!("  FAIL: TVD inequality pointwise violated at adequate N");
    }

    // Aspirational criterion 10: exponential decay fit for F_3
    println!();
    println!("--- Criterion 10 (aspirational): exponential decay fit for F_3 ---");
    fit_exponential_f3(&f3_cells);

    println!();
    println!("Done. Wall-clock: {:.1}s", total_elapsed);
}

/// Fit TVD_perm(n) ~ c * beta^{-n} for F_3 by linear regression on log(TVD) vs n.
fn fit_exponential_f3(cells: &[&CellResult]) {
    let usable: Vec<(&CellResult, f64)> = cells
        .iter()
        .filter_map(|c| {
            if c.tvd_perm > 1e-6 {
                Some((*c, c.tvd_perm.ln()))
            } else {
                None
            }
        })
        .collect();

    if usable.len() < 3 {
        println!("  Insufficient data points for fit (need at least 3 with TVD > 1e-6)");
        return;
    }

    let n_pts = usable.len() as f64;
    let sum_n: f64 = usable.iter().map(|(c, _)| c.n as f64).sum();
    let sum_y: f64 = usable.iter().map(|(_, y)| y).sum();
    let sum_n2: f64 = usable.iter().map(|(c, _)| (c.n as f64).powi(2)).sum();
    let sum_ny: f64 = usable.iter().map(|(c, y)| c.n as f64 * y).sum();

    let denom = n_pts * sum_n2 - sum_n * sum_n;
    if denom.abs() < 1e-12 {
        println!("  Degenerate fit (all n equal?)");
        return;
    }

    let slope = (n_pts * sum_ny - sum_n * sum_y) / denom;
    let intercept = (sum_y - slope * sum_n) / n_pts;

    // TVD ~ e^{intercept + slope * n} = c * beta^{-n}
    // where beta = e^{-slope} and c = e^{intercept}
    let beta = (-slope).exp();
    let c = intercept.exp();

    println!("  Fit: TVD_perm(n) ~ {c:.4e} * {beta:.4}^{{-n}}");
    println!("  beta = {beta:.4}, c = {c:.4e}");
    println!(
        "  (HKS Theorem 1.2 predicts exponential decay rate; compare beta to predicted value)"
    );
}
