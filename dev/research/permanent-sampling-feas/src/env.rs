//! Host, toolchain, and thermal metadata captured alongside every measurement.
//!
//! The repository requires that a published number trace to an artifact
//! recording seeds, git revision, hardware, and toolchain
//! (`@/inv/claims-trace-to-artifacts`). This module collects the non-seed part
//! of that record; seeds come from [`crate::sampler`].

use std::fmt::Write as _;
use std::process::Command;

/// Run a command and return its trimmed stdout, or `"unavailable"`.
fn capture(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn read_file(path: &str) -> String {
    std::fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string())
}

/// The first `/proc/cpuinfo` value for `key`.
fn cpuinfo(key: &str) -> String {
    let text = match std::fs::read_to_string("/proc/cpuinfo") {
        Ok(t) => t,
        Err(_) => return "unavailable".to_string(),
    };
    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim() == key {
                return v.trim().to_string();
            }
        }
    }
    "unavailable".to_string()
}

/// Static host and toolchain facts, captured once per run.
#[derive(Clone, Debug)]
pub struct HostInfo {
    pub git_sha: String,
    pub git_dirty: bool,
    /// SHA of the most recent commit touching this crate's own source tree.
    ///
    /// `git_sha` alone does not identify the measured source: the repository
    /// carries workflow state (`.jit/`) that other agents commit and modify
    /// independently, so `git_dirty` can be true for reasons unrelated to the
    /// harness. This field, with [`Self::harness_dirty`], pins the state of the
    /// code that actually produced the numbers.
    pub harness_sha: String,
    /// Whether this crate's own source tree has uncommitted changes. A receipt
    /// with `harness_dirty: true` does not identify a reproducible source state
    /// and must not be published as evidence.
    pub harness_dirty: bool,
    pub rustc: String,
    pub cargo: String,
    pub cpu_model: String,
    pub logical_cpus: usize,
    pub rayon_threads: usize,
    pub avx2: bool,
    pub avx512: bool,
    pub governor: String,
    pub gpu_model: String,
    pub rocm_version: String,
    pub hip_feature: bool,
    pub kernel: String,
    pub timestamp_utc: String,
    pub invocation: String,
}

