//! Compile-time values of the core selectors that are constants in generated
//! code or gate an operation too cheap for a runtime profile read. A build with
//! `RUSTFLAGS="--cfg gf2_tuning_baked"` selects them at their defining
//! modules' selection sites; the default build keeps each module's
//! conservative constant.

// Outside a baked build only the `tuning-profile` unit tests read these.
#![cfg_attr(not(any(gf2_tuning_baked, feature = "tuning-profile")), allow(dead_code))]

/// `bit_backend.simd_min_words`, from the calibration receipt
/// `dev/benchmarks/tuning_profiles/gf2-dbd8787d-20261001t230000z-2601601.md`
/// and the measured core owner
/// `crates/gf2-core/data/tuning-profiles/gf2-dbd8787d-20261001t230000z-2601601.json`
/// with SHA-256 `8904f7d0c9ef0577790b2af6c42b9da632306842e7928a54ea7251a51c3c4ffa`.
pub(crate) const SIMD_MIN_WORDS: usize = 8;

/// `bit_matrix.matvec_simd_min_words`, mirroring
/// `crate::matrix::MATVEC_SIMD_MIN_WORDS`: the core section's conservative
/// value.
pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;

/// `gemm.row_tile`, mirroring `crate::field::matrix::GEMM_ROW_TILE` and
/// selected jointly with [`GEMM_COL_TILE`] by the receipt cited at
/// [`SIMD_MIN_WORDS`].
pub(crate) const GEMM_ROW_TILE: usize = 32;

/// `gemm.col_tile`, mirroring `crate::field::matrix::GEMM_COL_TILE`; see
/// [`GEMM_ROW_TILE`].
pub(crate) const GEMM_COL_TILE: usize = 64;

/// `field_vec.dot_chunk_len`, mirroring `crate::field::vec::DOT_CHUNK_LEN`,
/// from the receipt cited at [`SIMD_MIN_WORDS`].
pub(crate) const DOT_CHUNK_LEN: usize = 256;

/// `prime_route.f32_min_prime`, mirroring
/// `crate::gfp::simd_ops::N_THRESH_PRIME`: the core section's conservative
/// value, a `u64` because its read site compares it with a const-generic
/// prime `P: u64`.
pub(crate) const N_THRESH_PRIME: u64 = 251;

/// `prime_route.f32_min_cols`, mirroring `crate::gfp::simd_ops::F32_MIN_COLS`:
/// the core section's conservative value.
pub(crate) const F32_MIN_COLS: usize = 512;

/// `prime_route.f64_min_cols`, mirroring `crate::gfp::simd_ops::F64_MIN_COLS`:
/// the core section's conservative value.
pub(crate) const F64_MIN_COLS: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::CoreTuning;
    #[cfg(feature = "tuning-profile")]
    use crate::tuning::{CoreTuningCodec, ProfileRegistryBuilder};
    use sha2::{Digest, Sha256};

    const MEASURED_FORMAT2: &[u8] =
        include_bytes!("../../data/tuning-profiles/gf2-dbd8787d-20261001t230000z-2601601.json");

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
            "8904f7d0c9ef0577790b2af6c42b9da632306842e7928a54ea7251a51c3c4ffa"
        );
    }

    #[cfg(feature = "tuning-profile")]
    fn measured_owner_section() -> CoreTuning {
        let registry = ProfileRegistryBuilder::new()
            .register::<CoreTuning, CoreTuningCodec>()
            .unwrap()
            .build()
            .unwrap();
        let owner = registry
            .from_json(std::str::from_utf8(MEASURED_FORMAT2).unwrap())
            .unwrap();
        owner
            .section::<CoreTuning>()
            .unwrap()
            .unwrap()
            .section
            .clone()
    }

    #[cfg(feature = "tuning-profile")]
    #[test]
    fn baked_simd_threshold_matches_the_strict_measured_owner() {
        assert_eq!(
            SIMD_MIN_WORDS,
            measured_owner_section().bit_backend().simd_min_words()
        );
    }

    #[cfg(feature = "tuning-profile")]
    #[test]
    fn baked_gemm_tile_pair_matches_the_strict_measured_owner() {
        let measured = measured_owner_section();
        assert_eq!(
            (GEMM_ROW_TILE, GEMM_COL_TILE),
            (measured.gemm().row_tile(), measured.gemm().col_tile())
        );
    }

    #[cfg(feature = "tuning-profile")]
    #[test]
    fn baked_dot_chunk_matches_the_strict_measured_owner() {
        assert_eq!(
            DOT_CHUNK_LEN,
            measured_owner_section().field_vec().dot_chunk_len()
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
