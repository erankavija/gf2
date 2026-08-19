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
