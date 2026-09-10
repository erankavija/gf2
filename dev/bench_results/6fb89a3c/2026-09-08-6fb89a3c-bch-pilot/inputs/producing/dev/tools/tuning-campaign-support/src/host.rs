//! Runtime host observation, CPU affinity, topology, and admission helpers.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

fn invalid(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

/// Runs a command and returns trimmed UTF-8 standard output.
///
/// Returns an error when the command cannot be run, exits unsuccessfully, or
/// emits invalid UTF-8.
pub fn command_text(program: &str, args: &[&str]) -> io::Result<String> {
    let result = Command::new(program).args(args).output()?;
    if !result.status.success() {
        return Err(invalid(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    Ok(String::from_utf8(result.stdout)
        .map_err(invalid)?
        .trim()
        .to_owned())
}

/// A nonempty, strictly increasing set of logical CPU identifiers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<u32>", into = "Vec<u32>")]
pub struct CpuAffinity(Vec<u32>);

impl TryFrom<Vec<u32>> for CpuAffinity {
    type Error = io::Error;

    fn try_from(cpus: Vec<u32>) -> io::Result<Self> {
        if cpus.is_empty() || cpus.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid(
                "CPU affinity must be a nonempty increasing CPU set",
            ));
        }
        Ok(Self(cpus))
    }
}

impl From<CpuAffinity> for Vec<u32> {
    fn from(value: CpuAffinity) -> Self {
        value.0
    }
}

impl CpuAffinity {
    /// Parses a Linux CPU-list expression such as `0-2,5`.
    ///
    /// Returns an error for malformed, empty, reversed, duplicate, or
    /// unordered CPU sets.
    pub fn parse(list: &str) -> io::Result<Self> {
        let mut cpus = Vec::new();
        for part in list.split(',') {
            if let Some((first, last)) = part.split_once('-') {
                let first = first.parse::<u32>().map_err(invalid)?;
                let last = last.parse::<u32>().map_err(invalid)?;
                if first > last {
                    return Err(invalid("reversed CPU affinity range"));
                }
                cpus.extend(first..=last);
            } else {
                cpus.push(part.parse::<u32>().map_err(invalid)?);
            }
        }
        Self::try_from(cpus)
    }

    /// Returns the logical CPUs in canonical ascending order.
    pub fn cpus(&self) -> &[u32] {
        &self.0
    }

    /// Observes the current process's Linux CPU affinity mask.
    ///
    /// Returns an error when the kernel does not expose a valid affinity list.
    pub fn observe() -> io::Result<Self> {
        let status = fs::read_to_string("/proc/self/status")?;
        let list = status
            .lines()
            .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
            .ok_or_else(|| invalid("OS CPU affinity unavailable"))?;
        Self::parse(list.trim())
    }

    /// Identifies the host and its observed affinity using the receipt format.
    ///
    /// Returns an error if hostname observation or JSON encoding fails.
    pub fn host_identity(&self) -> io::Result<String> {
        Ok(format!(
            "{};cpus={}",
            command_text("hostname", &[])?,
            serde_json::to_string(&self.0).map_err(invalid)?
        ))
    }
}

/// Requires the observed affinity to equal an immutable expected affinity.
///
/// Returns an error when the two masks differ.
pub fn require_affinity(expected: &CpuAffinity, observed: &CpuAffinity) -> io::Result<()> {
    if expected != observed {
        return Err(invalid(
            "actual CPU affinity differs from immutable preparation",
        ));
    }
    Ok(())
}

/// Reports whether a path's advisory lock can be acquired immediately.
///
/// Returns an error when the lock file cannot be opened or probed.
pub fn lock_available(path: &Path) -> io::Result<bool> {
    let file = File::options().read(true).write(true).open(path)?;
    match file.try_lock() {
        Ok(()) => {
            file.unlock()?;
            Ok(true)
        }
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Verifies that the current process inherited a descriptor for a held lock.
///
/// Returns the current process ID, or an error if the lock is available or no
/// inherited descriptor resolves to the canonical lock path.
pub fn inherited_lock(path: &Path) -> io::Result<u32> {
    if lock_available(path)? {
        return Err(invalid("benchmark lock is not held"));
    }
    let expected = fs::canonicalize(path)?;
    let pid = std::process::id();
    let mut inherited = false;
    for fd in fs::read_dir(format!("/proc/{pid}/fd"))? {
        let fd = fd?;
        if fs::read_link(fd.path()).ok().as_ref() == Some(&expected) {
            inherited = true;
            break;
        }
    }
    if !inherited {
        return Err(invalid(
            "driver has no inherited descriptor for the held lock",
        ));
    }
    Ok(pid)
}

fn parse_cpu_list(list: &str) -> io::Result<Vec<u32>> {
    if list.trim().is_empty() {
        return Ok(Vec::new());
    }
    CpuAffinity::parse(list).map(Into::into)
}

fn cpu_number(path: &Path) -> Option<u32> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("cpu"))
        .and_then(|number| number.parse().ok())
}

fn read_cpu_list(path: &Path) -> io::Result<Vec<u32>> {
    parse_cpu_list(fs::read_to_string(path)?.trim())
}

/// One logical CPU as sysfs reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalCpu {
    /// The logical CPU identifier.
    pub cpu: u32,
    /// The physical core identifier within the package.
    pub core_id: u32,
    /// The physical package identifier.
    pub package_id: u32,
    /// Logical CPUs sharing this core.
    pub thread_siblings: Vec<u32>,
    /// Logical CPUs in this CPU's highest-level shared cache.
    pub l3_shared: Vec<u32>,
}

/// Runtime-observed CPU topology from `/sys/devices/system/cpu`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuTopology {
    /// Every observed logical CPU, sorted by CPU identifier.
    pub cpus: Vec<LogicalCpu>,
    /// The kernel's SMT state, when exposed by sysfs.
    pub smt_active: Option<bool>,
}

