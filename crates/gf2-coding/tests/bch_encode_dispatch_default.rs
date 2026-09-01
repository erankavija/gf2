//! Guarded fresh-process witness that a process which installs no profile
//! encodes every batch under the scalar reference.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;

#[test]
fn an_uninstalled_process_encodes_under_the_scalar_reference() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::ConservativeDefault).unwrap();
    assert_eq!(result["family"], EncodeFamily::REFERENCE.name());
    assert_eq!(result["agrees_with_reference"], true);
    assert_eq!(result["resolution"], "frozen-before-install");
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}
