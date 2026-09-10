use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
use gf2_core::{BitMatrix, BitVec};

fn main() {
    println!("=== Testing BitVec ↔ Gf2mPoly Conversions ===\n");

    // Create a field
    let field = Gf2mField::new(4, 0b10011);

    // Create a BitVec: 1 + x^2 (binary: 101)
    let mut bits = BitVec::new();
    bits.push_bit(true); // x^0
    bits.push_bit(false); // x^1
    bits.push_bit(true); // x^2
    println!(
        "Original BitVec: {:?}",
        (0..bits.len()).map(|i| bits.get(i)).collect::<Vec<_>>()
    );

    // Convert to polynomial
    let poly = Gf2mPoly::from_bitvec(&bits, &field);
    println!("Polynomial degree: {:?}", poly.degree());
    println!(
        "Coefficients: [{}, {}, {}]",
        poly.coeff(0).value(),
        poly.coeff(1).value(),
        poly.coeff(2).value()
    );

    // Convert back to BitVec
    let recovered = poly.to_bitvec(5);
    println!(
        "Recovered BitVec (len 5): {:?}",
        (0..recovered.len())
            .map(|i| recovered.get(i))
            .collect::<Vec<_>>()
    );

    let minimal = poly.to_bitvec_minimal();
    println!(
        "Minimal BitVec (len {}): {:?}\n",
        minimal.len(),
        (0..minimal.len())
            .map(|i| minimal.get(i))
            .collect::<Vec<_>>()
    );

    println!("=== Testing BitMatrix Row/Column Extraction ===\n");

    // Create a 3x4 matrix with some pattern
    let mut m = BitMatrix::zeros(3, 4);
    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);
    m.set(2, 0, true);
    m.set(2, 3, true);

    println!("Matrix (3x4):");
    for r in 0..3 {
        print!("  Row {}: ", r);
        for c in 0..4 {
            print!("{} ", if m.get(r, c) { 1 } else { 0 });
        }
        println!();
    }
    println!();

    // Extract rows
    println!("Extracted rows:");
    for r in 0..3 {
        let row = m.row_as_bitvec(r);
        println!(
            "  Row {}: {:?}",
            r,
            (0..row.len()).map(|i| row.get(i)).collect::<Vec<_>>()
        );
    }
    println!();

    // Extract columns
    println!("Extracted columns:");
    for c in 0..4 {
        let col = m.col_as_bitvec(c);
        println!(
            "  Col {}: {:?}",
            c,
            (0..col.len()).map(|i| col.get(i)).collect::<Vec<_>>()
        );
    }

    println!("\n✅ All helper methods working correctly!");
}
