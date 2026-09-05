//! Canonical child-v2 request and result framing.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{self, Read, Write};

/// Environment variable guarding entry into a fresh calibration child.
pub const FRESH_CASE_VAR: &str = "GF2_TUNING_FRESH_CASE";
/// Reviewed fresh-child sentinel.
pub const FRESH_CASE_VALUE: &str = "child-v2";
/// Prefix of the one canonical result line emitted by a child.
pub const FRESH_RESULT_PREFIX: &str = "GF2_TUNING_RESULT=";

/// Serializes a request as compact canonical JSON.
///
/// Campaign wire types should use `#[serde(deny_unknown_fields)]`; decoding
/// below then rejects unknown fields as well as noncanonical spellings.
pub fn encode_case<T: Serialize>(case: &T) -> Result<String, String> {
    serde_json::to_string(case).map_err(|error| format!("cannot encode fresh tuning case: {error}"))
}

/// Decodes exactly one compact canonical JSON request.
pub fn decode_case<T>(input: &str) -> Result<T, String>
where
    T: DeserializeOwned + Serialize,
{
    decode_canonical(input, "fresh tuning stdin")
}

/// Reads and decodes a guarded fresh-child request.
pub fn read_guarded_case<T>(sentinel: Option<&str>, mut reader: impl Read) -> Result<T, String>
where
    T: DeserializeOwned + Serialize,
{
    match sentinel {
        Some(FRESH_CASE_VALUE) => {}
        Some(value) => return Err(format!("invalid fresh tuning sentinel {value:?}")),
        None => return Err("fresh tuning child sentinel is absent".to_owned()),
    }
    let mut input = String::new();
    reader
        .read_to_string(&mut input)
        .map_err(|error| format!("cannot read fresh tuning stdin: {error}"))?;
    decode_case(&input)
}

/// Encodes the one canonical result line expected on child stdout.
pub fn encode_result_line<T: Serialize>(report: &T) -> Result<String, String> {
    let payload = serde_json::to_string(report)
        .map_err(|error| format!("cannot encode fresh tuning result: {error}"))?;
    Ok(format!("{FRESH_RESULT_PREFIX}{payload}"))
}

/// Writes the one canonical result line and flushes it.
pub fn write_result_line<T: Serialize>(mut writer: impl Write, report: &T) -> io::Result<()> {
    let line = encode_result_line(report).map_err(io::Error::other)?;
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Parses exactly one canonical result line.
///
/// Missing, duplicate, unprefixed, non-UTF-8 (at the caller's byte boundary),
/// noncanonical, and schema-invalid results all fail closed.
pub fn parse_result<T>(text: &str) -> Result<T, String>
where
    T: DeserializeOwned + Serialize,
{
    let lines: Vec<_> = text.lines().collect();
    let [line] = lines.as_slice() else {
        return Err(format!(
            "the child emitted {} lines rather than one structured result",
            lines.len()
        ));
    };
    let payload = line
        .strip_prefix(FRESH_RESULT_PREFIX)
        .ok_or("the child result lacks the GF2_TUNING_RESULT= prefix")?;
    decode_canonical(payload, "child result")
}

/// Retains only protocol lines from libtest output without weakening the
/// exact-one-result parser.
pub fn retain_libtest_result_lines(stdout: &str) -> String {
    stdout
        .lines()
        .filter(|line| line.starts_with(FRESH_RESULT_PREFIX))
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_canonical<T>(input: &str, description: &str) -> Result<T, String>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_str(input)
        .map_err(|error| format!("{description} is not one valid value: {error}"))?;
    let canonical = serde_json::to_string(&value)
        .map_err(|error| format!("cannot re-encode {description}: {error}"))?;
    if input != canonical {
        return Err(format!("{description} is not canonical compact JSON"));
    }
    Ok(value)
}
