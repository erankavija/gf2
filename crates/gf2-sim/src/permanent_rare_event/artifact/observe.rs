//! Runtime observation collected from the operating system.
//!
//! Every value in this module is read from the kernel at the moment the
//! observation is taken and is stamped with the UTC instant of that read. No
//! constructor accepts a caller-authored process identity, occupant, or
//! observation time, so a liveness proof cannot be asserted from outside.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{
    canonical_bytes, evidence_from_value, normalized_host_evidence, ArtifactError,
    HostObservationV1, ObservationEvidenceV1,
};

/// Collector source for one OS-observed process-liveness record.
pub const PROCESS_LIVENESS_SOURCE_V1: &str = "gf2.process-liveness-observation-json/v1";
/// Collector source for one OS-observed self-identity record.
pub const PROCESS_IDENTITY_SOURCE_V1: &str = "gf2.process-identity-observation-json/v1";

/// Kernel path holding the boot identity shared by every process on this host.
const BOOT_ID_PATH: &str = "/proc/sys/kernel/random/boot_id";

/// Formats the current instant as the canonical artifact UTC timestamp.
///
/// # Errors
///
/// Refuses a system clock that precedes the Unix epoch.
pub fn observation_utc_now() -> Result<String, ArtifactError> {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ArtifactError::Identity("system clock precedes the Unix epoch".into()))?;
    Ok(format_utc(
        since_epoch.as_secs(),
        since_epoch.subsec_nanos(),
    ))
}

/// Renders Unix seconds and nanoseconds as `YYYY-MM-DDTHH:MM:SS.nnnnnnnnnZ`.
fn format_utc(seconds: u64, nanoseconds: u32) -> String {
    let days = (seconds / 86_400) as i64;
    let time_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (
        time_of_day / 3_600,
        (time_of_day % 3_600) / 60,
        time_of_day % 60,
    );
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanoseconds:09}Z")
}

/// Converts days since the Unix epoch to a proleptic Gregorian civil date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// One OS-observed process identity usable as a recorded child identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedProcessIdentityV1 {
    process_id: u32,
    process_start_token: String,
    boot_identity: String,
    evidence: ObservationEvidenceV1,
}

impl ObservedProcessIdentityV1 {
    /// Returns the observed PID.
    #[must_use]
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    /// Returns the kernel-reported process-start token.
    #[must_use]
    pub fn process_start_token(&self) -> &str {
        &self.process_start_token
    }

    /// Returns the kernel-reported boot identity.
    #[must_use]
    pub fn boot_identity(&self) -> &str {
        &self.boot_identity
    }

    /// Returns the immutable collector evidence for this identity.
    #[must_use]
    pub fn evidence(&self) -> &ObservationEvidenceV1 {
        &self.evidence
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ProcessIdentityEvidenceV1 {
    observed_at_utc: String,
    process_id: u32,
    process_start_token: String,
    boot_identity: String,
}

/// Closed occupant of a recorded PID at the observation instant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "occupant", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProcessOccupantV1 {
    /// The kernel holds no process under the recorded PID.
    Absent {},
    /// The kernel holds a process under the recorded PID.
    Present {
        /// That process's start token.
        process_start_token: String,
        /// The host boot identity read in the same observation.
        boot_identity: String,
    },
}

/// One OS-observed, timestamped liveness reading of a recorded PID.
///
/// The only constructor is [`observe_process_liveness`], which reads the
/// kernel at the recovery barrier. Fields are private, so no caller can assert
/// an occupant, a PID, or an observation time it did not observe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessLivenessObservationV1 {
    observed_at_utc: String,
    recorded_process_id: u32,
    occupant: ProcessOccupantV1,
    evidence: ObservationEvidenceV1,
}

impl ProcessLivenessObservationV1 {
    /// Returns the UTC instant at which the kernel was read.
    #[must_use]
    pub fn observed_at_utc(&self) -> &str {
        &self.observed_at_utc
    }

    /// Returns the PID this observation addressed.
    #[must_use]
    pub fn recorded_process_id(&self) -> u32 {
        self.recorded_process_id
    }

