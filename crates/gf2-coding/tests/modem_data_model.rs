//! Integration tests for the shared modem data model (`c87c5043`).
//!
//! These tests exercise the public surface of [`gf2_coding::modem`]:
//! preset construction, invariants observable through the public API,
//! view accessors, and property-based bijection properties.

use gf2_coding::modem::{
    BitChannelId, BitChannelSemantics, DemapMethod, LabelWord, ModemCapabilities, ModemSpec,
    ModemView, Normalization, SymbolPoint,
};

use proptest::prelude::*;

fn all_orders() -> [usize; 5] {
    [2, 4, 16, 64, 256]
}

#[test]
fn test_public_presets_construct_for_all_orders() {
    let _b: ModemSpec<f32> = ModemSpec::bpsk();
    for order in all_orders() {
        let spec = ModemSpec::gray_square_qam(order);
        assert_eq!(spec.num_symbols(), order);
    }
}

#[test]
fn test_public_view_slice_vs_per_item_agree() {
    let spec = ModemSpec::gray_square_qam(64);
    let v: ModemView<'_, f32> = spec.view();
    for (i, (p, l)) in v.points().iter().zip(v.labels()).enumerate() {
        assert_eq!(v.point(i), *p);
        assert_eq!(v.label(i), *l);
    }
    for k in 0..v.bits_per_symbol() {
        assert_eq!(v.bit_channel_id(k), BitChannelId { bit_index: k });
    }
}

#[test]
fn test_public_view_is_copy_across_boundaries() {
    fn count_points<S: gf2_coding::modem::ModemScalar>(v: ModemView<'_, S>) -> usize {
        v.num_symbols()
    }
    let spec = ModemSpec::gray_square_qam(16);
    let v = spec.view();
    let n1 = count_points(v);
    let n2 = count_points(v); // Copy is fine
    assert_eq!(n1, n2);
}

#[test]
fn test_public_capabilities_populated_for_presets() {
    let caps = ModemSpec::bpsk().capabilities();
    assert!(caps.supports_exact_log_map);
    assert!(caps.supports_max_log);
    for order in all_orders() {
        let c = ModemSpec::gray_square_qam(order).capabilities();
        assert!(c.supports_exact_log_map);
        assert!(c.supports_max_log);
    }
    assert_eq!(
        caps,
        ModemCapabilities {
            supports_exact_log_map: true,
            supports_max_log: true,
        }
    );
}

#[test]
fn test_public_normalization_contract_advertised() {
    let spec = ModemSpec::gray_square_qam(16);
    match spec.normalization() {
        Normalization::UnitAverageSymbolEnergy => {}
        other => panic!("expected UnitAverageSymbolEnergy, got {:?}", other),
    }
    assert!(spec.normalization_scale() > 0.0);
}

#[test]
fn test_public_bpsk_point_layout() {
    let spec = ModemSpec::bpsk();
    let v = spec.view();
    assert_eq!(v.point(0), SymbolPoint::<f32>::new(1.0, 0.0));
    assert_eq!(v.point(1), SymbolPoint::<f32>::new(-1.0, 0.0));
    assert_eq!(v.bit_channel(0), BitChannelSemantics::SingleAxisPam(0));
}

#[test]
fn test_public_qpsk_matches_legacy_layout_under_scaling() {
    // The new Gray-square-QAM(4) preset must match the legacy QpskModulator
    // with delta = 1/sqrt(2) (unit symbol energy).
    let spec = ModemSpec::gray_square_qam(4);
    let delta = (0.5_f64).sqrt();
    // bit0 (MSB) toggles I sign, bit1 (LSB) toggles Q sign.
    for label_bits in 0u16..4 {
        let idx = spec
            .view()
            .labels()
            .iter()
            .position(|l| l.bits == label_bits)
            .expect("label present");
        let p = spec.view().point(idx);
        let want_i = if (label_bits >> 1) & 1 == 0 {
            delta
        } else {
            -delta
        };
        let want_q = if label_bits & 1 == 0 { delta } else { -delta };
        assert!((p.i as f64 - want_i).abs() < 1e-6);
        assert!((p.q as f64 - want_q).abs() < 1e-6);
    }
}

#[test]
fn test_public_demap_method_values() {
    // DemapMethod variants remain distinct and constructible at crate
    // root so trait-layer (d36ae697) can consume them directly.
    assert_ne!(DemapMethod::ExactLogMap, DemapMethod::MaxLog);
}

#[test]
fn test_public_label_word_bit_order() {
    let l = LabelWord::new(0b1001, 4);
    assert!(l.bit(0));
    assert!(!l.bit(1));
    assert!(!l.bit(2));
    assert!(l.bit(3));
}

#[test]
fn test_public_presets_labels_are_bijection() {
    for order in all_orders() {
        let spec = ModemSpec::gray_square_qam(order);
        let mut seen = vec![false; order];
        for l in spec.view().labels() {
            assert_eq!(l.width, spec.bits_per_symbol());
            assert!(!seen[l.bits as usize]);
            seen[l.bits as usize] = true;
        }
        assert!(seen.iter().all(|b| *b));
    }
}

#[test]
fn test_public_f64_variants_construct() {
    let b: ModemSpec<f64> = ModemSpec::<f64>::bpsk_with_scalar();
    assert_eq!(b.bits_per_symbol(), 1);
    for order in all_orders() {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        assert_eq!(spec.num_symbols(), order);
        let mut acc = 0.0_f64;
        for p in spec.view().points() {
            acc += p.i * p.i + p.q * p.q;
        }
        assert!((acc / order as f64 - 1.0).abs() < 1e-10);
    }
}

proptest! {
    #[test]
    fn prop_gray_square_qam_labels_bijection(order_idx in 0usize..5usize) {
        let order = [2usize, 4, 16, 64, 256][order_idx];
        let spec = ModemSpec::gray_square_qam(order);
        let mut seen = vec![false; order];
        for l in spec.view().labels() {
            prop_assert!(!seen[l.bits as usize]);
            seen[l.bits as usize] = true;
        }
        prop_assert!(seen.iter().all(|b| *b));
    }

    #[test]
    fn prop_label_word_bit_matches_shift(bits in 0u16..=u16::MAX, width in 1u8..=16u8) {
        // Only consider bits that fit in width to avoid LabelWord::new panicking.
        let mask = if width == 16 { u16::MAX } else { (1u16 << width) - 1 };
        let bits = bits & mask;
        let l = LabelWord::new(bits, width);
        for k in 0..width {
            let shift = width - 1 - k;
            let expect = ((bits >> shift) & 1) == 1;
            prop_assert_eq!(l.bit(k), expect);
        }
    }

    #[test]
    fn prop_gray_square_qam_unit_energy_f32(order_idx in 0usize..5usize) {
        let order = [2usize, 4, 16, 64, 256][order_idx];
        let spec = ModemSpec::gray_square_qam(order);
        let mut acc = 0.0_f64;
        for p in spec.view().points() {
            acc += (p.i as f64).powi(2) + (p.q as f64).powi(2);
        }
        prop_assert!((acc / order as f64 - 1.0).abs() < 1e-5);
    }
}
