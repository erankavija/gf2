#![deny(unsafe_code)]

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec};
use gf2_core::tuning::{
    AssemblyProvenance, CompiledProfileProvenance, CoreTuning, CoreTuningCodec, GitRevision,
    MeasurementProvenance, PreparedEnvelope, ProfileId, ProfileRegistry, ProfileRegistryBuilder,
    RepoRelPath, Rfc3339Utc, Sha256, TuningSection,
};
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256 as Sha256Hasher};

const TOOL_PATH: &str = "dev/tools/tuning-profile-compose";
const USAGE: &str = "usage: tuning-profile-compose \
    (core-owner OUTPUT PROFILE_ID ASSEMBLED_AT SOURCE_REVISION SOURCE_DIRTY TOOL_SHA256 | \
     algebra-owner OUTPUT PROFILE_ID ASSEMBLED_AT SOURCE_REVISION SOURCE_DIRTY TOOL_SHA256 | \
     complete CORE_OWNER ALGEBRA_OWNER OUTPUT PROFILE_ID ASSEMBLED_AT SOURCE_REVISION SOURCE_DIRTY TOOL_SHA256)";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    CoreOwner {
        output_path: PathBuf,
        profile_id: ProfileId,
        assembly: AssemblyProvenance,
    },
    AlgebraOwner {
        output_path: PathBuf,
        profile_id: ProfileId,
        assembly: AssemblyProvenance,
    },
    Complete {
        core_path: PathBuf,
        algebra_path: PathBuf,
        output_path: PathBuf,
        profile_id: ProfileId,
        assembly: AssemblyProvenance,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("tuning-profile-compose: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    execute(parse_command(&arguments)?)
}

fn parse_command(arguments: &[String]) -> Result<Command, Box<dyn Error>> {
    match arguments {
        [mode, output_path, profile_id, assembled_at, source_revision, source_dirty, tool_sha256]
            if mode == "core-owner" || mode == "algebra-owner" =>
        {
            let (profile_id, assembly) = parse_identity_and_assembly(
                profile_id,
                assembled_at,
                source_revision,
                source_dirty,
                tool_sha256,
            )?;
            if mode == "core-owner" {
                Ok(Command::CoreOwner {
                    output_path: PathBuf::from(output_path),
                    profile_id,
                    assembly,
                })
            } else {
                Ok(Command::AlgebraOwner {
                    output_path: PathBuf::from(output_path),
                    profile_id,
                    assembly,
                })
            }
        }
        [mode, core_path, algebra_path, output_path, profile_id, assembled_at, source_revision, source_dirty, tool_sha256]
            if mode == "complete" =>
        {
            let (profile_id, assembly) = parse_identity_and_assembly(
                profile_id,
                assembled_at,
                source_revision,
                source_dirty,
                tool_sha256,
            )?;
            Ok(Command::Complete {
                core_path: PathBuf::from(core_path),
                algebra_path: PathBuf::from(algebra_path),
                output_path: PathBuf::from(output_path),
                profile_id,
                assembly,
            })
        }
        _ => Err(USAGE.into()),
    }
}

fn parse_identity_and_assembly(
    profile_id: &str,
    assembled_at: &str,
    source_revision: &str,
    source_dirty: &str,
    tool_sha256: &str,
) -> Result<(ProfileId, AssemblyProvenance), Box<dyn Error>> {
    let source_dirty = match source_dirty {
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
    Ok((ProfileId::parse(profile_id)?, assembly))
}

fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::CoreOwner {
            output_path,
            profile_id,
            assembly,
        } => emit_core_owner(&output_path, profile_id, &assembly),
        Command::AlgebraOwner {
            output_path,
            profile_id,
            assembly,
        } => emit_algebra_owner(&output_path, profile_id, &assembly),
        Command::Complete {
            core_path,
            algebra_path,
            output_path,
            profile_id,
            assembly,
        } => compose(
            &core_path,
            &algebra_path,
            &output_path,
            profile_id,
            &assembly,
        ),
    }
}

