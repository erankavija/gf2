//! Scratch-length witness for the baked `field_vec.dot_chunk_len` of the
//! measured format-2 core owner, compiled only under `--cfg gf2_tuning_baked`.
//! The observation is a process-wide maximum, so exactly one test per binary
//! resets and reads it.
#![cfg(all(gf2_tuning_baked, feature = "simd"))]

use gf2_core::field::vec::{max_effective_dot_chunk_len, reset_max_effective_dot_chunk_len};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::tuning::{self, CoreTuning};

#[path = "support/measured_format2.rs"]
mod measured_format2;
#[path = "support/core_tuning.rs"]
mod support;

fn measured_dot_chunk_section() -> CoreTuning {
    measured_format2::measured_section("field_vec", &["dot_chunk_len"], &[])
}

support::fresh_tuning_test!(
    baked_build_walks_the_dot_product_by_the_measured_chunk_length,
    {
        // `dot_chunk_len` is baked: the walk keeps its compile-time step whatever
        // the installed runtime profile states.
        let installed = support::prepared_core_json(r#"{"field_vec": {"dot_chunk_len": 3}}"#)
            .expect("test profile is valid");
        tuning::install(installed).expect("profile has not been resolved");
        assert_eq!(tuning::active().field_vec().dot_chunk_len(), 3);

        let chunk = measured_dot_chunk_section().field_vec().dot_chunk_len();
        let field = Gf2mField::gf256();
        let n = 2 * chunk as u64 + 1;
        let a: FieldVec<Gf2mElement> = (0..n).map(|i| field.element((i * 37 + 13) % 256)).collect();
        let b: FieldVec<Gf2mElement> = (0..n).map(|i| field.element((i * 53 + 7) % 256)).collect();

        reset_max_effective_dot_chunk_len();
        let simd = a.simd_dot_product(&b);

        assert_eq!(simd, a.dot_product(&b));
        assert_eq!(
            max_effective_dot_chunk_len(),
            chunk,
            "the SIMD dot-product walk did not step by the baked \
         field_vec.dot_chunk_len; a zero observation means the host offers no \
         batched carry-less multiply and the walk never ran"
        );
    }
);
