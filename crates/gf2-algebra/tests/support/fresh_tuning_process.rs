use std::io::Write;
use std::process::{Command, Stdio};

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{last_effective_chunk, permanent_chunk_len};
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal3_parallel};
use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec, PermanentSelectors};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{
    self, AssemblyProvenance, CompiledProfileProvenance, GitRevision, PreparedEnvelope, ProfileId,
    ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc, SectionResolution, Sha256,
};

const SENTINEL: &str = "GF2_TUNING_FRESH_CASE";
const SENTINEL_VALUE: &str = "child-v1";
const RESULT_PREFIX: &str = "GF2_TUNING_RESULT=";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshProcessCase {
    Chunk3,
    Chunk4000000,
}

impl FreshProcessCase {
    fn name(self) -> &'static str {
        match self {
            Self::Chunk3 => "chunk-3",
            Self::Chunk4000000 => "chunk-4000000",
        }
    }

    fn chunk(self) -> usize {
        match self {
            Self::Chunk3 => 3,
            Self::Chunk4000000 => 4_000_000,
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
            Some("chunk-3") => Ok(Self::Chunk3),
            Some("chunk-4000000") => Ok(Self::Chunk4000000),
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

fn prepared_algebra(chunk: usize) -> gf2_core::tuning::PreparedEnvelope {
    let id = ProfileId::parse("permanent-install-test").unwrap();
    let typed = PreparedEnvelope::compiled(
        id.clone(),
        CompiledProfileProvenance {
            artifact_id: id.clone(),
        },
    )
    .insert(AlgebraTuning::from_selectors(
        PermanentSelectors::try_new(chunk).unwrap(),
    ))
    .unwrap()
    .build()
    .unwrap();
    let registry = ProfileRegistryBuilder::new()
        .register::<AlgebraTuning, AlgebraTuningCodec>()
        .unwrap()
        .build()
        .unwrap();
    let assembly = AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("2026-08-25T19:00:00Z").unwrap(),
        source_revision: GitRevision::parse("0123456789abcdef0123456789abcdef01234567").unwrap(),
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-algebra/tests/support/fresh_tuning_process.rs")
            .unwrap(),
        tool_sha256: Sha256::parse(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap(),
    };
    let json = registry.to_json(&typed, &assembly).unwrap();
    registry.from_json(&json).unwrap()
}

fn test_matrix() -> Bipedal3Matrix {
    let n = 8;
    let data: Vec<Fp<3>> = (0..n * n)
        .map(|i| Fp::<3>::new((i as u64 * 7 + 1) % 3))
        .collect();
    Bipedal3Matrix::from_row_major(&data, n, n)
}

pub fn execute_child(case: FreshProcessCase) -> serde_json::Value {
    let chunk = case.chunk();
    tuning::install(prepared_algebra(chunk)).unwrap();
    let active = gf2_algebra::tuning::active();
    assert!(matches!(
        active.resolution,
        SectionResolution::Installed { .. }
    ));
    assert_eq!(permanent_chunk_len(), chunk);

    let matrix = test_matrix();
    let reference = permanent_bipedal3(&matrix);
    let result = permanent_bipedal3_parallel(&matrix);
    assert_eq!(last_effective_chunk(), chunk);
    assert_eq!(result, reference);
    serde_json::json!({
        "chunk": chunk,
        "resolution": "installed",
        "permanent": result.value(),
    })
}

pub fn emit_result(value: serde_json::Value) {
    println!(
        "{RESULT_PREFIX}{}",
        serde_json::to_string(&value).expect("structured result is serializable")
    );
}
