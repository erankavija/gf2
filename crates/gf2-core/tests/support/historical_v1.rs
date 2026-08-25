use sha2::{Digest, Sha256};

const ARCHIVED_V1: &[u8] = include_bytes!(
    "../../../../dev/archive/3fa7c9d0/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080-v1.json"
);
const ARCHIVED_V1_SHA256: &str = "674eea65379d1c814cd54584ad1ea4517fc3f2adbef3d5229d58593e9aad63bb";
const ARCHIVED_V1_RECEIPT: &str = "dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md";

/// Verifies the exact historical measurement and one selector-family omission.
///
/// This is intentionally a test-only raw format-1 evidence check, not a
/// production compatibility loader.
pub(crate) fn assert_family_was_omitted(family: &str) {
    assert_eq!(ARCHIVED_V1.len(), 1_087, "archived v1 bytes changed");
    assert_eq!(
        format!("{:x}", Sha256::digest(ARCHIVED_V1)),
        ARCHIVED_V1_SHA256,
        "archived v1 digest changed"
    );

    let document: serde_json::Value =
        serde_json::from_slice(ARCHIVED_V1).expect("archived v1 evidence is JSON");
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["profile_id"], "gf2-5ecc9bf8-calibration-e202c080");
    assert_eq!(document["provenance"]["receipt"], ARCHIVED_V1_RECEIPT);
    let selectors = document["selectors"]
        .as_object()
        .expect("archived v1 selectors are an object");
    assert!(
        !selectors.contains_key(family),
        "archived v1 unexpectedly measures selector family {family}"
    );
}