    /// Returns the observed occupant of that PID.
    #[must_use]
    pub fn occupant(&self) -> &ProcessOccupantV1 {
        &self.occupant
    }

    /// Returns the immutable collector evidence for this observation.
    #[must_use]
    pub fn evidence(&self) -> &ObservationEvidenceV1 {
        &self.evidence
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcessLivenessEvidenceV1 {
    pub(super) observed_at_utc: String,
    pub(super) recorded_process_id: u32,
    pub(super) occupant: ProcessOccupantV1,
}

/// Observes this process's own kernel-reported identity.
///
/// # Errors
///
/// Refuses when the kernel process interface is unavailable or malformed.
pub fn observe_self_identity() -> Result<ObservedProcessIdentityV1, ArtifactError> {
    observe_process_identity(std::process::id())?
        .ok_or_else(|| ArtifactError::Liveness("this process has no kernel process record".into()))
}

/// Observes the kernel-reported identity of one live process.
///
/// A launcher calls this immediately after spawning a child so the recorded
/// identity in its start receipt is what the kernel reported, not what the
/// launcher assumed. Absence is reported as `None`.
///
/// # Errors
///
/// Refuses when the kernel process interface is unavailable or malformed.
pub fn observe_process_identity(
    process_id: u32,
) -> Result<Option<ObservedProcessIdentityV1>, ArtifactError> {
    let observed_at_utc = observation_utc_now()?;
    let boot_identity = read_boot_identity()?;
    let Some(process_start_token) = read_process_start_token(process_id)? else {
        return Ok(None);
    };
    let record = ProcessIdentityEvidenceV1 {
        observed_at_utc,
        process_id,
        process_start_token: process_start_token.clone(),
        boot_identity: boot_identity.clone(),
    };
    let evidence = evidence_from_value(PROCESS_IDENTITY_SOURCE_V1, &record)?;
    Ok(Some(ObservedProcessIdentityV1 {
        process_id,
        process_start_token,
        boot_identity,
        evidence,
    }))
}

/// Observes whether the kernel currently holds a process under `recorded_process_id`.
///
/// The boot identity and, when the PID is occupied, that occupant's start
/// token are read in the same observation, so a PID reused by an unrelated
/// process is distinguishable from the recorded child still running.
///
/// # Errors
///
/// Refuses when the kernel process interface is unavailable or malformed.
pub fn observe_process_liveness(
    recorded_process_id: u32,
) -> Result<ProcessLivenessObservationV1, ArtifactError> {
    let observed_at_utc = observation_utc_now()?;
    let boot_identity = read_boot_identity()?;
    let occupant = match read_process_start_token(recorded_process_id)? {
        None => ProcessOccupantV1::Absent {},
        Some(process_start_token) => ProcessOccupantV1::Present {
            process_start_token,
            boot_identity,
        },
    };
    let record = ProcessLivenessEvidenceV1 {
        observed_at_utc: observed_at_utc.clone(),
        recorded_process_id,
        occupant: occupant.clone(),
    };
    let evidence = evidence_from_value(PROCESS_LIVENESS_SOURCE_V1, &record)?;
    Ok(ProcessLivenessObservationV1 {
        observed_at_utc,
        recorded_process_id,
        occupant,
        evidence,
    })
}

/// Reads the host boot identity shared by every process alive on this kernel.
fn read_boot_identity() -> Result<String, ArtifactError> {
    let raw = fs::read_to_string(BOOT_ID_PATH).map_err(|error| {
        ArtifactError::Liveness(format!("boot identity is unobservable: {error}"))
    })?;
    let boot_identity = raw.trim().to_owned();
    if boot_identity.is_empty() || !boot_identity.is_ascii() {
        return Err(ArtifactError::Liveness(
            "kernel boot identity is empty or non-ASCII".into(),
        ));
    }
    Ok(boot_identity)
}

/// Reads the kernel start token of one PID, reporting absence as `None`.
///
/// The token is the scheduler start time in clock ticks, which the kernel
/// never reissues to a later occupant of the same PID within one boot.
fn read_process_start_token(process_id: u32) -> Result<Option<String>, ArtifactError> {
    let stat = match fs::read_to_string(format!("/proc/{process_id}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.raw_os_error() == Some(3) => return Ok(None),
        Err(error) => {
            return Err(ArtifactError::Liveness(format!(
                "process {process_id} is unobservable: {error}"
            )))
        }
    };
    let after_comm = stat
        .rfind(") ")
        .map(|index| &stat[index + 2..])
        .ok_or_else(|| {
            ArtifactError::Liveness("kernel process record has no command terminator".into())
        })?;
    let start_time = after_comm.split_whitespace().nth(19).ok_or_else(|| {
        ArtifactError::Liveness("kernel process record has no start-time field".into())
    })?;
    if start_time.is_empty() || !start_time.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ArtifactError::Liveness(
            "kernel process start time is not a decimal token".into(),
        ));
    }
    Ok(Some(start_time.to_owned()))
}

/// Observes the runtime host facts recorded in every attempt start receipt.
///
/// # Errors
///
/// Refuses when a required kernel interface is unavailable or malformed.
pub fn observe_host() -> Result<HostObservationV1, ArtifactError> {
    let observation_utc = observation_utc_now()?;
    let cpuinfo = read_kernel_text("/proc/cpuinfo")?;
    let meminfo = read_kernel_text("/proc/meminfo")?;
    let online = read_kernel_text("/sys/devices/system/cpu/online")?;
    let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();

    let online_cpus = parse_cpu_list(online.trim())?;
    let logical_cpus = online_cpus.len() as u32;
    let physical_ids = cpuinfo_values(&cpuinfo, "physical id");
    let sockets = distinct_count(&physical_ids).max(1);
    let core_ids: Vec<_> = cpuinfo_values(&cpuinfo, "physical id")
        .into_iter()
        .zip(cpuinfo_values(&cpuinfo, "core id"))
        .map(|(socket, core)| format!("{socket}:{core}"))
        .collect();
    let physical_cores = distinct_count(&core_ids).max(1);
    let numa_nodes = count_numa_nodes().max(1);

    let mut host = HostObservationV1 {
        observation_utc,
        cpu_architecture: std::env::consts::ARCH.to_owned(),
        cpu_vendor: cpuinfo_first(&cpuinfo, "vendor_id").unwrap_or_else(|| "unknown".into()),
        cpu_model: cpuinfo_first(&cpuinfo, "model name").unwrap_or_else(|| "unknown".into()),
        sockets,
        numa_nodes,
        physical_cores,
        logical_cpus,
        online_cpus,
        total_ram_bytes: meminfo_kib(&meminfo, "MemTotal")? * 1_024,
        available_ram_bytes: meminfo_kib(&meminfo, "MemAvailable")? * 1_024,
        os_name: os_release_field(&os_release, "NAME").unwrap_or_else(|| "unknown".into()),
        os_version: os_release_field(&os_release, "VERSION_ID").unwrap_or_else(|| "unknown".into()),
        kernel_release: read_kernel_text("/proc/sys/kernel/osrelease")?
            .trim()
            .to_owned(),
        kernel_version: read_kernel_text("/proc/sys/kernel/version")?
            .trim()
            .to_owned(),
        evidence: Vec::new(),
    };
    host.available_ram_bytes = host.available_ram_bytes.min(host.total_ram_bytes);
    host.evidence.push(normalized_host_evidence(&host)?);
    host.evidence.push(raw_evidence(
        "gf2.host-observation-proc-cpuinfo/v1",
        cpuinfo.as_bytes(),
    )?);
    host.evidence.push(raw_evidence(
        "gf2.host-observation-proc-meminfo/v1",
        meminfo.as_bytes(),
    )?);
    Ok(host)
}

/// Encodes exact collector bytes as immutable evidence.
fn raw_evidence(source: &str, bytes: &[u8]) -> Result<ObservationEvidenceV1, ArtifactError> {
    Ok(ObservationEvidenceV1 {
        source: source.to_owned(),
        evidence_hex: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        evidence_sha256: super::sha256_hex(bytes),
    })
}

/// Reads one required kernel text interface.
fn read_kernel_text(path: &str) -> Result<String, ArtifactError> {
    fs::read_to_string(Path::new(path)).map_err(|error| {
        ArtifactError::Identity(format!("host interface {path} is unobservable: {error}"))
    })
}

/// Returns every value of one `/proc/cpuinfo` key in file order.
fn cpuinfo_values(cpuinfo: &str, key: &str) -> Vec<String> {
    cpuinfo
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.trim() == key)
        .map(|(_, value)| value.trim().to_owned())
        .collect()
}

