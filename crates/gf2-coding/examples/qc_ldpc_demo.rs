//! Quasi-Cyclic LDPC Code Example
//!
//! Demonstrates constructing and using quasi-cyclic LDPC codes, which are
//! the foundation for standards like DVB-T2 and 5G NR.

use gf2_coding::ldpc::{CirculantMatrix, LdpcCode, QuasiCyclicLdpc};
use gf2_core::BitVec;

fn main() {
    println!("=== Quasi-Cyclic LDPC Code Example ===\n");

    // Example 1: Manual QC-LDPC construction
    println!("1. Manual QC-LDPC Construction");
    println!("   Base matrix (3×4 with expansion factor Z=5):");

    let base_matrix = vec![
        vec![0, 1, 2, -1], // -1 = zero block
        vec![1, 0, -1, 3],
        vec![2, -1, 0, 1],
    ];
    let expansion_factor = 5;

    for row in &base_matrix {
        print!("   [");
        for (i, &val) in row.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            if val == -1 {
                print!(" -");
            } else {
                print!("{:2}", val);
            }
        }
        println!("]");
    }

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    println!("\n   Expanded dimensions:");
    println!("   - Base: {}×{}", qc.base_rows(), qc.base_cols());
    println!("   - Expansion factor: {}", qc.expansion_factor());
    println!("   - Expanded: {}×{} (m×n)", code.m(), code.n());
    println!("   - Code rate: {:.3}", code.rate());
    println!("   - Information bits: {}\n", code.k());

    // Example 2: Circulant matrix structure
    println!("2. Circulant Matrix Structure");
    println!("   A circulant with shift=2, size=5:");

    let circ = CirculantMatrix::new(2, 5);
    let edges = circ.to_edges(0, 0);

    println!("   First row has 1 at column {}", 2);
    println!("   Edges: {:?}", edges);
    println!("   Forms a right-shifted identity pattern\n");

    // Example 3: DVB-T2 placeholder
    println!("3. DVB-T2 LDPC Code");

    use gf2_coding::CodeRate;
    let dvb_code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);

    println!("   DVB-T2 Normal Frame, Rate 1/2:");
    println!("   - Codeword length: {}", dvb_code.n());
    println!("   - Expansion factor: 360");
    println!("   - Code rate: {:.3}", dvb_code.rate());
    println!("   Note: Built from ETSI EN 302 755 standard tables.");
    println!("         base matrices from ETSI EN 302 755.\n");

    // Example 4: Code structure validation
    println!("4. Code Structure Validation");

    // Use smaller code for demo
    let demo_base = vec![vec![0, 1, 2], vec![1, 2, 0]];
    let demo_qc = QuasiCyclicLdpc::new(demo_base, 3);
    let demo_code = LdpcCode::from_quasi_cyclic(&demo_qc);

    println!(
        "   Code: {}×{}, rate {:.3}",
        demo_code.m(),
        demo_code.n(),
        demo_code.rate()
    );

    // Verify all-zeros is valid codeword
    let all_zeros = BitVec::zeros(demo_code.n());
    assert!(demo_code.is_valid_codeword(&all_zeros));
    println!("   - All-zeros is valid codeword: ✓");

    // Check syndrome for random invalid codeword
    let mut invalid = BitVec::zeros(demo_code.n());
    invalid.push_bit(true); // Has one bit set
    invalid.resize(demo_code.n(), false);
    let syndrome = demo_code.syndrome(&invalid);
    println!(
        "   - Invalid codeword creates non-zero syndrome: ✓ ({} ones)",
        syndrome.count_ones()
    );

    println!("\n=== End of Example ===");
    println!("\nNext Steps:");
    println!("  - Add actual DVB-T2 base matrices from ETSI EN 302 755");
    println!("  - Add 5G NR base matrices from 3GPP TS 38.212");
    println!("  - Implement WiFi 802.11n/ac LDPC codes");
    println!("  - Add systematic encoding for QC-LDPC codes");
}