impl HostInfo {
    /// Probe the host. Every field degrades to `"unavailable"` rather than
    /// failing, so a missing GPU never aborts a CPU-only run.
    #[must_use]
    pub fn probe() -> Self {
        let git_status = capture("git", &["status", "--porcelain"]);
        let invocation = std::env::args().collect::<Vec<_>>().join(" ");
        // This crate's own directory, resolved at build time.
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        let harness_status = capture("git", &["status", "--porcelain", "--", crate_dir]);
        Self {
            git_sha: capture("git", &["rev-parse", "HEAD"]),
            git_dirty: git_status != "unavailable" && !git_status.is_empty(),
            harness_sha: capture("git", &["log", "-1", "--format=%H", "--", crate_dir]),
            harness_dirty: harness_status != "unavailable" && !harness_status.is_empty(),
            rustc: capture("rustc", &["--version"]),
            cargo: capture("cargo", &["--version"]),
            cpu_model: cpuinfo("model name"),
            logical_cpus: std::thread::available_parallelism().map_or(0, std::num::NonZero::get),
            rayon_threads: rayon::current_num_threads(),
            avx2: cpuinfo("flags").split_whitespace().any(|f| f == "avx2"),
            avx512: cpuinfo("flags").split_whitespace().any(|f| f == "avx512f"),
            governor: read_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            gpu_model: gpu_model(),
            rocm_version: rocm_version(),
            hip_feature: cfg!(feature = "hip"),
            kernel: capture("uname", &["-r"]),
            timestamp_utc: capture("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
            invocation,
        }
    }

    /// Render as `# key: value` comment lines for a CSV preamble.
    #[must_use]
    pub fn csv_preamble(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# git_sha: {}", self.git_sha);
        let _ = writeln!(s, "# git_worktree_dirty: {}", self.git_dirty);
        let _ = writeln!(s, "# harness_source_sha: {}", self.harness_sha);
        let _ = writeln!(s, "# harness_source_dirty: {}", self.harness_dirty);
        let _ = writeln!(
            s,
            "# note: git_worktree_dirty covers the whole repository, including \
.jit/ workflow state other agents own; harness_source_* pin the state of the \
crate that produced these numbers, which is what makes the run reproducible"
        );
        let _ = writeln!(s, "# rustc: {}", self.rustc);
        let _ = writeln!(s, "# cargo: {}", self.cargo);
        let _ = writeln!(s, "# cpu: {}", self.cpu_model);
        let _ = writeln!(s, "# logical_cpus: {}", self.logical_cpus);
        let _ = writeln!(s, "# rayon_threads: {}", self.rayon_threads);
        let _ = writeln!(s, "# avx2: {}, avx512f: {}", self.avx2, self.avx512);
        let _ = writeln!(s, "# cpu_governor: {}", self.governor);
        let _ = writeln!(s, "# gpu: {}", self.gpu_model);
        let _ = writeln!(s, "# rocm: {}", self.rocm_version);
        let _ = writeln!(s, "# hip_feature: {}", self.hip_feature);
        let _ = writeln!(s, "# kernel: {}", self.kernel);
        let _ = writeln!(s, "# timestamp_utc: {}", self.timestamp_utc);
        let _ = writeln!(s, "# invocation: {}", self.invocation);
        s
    }
}

fn gpu_model() -> String {
    let raw = capture("/opt/rocm/bin/rocm-smi", &["--showproductname", "--csv"]);
    for line in raw.lines() {
        if line.starts_with("card") {
            return line.to_string();
        }
    }
    raw.lines().next().unwrap_or("unavailable").to_string()
}

fn rocm_version() -> String {
    let v = read_file("/opt/rocm/.info/version");
    if v != "unavailable" {
        return v;
    }
    capture("/opt/rocm/bin/hipcc", &["--version"])
        .lines()
        .next()
        .unwrap_or("unavailable")
        .to_string()
}

/// Thermal and clock state, sampled per measurement cell.
#[derive(Clone, Debug, Default)]
pub struct ThermalSample {
    /// Mean of every `scaling_cur_freq`, in MHz.
    pub cpu_mhz_mean: f64,
    /// `k10temp` Tctl, in degrees Celsius.
    pub cpu_temp_c: f64,
    /// GPU edge temperature in degrees Celsius, or `f64::NAN` without ROCm.
    pub gpu_temp_c: f64,
}

impl ThermalSample {
    #[must_use]
    pub fn probe() -> Self {
        Self {
            cpu_mhz_mean: cpu_mhz_mean(),
            cpu_temp_c: cpu_temp_c(),
            gpu_temp_c: gpu_temp_c(),
        }
    }
}

fn cpu_mhz_mean() -> f64 {
    let mut total = 0.0f64;
    let mut count = 0usize;
    let Ok(dir) = std::fs::read_dir("/sys/devices/system/cpu") else {
        return f64::NAN;
    };
    for entry in dir.flatten() {
        let path = entry.path().join("cpufreq/scaling_cur_freq");
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(khz) = text.trim().parse::<f64>() {
                total += khz / 1000.0;
                count += 1;
            }
        }
    }
    if count == 0 {
        f64::NAN
    } else {
        total / count as f64
    }
}

/// Read Tctl from the first `k10temp` hwmon node.
fn cpu_temp_c() -> f64 {
    let Ok(dir) = std::fs::read_dir("/sys/class/hwmon") else {
        return f64::NAN;
    };
    for entry in dir.flatten() {
        let name = read_file(&entry.path().join("name").to_string_lossy());
        if name == "k10temp" {
            let raw = read_file(&entry.path().join("temp1_input").to_string_lossy());
            if let Ok(milli) = raw.parse::<f64>() {
                return milli / 1000.0;
            }
        }
    }
    f64::NAN
}

fn gpu_temp_c() -> f64 {
    let raw = capture("/opt/rocm/bin/rocm-smi", &["--showtemp", "--csv"]);
    for line in raw.lines().skip(1) {
        for field in line.split(',').skip(1) {
            if let Ok(v) = field.trim().parse::<f64>() {
                return v;
            }
        }
    }
    f64::NAN
}

/// Pin the calling thread to `core`, or release it to every logical CPU.
///
/// Single-thread cells run pinned so that a migration between a physical core
/// and its SMT sibling cannot masquerade as a throughput difference; rayon
/// cells release the mask so the pool can use the whole machine.
///
/// Returns `false` if the affinity call failed, in which case the caller
/// records the cell as unpinned rather than aborting.
pub fn pin_thread(core: Option<usize>) -> bool {
    // SAFETY: `cpu_set_t` is a plain bitset that `CPU_ZERO`/`CPU_SET` fully
    // initialise before `sched_setaffinity` reads it, and passing pid 0 targets
    // the calling thread on Linux.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        match core {
            Some(c) => libc::CPU_SET(c, &mut set),
            None => {
                let n = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
                for c in 0..n {
                    libc::CPU_SET(c, &mut set);
                }
            }
        }
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
    }
}
