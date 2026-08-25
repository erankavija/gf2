use std::io::Write;
use std::process::{Command, Stdio};

use gf2_core::tuning::{
    CanonicalValue, CompiledProfileProvenance, CoreTuning, CoreTuningCodec, PreparedEnvelope,
    ProfileId, ProfileRegistryBuilder, SectionCodec,
};

const SENTINEL: &str = "GF2_TUNING_FRESH_CASE";
const SENTINEL_VALUE: &str = "child-v1";
const RESULT_PREFIX: &str = "GF2_TUNING_RESULT=";

#[allow(dead_code)]
pub fn prepared_core_json(body: &str) -> Result<PreparedEnvelope, String> {
    let canonical: CanonicalValue = serde_json::from_str(body)
        .map_err(|error| format!("core selector body is not JSON: {error}"))?;
    let section = CoreTuningCodec::decode_body(canonical)
        .map_err(|error| format!("core selector body is invalid: {error}"))?;
    Ok(prepared_core(section))
}

pub fn prepared_core(section: CoreTuning) -> PreparedEnvelope {
    let id = ProfileId::parse("core-route-test").unwrap();
    PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
        .insert(section)
        .unwrap()
        .build()
        .unwrap()
}

#[allow(dead_code)]
pub fn committed_core_owner(text: &str) -> Result<CoreTuning, String> {
    let registry = ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .map_err(|error| format!("cannot register the core tuning codec: {error}"))?
        .build()
        .map_err(|error| format!("cannot build the core tuning registry: {error}"))?;
    let prepared = registry
        .from_json(text)
        .map_err(|error| format!("committed core owner envelope is invalid: {error}"))?;
    let ids = prepared.section_ids().collect::<Vec<_>>();
    if ids != ["gf2-core/selectors"] {
        return Err(format!("unexpected core owner section IDs {ids:?}"));
    }
    prepared
        .section::<CoreTuning>()
        .map_err(|error| format!("cannot project the core tuning section: {error}"))?
        .map(|section| section.section.clone())
        .ok_or_else(|| "committed core owner section is absent".to_owned())
}

pub fn fresh_tuning_process(case: &str) -> Result<(), String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("test binary has no path: {error}"))?;
    let input = format!("{{\"case\":{}}}", serde_json::to_string(case).unwrap());
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
        .write_all(input.as_bytes())
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
    if results != [r#"{"status":"ok"}"#] {
        return Err(format!(
            "fresh-process child emitted invalid results {results:?}"
        ));
    }
    Ok(())
}

pub fn child_case() -> Result<Option<String>, String> {
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
    let object = value
        .as_object()
        .ok_or("fresh-process case is not an object")?;
    if object.len() != 1 {
        return Err("fresh-process case has unexpected fields".to_owned());
    }
    let case = object
        .get("case")
        .and_then(serde_json::Value::as_str)
        .ok_or("fresh-process case name is missing")?
        .to_owned();
    let canonical = format!("{{\"case\":{}}}", serde_json::to_string(&case).unwrap());
    if input != canonical {
        return Err("fresh-process case is not in canonical compact form".to_owned());
    }
    Ok(Some(case))
}

pub fn emit_ok() {
    println!("{RESULT_PREFIX}{{\"status\":\"ok\"}}");
}

macro_rules! fresh_tuning_test {
    ($name:ident, $body:block) => {
        #[test]
        fn $name() {
            crate::support::fresh_tuning_process(stringify!($name)).unwrap();
        }

        #[test]
        fn fresh_tuning_process_child() {
            let Some(case) =
                crate::support::child_case().expect("fresh-process protocol is valid")
            else {
                return;
            };
            assert_eq!(case, stringify!($name));
            $body
            crate::support::emit_ok();
        }
    };
}

pub(crate) use fresh_tuning_test;
