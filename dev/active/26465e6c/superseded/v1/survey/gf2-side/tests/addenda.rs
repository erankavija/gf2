//! The four family addenda this survey freezes must decode and validate through
//! the protocol's own decoder, not only through the launcher's plan check.
//!
//! `run-campaign.sh` refuses a plan whose cells disagree with the addendum, but
//! it reaches `FamilyAddendum::validate` only inside a timed launch. These tests
//! run the same validation without taking the timing lock, so a freeze is
//! checked when it is written rather than when the campaign starts.

use std::fs;
use std::path::PathBuf;
use tuning_campaign_support::protocol::{CellRole, FamilyAddendum};

/// Addenda live two directories above the harness crate root.
fn addendum(name: &str) -> (PathBuf, FamilyAddendum) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let decoded = FamilyAddendum::decode(&bytes)
        .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()));
    (path, decoded)
}

const PILOTS: &[&str] = &[
    "addendum-popcount-pilot.json",
    "addendum-and-popcnt-pilot.json",
];
const CONFIRMATORY: &[&str] = &["addendum-popcount.json", "addendum-and-popcnt.json"];

#[test]
fn every_addendum_decodes_and_validates() {
    for name in PILOTS.iter().chain(CONFIRMATORY) {
        let (path, decoded) = addendum(name);
        decoded
            .validate()
            .unwrap_or_else(|errors| panic!("{}: {}", path.display(), errors.join("; ")));
    }
}

#[test]
fn a_frozen_confirmatory_addendum_leaves_no_cell_setting_unresolved() {
    for name in CONFIRMATORY {
        let (path, decoded) = addendum(name);
        assert!(
            decoded.frozen.frozen_utc.is_some(),
            "{}: a confirmatory addendum must be frozen before it governs a run",
            path.display()
        );
        for cell in &decoded.cells {
            let unresolved = decoded.unresolved_settings(cell);
            assert!(
                unresolved.is_empty(),
                "{} cell {}: unresolved {}",
                path.display(),
                cell.cell_id,
                unresolved.join(", ")
            );
            assert!(
                matches!(cell.role, CellRole::Confirmatory),
                "{} cell {}: this survey declares only confirmatory cells here",
                path.display(),
                cell.cell_id
            );
        }
    }
}

#[test]
fn a_pilot_addendum_declares_only_exploratory_cells_and_resolves_nothing() {
    for name in PILOTS {
        let (path, decoded) = addendum(name);
        assert!(
            decoded.effect.measurement_resolution.is_none(),
            "{}: a pilot measures the resolution, it does not declare one",
            path.display()
        );
        for cell in &decoded.cells {
            assert!(
                matches!(cell.role, CellRole::Exploratory),
                "{} cell {}: a pilot declares only exploratory cells",
                path.display(),
                cell.cell_id
            );
        }
    }
}

/// The margins a frozen addendum carries must sit strictly outside the
/// resolution its pilot measured. This is the rule
/// `survey/freeze_rule.py` derives; asserting it on the committed bytes keeps a
/// hand edit from reintroducing a margin the design cannot resolve.
///
/// The comparison is done on integer hundredths. Both quantities are declared
/// whole percentages, and `margin - 1.0` in binary floating point is not the
/// declared decimal: 1.05 - 1.0 is 0.05000000000000004, which is larger than
/// the double nearest 0.05, so an equal margin and resolution compare as
/// unequal and the rule silently does not fire.
#[test]
fn frozen_margins_lie_outside_the_measured_resolution() {
    fn hundredths(value: f64) -> i64 {
        (value * 100.0).round() as i64
    }
    for name in CONFIRMATORY {
        let (path, decoded) = addendum(name);
        let effect = &decoded.effect;
        let resolution = effect
            .measurement_resolution
            .unwrap_or_else(|| panic!("{}: frozen without a resolution", path.display()));
        assert!(
            effect.resolution_evidence.is_some(),
            "{}: a resolution needs the pilot receipt that measured it",
            path.display()
        );
        for (label, margin) in [
            ("equivalence_margin", effect.equivalence_margin),
            ("material_gap_threshold", effect.material_gap_threshold),
        ] {
            let Some(margin) = margin else { continue };
            assert!(
                hundredths(margin) - 100 > hundredths(resolution),
                "{} {label} {margin} does not lie strictly outside the resolution {resolution}",
                path.display()
            );
        }
    }
}
