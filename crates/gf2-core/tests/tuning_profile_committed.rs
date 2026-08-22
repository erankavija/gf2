use std::fs;
use std::path::PathBuf;

#[test]
fn every_committed_tuning_profile_is_valid() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("tuning-profiles");
    let mut paths = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());
    for path in paths {
        let text = fs::read_to_string(path).unwrap();
        gf2_core::tuning::TuningProfile::from_json(&text).unwrap();
    }
}

#[test]
fn committed_calibrated_profile_inherits_follow_on_defaults() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("tuning-profiles")
        .join("gf2-5ecc9bf8-calibration-e202c080.json");
    let text = fs::read_to_string(path).unwrap();
    let profile = gf2_core::tuning::TuningProfile::from_json(&text).unwrap();
    let conservative = &gf2_core::tuning::TuningProfile::CONSERVATIVE;
    assert_eq!(profile.bit_matrix(), conservative.bit_matrix());
    assert_eq!(profile.soa_batch(), conservative.soa_batch());
    assert_eq!(profile.m4rm(), conservative.m4rm());
    assert_eq!(profile.dense_inverse(), conservative.dense_inverse());
    assert_eq!(profile.triangular(), conservative.triangular());
    assert_eq!(profile.ple(), conservative.ple());
    assert_eq!(profile.gemm(), conservative.gemm());
    assert_eq!(profile.field_vec(), conservative.field_vec());
    assert_eq!(profile.charpoly(), conservative.charpoly());
    assert_eq!(profile.prime_route(), conservative.prime_route());
    assert_eq!(profile.permanent(), conservative.permanent());
    assert_eq!(
        profile.polynomial().interpolate_fast_min_points(),
        conservative.polynomial().interpolate_fast_min_points()
    );
}