/// Returns the first value of one `/proc/cpuinfo` key.
fn cpuinfo_first(cpuinfo: &str, key: &str) -> Option<String> {
    cpuinfo_values(cpuinfo, key).into_iter().next()
}

/// Counts distinct values, treating an empty observation as zero.
fn distinct_count(values: &[String]) -> u32 {
    let mut distinct: Vec<_> = values.iter().collect();
    distinct.sort();
    distinct.dedup();
    distinct.len() as u32
}

/// Counts NUMA nodes exposed by the kernel.
fn count_numa_nodes() -> u32 {
    let Ok(entries) = fs::read_dir("/sys/devices/system/node") else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("node") && name[4..].parse::<u32>().is_ok())
        })
        .count() as u32
}

/// Parses a kernel CPU list such as `0-3,8` into strictly sorted identifiers.
fn parse_cpu_list(list: &str) -> Result<Vec<u32>, ArtifactError> {
    let mut cpus = Vec::new();
    for range in list.split(',').filter(|range| !range.is_empty()) {
        let (start, end) = match range.split_once('-') {
            Some((start, end)) => (start, end),
            None => (range, range),
        };
        let parse = |value: &str| {
            value.trim().parse::<u32>().map_err(|_| {
                ArtifactError::Identity("kernel CPU list is not a decimal range".into())
            })
        };
        let (start, end) = (parse(start)?, parse(end)?);
        if end < start {
            return Err(ArtifactError::Identity(
                "kernel CPU list has a descending range".into(),
            ));
        }
        cpus.extend(start..=end);
    }
    cpus.sort_unstable();
    cpus.dedup();
    if cpus.is_empty() {
        return Err(ArtifactError::Identity(
            "kernel reports no online logical CPU".into(),
        ));
    }
    Ok(cpus)
}

