//! Integration test: [`ReferenceMapper`] end-to-end on a custom
//! constellation built through the public [`ModemSpecBuilder`] entry
//! point.
//!
//! This test imports from the crate root so it exercises the re-export
//! surface that downstream users will actually consume.

use gf2_coding::modem::{
    BatchMapper, LabelWord, ModemSpecBuilder, Normalization, ReferenceMapper, SymbolPoint,
};

#[test]
fn test_reference_mapper_end_to_end_custom_constellation() {
    // 4 custom points with a non-identity label permutation.
    let points = vec![
        SymbolPoint::<f32>::new(1.0, 0.0),
        SymbolPoint::<f32>::new(0.0, 1.0),
        SymbolPoint::<f32>::new(-1.0, 0.0),
        SymbolPoint::<f32>::new(0.0, -1.0),
    ];
    let labels_perm: [u16; 4] = [3, 0, 1, 2];
    let labels = labels_perm
        .iter()
        .map(|&b| LabelWord::new(b, 2))
        .collect::<Vec<_>>();
    let spec = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(2)
        .points(points)
        .labels(labels)
        .normalization(Normalization::UnitAverageSymbolEnergy)
        .build();

    // Snapshot expected (i, q) by label.bits.
    let view = spec.view();
    let mut expected: [(f32, f32); 4] = [(0.0, 0.0); 4];
    for k in 0..4 {
        let l = view.label(k);
        let p = view.point(k);
        expected[l.bits as usize] = (p.i, p.q);
    }

    let mapper = ReferenceMapper::new(spec);

    // Map every label value once in a single 4-symbol batch.
    // Bits are MSB-first within each symbol.
    let mut bits: Vec<bool> = Vec::with_capacity(8);
    for v in 0u16..4 {
        bits.push(((v >> 1) & 1) == 1);
        bits.push((v & 1) == 1);
    }

    let mut out_i = [0.0_f32; 4];
    let mut out_q = [0.0_f32; 4];
    mapper.map_bits(&bits, &mut out_i, &mut out_q);

    for v in 0usize..4 {
        assert_eq!(
            (out_i[v], out_q[v]),
            expected[v],
            "mapper output for label {v} did not match spec point"
        );
    }
}
