//! Scratch-length witness for the baked `field_vec.dot_chunk_len` field.
//!
//! The field sizes `try_simd_dot_product`'s three stack buffers, so only the
//! bake mechanism can carry it (`dev/active/7d824b2f/design.md` §3.8). This
//! witness asserts the committed conservative length, so it is compiled only
//! under the declared cfg `gf2_tuning_baked`
//! (`RUSTFLAGS="--cfg gf2_tuning_baked"`); the default build's conservative
//! length is asserted by `field_vec_dot_chunk.rs`.
//!
//! The format-1 calibration archived at
//! `dev/archive/3fa7c9d0/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080-v1.json`
//! (SHA-256 `674eea65379d1c814cd54584ad1ea4517fc3f2adbef3d5229d58593e9aad63bb`)
//! cites `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md` but
//! omits `field_vec`. The baked field therefore retains its conservative
//! default until measured format-2 evidence supersedes that anchor, as
//! `dev/active/3fa7c9d0/design.md` §7.1 requires.
//!
//! The observation is a process-wide maximum, so exactly one test per binary
//! resets and reads it.
#![cfg(all(gf2_tuning_baked, feature = "simd"))]

use gf2_core::field::vec::{max_effective_dot_chunk_len, reset_max_effective_dot_chunk_len};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::tuning::{self, CoreTuning};

#[path = "support/historical_v1.rs"]
mod historical_v1;
#[path = "support/core_tuning.rs"]
mod support;

/// Returns the default justified by the archived calibration's omission.
fn archived_default_section() -> CoreTuning {
    historical_v1::assert_family_was_omitted("field_vec");
    CoreTuning::CONSERVATIVE
}

support::fresh_tuning_test!(
    baked_build_walks_the_dot_product_by_the_archived_default_chunk_length,
    {
        // Installing a chunk length the walk would have to honour if it read the
        // runtime profile: the field is baked, so the walk keeps its compile-time
        // step and the result is unchanged.
        let installed = support::prepared_core_json(r#"{"field_vec": {"dot_chunk_len": 3}}"#)
            .expect("test profile is valid");
        tuning::install(installed).expect("profile has not been resolved");
        assert_eq!(tuning::active().field_vec().dot_chunk_len(), 3);

        let chunk = archived_default_section().field_vec().dot_chunk_len();
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
