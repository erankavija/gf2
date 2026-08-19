use gf2_core::field::poly::{
    DIV_REM_THRESHOLD, KARATSUBA_THRESHOLD, NTT_THRESHOLD, SUBPRODUCT_THRESHOLD,
};
use gf2_core::tuning::{self, AlreadyResolved, TuningProfile};

#[test]
fn conservative_profile_resolves_once_and_rejects_late_install() {
    assert_eq!(tuning::active(), &TuningProfile::CONSERVATIVE);
    let polynomial = tuning::active().polynomial();
    assert_eq!(
        polynomial.karatsuba_min_degree(),
        KARATSUBA_THRESHOLD,
        "karatsuba_min_degree uses the conservative default"
    );
    assert_eq!(
        polynomial.karatsuba_max_out_len(),
        NTT_THRESHOLD,
        "karatsuba_max_out_len uses the conservative default"
    );
    assert_eq!(
        polynomial.div_rem_fast_min_len(),
        DIV_REM_THRESHOLD,
        "div_rem_fast_min_len uses the conservative default"
    );
    assert_eq!(
        polynomial.subproduct_min_len(),
        SUBPRODUCT_THRESHOLD,
        "subproduct_min_len uses the conservative default"
    );
    let first = tuning::active();
    let second = tuning::active();
    assert_eq!(first, second);
    assert_eq!(
        tuning::install(TuningProfile::CONSERVATIVE),
        Err(AlreadyResolved)
    );
}
