use std::io::Write;
use std::process::{Command, Stdio};

pub const SENTINEL: &str = "GF2_TUNING_FRESH_CASE";
const SENTINEL_VALUE: &str = "child-v1";
const RESULT_PREFIX: &str = "GF2_TUNING_RESULT=";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshProcessCase {
    FreezeBeforeInstall,
    InstallPresent,
    InstallMissing,
    ProjectionThenInstall,
    ActiveTypeMismatch,
}

impl FreshProcessCase {
    fn name(self) -> &'static str {
        match self {
            Self::FreezeBeforeInstall => "freeze-before-install",
            Self::InstallPresent => "install-present",
            Self::InstallMissing => "install-missing",
            Self::ProjectionThenInstall => "projection-then-install",
            Self::ActiveTypeMismatch => "active-type-mismatch",
        }
    }

    fn canonical_json(self) -> String {
        format!("{{\"case\":\"{}\"}}", self.name())
    }

    fn parse(value: &serde_json::Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("fresh-process case is not an object")?;
        if object.len() != 1 {
            return Err("fresh-process case has unexpected fields".to_owned());
        }
        match object.get("case").and_then(serde_json::Value::as_str) {
            Some("freeze-before-install") => Ok(Self::FreezeBeforeInstall),
            Some("install-present") => Ok(Self::InstallPresent),
            Some("install-missing") => Ok(Self::InstallMissing),
            Some("projection-then-install") => Ok(Self::ProjectionThenInstall),
            Some("active-type-mismatch") => Ok(Self::ActiveTypeMismatch),
            _ => Err("fresh-process case has an unknown case name".to_owned()),
        }
    }
}

pub fn fresh_tuning_process(case: FreshProcessCase) -> Result<serde_json::Value, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("test binary has no path: {error}"))?;
    let mut child = Command::new(&executable)
        .args(["--exact", "fresh_tuning_process_child", "--nocapture"])
        .env(SENTINEL, SENTINEL_VALUE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot spawn {}: {error}", executable.display()))?;
    child
        .stdin
        .take()
        .ok_or("fresh-process child has no stdin")?
        .write_all(case.canonical_json().as_bytes())
        .map_err(|error| format!("cannot write child case: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("cannot wait for child: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "fresh-process child exited with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "fresh-process child stdout is not UTF-8".to_owned())?;
    let results: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(RESULT_PREFIX))
        .collect();
    if results.len() != 1 {
        return Err(format!(
            "fresh-process child emitted {} structured results, expected one\n{stdout}",
            results.len()
        ));
    }
    serde_json::from_str(results[0])
        .map_err(|error| format!("fresh-process result is not JSON: {error}"))
}

pub fn child_case() -> Result<Option<FreshProcessCase>, String> {
    let sentinel = match std::env::var(SENTINEL) {
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(format!("cannot read fresh-process sentinel: {error}")),
        Ok(value) => value,
    };
    if sentinel != SENTINEL_VALUE {
        return Err(format!("invalid fresh-process sentinel {sentinel:?}"));
    }
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .map_err(|error| format!("cannot read fresh-process stdin: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&input)
        .map_err(|error| format!("fresh-process case is not JSON: {error}"))?;
    let case = FreshProcessCase::parse(&value)?;
    if input != case.canonical_json() {
        return Err("fresh-process case is not in canonical compact form".to_owned());
    }
    Ok(Some(case))
}

pub fn emit_result(value: serde_json::Value) {
    println!(
        "{RESULT_PREFIX}{}",
        serde_json::to_string(&value).expect("structured result is serializable")
    );
}

pub fn invalid_sentinel_is_rejected() -> Result<(), String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("test binary has no path: {error}"))?;
    let output = Command::new(&executable)
        .args(["--exact", "fresh_tuning_process_child", "--nocapture"])
        .env(SENTINEL, "not-child-v1")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot spawn {}: {error}", executable.display()))?;
    if output.status.success() {
        Err("child accepted an invalid sentinel".to_owned())
    } else {
        Ok(())
    }
}
