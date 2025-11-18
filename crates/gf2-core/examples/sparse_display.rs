use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SparseMatrix, SparseMatrixDual};

fn main() {
    println!("=== SparseMatrix Display ===");
    let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    let s = SparseMatrix::from_coo(3, 4, &coo);
    println!("{}", s);

    println!("\n=== BitMatrix Display (for comparison) ===");
    let mut m = BitMatrix::zeros(3, 4);
    m.set(0, 0, true);
    m.set(0, 3, true);
    m.set(1, 1, true);
    m.set(2, 2, true);
    println!("{}", m);

    println!("\n=== SparseMatrixDual Display ===");
    let sd = SparseMatrixDual::from_coo(3, 4, &coo);
    println!("{}", sd);

    println!("\n=== Identity Matrix (4x4) ===");
    let id = SparseMatrix::identity(4);
    println!("{}", id);

    println!("\n=== Empty Matrix ===");
    let empty = SparseMatrix::zeros(0, 0);
    println!("{}", empty);

    println!("\n=== Zero Matrix (2x3) ===");
    let zeros = SparseMatrix::zeros(2, 3);
    println!("{}", zeros);
}
