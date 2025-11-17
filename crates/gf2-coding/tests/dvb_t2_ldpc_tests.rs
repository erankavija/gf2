//! Tests for DVB-T2 LDPC code construction.
//!
//! These tests verify the basic structure of DVB-T2 LDPC codes.
//! Full validation requires complete base matrices from ETSI EN 302 755.

use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
use gf2_coding::CodeRate;

/// Test DVB-T2 short frame dimensions.
#[test]
fn test_dvb_t2_short_dimensions() {
    let qc = QuasiCyclicLdpc::dvb_t2_short(CodeRate::Rate1_2);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    // DVB-T2 short frame should have n=16200
    assert_eq!(code.n(), qc.expanded_cols());
    assert_eq!(code.n(), 16200);

    // Expansion factor is 360
    assert_eq!(qc.expansion_factor(), 360);
}

/// Test that DVB-T2 short codes are valid QC-LDPC codes.
#[test]
fn test_dvb_t2_short_validity() {
    let qc = QuasiCyclicLdpc::dvb_t2_short(CodeRate::Rate1_2);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    // All-zeros should be a valid codeword
    let mut all_zeros = gf2_core::BitVec::new();
    for _ in 0..code.n() {
        all_zeros.push_bit(false);
    }

    assert!(code.is_valid_codeword(&all_zeros));
}

/// Test that DVB-T2 structure is quasi-cyclic.
#[test]
fn test_dvb_t2_quasi_cyclic_structure() {
    let qc = QuasiCyclicLdpc::dvb_t2_short(CodeRate::Rate1_2);

    // Verify base matrix dimensions are reasonable
    assert!(qc.base_rows() > 0);
    assert!(qc.base_cols() > 0);

    // Verify expansion produces correct total dimensions
    assert_eq!(qc.expanded_rows(), qc.base_rows() * qc.expansion_factor());
    assert_eq!(qc.expanded_cols(), qc.base_cols() * qc.expansion_factor());
}

/// Test that DVB-T2 short rate 3/5 constructs properly (placeholder matrix).
#[test]
fn test_dvb_t2_short_rate_3_5() {
    let qc = QuasiCyclicLdpc::dvb_t2_short(CodeRate::Rate3_5);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    // Should have n=16200 for short frame
    assert_eq!(qc.expansion_factor(), 360);
    assert_eq!(code.n(), 16200);

    // NOTE: This uses a placeholder matrix until actual ETSI EN 302 755 Table 6b is entered
}

/// Test that DVB-T2 normal frame constructs properly (placeholder matrix).
#[test]
fn test_dvb_t2_normal() {
    let qc = QuasiCyclicLdpc::dvb_t2_normal(CodeRate::Rate1_2);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    // Should have n=64800 for normal frame
    assert_eq!(qc.expansion_factor(), 360);
    assert_eq!(code.n(), 64800);

    // NOTE: This uses a placeholder matrix until actual ETSI EN 302 755 Table 7a is entered
}

/// Test that 5G NR panics (not yet implemented).
#[test]
#[should_panic(expected = "not yet implemented")]
fn test_5g_nr_not_implemented() {
    let _qc = QuasiCyclicLdpc::nr_5g(1, 360);
}
