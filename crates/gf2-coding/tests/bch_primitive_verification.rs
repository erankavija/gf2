//! Integration tests ensuring BCH codes use verified primitive polynomials.
//!
//! These tests verify that:
//! 1. All DVB-T2 BCH configurations use primitive polynomials
//! 2. The polynomials match the ETSI EN 302 755 standard
//! 3. The wrong polynomial that caused the bug is detected

use gf2_coding::bch::dvb_t2::{DvbBchParams, FrameSize};
use gf2_coding::CodeRate;
use gf2_core::gf2m::Gf2mField;

#[test]
fn test_all_dvb_t2_configurations_use_primitive_polynomials() {
    let frame_sizes = [FrameSize::Short, FrameSize::Normal];
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];

    for &frame_size in &frame_sizes {
        for &rate in &rates {
            let params = DvbBchParams::for_code(frame_size, rate);
            let field = Gf2mField::new(params.field_m, params.primitive_poly);
            
            assert!(
                field.verify_primitive(),
                "DVB-T2 {:?} {:?} uses non-primitive polynomial {:#b}",
                frame_size, rate, params.primitive_poly
            );
        }
    }
}

#[test]
fn test_dvb_t2_short_uses_correct_polynomial() {
    let params = DvbBchParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
    
    // Must use the correct x^14 + x^5 + x^3 + x + 1
    assert_eq!(params.primitive_poly, 0b100000000101011,
        "DVB-T2 short frames must use standard GF(2^14) polynomial");
    
    // Must NOT use the wrong polynomial that caused the bug
    assert_ne!(params.primitive_poly, 0b100000000100001,
        "Must not use alternative primitive polynomial");
}

#[test]
fn test_dvb_t2_normal_uses_correct_polynomial() {
    let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
    
    // Must use the correct x^16 + x^5 + x^3 + x^2 + 1
    assert_eq!(params.primitive_poly, 0b10000000000101101,
        "DVB-T2 normal frames must use standard GF(2^16) polynomial");
}

#[test]
fn test_dvb_t2_polynomials_match_database() {
    use gf2_core::primitive_polys::{PrimitivePolynomialDatabase, VerificationResult};
    
    let frame_sizes = [FrameSize::Short, FrameSize::Normal];
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];

    for &frame_size in &frame_sizes {
        for &rate in &rates {
            let params = DvbBchParams::for_code(frame_size, rate);
            
            let result = PrimitivePolynomialDatabase::verify(
                params.field_m,
                params.primitive_poly
            );
            
            assert_eq!(
                result,
                VerificationResult::Matches,
                "DVB-T2 {:?} {:?} polynomial must match database standard",
                frame_size, rate
            );
        }
    }
}

#[test]
fn test_wrong_polynomial_is_detected() {
    use gf2_core::primitive_polys::{PrimitivePolynomialDatabase, VerificationResult};
    
    // The bug case: using wrong (but still primitive) polynomial
    let wrong_poly = 0b100000000100001; // x^14 + x^5 + 1
    
    let result = PrimitivePolynomialDatabase::verify(14, wrong_poly);
    
    assert_eq!(
        result,
        VerificationResult::Conflict,
        "Wrong DVB-T2 polynomial should be flagged as conflict"
    );
}

#[test]
fn test_all_dvb_t2_short_frames_use_same_field() {
    // All short frame codes should use the same field parameters
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];
    
    let expected_m = 14;
    let expected_poly = 0b100000000101011;
    
    for &rate in &rates {
        let params = DvbBchParams::for_code(FrameSize::Short, rate);
        assert_eq!(params.field_m, expected_m,
            "All short frames use GF(2^14)");
        assert_eq!(params.primitive_poly, expected_poly,
            "All short frames use same primitive polynomial");
    }
}

#[test]
fn test_all_dvb_t2_normal_frames_use_same_field() {
    // All normal frame codes should use the same field parameters
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];
    
    let expected_m = 16;
    let expected_poly = 0b10000000000101101;
    
    for &rate in &rates {
        let params = DvbBchParams::for_code(FrameSize::Normal, rate);
        assert_eq!(params.field_m, expected_m,
            "All normal frames use GF(2^16)");
        assert_eq!(params.primitive_poly, expected_poly,
            "All normal frames use same primitive polynomial");
    }
}
