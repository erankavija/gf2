//! Example: Visualizing BitMatrix as PNG images
//!
//! Run with: cargo run --example visualize_matrix --features visualization

#[cfg(not(feature = "visualization"))]
fn main() {
    eprintln!("This example requires the 'visualization' feature.");
    eprintln!("Run with: cargo run --example visualize_matrix --features visualization");
}

#[cfg(feature = "visualization")]
fn main() {
    use gf2_core::matrix::BitMatrix;

    // Example 1: Identity matrix
    println!("Creating 64×64 identity matrix...");
    let id = BitMatrix::identity(64);
    id.save_image("output_identity_64.png").unwrap();
    println!("Saved: output_identity_64.png");

    // Example 2: Random sparse matrix
    #[cfg(feature = "rand")]
    {
        use rand::thread_rng;
        println!("Creating 100×100 random sparse matrix...");
        let m = BitMatrix::random_with_probability(100, 100, 0.1, &mut thread_rng());
        m.save_image("output_sparse_100.png").unwrap();
        println!("Saved: output_sparse_100.png");
    }

    // Example 3: X pattern
    println!("Creating 16×16 X pattern...");
    let mut small = BitMatrix::zeros(16, 16);
    for i in 0..16 {
        small.set(i, i, true);
        small.set(i, 15 - i, true);
    }
    small.save_image("output_x_pattern.png").unwrap();
    println!("Saved: output_x_pattern.png");

    println!("\nVisualization complete!");
    println!("To change colors, edit ZERO_COLOR and ONE_COLOR in src/matrix.rs");
}
