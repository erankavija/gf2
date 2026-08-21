//! Compile-time tuning values for the bit-backend selection boundary.

/// Calibrated bit-backend threshold from profile
/// `gf2-5ecc9bf8-calibration-e202c080` (`simd_min_words = 4`), recorded in
/// `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`.
#[allow(dead_code)]
pub(crate) const SIMD_MIN_WORDS: usize = 4;

#[cfg(test)]
mod tests {
    use super::SIMD_MIN_WORDS;
    use crate::tuning::TuningProfile;

    #[test]
    fn default_constant_matches_conservative_table() {
        assert_eq!(
            crate::kernels::backend::SIMD_MIN_WORDS_DEFAULT,
            TuningProfile::CONSERVATIVE.bit_backend().simd_min_words()
        );
    }

    #[test]
    fn baked_constant_matches_committed_profile_file() {
        let document: serde_json::Value = serde_json::from_str(include_str!(
            "../../data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json"
        ))
        .expect("committed calibrated profile is valid JSON");
        let profile_value = document["selectors"]["bit_backend"]["simd_min_words"]
            .as_u64()
            .expect("committed profile contains bit_backend.simd_min_words");
        assert_eq!(SIMD_MIN_WORDS, profile_value as usize);
    }
}