impl CpuTopology {
    /// Reads CPU identity, sibling, cache, and SMT state from Linux sysfs.
    ///
    /// Returns an error when a CPU directory or required topology file cannot
    /// be read or parsed. Missing cache directories produce an empty cache list.
    pub fn observe() -> io::Result<Self> {
        let root = Path::new("/sys/devices/system/cpu");
        let mut cpu_paths = Vec::new();
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            if cpu_number(&path).is_some() {
                cpu_paths.push(path);
            }
        }
        cpu_paths.sort_by_key(|path| cpu_number(path).unwrap_or(u32::MAX));
        if cpu_paths.is_empty() {
            return Err(invalid("CPU topology unavailable"));
        }

        let mut cpus = Vec::with_capacity(cpu_paths.len());
        for path in cpu_paths {
            let cpu = cpu_number(&path).ok_or_else(|| invalid("invalid CPU directory"))?;
            let topology = path.join("topology");
            let core_id = fs::read_to_string(topology.join("core_id"))?
                .trim()
                .parse()
                .map_err(invalid)?;
            let package_id = fs::read_to_string(topology.join("physical_package_id"))?
                .trim()
                .parse()
                .map_err(invalid)?;
            let thread_siblings = match read_cpu_list(&topology.join("thread_siblings_list")) {
                Ok(list) => list,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    read_cpu_list(&topology.join("core_cpus_list"))?
                }
                Err(error) => return Err(error),
            };
            let l3_shared = highest_level_cache(&path)?;
            cpus.push(LogicalCpu {
                cpu,
                core_id,
                package_id,
                thread_siblings,
                l3_shared,
            });
        }
        let smt_active = match fs::read_to_string(root.join("smt/active")) {
            Ok(value) => match value.trim() {
                "1" => Some(true),
                "0" => Some(false),
                other => return Err(invalid(format!("invalid SMT state {other}"))),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        Ok(Self { cpus, smt_active })
    }

    /// Returns distinct physical cores with their logical CPUs sorted.
    pub fn physical_cores(&self) -> Vec<Vec<u32>> {
        let mut cores: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
        for cpu in &self.cpus {
            cores
                .entry((cpu.package_id, cpu.core_id))
                .or_default()
                .push(cpu.cpu);
        }
        cores
            .into_values()
            .map(|mut cpus| {
                cpus.sort_unstable();
                cpus
            })
            .collect()
    }
}

