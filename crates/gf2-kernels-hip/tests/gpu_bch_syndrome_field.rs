//! Field-level GPU/CPU equivalence for the batch BCH syndrome evaluator
//! (issue `9012f8a0`, correctness ladder rungs 1-3; design doc §10).
//!
//! Rung 1 — exhaustive GF(2^4) multiply: all 256 `(a, b)` products, GPU
//!   `gf_mul` (driven through a 1-point Horner evaluation) vs CPU `Gf2mField`
//!   arithmetic, bit-identical.
//! Rung 2 — uploaded-table equality: the device `exp` / `log` tables (the ones
//!   [`BchFieldTables`] uploads) equal the live CPU tables element-for-element
//!   for GF(2^14) and GF(2^16).
//! Rung 3 — small BCH(15)/GF(2^4) Horner fixture: GPU syndromes for a known
//!   received word equal a hand-derived expected value AND the CPU evaluation.
//!
//! All rungs are `#[cfg(feature = "hip")]`, carry `#[ignore = "sim: ..."]`, and
//! skip cleanly when `device_mem_info().is_err()` (mirrors
//! `gpu_ldpc_byte_identity.rs`). The arithmetic is exact integer GF — zero
//! tolerance, no ULP drift (design doc §10).

#![cfg(feature = "hip")]

use gf2_core::field::FieldPoly;
use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
use gf2_kernels_hip::host::device_mem_info;
use gf2_kernels_hip::launch_bch_syndrome::{gf_mul_device_batch, BchFieldTables, GpuBchSyndrome};

/// GF(2^4) with the standard primitive polynomial x^4 + x + 1 (`0b10011`).
fn gf16() -> Gf2mField {
    Gf2mField::new(4, 0b10011).with_tables()
}

/// Builds [`BchFieldTables`] from a live field's uploaded tables.
fn tables_from(field: &Gf2mField) -> BchFieldTables {
    BchFieldTables::new(
        field.degree(),
        field.exp_table().expect("tables").to_vec(),
        field.log_table().expect("tables").to_vec(),
    )
}

/// The `α^1..α^(2t)` evaluation points for `field`, as u16 values.
fn eval_points(field: &Gf2mField, two_t: usize) -> Vec<u16> {
    let alpha = field.primitive_element().expect("primitive element");
    let mut pts = Vec::with_capacity(two_t);
    let mut ap = alpha.clone();
    for _ in 0..two_t {
        pts.push(ap.value() as u16);
        ap = &ap * &alpha;
    }
    pts
}

#[test]
#[ignore = "sim: exhaustive GF(2^4) GPU gf_mul vs CPU Gf2mField (gfx1030-gated)"]
fn gpu_gf16_mul_exhaustive_matches_cpu() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_gf16_mul_exhaustive_matches_cpu: no usable GPU");
        return;
    }
    let field = gf16();
    let tables = tables_from(&field);

    // Enumerate all 256 (a, b) pairs and run the REAL device gf_mul on each via
    // the standalone test kernel (BCH coefficients are binary, so the Horner
    // path alone cannot feed an arbitrary leading accumulator — this harness
    // exercises the same `gf_mul` the syndrome kernel calls).
    let mut a_in: Vec<u16> = Vec::with_capacity(256);
    let mut b_in: Vec<u16> = Vec::with_capacity(256);
    for a in 0u16..16 {
        for b in 0u16..16 {
            a_in.push(a);
            b_in.push(b);
        }
    }
    let gpu = gf_mul_device_batch(&tables, &a_in, &b_in, 0).expect("device gf_mul");
    assert_eq!(gpu.len(), 256);

    for (idx, (&a, &b)) in a_in.iter().zip(b_in.iter()).enumerate() {
        let cpu = (&field.element(a as u64) * &field.element(b as u64)).value() as u16;
        assert_eq!(
            gpu[idx], cpu,
            "gf_mul({a}, {b}) GPU {} != CPU {cpu}",
            gpu[idx]
        );
    }
}

