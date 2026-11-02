use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::identity::{GitRevision, Sha256Digest};
use crate::repository::RepositoryRoot;

/// Deterministic random-number generator algorithm of a run.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RngAlgorithm {
    ChaCha20,
}

/// Random-number generator implementation of a run.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RngIdentity {
    pub algorithm: RngAlgorithm,
    /// Implementation and version, for example `rand_chacha 0.9.0`.
    pub version: String,
}

/// Effective build flags of the executable, stamped when this crate compiled.
/// The build of this crate fails on a package override of the profile, so the
/// flags describe every crate the executable links. A `--config` override on the
/// Cargo command line stays invisible to the stamp and shows only in the
/// executable digest.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildFlags {
    /// Cargo profile name, custom profiles included; `debug` names `dev`.
    pub profile: String,
    pub opt_level: String,
    /// Link-time optimization value as Cargo configuration spells it; `false`
    /// when the profile leaves it unset.
    pub lto: String,
    /// Absent when the profile leaves the choice to the compiler.
    pub codegen_units: Option<u32>,
    pub target: String,
    /// Compiler flags from `RUSTFLAGS` and Cargo configuration, in order.
    pub rustflags: Vec<String>,
}

/// Processor identity and topology of the host.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuIdentity {
    /// Model string reported by the operating system.
    pub model: String,
    pub physical_cores: u32,
    pub logical_threads: u32,
}

/// Accelerator identity of the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceleratorIdentity {
    /// Installed ROCm runtime version, when present.
    pub runtime: Option<String>,
    /// AMD accelerator product name, when present.
    pub model: Option<String>,
}

/// Host facts observed at run time.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostFacts {
    pub cpu: CpuIdentity,
    pub accelerator: AcceleratorIdentity,
}

/// The closed identity set of `@/inv/behavioral-evidence-validity`. Equality of
/// two values is equality of that set; only a change to it invalidates evidence.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunIdentity {
    /// Digest of the running executable.
    pub executable_sha256: Sha256Digest,
    pub build: BuildFlags,
    /// Digest of the frozen configuration.
    pub configuration_sha256: Sha256Digest,
    /// Root seed.
    pub seed: u64,
    pub rng: RngIdentity,
    /// Version of the compiler that built this crate.
    pub toolchain_version: String,
    pub host: HostFacts,
}

/// What a run records about itself: its identity, and the source revision as a
/// pointer for rebuilding that takes no part in the identity.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub identity: RunIdentity,
    /// Revision to rebuild from.
    pub source_revision: GitRevision,
}

impl RunRecord {
    /// Observes the running executable and host and combines them with the
    /// caller's inputs.
    ///
    /// # Errors
    ///
    /// Fails when the executable cannot be digested, the host exposes no valid
    /// processor identity, or the revision at `root` cannot be read.
    pub fn observe(
        root: &RepositoryRoot,
        configuration_sha256: Sha256Digest,
        seed: u64,
        rng: RngIdentity,
    ) -> io::Result<Self> {
        Ok(Self {
            identity: RunIdentity {
                executable_sha256: Sha256Digest::of_file(Path::new("/proc/self/exe"))?,
                build: BuildFlags::stamped(),
                configuration_sha256,
                seed,
                rng,
                toolchain_version: env!("EXPERIMENTS_BUILD_TOOLCHAIN").to_owned(),
                host: HostFacts {
                    cpu: observe_cpu()?,
                    accelerator: observe_accelerator(),
                },
            },
            source_revision: root.revision()?,
        })
    }
}

impl BuildFlags {
    fn stamped() -> Self {
        Self {
            profile: env!("EXPERIMENTS_BUILD_PROFILE").to_owned(),
            opt_level: env!("EXPERIMENTS_BUILD_OPT_LEVEL").to_owned(),
            lto: env!("EXPERIMENTS_BUILD_LTO").to_owned(),
            codegen_units: env!("EXPERIMENTS_BUILD_CODEGEN_UNITS").parse().ok(),
            target: env!("EXPERIMENTS_BUILD_TARGET").to_owned(),
            rustflags: env!("EXPERIMENTS_BUILD_RUSTFLAGS")
                .split('\x1f')
                .filter(|flag| !flag.is_empty())
                .map(str::to_owned)
                .collect(),
        }
    }
}

