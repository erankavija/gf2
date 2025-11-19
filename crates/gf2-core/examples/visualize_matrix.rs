//! Example: Visualizing BitMatrix and SpBitMatrix as PNG images
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
    use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};

    // Example 1: Identity matrix (dense)
    println!("Creating 64×64 identity matrix (dense)...");
    let id = BitMatrix::identity(64);
    id.save_image("output_identity_64.png").unwrap();
    println!("Saved: output_identity_64.png");

    // Example 2: Identity matrix (sparse)
    println!("Creating 64×64 identity matrix (sparse)...");
    let id_sparse = SpBitMatrix::identity(64);
    id_sparse
        .save_image("output_identity_64_sparse.png")
        .unwrap();
    println!("Saved: output_identity_64_sparse.png");

    // Example 3: Random sparse matrix
    #[cfg(feature = "rand")]
    {
        use rand::thread_rng;
        println!("Creating 100×100 random sparse matrix (dense)...");
        let m = BitMatrix::random_with_probability(100, 100, 0.1, &mut thread_rng());
        m.save_image("output_sparse_100.png").unwrap();
        println!("Saved: output_sparse_100.png");

        println!("Creating 100×100 random sparse matrix (from sparse format)...");
        let s = SpBitMatrix::from_dense(&m);
        s.save_image("output_sparse_100_from_sparse.png").unwrap();
        println!("Saved: output_sparse_100_from_sparse.png");
    }

    // Example 4: X pattern (dense)
    println!("Creating 16×16 X pattern (dense)...");
    let mut small = BitMatrix::zeros(16, 16);
    for i in 0..16 {
        small.set(i, i, true);
        small.set(i, 15 - i, true);
    }
    small.save_image("output_x_pattern.png").unwrap();
    println!("Saved: output_x_pattern.png");

    // Example 5: Structured sparse pattern
    println!("Creating 32×32 checkerboard pattern (sparse)...");
    let mut coo = Vec::new();
    for i in 0..32 {
        for j in 0..32 {
            if (i + j) % 2 == 0 {
                coo.push((i, j));
            }
        }
    }
    let checkerboard = SpBitMatrix::from_coo(32, 32, &coo);
    checkerboard
        .save_image("output_checkerboard_sparse.png")
        .unwrap();
    println!("Saved: output_checkerboard_sparse.png");

    // Example 6: SpBitMatrixDual
    println!("Creating 24×24 border pattern (sparse dual)...");
    let mut border_coo = Vec::new();
    for i in 0..24 {
        border_coo.push((0, i)); // top
        border_coo.push((23, i)); // bottom
        border_coo.push((i, 0)); // left
        border_coo.push((i, 23)); // right
    }
    let border = SpBitMatrixDual::from_coo(24, 24, &border_coo);
    border.save_image("output_border_dual.png").unwrap();
    println!("Saved: output_border_dual.png");

    println!("\nVisualization complete!");
    println!("To change colors, edit ZERO_COLOR and ONE_COLOR constants in the implementation.");
}