/// Reads one `/proc/meminfo` kibibyte quantity.
fn meminfo_kib(meminfo: &str, key: &str) -> Result<u64, ArtifactError> {
    meminfo
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim() == key)
        .and_then(|(_, value)| value.split_whitespace().next()?.parse::<u64>().ok())
        .ok_or_else(|| ArtifactError::Identity(format!("kernel does not report {key}")))
}

/// Reads one unquoted `/etc/os-release` field.
fn os_release_field(os_release: &str, key: &str) -> Option<String> {
    os_release
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
}

/// Builds one liveness evidence record for stored-artifact conformance fixtures.
///
/// This produces only the immutable evidence a published terminal carries. It
/// cannot produce a [`ProcessLivenessObservationV1`], so it is not a liveness
/// proof and cannot reach [`super::recover_interrupted_attempt`], which always
/// reads the operating system itself.
///
/// # Errors
///
/// Refuses a record that fails canonical serialization.
#[cfg(feature = "test-support")]
pub fn process_liveness_evidence_fixture(
    observed_at_utc: &str,
    recorded_process_id: u32,
    occupant: ProcessOccupantV1,
) -> Result<ObservationEvidenceV1, ArtifactError> {
    evidence_from_value(
        PROCESS_LIVENESS_SOURCE_V1,
        &ProcessLivenessEvidenceV1 {
            observed_at_utc: observed_at_utc.to_owned(),
            recorded_process_id,
            occupant,
        },
    )
}

/// Serializes canonical liveness evidence for schema-level revalidation.
pub(super) fn liveness_evidence_record(
    observation: &ProcessLivenessObservationV1,
) -> Result<Vec<u8>, ArtifactError> {
    canonical_bytes(&ProcessLivenessEvidenceV1 {
        observed_at_utc: observation.observed_at_utc.clone(),
        recorded_process_id: observation.recorded_process_id,
        occupant: observation.occupant.clone(),
    })
}
