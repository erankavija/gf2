//! Compile-time values for baked core selectors.
//!
//! Each constant here is the compile-time counterpart of one core selector
//! that `dev/active/7d824b2f/design.md` §3 admits under the bake mechanism
//! of §2.2: a value that appears as a compile-time constant in generated
//! code, or whose gated operation is too cheap to absorb a runtime profile
//! read. Building with `RUSTFLAGS="--cfg gf2_tuning_baked"` selects a
//! constant here at its defining module's selection site; the default build
//! keeps that module's conservative constant. Selection sites own their
//! compile-time wiring; this module owns the values.

/// Calibrated bit-backend threshold (`simd_min_words = 4`) recorded in
/// `dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md`. The exact measured
/// format-2 core owner is
/// `crates/gf2-core/data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json`
/// with SHA-256 `0296a498b2dcaf303af0dc88afba2feb708deda60c24fd94236b4533cfdc138a`.
pub(crate) const SIMD_MIN_WORDS: usize = 4;

/// Baked value for `bit_matrix.matvec_simd_min_words`, mirroring
/// `crate::matrix::MATVEC_SIMD_MIN_WORDS`.
///
/// Both of `BitMatrix::matvec`'s arms are private, so no runtime steering
/// reaches this field and it is non-sweepable
/// (`dev/active/7d824b2f/design.md` §5.2). Its value is the core section's
/// conservative declaration rather than a measured figure.
pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;

/// Baked value for `gemm.row_tile`, mirroring
/// `crate::field::matrix::GEMM_ROW_TILE`.
///
/// The field is an extent, not a threshold, so it is non-sweepable
/// (`dev/active/7d824b2f/design.md` §5.2). Its value is the core section's
/// conservative declaration.
pub(crate) const GEMM_ROW_TILE: usize = 32;

/// Baked value for `gemm.col_tile`, mirroring
/// `crate::field::matrix::GEMM_COL_TILE`.
///
/// See `GEMM_ROW_TILE` for the tiling rationale; the value is the conservative
/// default for the same reason.
pub(crate) const GEMM_COL_TILE: usize = 64;

/// Baked value for `field_vec.dot_chunk_len`, mirroring
/// `crate::field::vec::DOT_CHUNK_LEN`.
///
/// The field sizes `try_simd_dot_product`'s stack scratch buffers, so only
/// the bake mechanism can carry it (`dev/active/7d824b2f/design.md` §3.8);
/// as an extent it is also non-sweepable (§5.2). Its value is the core
/// section's conservative declaration.
pub(crate) const DOT_CHUNK_LEN: usize = 256;

/// Baked value for `prime_route.f32_min_prime`, mirroring
/// `crate::gfp::simd_ops::N_THRESH_PRIME`.
///
/// Declared as `u64` rather than the schema field's `usize`: the read site
/// compares it against a const-generic prime `P: u64`
/// (`dev/active/7d824b2f/design.md` §3.11), and only the bake mechanism
/// reaches a `const fn` predicate over that parameter. Its grid is a set of
/// primes selected by a type parameter rather than a size grid, so the field
/// is non-sweepable (§5.2); its value is the core section's conservative
/// declaration.
pub(crate) const N_THRESH_PRIME: u64 = 251;

/// Baked value for `prime_route.f32_min_cols`, mirroring
/// `crate::gfp::simd_ops::F32_MIN_COLS`.
///
/// The public route-A toggle forces the route on but cannot force it off at
/// `n >= 512`, so no grid point offers both arms across the default and the
/// field is non-sweepable (`dev/active/7d824b2f/design.md` §5.2). Its value is
/// the core section's conservative declaration.
pub(crate) const F32_MIN_COLS: usize = 512;

/// Baked value for `prime_route.f64_min_cols`, mirroring
/// `crate::gfp::simd_ops::F64_MIN_COLS`.
///
/// No public toggle offers both arms of this boundary, so the field is
/// non-sweepable (`dev/active/7d824b2f/design.md` §5.2); its value is the core
/// section's conservative declaration.
pub(crate) const F64_MIN_COLS: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::CoreTuning;
    #[cfg(feature = "tuning-profile")]
    use crate::tuning::{CoreTuningCodec, ProfileRegistryBuilder};
    use sha2::{Digest, Sha256};

    const MEASURED_FORMAT2: &[u8] =
        include_bytes!("../../data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json");

    #[test]
    fn default_constant_matches_conservative_table() {
        assert_eq!(
            crate::kernels::backend::SIMD_MIN_WORDS_DEFAULT,
            CoreTuning::CONSERVATIVE.bit_backend().simd_min_words()
        );
    }

    #[test]
    fn current_measured_owner_has_the_pinned_content_hash() {
        assert_eq!(
            format!("{:x}", Sha256::digest(MEASURED_FORMAT2)),
            "0296a498b2dcaf303af0dc88afba2feb708deda60c24fd94236b4533cfdc138a"
        );
    }

    #[cfg(feature = "tuning-profile")]
    #[test]
    fn baked_simd_threshold_matches_the_strict_measured_owner() {
        let registry = ProfileRegistryBuilder::new()
            .register::<CoreTuning, CoreTuningCodec>()
            .unwrap()
            .build()
            .unwrap();
        let owner = registry
            .from_json(std::str::from_utf8(MEASURED_FORMAT2).unwrap())
            .unwrap();
        let measured = owner.section::<CoreTuning>().unwrap().unwrap();

        assert_eq!(
            SIMD_MIN_WORDS,
            measured.section.bit_backend().simd_min_words()
        );
    }

    fn assert_matches_conservative(baked: u64, conservative_default: u64) {
        assert_eq!(baked, conservative_default);
    }

    #[test]
    fn matvec_simd_min_words_matches_conservative_section() {
        assert_matches_conservative(
            MATVEC_SIMD_MIN_WORDS as u64,
            CoreTuning::CONSERVATIVE
                .bit_matrix()
                .matvec_simd_min_words() as u64,
        );
    }

    #[test]
    fn gemm_row_tile_matches_conservative_section() {
        assert_matches_conservative(
            GEMM_ROW_TILE as u64,
            CoreTuning::CONSERVATIVE.gemm().row_tile() as u64,
        );
    }

    #[test]
    fn gemm_col_tile_matches_conservative_section() {
        assert_matches_conservative(
            GEMM_COL_TILE as u64,
            CoreTuning::CONSERVATIVE.gemm().col_tile() as u64,
        );
    }

    #[test]
    fn dot_chunk_len_matches_conservative_section() {
        assert_matches_conservative(
            DOT_CHUNK_LEN as u64,
            CoreTuning::CONSERVATIVE.field_vec().dot_chunk_len() as u64,
        );
    }

    #[test]
    fn f32_min_prime_matches_conservative_section() {
        assert_matches_conservative(
            N_THRESH_PRIME,
            CoreTuning::CONSERVATIVE.prime_route().f32_min_prime() as u64,
        );
    }

    #[test]
    fn f32_min_cols_matches_conservative_section() {
        assert_matches_conservative(
            F32_MIN_COLS as u64,
            CoreTuning::CONSERVATIVE.prime_route().f32_min_cols() as u64,
        );
    }

    #[test]
    fn f64_min_cols_matches_conservative_section() {
        assert_matches_conservative(
            F64_MIN_COLS as u64,
            CoreTuning::CONSERVATIVE.prime_route().f64_min_cols() as u64,
        );
    }
}