fn emit_core_owner(
    output_path: &Path,
    profile_id: ProfileId,
    assembly: &AssemblyProvenance,
) -> Result<(), Box<dyn Error>> {
    let text = core_owner_text(profile_id.clone(), assembly)?;
    write_validated_atomic(output_path, text.as_bytes(), |written_path| {
        let written_text = fs::read_to_string(written_path)?;
        verify_core_owner(&written_text, &profile_id, assembly)
    })
}

fn emit_algebra_owner(
    output_path: &Path,
    profile_id: ProfileId,
    assembly: &AssemblyProvenance,
) -> Result<(), Box<dyn Error>> {
    let text = algebra_owner_text(profile_id.clone(), assembly)?;
    write_validated_atomic(output_path, text.as_bytes(), |written_path| {
        let written_text = fs::read_to_string(written_path)?;
        verify_algebra_owner(&written_text, &profile_id, assembly)
    })
}

fn core_owner_text(
    profile_id: ProfileId,
    assembly: &AssemblyProvenance,
) -> Result<String, Box<dyn Error>> {
    let provenance = CompiledProfileProvenance {
        artifact_id: profile_id.clone(),
    };
    let prepared = PreparedEnvelope::compiled(profile_id.clone(), provenance)
        .insert(CoreTuning::CONSERVATIVE)?
        .build()?;
    let text = owner_core_registry()?.to_json(&prepared, assembly)?;
    verify_core_owner(&text, &profile_id, assembly)?;
    Ok(text)
}

fn algebra_owner_text(
    profile_id: ProfileId,
    assembly: &AssemblyProvenance,
) -> Result<String, Box<dyn Error>> {
    let provenance = CompiledProfileProvenance {
        artifact_id: profile_id.clone(),
    };
    let prepared = PreparedEnvelope::compiled(profile_id.clone(), provenance)
        .insert(AlgebraTuning::CONSERVATIVE)?
        .build()?;
    let text = owner_algebra_registry()?.to_json(&prepared, assembly)?;
    verify_algebra_owner(&text, &profile_id, assembly)?;
    Ok(text)
}

fn verify_core_owner(
    text: &str,
    expected_profile_id: &ProfileId,
    expected_assembly: &AssemblyProvenance,
) -> Result<(), Box<dyn Error>> {
    let registry = owner_core_registry()?;
    let reopened = registry.from_json(text)?;
    verify_owner_identity(
        &registry,
        &reopened,
        text,
        expected_profile_id,
        expected_assembly,
        &[CoreTuning::ID.as_str()],
    )?;
    let projection = reopened
        .section::<CoreTuning>()?
        .ok_or("core owner envelope is missing its typed section")?;
    if projection.section != &CoreTuning::CONSERVATIVE
        || projection.measurement != &MeasurementProvenance::Inherited
    {
        return Err("core owner is not the complete inherited conservative section".into());
    }
    Ok(())
}

fn verify_algebra_owner(
    text: &str,
    expected_profile_id: &ProfileId,
    expected_assembly: &AssemblyProvenance,
) -> Result<(), Box<dyn Error>> {
    let registry = owner_algebra_registry()?;
    let reopened = registry.from_json(text)?;
    verify_owner_identity(
        &registry,
        &reopened,
        text,
        expected_profile_id,
        expected_assembly,
        &[AlgebraTuning::ID.as_str()],
    )?;
    let projection = reopened
        .section::<AlgebraTuning>()?
        .ok_or("algebra owner envelope is missing its typed section")?;
    if projection.section != &AlgebraTuning::CONSERVATIVE
        || projection.measurement != &MeasurementProvenance::Inherited
    {
        return Err("algebra owner is not the inherited conservative section".into());
    }
    Ok(())
}

