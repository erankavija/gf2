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

/// Parse the CPU model string from `/proc/cpuinfo` contents.
///
/// Returns `None` if no `model name` line is present (caller records
/// `"unknown"` rather than fabricating a value).
fn parse_cpu_model(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
}

/// Parse the gfx ISA target (e.g. `gfx1030`) from `rocminfo` stdout.
///
/// Returns `None` if no `Name:` line containing a `gfx*` token is present.
fn parse_gfx_target(rocminfo_stdout: &str) -> Option<String> {
    rocminfo_stdout
        .lines()
        .filter(|l| l.contains("Name") && l.contains("gfx"))
        .find_map(|l| {
            l.split_whitespace()
                .find(|w| w.starts_with("gfx"))
                .map(|s| s.to_string())
        })
}

/// Parse the GPU marketing name from `rocminfo` stdout.
///
/// Returns `None` if no `Marketing Name:` line for a Radeon device is present.
fn parse_gpu_name(rocminfo_stdout: &str) -> Option<String> {
    rocminfo_stdout
        .lines()
        .find(|l| l.contains("Marketing Name") && l.contains("Radeon"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
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
/// Every field that cannot be probed is recorded as the literal string
/// `"unknown"`. The harness never substitutes a plausible-but-unverified
/// hardware identity (e.g. a hardcoded dev-host GPU name): a CSV must never
/// claim a device that was not actually observed at run time. A missing tool
/// therefore yields `unknown`, not fabricated provenance.
///
/// # Complexity
/// Spawns up to two short-lived subprocesses (`rocminfo` once, `uname` once).
pub fn hw_fingerprint() -> HwInfo {
    let cpu_model = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .as_deref()
        .and_then(parse_cpu_model)
        .unwrap_or_else(|| "unknown".to_string());

    let rocm_ver = std::fs::read_to_string("/opt/rocm/.info/version")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let rocminfo_stdout = std::process::Command::new("rocminfo")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    let gfx_target = parse_gfx_target(&rocminfo_stdout).unwrap_or_else(|| "unknown".to_string());
    let gpu_name = parse_gpu_name(&rocminfo_stdout).unwrap_or_else(|| "unknown".to_string());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cpu_model_extracts_trimmed_value() {
        let cpuinfo = "processor\t: 0\nmodel name\t: AMD Ryzen 9 5900X 12-Core Processor\ncpu MHz\t\t: 3700.0\n";
        assert_eq!(
            parse_cpu_model(cpuinfo).as_deref(),
            Some("AMD Ryzen 9 5900X 12-Core Processor")
        );
    }

    #[test]
    fn parse_cpu_model_none_on_missing_or_garbage() {
        // Probe failure / empty file must NOT fabricate a value.
        assert_eq!(parse_cpu_model(""), None);
        assert_eq!(parse_cpu_model("flags\t: sse avx2\nbogomips: 7400\n"), None);
    }

    #[test]
    fn parse_gfx_target_extracts_token() {
        let rocminfo = "*** Agent 2 ***\n  Name:                    gfx1030\n  Marketing Name:          AMD Radeon RX 6950 XT\n";
        assert_eq!(parse_gfx_target(rocminfo).as_deref(), Some("gfx1030"));
    }

    #[test]
    fn parse_gfx_target_none_on_missing_or_garbage() {
        assert_eq!(parse_gfx_target(""), None);
        // A CPU agent's Name line has no gfx token — must not match.
        assert_eq!(
            parse_gfx_target("  Name:                    AMD Ryzen 9 5900X\n"),
            None
        );
    }

    #[test]
    fn parse_gpu_name_extracts_trimmed_value() {
        let rocminfo = "  Name:                    gfx1030\n  Marketing Name:          AMD Radeon RX 6950 XT\n";
        assert_eq!(
            parse_gpu_name(rocminfo).as_deref(),
            Some("AMD Radeon RX 6950 XT")
        );
    }

    #[test]
    fn parse_gpu_name_none_on_missing_or_non_radeon() {
        assert_eq!(parse_gpu_name(""), None);
        // Non-Radeon marketing name must not be mistaken for the dev GPU.
        assert_eq!(
            parse_gpu_name("  Marketing Name:          Intel Arc A770\n"),
            None
        );
    }

    #[test]
    fn median_vec_odd_even_and_singleton() {
        assert_eq!(median_vec(&[5.0]), 5.0);
        assert_eq!(median_vec(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median_vec(&[4.0, 1.0, 3.0, 2.0]), 2.5);
    }
}
