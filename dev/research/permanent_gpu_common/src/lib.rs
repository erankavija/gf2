//! Shared harness plumbing for the GPU permanent research stubs.
//!
//! Single source of truth for code that was previously duplicated between
//! `dev/research/permanent_gpu_speedup` (jit:9480f8a6) and
//! `dev/research/permanent_gpu_crossover` (jit:a9e461de): median reduction,
//! deterministic matrix construction, hardware fingerprinting, the common CSV
//! header block, and the rustc/git provenance collectors.
//!
//! The two harnesses still own their *divergent* logic locally (sweep
//! constants, per-metric timing wrappers, and the harness-specific CSV header
//! tail and data rows); only the genuinely shared mechanics live here.

#![deny(unsafe_code)]

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::testutil::random_matrix_with_rng;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;
use std::io::Write;

/// Compute the median of a non-empty slice of `f64` values.
///
/// # Arguments
/// * `v` - non-empty slice; a sorted copy is taken (input is not mutated).
///
/// # Panics
/// Panics if `v` is empty, or if any element is NaN (the partial comparison
/// unwraps).
///
/// # Complexity
/// `O(k log k)` in the slice length `k`.
pub fn median_vec(v: &[f64]) -> f64 {
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

/// Build `m` random `n x n` F_3 matrices from a deterministic LCG seed.
///
/// The same `seed` always yields the same sequence of matrices, so callers get
/// reproducible benchmark inputs across runs and across the two harnesses.
///
/// # Arguments
/// * `n` - matrix dimension (rows = cols = `n`).
/// * `m` - number of matrices to generate.
/// * `seed` - LCG seed.
///
/// # Complexity
/// `O(m * n^2)` element draws.
pub fn build_matrices(n: usize, m: usize, seed: u64) -> Vec<Bipedal3Matrix> {
    let mut rng = Lcg::new(seed);
    (0..m)
        .map(|_| {
            let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
            Bipedal3Matrix::from_row_major(&elems, n, n)
        })
        .collect()
}

/// Host + device fingerprint recorded in benchmark CSV headers.
///
/// `kernel_ver` is always collected; the crossover harness simply does not emit
/// it in its header (its CSV schema predates the kernel line).
pub struct HwInfo {
    /// CPU marketing model string (from `/proc/cpuinfo` `model name`).
    pub cpu_model: String,
    /// GPU marketing name (from `rocminfo` `Marketing Name`).
    pub gpu_name: String,
    /// GPU gfx ISA target (e.g. `gfx1030`, from `rocminfo`).
    pub gfx_target: String,
    /// ROCm version (from `/opt/rocm/.info/version`).
    pub rocm_ver: String,
    /// Kernel release (from `uname -r`).
    pub kernel_ver: String,
}

/// Collect a best-effort hardware fingerprint for the CSV header.
///
/// Every field falls back to a known-good default for the dev host if the
/// probe fails, so the harness never aborts on a missing tool.
///
/// # Complexity
/// Spawns up to three short-lived subprocesses (`rocminfo` twice, `uname`).
pub fn hw_fingerprint() -> HwInfo {
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

/// Write the common CSV header prefix shared by both GPU harnesses.
///
/// Emits, in order: `# {title}`, then `# commit:`, `# rustc:`, `# cpu:`,
/// `# gpu:`, `# gfx_target:`, `# rocm_version:`, an optional `# kernel:`
/// line (only when `include_kernel`), and `# seed:` (`{:#018x}`). The caller
/// is responsible for any harness-specific tail lines (sweep / methodology /
/// note) and the column header row.
///
/// # Arguments
/// * `f` - sink (any [`std::io::Write`]).
/// * `title` - first comment line, written verbatim after `# `.
/// * `hw` - fingerprint whose fields populate the cpu/gpu/gfx/rocm/kernel lines.
/// * `commit_sha` / `rustc_ver` - provenance strings.
/// * `include_kernel` - emit the `# kernel:` line (speedup: `true`,
///   crossover: `false`, preserving each CSV's exact byte layout).
/// * `seed` - deterministic seed, formatted as `{:#018x}`.
pub fn write_csv_header_common(
    f: &mut impl Write,
    title: &str,
    hw: &HwInfo,
    commit_sha: &str,
    rustc_ver: &str,
    include_kernel: bool,
    seed: u64,
) {
    writeln!(f, "# {title}").unwrap();
    writeln!(f, "# commit: {commit_sha}").unwrap();
    writeln!(f, "# rustc: {rustc_ver}").unwrap();
    writeln!(f, "# cpu: {}", hw.cpu_model).unwrap();
    writeln!(f, "# gpu: {}", hw.gpu_name).unwrap();
    writeln!(f, "# gfx_target: {}", hw.gfx_target).unwrap();
    writeln!(f, "# rocm_version: {}", hw.rocm_ver).unwrap();
    if include_kernel {
        writeln!(f, "# kernel: {}", hw.kernel_ver).unwrap();
    }
    writeln!(f, "# seed: {seed:#018x}").unwrap();
}

/// Best-effort `rustc --version` string, or `"unknown"` on failure.
pub fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Best-effort short git SHA of `HEAD`, resolved from `manifest_dir`.
///
/// # Arguments
/// * `manifest_dir` - directory to run `git` in (pass the calling crate's
///   `env!("CARGO_MANIFEST_DIR")`, since that macro is per-crate).
pub fn git_short_sha(manifest_dir: &str) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(manifest_dir)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
