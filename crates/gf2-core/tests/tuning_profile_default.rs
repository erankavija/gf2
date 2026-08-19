use gf2_core::tuning::{self, AlreadyResolved, TuningProfile};

#[test]
fn conservative_profile_resolves_once_and_rejects_late_install() {
    assert_eq!(tuning::active(), &TuningProfile::CONSERVATIVE);
    let first = tuning::active();
    let second = tuning::active();
    assert_eq!(first, second);
    assert_eq!(
        tuning::install(TuningProfile::CONSERVATIVE),
        Err(AlreadyResolved)
    );
}
