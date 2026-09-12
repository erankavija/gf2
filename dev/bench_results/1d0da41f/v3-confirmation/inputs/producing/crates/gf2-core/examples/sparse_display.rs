use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};

fn main() {
    println!("=== SpBitMatrix Display ===");
    let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    let s = SpBitMatrix::from_coo(3, 4, &coo);
    println!("{}", s);

    println!("\n=== BitMatrix Display (for comparison) ===");
    let mut m = BitMatrix::zeros(3, 4);
    m.set(0, 0, true);
    m.set(0, 3, true);
    m.set(1, 1, true);
    m.set(2, 2, true);
    println!("{}", m);

    println!("\n=== SpBitMatrixDual Display ===");
    let sd = SpBitMatrixDual::from_coo(3, 4, &coo);
    println!("{}", sd);

    println!("\n=== Identity Matrix (4x4) ===");
    let id = SpBitMatrix::identity(4);
    println!("{}", id);

    println!("\n=== Empty Matrix ===");
    let empty = SpBitMatrix::zeros(0, 0);
    println!("{}", empty);

    println!("\n=== Zero Matrix (2x3) ===");
    let zeros = SpBitMatrix::zeros(2, 3);
    println!("{}", zeros);
}
