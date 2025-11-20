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
fn test_dvb_t2_normal_rate_3_5_construction() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    assert_eq!(code.n(), 64800);
    assert_eq!(code.k(), 38880);
    assert_eq!(code.m(), 25920);
}

#[test]
fn test_dvb_t2_normal_rate_2_3_construction() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate2_3);
    assert_eq!(code.n(), 64800);
    assert_eq!(code.k(), 43200);
    assert_eq!(code.m(), 21600);
}

#[test]
fn test_dvb_t2_normal_rate_3_4_construction() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_4);
    assert_eq!(code.n(), 64800);
    assert_eq!(code.k(), 48600);
    assert_eq!(code.m(), 16200);
}

#[test]
fn test_dvb_t2_normal_rate_4_5_construction() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate4_5);
    assert_eq!(code.n(), 64800);
    assert_eq!(code.k(), 51840);
    assert_eq!(code.m(), 12960);
}

#[test]
fn test_dvb_t2_normal_rate_5_6_construction() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate5_6);
    assert_eq!(code.n(), 64800);
    assert_eq!(code.k(), 54000);
    assert_eq!(code.m(), 10800);
}

#[test]
fn test_dvb_t2_short_rate_1_2_construction() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    assert_eq!(code.n(), 16200);
    assert_eq!(code.k(), 7200);
    assert_eq!(code.m(), 9000);
}

#[test]
fn test_dvb_t2_short_rate_3_5_construction() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate3_5);
    assert_eq!(code.n(), 16200);
    assert_eq!(code.k(), 9720);
    assert_eq!(code.m(), 6480);
}

#[test]
fn test_dvb_t2_short_rate_2_3_construction() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate2_3);
    assert_eq!(code.n(), 16200);
    assert_eq!(code.k(), 10800);
    assert_eq!(code.m(), 5400);
}

#[test]
fn test_dvb_t2_short_rate_3_4_construction() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate3_4);
    assert_eq!(code.n(), 16200);
    assert_eq!(code.k(), 11880);
    assert_eq!(code.m(), 4320);
}

#[test]
fn test_dvb_t2_short_rate_4_5_construction() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate4_5);
    assert_eq!(code.n(), 16200);
    assert_eq!(code.k(), 12600);
    assert_eq!(code.m(), 3600);
}

#[test]
fn test_dvb_t2_short_rate_5_6_construction() {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate5_6);
    assert_eq!(code.n(), 16200);
    assert_eq!(code.k(), 13320);
    assert_eq!(code.m(), 2880);
}