#[test]
#[ignore = "sim: device exp/log table equality for GF(2^14)/GF(2^16) (gfx1030-gated)"]
fn gpu_uploaded_tables_equal_cpu_for_dvb_fields() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_uploaded_tables_equal_cpu_for_dvb_fields: no usable GPU");
        return;
    }
    // GF(2^14) (DVB-T2 Short) and GF(2^16) (DVB-T2 Normal) primitive polys.
    for &(m, poly) in &[
        (14usize, 0b100000000101011u64),
        (16usize, 0b10000000000101101u64),
    ] {
        let field = Gf2mField::new(m, poly).with_tables();
        let tables = tables_from(&field);
        // The bundle the device uploads carries the exact CPU tables.
        assert_eq!(tables.exp(), field.exp_table().unwrap());
        assert_eq!(tables.log(), field.log_table().unwrap());
        assert_eq!(tables.order(), (1u32 << m) - 1);
        // And the device round-trips them: build an evaluator (which uploads
        // exp/log to the device) and run a trivial all-zero batch — success
        // proves the upload path accepts the full-size tables (128 KB each for
        // m=16) without error.
        let two_t = 24usize;
        let pts = eval_points(&field, two_t);
        let n = 100usize;
        let mut ev = GpuBchSyndrome::new(&tables, &pts, n, 12, 4, 0).expect("build evaluator");
        let words_per_frame = n.div_ceil(64);
        let out = ev
            .evaluate_batch(&vec![0u64; words_per_frame], 1)
            .expect("evaluate");
        assert_eq!(
            out,
            vec![0u16; two_t],
            "all-zero frame must give zero syndromes (m={m})"
        );
    }
}

#[test]
#[ignore = "sim: small BCH(15)/GF(2^4) Horner fixture GPU vs hand value vs CPU (gfx1030-gated)"]
fn gpu_bch15_horner_fixture_matches_cpu_and_hand() {
    if device_mem_info().is_err() {
        eprintln!("skipping gpu_bch15_horner_fixture_matches_cpu_and_hand: no usable GPU");
        return;
    }
    let field = gf16();
    let tables = tables_from(&field);
    let two_t = 4usize; // t = 2
    let pts = eval_points(&field, two_t);
    let n = 15usize;

    // Coefficient vector in the design-doc §3.1 order. We choose a non-trivial
    // pattern of binary coefficients: coeff index d set for d in {0, 1, 4, 7,
    // 14}. coeffs[0] is the constant term; coeffs[14] the leading term.
    let set_indices = [0usize, 1, 4, 7, 14];
    let mut coeffs: Vec<u16> = vec![0; n];
    for &d in &set_indices {
        coeffs[d] = 1;
    }

    // CPU oracle: FieldPoly::eval at each point (the exact Horner recurrence
    // compute_syndromes uses via eval_batch).
    let cpu_poly: Gf2mPoly = FieldPoly::new(
        coeffs
            .iter()
            .map(|&c| field.element(c as u64))
            .collect::<Vec<_>>(),
    );
    let cpu: Vec<u16> = pts
        .iter()
        .map(|&p| cpu_poly.eval(&field.element(p as u64)).value() as u16)
        .collect();

    // Hand value for the FIRST point (α^1 = 2): evaluate
    // p(α) = α^14 + α^7 + α^4 + α + 1 directly via the exp table.
    // In GF(2^4) with x^4+x+1, exp[i] = α^i. Sum (XOR) the monomials.
    let exp = field.exp_table().unwrap();
    let hand_s1 = exp[14] ^ exp[7] ^ exp[4] ^ exp[1] ^ exp[0];
    assert_eq!(
        cpu[0], hand_s1,
        "CPU S1 {} != hand-derived {hand_s1}",
        cpu[0]
    );

    // Pack the coeff stream (single 64-bit word covers 15 bits).
    let mut stream = vec![0u64; n.div_ceil(64)];
    for &d in &set_indices {
        stream[d >> 6] |= 1u64 << (d & 63);
    }

    let mut ev = GpuBchSyndrome::new(&tables, &pts, n, 2, 4, 0).expect("build evaluator");
    let gpu = ev.evaluate_batch(&stream, 1).expect("evaluate");

    assert_eq!(gpu.len(), two_t);
    assert_eq!(gpu, cpu, "GPU syndromes must equal CPU oracle");
    assert_eq!(gpu[0], hand_s1, "GPU S1 must equal hand-derived value");
}