fn highest_level_cache(cpu_path: &Path) -> io::Result<Vec<u32>> {
    let cache_root = cpu_path.join("cache");
    let mut candidates: Vec<(u32, PathBuf)> = Vec::new();
    let entries = match fs::read_dir(&cache_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("index") {
            continue;
        }
        let level = fs::read_to_string(path.join("level"))?
            .trim()
            .parse::<u32>()
            .map_err(invalid)?;
        candidates.push((level, path));
    }
    let Some((_, path)) = candidates.into_iter().max_by_key(|(level, _)| *level) else {
        return Ok(Vec::new());
    };
    match read_cpu_list(&path.join("shared_cpu_list")) {
        Ok(list) => Ok(list),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Physical-core arms named by the Zen 3 measurement contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoreArm {
    /// One representative logical CPU from the lowest physical core.
    SingleCore,
    /// Six physical-core representatives.
    #[serde(rename = "physical-cores-6")]
    PhysicalCores6,
    /// Twelve physical-core representatives.
    #[serde(rename = "physical-cores-12")]
    PhysicalCores12,
    /// Twenty-four logical CPUs.
    #[serde(rename = "logical-cpus-24")]
    LogicalCpus24,
}

/// Resolves a measurement arm within an observed affinity mask.
///
/// Returns the selected CPU IDs in ascending order, or a reason explaining
/// why the arm is inapplicable. No CPU outside the affinity mask is returned.
pub fn resolve_core_arm(
    arm: CoreArm,
    topology: &CpuTopology,
    affinity: &CpuAffinity,
) -> Result<Vec<u32>, String> {
    let mask: BTreeSet<u32> = affinity.cpus().iter().copied().collect();
    let mut cores = BTreeMap::<(u32, u32), Vec<u32>>::new();
    for cpu in &topology.cpus {
        if mask.contains(&cpu.cpu) {
            cores
                .entry((cpu.package_id, cpu.core_id))
                .or_default()
                .push(cpu.cpu);
        }
    }
    for cpus in cores.values_mut() {
        cpus.sort_unstable();
    }
    let representatives: Vec<u32> = cores
        .values()
        .filter_map(|cpus| cpus.first().copied())
        .collect();
    match arm {
        CoreArm::SingleCore => representatives
            .first()
            .copied()
            .map(|cpu| vec![cpu])
            .ok_or_else(|| "affinity mask holds 0 physical cores, 1 required".to_owned()),
        CoreArm::PhysicalCores6 => {
            let mut domains = BTreeMap::<Vec<u32>, Vec<u32>>::new();
            for (core, logical) in &cores {
                let l3 = topology
                    .cpus
                    .iter()
                    .find(|cpu| cpu.cpu == logical[0])
                    .map(|cpu| cpu.l3_shared.clone())
                    .unwrap_or_default();
                domains
                    .entry(l3)
                    .or_default()
                    .push(representatives_for_core(&cores, core));
            }
            let selected = domains
                .values()
                .max_by(|left, right| left.len().cmp(&right.len()).then_with(|| right.cmp(left)))
                .filter(|domain| domain.len() >= 6)
                .map(|domain| domain.iter().copied().take(6).collect::<Vec<_>>())
                .unwrap_or_else(|| representatives.iter().copied().take(6).collect());
            if selected.len() < 6 {
                return Err(format!(
                    "affinity mask holds {} physical cores, 6 required",
                    representatives.len()
                ));
            }
            sorted(selected)
        }
        CoreArm::PhysicalCores12 => {
            if representatives.len() < 12 {
                return Err(format!(
                    "affinity mask holds {} physical cores, 12 required",
                    representatives.len()
                ));
            }
            sorted(representatives.into_iter().take(12).collect())
        }
        CoreArm::LogicalCpus24 => {
            if mask.len() < 24 {
                return Err(format!(
                    "affinity mask holds {} logical CPUs, 24 required",
                    mask.len()
                ));
            }
            Ok(mask.into_iter().collect())
        }
    }
}

fn representatives_for_core(cores: &BTreeMap<(u32, u32), Vec<u32>>, core: &(u32, u32)) -> u32 {
    cores[core][0]
}

fn sorted(mut cpus: Vec<u32>) -> Result<Vec<u32>, String> {
    cpus.sort_unstable();
    Ok(cpus)
}

/// Complete runtime host observation recorded in every receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostObservation {
    /// UTC timestamp at which this observation was collected.
    pub observed_utc: String,
    /// Runtime hostname.
    pub hostname: String,
    /// CPU model string from `/proc/cpuinfo`.
    pub cpu_model: String,
    /// Deduplicated, sorted CPU flags from `/proc/cpuinfo`.
    pub cpu_flags: Vec<String>,
    /// Kernel release and name from `uname -sr`.
    pub os_kernel: String,
    /// Scaling governor keyed by CPU directory.
    pub governors: BTreeMap<String, String>,
    /// Runtime SMT state, when exposed.
    pub smt_active: Option<bool>,
    /// Current process affinity.
    pub affinity: CpuAffinity,
    /// Runtime CPU topology.
    pub topology: CpuTopology,
    /// The first three fields of `/proc/loadavg`.
    pub load_average: [f64; 3],
    /// Available memory in kiB from `/proc/meminfo`.
    pub available_memory_kib: u64,
}

impl HostObservation {
    /// Observes all host fields at call time from procfs, sysfs, and commands.
    ///
    /// Returns an error when any required runtime observation is unavailable or
    /// malformed, including when no CPU governors can be observed.
    pub fn observe() -> io::Result<Self> {
        let cpu = fs::read_to_string("/proc/cpuinfo")?;
        let value = |key: &str| {
            cpu.lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    (name.trim() == key).then(|| value.trim().to_owned())
                })
                .ok_or_else(|| invalid(format!("missing {key}")))
        };
        let mut flags: Vec<String> = value("flags")?
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        flags.sort_unstable();
        flags.dedup();
        let mut governors = BTreeMap::new();
        for entry in fs::read_dir("/sys/devices/system/cpu")? {
            let path = entry?.path();
            if cpu_number(&path).is_none() {
                continue;
            }
            let governor = path.join("cpufreq/scaling_governor");
            if let Ok(value) = fs::read_to_string(&governor) {
                governors.insert(
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    value.trim().to_owned(),
                );
            }
        }
        if governors.is_empty() {
            return Err(invalid("CPU governor observation unavailable"));
        }
        let load = fs::read_to_string("/proc/loadavg")?;
        let mut load_fields = load.split_whitespace();
        let load_average = [
            load_fields
                .next()
                .ok_or_else(|| invalid("load average unavailable"))?
                .parse()
                .map_err(invalid)?,
            load_fields
                .next()
                .ok_or_else(|| invalid("load average unavailable"))?
                .parse()
                .map_err(invalid)?,
            load_fields
                .next()
                .ok_or_else(|| invalid("load average unavailable"))?
                .parse()
                .map_err(invalid)?,
        ];
        let available_memory_kib = fs::read_to_string("/proc/meminfo")?
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                (name == "MemAvailable")
                    .then(|| value.split_whitespace().next()?.parse::<u64>().ok())
                    .flatten()
            })
            .ok_or_else(|| invalid("MemAvailable unavailable"))?;
        let topology = CpuTopology::observe()?;
        let affinity = CpuAffinity::observe()?;
        Ok(Self {
            observed_utc: crate::journal::utc_now()?,
            hostname: command_text("hostname", &[])?,
            cpu_model: value("model name")?,
            cpu_flags: flags,
            os_kernel: command_text("uname", &["-sr"])?,
            governors,
            smt_active: topology.smt_active,
            affinity,
            topology,
            load_average,
            available_memory_kib,
        })
    }
}
