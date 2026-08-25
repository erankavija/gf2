#![deny(unsafe_code)]

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec};
use gf2_core::tuning::{
    AssemblyProvenance, CompiledProfileProvenance, CoreTuning, CoreTuningCodec, GitRevision,
    PreparedEnvelope, ProfileId, ProfileRegistry, ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc,
    Sha256, TuningSection,
};
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256 as Sha256Hasher};

const TOOL_PATH: &str = "dev/tools/tuning-profile-compose";

fn main() {
    if let Err(error) = run() {
        eprintln!("tuning-profile-compose: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [core_path, algebra_path, output_path, profile_id, assembled_at, source_revision, source_dirty, tool_sha256] =
        arguments.as_slice()
    else {
        return Err("usage: tuning-profile-compose CORE_OWNER ALGEBRA_OWNER OUTPUT PROFILE_ID ASSEMBLED_AT SOURCE_REVISION SOURCE_DIRTY TOOL_SHA256".into());
    };

    let source_dirty = match source_dirty.as_str() {
        "true" => true,
        "false" => false,
        _ => return Err("SOURCE_DIRTY must be true or false".into()),
    };
    let assembly = AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse(assembled_at)?,
        source_revision: GitRevision::parse(source_revision)?,
        source_dirty,
        tool: RepoRelPath::parse(TOOL_PATH)?,
        tool_sha256: Sha256::parse(tool_sha256)?,
    };
    compose(
        Path::new(core_path),
        Path::new(algebra_path),
        Path::new(output_path),
        ProfileId::parse(profile_id)?,
        &assembly,
    )
}

fn compose(
    core_path: &Path,
    algebra_path: &Path,
    output_path: &Path,
    profile_id: ProfileId,
    assembly: &AssemblyProvenance,
) -> Result<(), Box<dyn Error>> {
    let core_text = fs::read_to_string(core_path)?;
    let algebra_text = fs::read_to_string(algebra_path)?;
    let complete_text = compose_texts(&core_text, &algebra_text, profile_id, assembly)?;
    write_atomic(output_path, complete_text.as_bytes())?;
    Ok(())
}

fn compose_texts(
    core_text: &str,
    algebra_text: &str,
    profile_id: ProfileId,
    assembly: &AssemblyProvenance,
) -> Result<String, Box<dyn Error>> {
    let core_registry = owner_core_registry()?;
    let algebra_registry = owner_algebra_registry()?;
    let both_registry = complete_registry()?;

    let core = core_registry.from_json(core_text)?;
    let algebra = algebra_registry.from_json(algebra_text)?;
    require_ids(&core, &[CoreTuning::ID.as_str()])?;
    require_ids(&algebra, &[AlgebraTuning::ID.as_str()])?;

    let core_projection = core
        .section::<CoreTuning>()?
        .ok_or("core owner envelope is missing its typed section")?;
    let algebra_projection = algebra
        .section::<AlgebraTuning>()?
        .ok_or("algebra owner envelope is missing its typed section")?;
    let provenance = CompiledProfileProvenance {
        artifact_id: profile_id.clone(),
    };
    let complete = PreparedEnvelope::compiled(profile_id, provenance)
        .insert_measured::<CoreTuning, CoreTuningCodec>(
            core_projection.section.clone(),
            core_projection.measurement.clone(),
        )?
        .insert_measured::<AlgebraTuning, AlgebraTuningCodec>(
            algebra_projection.section.clone(),
            algebra_projection.measurement.clone(),
        )?
        .build()?;
    let complete_text = both_registry.to_json(&complete, assembly)?;
    let reopened = both_registry.from_json(&complete_text)?;
    require_ids(
        &reopened,
        &[AlgebraTuning::ID.as_str(), CoreTuning::ID.as_str()],
    )?;

    let canonical_core = core_registry.to_json(
        &core,
        &core
            .verified_assembly()
            .ok_or("core owner lacks verified assembly")?
            .provenance,
    )?;
    let canonical_algebra = algebra_registry.to_json(
        &algebra,
        &algebra
            .verified_assembly()
            .ok_or("algebra owner lacks verified assembly")?
            .provenance,
    )?;
    verify_wrapper_identity(&canonical_core, &complete_text, CoreTuning::ID.as_str())?;
    verify_wrapper_identity(
        &canonical_algebra,
        &complete_text,
        AlgebraTuning::ID.as_str(),
    )?;

    Ok(complete_text)
}

fn owner_core_registry() -> Result<ProfileRegistry, Box<dyn Error>> {
    Ok(ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()?
        .build()?)
}

fn owner_algebra_registry() -> Result<ProfileRegistry, Box<dyn Error>> {
    Ok(ProfileRegistryBuilder::new()
        .register::<AlgebraTuning, AlgebraTuningCodec>()?
        .build()?)
}

fn complete_registry() -> Result<ProfileRegistry, Box<dyn Error>> {
    Ok(ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()?
        .register::<AlgebraTuning, AlgebraTuningCodec>()?
        .build()?)
}

fn require_ids(envelope: &PreparedEnvelope, expected: &[&str]) -> Result<(), Box<dyn Error>> {
    let found: Vec<&str> = envelope.section_ids().collect();
    if found != expected {
        return Err(format!("section IDs {found:?} do not match {expected:?}").into());
    }
    Ok(())
}

#[derive(Deserialize)]
struct RawEnvelope<'a> {
    #[serde(borrow)]
    sections: BTreeMap<String, &'a RawValue>,
}