fn verify_owner_identity(
    registry: &ProfileRegistry,
    reopened: &PreparedEnvelope,
    text: &str,
    expected_profile_id: &ProfileId,
    expected_assembly: &AssemblyProvenance,
    expected_ids: &[&str],
) -> Result<(), Box<dyn Error>> {
    require_ids(reopened, expected_ids)?;
    if reopened.profile_id() != expected_profile_id {
        return Err(format!(
            "owner profile ID {} does not match caller ID {}",
            reopened.profile_id().as_str(),
            expected_profile_id.as_str()
        )
        .into());
    }
    let verified = reopened
        .verified_assembly()
        .ok_or("owner envelope lacks verified assembly")?;
    if &verified.provenance != expected_assembly {
        return Err("owner assembly provenance changed during publication".into());
    }
    let canonical = registry.to_json(reopened, expected_assembly)?;
    if canonical != text {
        return Err("owner envelope or content digest is not canonical".into());
    }
    Ok(())
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
    let complete_text = compose_texts(&core_text, &algebra_text, profile_id.clone(), assembly)?;
    write_validated_atomic(output_path, complete_text.as_bytes(), |written_path| {
        let written_text = fs::read_to_string(written_path)?;
        verify_composition(
            &core_text,
            &algebra_text,
            &written_text,
            &profile_id,
            assembly,
        )
    })?;
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
    let complete = PreparedEnvelope::compiled(profile_id.clone(), provenance)
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
    verify_composition(
        core_text,
        algebra_text,
        &complete_text,
        &profile_id,
        assembly,
    )?;

    Ok(complete_text)
}

