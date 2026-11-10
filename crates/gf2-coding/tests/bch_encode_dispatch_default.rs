//! Guarded fresh-process witness that a process which installs no profile
//! encodes under the cheapest family its kernel bundle offers.

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use gf2_coding::bch::encode::EncodeFamily;

#[test]
fn an_uninstalled_process_encodes_under_the_preferred_family() {
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::ConservativeDefault).unwrap();
    // The corpus row's redundancy is one table block, so the default
    // selectors admit every family at the child's batch and the preference
    // order decides.
    let accelerated = gf2_coding::test_support::selected_encode_kernel() != "scalar";
    assert_eq!(
        result["family"],
        EncodeFamily::preference(accelerated)[0].name()
    );
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
