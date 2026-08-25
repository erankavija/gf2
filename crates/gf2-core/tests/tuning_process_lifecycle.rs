#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use fresh::{fresh_tuning_process, FreshProcessCase};
use gf2_core::tuning::{
    self, BitBackendSelectors, CompiledProfileProvenance, CoreSelectors, CoreTuning,
    PreparedEnvelope, ProfileId, ResolutionCause, SectionResolution,
};

fn profile_id(name: &str) -> ProfileId {
    ProfileId::parse(name).unwrap()
}

fn compiled(name: &str) -> CompiledProfileProvenance {
    CompiledProfileProvenance {
        artifact_id: profile_id(name),
    }
}

fn prepared_core(value: usize) -> PreparedEnvelope {
    let mut selectors = CoreSelectors::CONSERVATIVE.clone();
    selectors.bit_backend = BitBackendSelectors::try_new(value).unwrap();
    PreparedEnvelope::compiled(profile_id("process-test"), compiled("process-test"))
        .insert(CoreTuning::from_selectors(selectors))
        .unwrap()
        .build()
        .unwrap()
}

#[track_caller]
fn first_core_access() -> gf2_core::tuning::ActiveSection<'static, CoreTuning> {
    tuning::active()
}

#[test]
fn access_before_install_freezes_and_reports_the_original_site() {
    let result = fresh_tuning_process(FreshProcessCase::FreezeBeforeInstall).unwrap();
    assert_eq!(result["resolution"], "frozen-before-install");
    assert_eq!(result["late_install"], "already-resolved");
    assert_eq!(result["same_site"], true);
    assert_eq!(result["cause"], "first-access");
}

#[test]
fn install_before_access_reports_installed_and_rejects_a_second_install() {
    let result = fresh_tuning_process(FreshProcessCase::InstallPresent).unwrap();
    assert_eq!(result["resolution"], "installed");
    assert_eq!(result["value"], 16);
    assert_eq!(result["late_install"], "already-resolved");
    assert_eq!(result["cause"], "installed");
}

#[test]
fn installed_envelope_missing_the_type_defaults_conservatively() {
    let result = fresh_tuning_process(FreshProcessCase::InstallMissing).unwrap();
    assert_eq!(result["resolution"], "defaulted-missing");
    assert_eq!(
        result["value"],
        CoreTuning::CONSERVATIVE.bit_backend().simd_min_words()
    );
}

#[test]
fn prepared_projection_is_read_only_and_does_not_freeze_installation() {
    let result = fresh_tuning_process(FreshProcessCase::ProjectionThenInstall).unwrap();
    assert_eq!(result["projected"], 31);
    assert_eq!(result["resolution"], "installed");
    assert_eq!(result["value"], 31);
}

#[test]
fn invalid_child_sentinel_is_a_protocol_error() {
    fresh::invalid_sentinel_is_rejected().unwrap();
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    let result = match case {
        FreshProcessCase::FreezeBeforeInstall => {
            let active = first_core_access();
            let SectionResolution::FrozenBeforeInstall {
                first_resolution, ..
            } = active.resolution
            else {
                panic!("pre-install access did not freeze");
            };
            let error = tuning::install(prepared_core(16)).unwrap_err();
            serde_json::json!({
                "resolution": "frozen-before-install",
                "late_install": "already-resolved",
                "same_site": error.first_resolution == first_resolution,
                "cause": match first_resolution.cause {
                    ResolutionCause::FirstAccess { .. } => "first-access",
                    ResolutionCause::Installed => "installed",
                }
            })
        }
        FreshProcessCase::InstallPresent => {
            tuning::install(prepared_core(16)).unwrap();
            let active = tuning::active();
            assert!(matches!(
                active.resolution,
                SectionResolution::Installed { .. }
            ));
            let error = tuning::install(prepared_core(17)).unwrap_err();
            serde_json::json!({
                "resolution": "installed",
                "value": active.bit_backend().simd_min_words(),
                "late_install": "already-resolved",
                "cause": match error.first_resolution.cause {
                    ResolutionCause::Installed => "installed",
                    ResolutionCause::FirstAccess { .. } => "first-access",
                }
            })
        }
        FreshProcessCase::InstallMissing => {
            let prepared =
                PreparedEnvelope::compiled(profile_id("missing-test"), compiled("missing-test"))
                    .build()
                    .unwrap();
            tuning::install(prepared).unwrap();
            let active = tuning::active();
            assert!(matches!(
                active.resolution,
                SectionResolution::DefaultedMissing { .. }
            ));
            serde_json::json!({
                "resolution": "defaulted-missing",
                "value": active.bit_backend().simd_min_words()
            })
        }
        FreshProcessCase::ProjectionThenInstall => {
            let prepared = prepared_core(31);
            let projected = prepared
                .section::<CoreTuning>()
                .unwrap()
                .unwrap()
                .section
                .bit_backend()
                .simd_min_words();
            tuning::install(prepared).unwrap();
            let active = tuning::active();
            assert!(matches!(
                active.resolution,
                SectionResolution::Installed { .. }
            ));
            serde_json::json!({
                "projected": projected,
                "resolution": "installed",
                "value": active.bit_backend().simd_min_words()
            })
        }
    };
    fresh::emit_result(result);
}