/// Strictly reopens one complete artifact and verifies its composition inputs.
fn verify_composition(
    core_text: &str,
    algebra_text: &str,
    complete_text: &str,
    expected_profile_id: &ProfileId,
    expected_assembly: &AssemblyProvenance,
) -> Result<(), Box<dyn Error>> {
    let core_registry = owner_core_registry()?;
    let algebra_registry = owner_algebra_registry()?;
    let both_registry = complete_registry()?;
    let core = core_registry.from_json(core_text)?;
    let algebra = algebra_registry.from_json(algebra_text)?;
    require_ids(&core, &[CoreTuning::ID.as_str()])?;
    require_ids(&algebra, &[AlgebraTuning::ID.as_str()])?;

    let reopened = both_registry.from_json(complete_text)?;
    require_ids(
        &reopened,
        &[AlgebraTuning::ID.as_str(), CoreTuning::ID.as_str()],
    )?;
    if reopened.profile_id() != expected_profile_id {
        return Err(format!(
            "complete profile ID {} does not match caller ID {}",
            reopened.profile_id().as_str(),
            expected_profile_id.as_str()
        )
        .into());
    }
    let reopened_assembly = &reopened
        .verified_assembly()
        .ok_or("complete envelope lacks verified assembly")?
        .provenance;
    if reopened_assembly != expected_assembly {
        return Err("complete assembly provenance changed during publication".into());
    }
    let canonical_complete = both_registry.to_json(&reopened, expected_assembly)?;
    if canonical_complete != complete_text {
        return Err("complete envelope or content digest is not canonical".into());
    }

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
    verify_wrapper_identity(&canonical_core, complete_text, CoreTuning::ID.as_str())?;
    verify_wrapper_identity(
        &canonical_algebra,
        complete_text,
        AlgebraTuning::ID.as_str(),
    )?;

    Ok(())
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

fn write_validated_atomic(
    path: &Path,
    bytes: &[u8],
    validate: impl Fn(&Path) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("output path has no UTF-8 file name")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let temporary_name = format!(".{file_name}.{}-{nonce}.tmp", std::process::id());
    let temporary_path: PathBuf = path.with_file_name(temporary_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        validate(&temporary_path)?;
        fs::hard_link(&temporary_path, path).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("refusing to replace existing output {}", path.display()),
                )
            } else {
                error
            }
        })?;
        if let Err(error) = validate(path) {
            fs::remove_file(path)?;
            return Err(error);
        }
        Ok(())
    })();
    if let Err(error) = fs::remove_file(&temporary_path) {
        eprintln!(
            "warning: could not remove temporary composition output {}: {error}",
            temporary_path.display()
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPOSER_SOURCE: &[u8] = include_bytes!("main.rs");
    const CORE_OWNER: &str =
        include_str!("../../../../crates/gf2-core/data/tuning-profiles/conservative.json");
    const ALGEBRA_OWNER: &str =
        include_str!("../../../../crates/gf2-algebra/data/tuning-profiles/conservative.json");
    const COMPLETE: &str =
        include_str!("../../../../dev/reference_data/tuning-profiles/conservative.json");

    struct TestOutput {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TestOutput {
        fn new(file_name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = env::temp_dir().join(format!(
                "gf2-tuning-compose-{}-{serial}",
                std::process::id()
            ));
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join(file_name);
            Self { directory, path }
        }
    }

    impl Drop for TestOutput {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

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

    fn command_arguments(mode: &str, paths: &[&str]) -> Vec<String> {
        let mut arguments = vec![mode.to_owned()];
        arguments.extend(paths.iter().map(|path| (*path).to_owned()));
        arguments.extend([
            "campaign".to_owned(),
            "2026-08-25T19:00:00Z".to_owned(),
            "0123456789abcdef0123456789abcdef01234567".to_owned(),
            "false".to_owned(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        ]);
        arguments
    }

    fn artifact_arguments(
        mode: &str,
        paths: &[&Path],
        profile_id: &ProfileId,
        assembly: &AssemblyProvenance,
    ) -> Vec<String> {
        let mut arguments = vec![mode.to_owned()];
        arguments.extend(paths.iter().map(|path| path.to_string_lossy().into_owned()));
        arguments.extend([
            profile_id.as_str().to_owned(),
            assembly.assembled_at.as_str().to_owned(),
            assembly.source_revision.as_str().to_owned(),
            assembly.source_dirty.to_string(),
            assembly.tool_sha256.as_str().to_owned(),
        ]);
        arguments
    }

    #[test]
    fn parser_routes_only_the_three_explicit_producer_modes() {
        let core = parse_command(&command_arguments("core-owner", &["core.json"])).unwrap();
        assert!(matches!(
            core,
            Command::CoreOwner {
                output_path,
                profile_id,
                ..
            } if output_path == Path::new("core.json") && profile_id.as_str() == "campaign"
        ));

        let algebra =
            parse_command(&command_arguments("algebra-owner", &["algebra.json"])).unwrap();
        assert!(matches!(
            algebra,
            Command::AlgebraOwner {
                output_path,
                profile_id,
                ..
            } if output_path == Path::new("algebra.json") && profile_id.as_str() == "campaign"
        ));

        let complete = parse_command(&command_arguments(
            "complete",
            &["core.json", "algebra.json", "complete.json"],
        ))
        .unwrap();
        assert!(matches!(
            complete,
            Command::Complete {
                core_path,
                algebra_path,
                output_path,
                profile_id,
                ..
            } if core_path == Path::new("core.json")
                && algebra_path == Path::new("algebra.json")
                && output_path == Path::new("complete.json")
                && profile_id.as_str() == "campaign"
        ));

        assert!(parse_command(&command_arguments(
            "unknown",
            &["core.json", "algebra.json", "complete.json"]
        ))
        .unwrap_err()
        .to_string()
        .contains(USAGE));
        assert!(parse_command(&command_arguments(
            "complete",
            &["core.json", "algebra.json"]
        ))
        .unwrap_err()
        .to_string()
        .contains(USAGE));

        let mut invalid_dirty = command_arguments("core-owner", &["core.json"]);
        invalid_dirty[5] = "maybe".to_owned();
        assert_eq!(
            parse_command(&invalid_dirty).unwrap_err().to_string(),
            "SOURCE_DIRTY must be true or false"
        );
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

    #[test]
    fn atomic_publication_never_replaces_an_existing_complete_envelope() {
        let output = TestOutput::new("complete.json");

        write_validated_atomic(&output.path, b"first", |_| Ok(())).unwrap();
        let error = write_validated_atomic(&output.path, b"second", |_| Ok(()))
            .expect_err("composition must not replace an existing artifact");

        assert!(error.to_string().contains("refusing to replace"));
        assert_eq!(fs::read(&output.path).unwrap(), b"first");
        assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 1);
    }

    #[test]
    fn strict_validation_failure_leaves_no_published_or_temporary_artifact() {
        let output = TestOutput::new("complete.json");

        write_validated_atomic(&output.path, b"{}", |written_path| {
            let text = fs::read_to_string(written_path)?;
            complete_registry()?.from_json(&text)?;
            Ok(())
        })
        .expect_err("an invalid envelope must fail strict reopening");

        assert!(!output.path.exists());
        assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 0);
    }

    #[test]
    fn validated_publication_reopens_the_temporary_and_final_artifact() {
        let output = TestOutput::new("complete.json");
        let calls = std::cell::Cell::new(0);
        let complete = complete_registry().unwrap().from_json(COMPLETE).unwrap();
        let profile_id = complete.profile_id().clone();
        let assembly = complete.verified_assembly().unwrap().provenance.clone();

        write_validated_atomic(&output.path, COMPLETE.as_bytes(), |written_path| {
            calls.set(calls.get() + 1);
            let text = fs::read_to_string(written_path)?;
            verify_composition(CORE_OWNER, ALGEBRA_OWNER, &text, &profile_id, &assembly)
        })
        .unwrap();

        assert_eq!(calls.get(), 2);
        assert_eq!(fs::read_to_string(&output.path).unwrap(), COMPLETE);
        assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 1);
    }

    #[test]
    fn committed_artifacts_are_exact_output_of_this_composer_source() {
        let source_digest = format!("{:x}", Sha256Hasher::digest(COMPOSER_SOURCE));
        let core = owner_core_registry()
            .unwrap()
            .from_json(CORE_OWNER)
            .unwrap();
        let algebra = owner_algebra_registry()
            .unwrap()
            .from_json(ALGEBRA_OWNER)
            .unwrap();
        let complete = complete_registry().unwrap().from_json(COMPLETE).unwrap();

        for prepared in [&core, &algebra, &complete] {
            let assembly = &prepared.verified_assembly().unwrap().provenance;
            assert_eq!(assembly.tool.as_str(), TOOL_PATH);
            assert_eq!(assembly.tool_sha256.as_str(), source_digest);
        }

        let core_output = TestOutput::new("core.json");
        let core_arguments = artifact_arguments(
            "core-owner",
            &[&core_output.path],
            core.profile_id(),
            &core.verified_assembly().unwrap().provenance,
        );
        execute(parse_command(&core_arguments).unwrap()).unwrap();
        assert_eq!(fs::read_to_string(&core_output.path).unwrap(), CORE_OWNER);

        let algebra_output = TestOutput::new("algebra.json");
        let algebra_arguments = artifact_arguments(
            "algebra-owner",
            &[&algebra_output.path],
            algebra.profile_id(),
            &algebra.verified_assembly().unwrap().provenance,
        );
        execute(parse_command(&algebra_arguments).unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(&algebra_output.path).unwrap(),
            ALGEBRA_OWNER
        );

        let complete_output = TestOutput::new("complete.json");
        let complete_arguments = artifact_arguments(
            "complete",
            &[
                &core_output.path,
                &algebra_output.path,
                &complete_output.path,
            ],
            complete.profile_id(),
            &complete.verified_assembly().unwrap().provenance,
        );
        execute(parse_command(&complete_arguments).unwrap()).unwrap();
        assert_eq!(fs::read_to_string(&complete_output.path).unwrap(), COMPLETE);

        assert_eq!(
            compose_texts(
                CORE_OWNER,
                ALGEBRA_OWNER,
                complete.profile_id().clone(),
                &complete.verified_assembly().unwrap().provenance,
            )
            .unwrap(),
            COMPLETE
        );
    }
}
