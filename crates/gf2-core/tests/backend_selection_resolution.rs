#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::{
    kernels::select_backend_for_size,
    tuning::{self, CoreTuning},
};

support::fresh_tuning_test!(first_backend_selection_does_not_resolve_tuning_profile, {
    assert_eq!(select_backend_for_size(0).name(), "scalar");
    assert_eq!(
        tuning::install(support::prepared_core(CoreTuning::CONSERVATIVE)),
        Ok(())
    );
    assert_eq!(tuning::active().section, &CoreTuning::CONSERVATIVE);
});
