//! The next confirmatory attempt a family ledger admits, against the P-20
//! tail-support rule and the sequential attempt budget acceptance applies.

use tuning_campaign_support::protocol::{SharedSettings, PROTOCOL_VERSION, SHARED_SETTINGS};
use tuning_campaign_support::receipt::tail_supported;
use tuning_campaign_support::trial_ledger::{next_attempt, Reservation};

const FAMILY: &str = "next-attempt-family";

fn spent(comparisons: u32) -> Vec<u8> {
    let reservation = Reservation {
        sequence: 0,
        predecessor: "0".repeat(64),
        family: FAMILY.to_owned(),
        campaign: "next-attempt-first".to_owned(),
        addendum_sha256: "a".repeat(64),
        comparisons,
        protocol_version: PROTOCOL_VERSION,
        candidates: vec!["b".repeat(64)],
    };
    let mut line = serde_json::to_vec(&reservation).expect("reservation encodes");
    line.push(b'\n');
    line
}

fn admits(settings: &SharedSettings, attempt_alpha: f64, comparisons: u32) -> bool {
    tail_supported(
        settings.bootstrap_resamples,
        attempt_alpha / f64::from(comparisons),
    )
}

#[test]
fn a_genesis_ledger_admits_the_largest_count_with_tail_support() {
    let next = next_attempt(b"", FAMILY, &SHARED_SETTINGS).expect("genesis decodes");
    assert_eq!(
        (next.reservations, next.prior_comparisons, next.attempt),
        (0, 0, 1)
    );
    assert_eq!(next.attempt_alpha, SHARED_SETTINGS.family_alpha / 2.0);
    assert!(next.max_cells > 0);
    assert!(admits(&SHARED_SETTINGS, next.attempt_alpha, next.max_cells));
    assert!(!admits(
        &SHARED_SETTINGS,
        next.attempt_alpha,
        next.max_cells + 1
    ));
}

#[test]
fn a_spent_attempt_lowers_the_budget_and_joins_the_divisor() {
    let first = next_attempt(b"", FAMILY, &SHARED_SETTINGS).expect("genesis decodes");
    let next = next_attempt(&spent(1), FAMILY, &SHARED_SETTINGS).expect("ledger decodes");
    assert_eq!((next.spending_reservations, next.prior_comparisons), (1, 1));
    assert_eq!(next.attempt, 2);
    assert!(next.attempt_alpha < first.attempt_alpha);
    let total = next.prior_comparisons + next.max_cells;
    assert!(next.max_cells == 0 || admits(&SHARED_SETTINGS, next.attempt_alpha, total));
    assert!(!admits(&SHARED_SETTINGS, next.attempt_alpha, total + 1));
}

#[test]
fn a_first_attempt_at_its_bound_leaves_no_cell_for_the_second() {
    let first = next_attempt(b"", FAMILY, &SHARED_SETTINGS).expect("genesis decodes");
    let next =
        next_attempt(&spent(first.max_cells), FAMILY, &SHARED_SETTINGS).expect("ledger decodes");
    assert_eq!(next.max_cells, 0);
}

#[test]
fn an_exploratory_reservation_spends_no_attempt() {
    let mut line = spent(0);
    let mut reservation: Reservation = serde_json::from_slice(&line).expect("decodes");
    reservation.candidates.clear();
    line = serde_json::to_vec(&reservation).expect("encodes");
    line.push(b'\n');
    let genesis = next_attempt(b"", FAMILY, &SHARED_SETTINGS).expect("genesis decodes");
    let next = next_attempt(&line, FAMILY, &SHARED_SETTINGS).expect("ledger decodes");
    assert_eq!((next.reservations, next.spending_reservations), (1, 0));
    assert_eq!(
        (next.attempt, next.max_cells),
        (genesis.attempt, genesis.max_cells)
    );
}
