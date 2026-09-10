//! Scratch-length witness for the baked `field_vec.dot_chunk_len` field.
//!
//! The field sizes `try_simd_dot_product`'s three stack buffers, so only the
//! bake mechanism can carry it (`dev/active/7d824b2f/design.md` §3.8). This
//! witness asserts the committed conservative length, so it is compiled only
//! under the declared cfg `gf2_tuning_baked`
//! (`RUSTFLAGS="--cfg gf2_tuning_baked"`); the default build's conservative
//! length is asserted by `field_vec_dot_chunk.rs`.
//!
//! The measured format-2 core owner at
//! `crates/gf2-core/data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json`
//! cites `dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` and omits
//! `field_vec`. The baked field therefore retains its conservative default, as
//! `dev/active/3fa7c9d0/design.md` §7.1 requires.
//!
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

/// Returns the default resolved from the measured owner's omission.
fn measured_default_section() -> CoreTuning {
    measured_format2::omitted_fields_section("field_vec", &["dot_chunk_len"])
}

support::fresh_tuning_test!(
    baked_build_walks_the_dot_product_by_the_measured_default_chunk_length,
    {
        // Installing a chunk length the walk would have to honour if it read the
        // runtime profile: the field is baked, so the walk keeps its compile-time
        // step and the result is unchanged.
        let installed = support::prepared_core_json(r#"{"field_vec": {"dot_chunk_len": 3}}"#)
            .expect("test profile is valid");
        tuning::install(installed).expect("profile has not been resolved");
        assert_eq!(tuning::active().field_vec().dot_chunk_len(), 3);

        let chunk = measured_default_section().field_vec().dot_chunk_len();
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
