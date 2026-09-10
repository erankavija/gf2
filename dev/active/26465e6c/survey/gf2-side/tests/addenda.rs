//! Every protocol-v3 addendum of the issue decodes and validates through the
//! protocol's own decoder, and a confirmation addendum leaves no setting
//! unresolved, so a freeze is checked when it is written rather than when a
//! timed campaign opens.

use std::path::PathBuf;
use tuning_campaign_support::protocol::{CellRole, FamilyAddendum};

fn addenda() -> Vec<(String, FamilyAddendum)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("addendum-") && name.contains("-v3-"))
        .collect();
    found.sort();
    found
        .into_iter()
        .map(|name| {
            let bytes = std::fs::read(dir.join(&name)).unwrap();
            let decoded = FamilyAddendum::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, decoded)
        })
        .collect()
}

#[test]
fn every_v3_addendum_validates() {
    let all = addenda();
    assert!(
        all.iter()
            .filter(|(name, _)| name.ends_with("-pilot.json"))
            .count()
            >= 2
    );
    for (name, addendum) in &all {
        addendum
            .validate()
            .unwrap_or_else(|errors| panic!("{name}: {}", errors.join("; ")));
        assert_eq!(addendum.protocol.version, 3, "{name}");
        assert_eq!(addendum.family.issue, "26465e6c", "{name}");
    }
}

#[test]
fn pilots_are_exploratory_and_confirmations_fully_resolved() {
    for (name, addendum) in addenda() {
        let pilot = name.ends_with("-pilot.json");
        for cell in &addendum.cells {
            if pilot {
                assert_eq!(cell.role, CellRole::Exploratory, "{name} {}", cell.cell_id);
            } else {
                assert_eq!(cell.role, CellRole::Confirmatory, "{name} {}", cell.cell_id);
                assert!(
                    addendum.unresolved_settings(cell).is_empty(),
                    "{name} {}: {:?}",
                    cell.cell_id,
                    addendum.unresolved_settings(cell)
                );
            }
        }
    }
}