fn verify_wrapper_identity(
    owner: &str,
    complete: &str,
    section_id: &str,
) -> Result<(), Box<dyn Error>> {
    let owner: RawEnvelope<'_> = serde_json::from_str(owner)?;
    let complete: RawEnvelope<'_> = serde_json::from_str(complete)?;
    let owner_wrapper = owner
        .sections
        .get(section_id)
        .ok_or("owner canonical output lacks expected section")?
        .get();
    let complete_wrapper = complete
        .sections
        .get(section_id)
        .ok_or("complete canonical output lacks expected section")?
        .get();
    let owner_digest = Sha256Hasher::digest(owner_wrapper.as_bytes());
    let complete_digest = Sha256Hasher::digest(complete_wrapper.as_bytes());
    if owner_wrapper != complete_wrapper || owner_digest != complete_digest {
        return Err(format!("canonical wrapper identity changed for {section_id}").into());
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("output path has no UTF-8 file name")?;
    let temporary_name = format!(".{file_name}.{}.tmp", std::process::id());
    let temporary_path: PathBuf = path.with_file_name(temporary_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ProfileId {
        ProfileId::parse(value).unwrap()
    }

    fn compiled(value: &str) -> CompiledProfileProvenance {
        CompiledProfileProvenance {
            artifact_id: id(value),
        }
    }

    fn assembly() -> AssemblyProvenance {
        AssemblyProvenance {
            assembled_at: Rfc3339Utc::parse("2026-08-25T19:00:00Z").unwrap(),
            source_revision: GitRevision::parse("0123456789abcdef0123456789abcdef01234567")
                .unwrap(),
            source_dirty: false,
            tool: RepoRelPath::parse(TOOL_PATH).unwrap(),
            tool_sha256: Sha256::parse(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap(),
        }
    }

    #[test]
    fn independent_owner_ids_compose_under_the_callers_explicit_id_without_freezing() {
        let core = PreparedEnvelope::compiled(id("core-campaign"), compiled("core-campaign"))
            .insert(CoreTuning::CONSERVATIVE)
            .unwrap()
            .build()
            .unwrap();
        let algebra =
            PreparedEnvelope::compiled(id("algebra-campaign"), compiled("algebra-campaign"))
                .insert(AlgebraTuning::CONSERVATIVE)
                .unwrap()
                .build()
                .unwrap();
        let core_text = owner_core_registry()
            .unwrap()
            .to_json(&core, &assembly())
            .unwrap();
        let algebra_text = owner_algebra_registry()
            .unwrap()
            .to_json(&algebra, &assembly())
            .unwrap();

        let complete_text = compose_texts(
            &core_text,
            &algebra_text,
            id("complete-campaign"),
            &assembly(),
        )
        .unwrap();
        let complete = complete_registry()
            .unwrap()
            .from_json(&complete_text)
            .unwrap();
        assert_eq!(complete.profile_id().as_str(), "complete-campaign");
        require_ids(
            &complete,
            &[AlgebraTuning::ID.as_str(), CoreTuning::ID.as_str()],
        )
        .unwrap();

        assert_eq!(gf2_core::tuning::install(complete), Ok(()));
    }
}
