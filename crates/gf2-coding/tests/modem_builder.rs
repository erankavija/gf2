//! Integration tests for the public [`ModemSpecBuilder`] surface.
//!
//! Exercises the fluent builder from the crate root, mirroring the usage
//! pattern expected of downstream code that consumes a custom modem
//! constellation.

use gf2_coding::modem::{
    BitChannelSemantics, LabelWord, ModemSpec, ModemSpecBuilder, Normalization, SymbolPoint,
};

/// Builds an 8-PSK constellation (8 points on the unit circle) with an
/// arbitrary bijective labelling, using the public builder surface. This
/// exercises the "custom N-point constellation" path end-to-end.
#[test]
fn test_build_custom_8psk_via_builder() {
    let n = 8usize;
    let points: Vec<SymbolPoint<f32>> = (0..n)
        .map(|k| {
            // Unnormalized points on a radius-2 circle to show the
            // builder rescales to unit average symbol energy.
            let theta = (k as f32) * core::f32::consts::TAU / (n as f32);
            SymbolPoint::new(2.0 * theta.cos(), 2.0 * theta.sin())
        })
        .collect();

    // Non-trivial bijective labelling (Gray-ish on a circle).
    let raw_labels: [u16; 8] = [0b000, 0b001, 0b011, 0b010, 0b110, 0b111, 0b101, 0b100];
    let labels: Vec<LabelWord> = raw_labels.iter().map(|&b| LabelWord::new(b, 3)).collect();

    let spec: ModemSpec<f32> = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(3)
        .points(points)
        .labels(labels)
        .build();

    assert_eq!(spec.num_symbols(), 8);
    assert_eq!(spec.bits_per_symbol(), 3);

    // Defaults applied by the builder: Opaque bit channels and both demap
    // methods supported.
    let view = spec.view();
    assert_eq!(view.bit_channels().len(), 3);
    for (k, bc) in view.bit_channels().iter().enumerate() {
        assert_eq!(*bc, BitChannelSemantics::Opaque(k as u8));
    }
    assert!(view.capabilities().supports_exact_log_map);
    assert!(view.capabilities().supports_max_log);

    // Builder renormalized to unit average symbol energy.
    let mean: f64 = view
        .points()
        .iter()
        .map(|p| (p.i as f64).powi(2) + (p.q as f64).powi(2))
        .sum::<f64>()
        / view.num_symbols() as f64;
    assert!((mean - 1.0).abs() < 1e-5, "post-build mean energy = {mean}");
}

/// Ensures [`ModemSpec::builder`] is callable at the crate boundary and
/// returns a builder that rolls forward into a valid spec.
#[test]
fn test_build_custom_spec_via_modem_spec_builder_entry() {
    let spec = ModemSpec::<f32>::builder()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 1.0),
            SymbolPoint::new(1.0, -1.0),
            SymbolPoint::new(-1.0, 1.0),
            SymbolPoint::new(-1.0, -1.0),
        ])
        .labels(vec![
            LabelWord::new(0b00, 2),
            LabelWord::new(0b01, 2),
            LabelWord::new(0b10, 2),
            LabelWord::new(0b11, 2),
        ])
        .build();

    assert_eq!(spec.num_symbols(), 4);
    assert_eq!(spec.bits_per_symbol(), 2);
    let mean: f64 = spec
        .view()
        .points()
        .iter()
        .map(|p| (p.i as f64).powi(2) + (p.q as f64).powi(2))
        .sum::<f64>()
        / spec.num_symbols() as f64;
    assert!((mean - 1.0).abs() < 1e-5);
}

/// Exercises the `ExplicitEs` normalization through the public surface.
#[test]
fn test_build_custom_spec_explicit_es_target() {
    let target = 9.0_f32;
    let spec: ModemSpec<f32> = ModemSpec::<f32>::builder()
        .bits_per_symbol(1)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
        ])
        .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
        .normalization(Normalization::ExplicitEs(target))
        .build();
    let mean: f64 = spec
        .view()
        .points()
        .iter()
        .map(|p| (p.i as f64).powi(2) + (p.q as f64).powi(2))
        .sum::<f64>()
        / spec.num_symbols() as f64;
    assert!((mean - target as f64).abs() < 1e-5);
}
