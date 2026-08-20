use gf2_core::{
    kernels::select_backend_for_size,
    tuning::{self, AlreadyResolved, TuningProfile},
};

#[test]
fn first_backend_selection_resolves_tuning_profile() {
    assert_eq!(select_backend_for_size(0).name(), "scalar");
    assert_eq!(
        tuning::install(TuningProfile::CONSERVATIVE),
        Err(AlreadyResolved)
    );
}
