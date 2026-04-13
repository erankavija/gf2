//! Integration test for the public [`GrayQamMapper`] surface.
//!
//! Verifies that the mapper is reachable from the crate root and that its
//! [`BatchMapper`] methods produce outputs equivalent to the preset
//! [`ModemSpec::gray_square_qam`] per-label point lookup for 16-QAM.

use gf2_coding::modem::{BatchMapper, GrayQamMapper, ModemSpec};

/// Flatten a u16 label of `width` bits into an MSB-first bool vector.
fn bits_msb_first(label: u16, width: u8) -> Vec<bool> {
    (0..width)
        .map(|k| ((label >> (width - 1 - k)) & 1) == 1)
        .collect()
}

#[test]
fn test_gray_qam_mapper_public_surface_16qam() {
    let mapper = GrayQamMapper::from_preset_order(16);
    // `spec()` accessor exists and matches the preset.
    let view = mapper.spec();
    assert_eq!(view.num_symbols(), 16);
    assert_eq!(view.bits_per_symbol(), 4);

    let preset = ModemSpec::gray_square_qam(16);
    let preset_view = preset.view();

    // All 16 labels, one-symbol batches.
    for label in 0u16..16 {
        let bits = bits_msb_first(label, 4);
        let mut oi = [0.0_f32; 1];
        let mut oq = [0.0_f32; 1];
        mapper.map_bits(&bits, &mut oi, &mut oq);

        let idx = preset_view
            .labels()
            .iter()
            .position(|l| l.bits == label)
            .expect("label present");
        let expected = preset_view.point(idx);
        assert!(
            (oi[0] - expected.i).abs() < 1e-6,
            "label {label}: I got {} want {}",
            oi[0],
            expected.i
        );
        assert!(
            (oq[0] - expected.q).abs() < 1e-6,
            "label {label}: Q got {} want {}",
            oq[0],
            expected.q
        );
    }

    // Multi-symbol batch.
    let mut all_bits = Vec::with_capacity(16 * 4);
    for label in 0u16..16 {
        all_bits.extend(bits_msb_first(label, 4));
    }
    let mut oi = vec![0.0_f32; 16];
    let mut oq = vec![0.0_f32; 16];
    mapper.map_bits(&all_bits, &mut oi, &mut oq);
    for label in 0u16..16 {
        let idx = preset_view
            .labels()
            .iter()
            .position(|l| l.bits == label)
            .expect("label present");
        let expected = preset_view.point(idx);
        assert!((oi[label as usize] - expected.i).abs() < 1e-6);
        assert!((oq[label as usize] - expected.q).abs() < 1e-6);
    }
}
