//! Tests for DVB-T2 LDPC code construction.
//!
//! These tests verify DVB-T2 LDPC codes built directly from standard tables.

use gf2_coding::ldpc::LdpcCode;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

#[test]
fn test_dvb_t2_normal_rate_1_2_construction() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);

    assert_eq!(code.n(), 64800);
    assert_eq!(code.m(), 32400);
    assert_eq!(code.k(), 32400);
    assert_eq!(code.rate(), 0.5);
}

#[test]
fn test_dvb_t2_normal_rate_1_2_zero_codeword() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let zero_cw = BitVec::zeros(64800);

    assert!(code.is_valid_codeword(&zero_cw));
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_normal_rate_3_5_panics() {
    let _code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_normal_rate_2_3_panics() {
    let _code = LdpcCode::dvb_t2_normal(CodeRate::Rate2_3);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_normal_rate_3_4_panics() {
    let _code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_4);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_normal_rate_4_5_panics() {
    let _code = LdpcCode::dvb_t2_normal(CodeRate::Rate4_5);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_normal_rate_5_6_panics() {
    let _code = LdpcCode::dvb_t2_normal(CodeRate::Rate5_6);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_short_rate_1_2_panics() {
    let _code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_short_rate_3_5_panics() {
    let _code = LdpcCode::dvb_t2_short(CodeRate::Rate3_5);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_short_rate_2_3_panics() {
    let _code = LdpcCode::dvb_t2_short(CodeRate::Rate2_3);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_short_rate_3_4_panics() {
    let _code = LdpcCode::dvb_t2_short(CodeRate::Rate3_4);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_short_rate_4_5_panics() {
    let _code = LdpcCode::dvb_t2_short(CodeRate::Rate4_5);
}

#[test]
#[should_panic(expected = "placeholder")]
fn test_dvb_t2_short_rate_5_6_panics() {
    let _code = LdpcCode::dvb_t2_short(CodeRate::Rate5_6);
}