fn observe_accelerator() -> AcceleratorIdentity {
    let runtime = fs::read_to_string("/opt/rocm/.info/version")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let model = fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("card"))
        .find_map(|entry| {
            let device = entry.path().join("device");
            let vendor = fs::read_to_string(device.join("vendor")).ok()?;
            if vendor.trim() != AMD_PCI_VENDOR {
                return None;
            }
            fs::read_to_string(device.join("product_name"))
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        });
    AcceleratorIdentity { runtime, model }
}

const AMD_PCI_VENDOR: &str = "0x1002";

fn observe_cpu() -> io::Result<CpuIdentity> {
    parse_proc_cpuinfo(&fs::read_to_string("/proc/cpuinfo")?)
}

/// Counts distinct `(physical id, core id)` pairs, falling back to sysfs
/// topology where `/proc/cpuinfo` omits them, as it does on some architectures.
fn parse_proc_cpuinfo(cpuinfo: &str) -> io::Result<CpuIdentity> {
    let fields = || cpuinfo.lines().filter_map(cpuinfo_field);
    let model = fields()
        .find(|(name, _)| matches!(*name, "model name" | "Processor" | "Hardware"))
        .map(|(_, value)| value.to_owned())
        .unwrap_or_default();
    let logical_threads = fields().filter(|(name, _)| *name == "processor").count();
    let core_ids: BTreeSet<_> = cpuinfo
        .split("\n\n")
        .filter_map(|processor| {
            let (mut package, mut core) = (None, None);
            for (name, value) in processor.lines().filter_map(cpuinfo_field) {
                match name {
                    "physical id" => package = Some(value.to_owned()),
                    "core id" => core = Some(value.to_owned()),
                    _ => {}
                }
            }
            package.zip(core)
        })
        .collect();
    let physical_cores = if core_ids.is_empty() {
        sysfs_core_ids()?.len()
    } else {
        core_ids.len()
    };
    if model.is_empty() || physical_cores == 0 || logical_threads < physical_cores {
        return Err(invalid_cpu_data("inconsistent processor model or topology"));
    }
    Ok(CpuIdentity {
        model,
        physical_cores: u32::try_from(physical_cores)
            .map_err(|_| invalid_cpu_data("physical core count exceeds u32"))?,
        logical_threads: u32::try_from(logical_threads)
            .map_err(|_| invalid_cpu_data("logical thread count exceeds u32"))?,
    })
}

fn sysfs_core_ids() -> io::Result<BTreeSet<(String, String)>> {
    let mut core_ids = BTreeSet::new();
    for entry in fs::read_dir("/sys/devices/system/cpu")? {
        let entry = entry?;
        let name = entry.file_name();
        let is_cpu = name
            .to_string_lossy()
            .strip_prefix("cpu")
            .is_some_and(|index| {
                !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
            });
        if is_cpu {
            let topology = entry.path().join("topology");
            core_ids.insert((
                fs::read_to_string(topology.join("physical_package_id"))?
                    .trim()
                    .to_owned(),
                fs::read_to_string(topology.join("core_id"))?
                    .trim()
                    .to_owned(),
            ));
        }
    }
    Ok(core_ids)
}

fn cpuinfo_field(line: &str) -> Option<(&str, &str)> {
    line.split_once(':')
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn invalid_cpu_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn processor(index: u32, core: u32) -> String {
        format!(
            "processor\t: {index}\nmodel name\t: Test CPU\nphysical id\t: 0\ncore id\t\t: {core}\n"
        )
    }

    #[test]
    fn simultaneous_threads_count_once_as_physical_cores() {
        let cpuinfo = [
            processor(0, 0),
            processor(1, 1),
            processor(2, 0),
            processor(3, 1),
        ]
        .join("\n");
        let cpu = parse_proc_cpuinfo(&cpuinfo).unwrap();
        assert_eq!((cpu.physical_cores, cpu.logical_threads), (2, 4));
    }

    #[test]
    fn missing_model_is_invalid() {
        let cpuinfo = processor(0, 0).replace("model name", "unknown");
        assert_eq!(
            parse_proc_cpuinfo(&cpuinfo).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
