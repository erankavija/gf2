//! Example: Visualizing Large Generator Matrices
//!
//! This example demonstrates visualization of large generator matrices (>500 rows or columns)
//! using the BitMatrix image saving functionality. We create various LDPC codes and save
//! their generator matrices as PNG images.
//!
//! Run with: cargo run --example visualize_large_matrices --features visualization
//!
//! The example includes:
//! - DVB-T2 short frame LDPC codes (16200 bits)
//! - Regular LDPC codes with various dimensions
//! - Visualization of generator matrices (typically dense)
//!
//! Generator matrices (G) encode messages: codeword = message × G
//! Unlike sparse parity-check matrices, generator matrices are typically dense.

#[cfg(not(feature = "visualization"))]
fn main() {
    eprintln!("This example requires the 'visualization' feature.");
    eprintln!("Run with: cargo run --example visualize_large_matrices --features visualization");
}

#[cfg(feature = "visualization")]
fn main() {
    use gf2_coding::ldpc::LdpcCode;
    use gf2_coding::traits::GeneratorMatrixAccess;
    use gf2_coding::CodeRate;

    println!("=== Visualizing Large Generator Matrices ===\n");
    println!("Using DVB-T2 LDPC codes - industry-standard codes from digital TV broadcasting.\n");

    // Example 1: DVB-T2 Short Frame (Rate 1/2)
    println!("Example 1: DVB-T2 short frame LDPC code (rate 1/2)...");
    let code1 = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);

    println!("  Code parameters:");
    println!("    n (codeword length):  {}", code1.n());
    println!("    m (parity checks):    {}", code1.m());
    println!("    k (message bits):     {}", code1.k());
    println!("    Rate:                 {:.4}", code1.rate());

    println!("  Computing generator matrix (this takes ~1 minute)...");
    let g1 = code1.generator_matrix();
    g1.save_image("output_dvb_t2_rate_1_2_generator.png")
        .unwrap();
    println!("  Saved: output_dvb_t2_rate_1_2_generator.png");
    println!(
        "    Matrix size: {}×{} ({} million elements)",
        g1.rows(),
        g1.cols(),
        (g1.rows() * g1.cols()) / 1_000_000
    );
    println!();

    // Example 2: DVB-T2 Short Frame (Rate 2/3)
    println!("Example 2: DVB-T2 short frame LDPC code (rate 2/3)...");
    let code2 = LdpcCode::dvb_t2_short(CodeRate::Rate2_3);

    println!("  Code parameters:");
    println!("    n (codeword length):  {}", code2.n());
    println!("    m (parity checks):    {}", code2.m());
    println!("    k (message bits):     {}", code2.k());
    println!("    Rate:                 {:.4}", code2.rate());

    println!("  Computing generator matrix...");
    let g2 = code2.generator_matrix();
    g2.save_image("output_dvb_t2_rate_2_3_generator.png")
        .unwrap();
    println!("  Saved: output_dvb_t2_rate_2_3_generator.png");
    println!(
        "    Matrix size: {}×{} ({} million elements)",
        g2.rows(),
        g2.cols(),
        (g2.rows() * g2.cols()) / 1_000_000
    );
    println!();

    // Example 3: DVB-T2 Short Frame (Rate 3/4)
    println!("Example 3: DVB-T2 short frame LDPC code (rate 3/4)...");
    let code3 = LdpcCode::dvb_t2_short(CodeRate::Rate3_4);

    println!("  Code parameters:");
    println!("    n (codeword length):  {}", code3.n());
    println!("    m (parity checks):    {}", code3.m());
    println!("    k (message bits):     {}", code3.k());
    println!("    Rate:                 {:.4}", code3.rate());

    println!("  Computing generator matrix...");
    let g3 = code3.generator_matrix();
    g3.save_image("output_dvb_t2_rate_3_4_generator.png")
        .unwrap();
    println!("  Saved: output_dvb_t2_rate_3_4_generator.png");
    println!(
        "    Matrix size: {}×{} ({} million elements)",
        g3.rows(),
        g3.cols(),
        (g3.rows() * g3.cols()) / 1_000_000
    );
    println!();

    // Summary
    println!("=== Visualization Complete ===");
    println!("\nGenerated visualizations:");
    println!(
        "  1. output_dvb_t2_rate_1_2_generator.png  - Rate 1/2 (k={}, n={})",
        code1.k(),
        code1.n()
    );
    println!(
        "  2. output_dvb_t2_rate_2_3_generator.png  - Rate 2/3 (k={}, n={})",
        code2.k(),
        code2.n()
    );
    println!(
        "  3. output_dvb_t2_rate_3_4_generator.png  - Rate 3/4 (k={}, n={})",
        code3.k(),
        code3.n()
    );
    println!("\nVisualization notes:");
    println!("  - Black pixels represent 0");
    println!("  - White pixels represent 1");
    println!("  - Generator matrices encode: codeword = message × G");
    println!("  - Unlike sparse parity-check matrices, generators are typically dense");
    println!("  - Matrix dimensions: k rows (message bits) × n columns (codeword bits)");
    println!("  - All matrices have n=16200 columns (DVB-T2 short frame size)");
    println!("  - As code rate increases, k increases (more message bits, less redundancy)");
    println!("\nTo change colors, modify ZERO_COLOR and ONE_COLOR in gf2-core::matrix.");
}
