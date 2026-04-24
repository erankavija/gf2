//! Single-shot wall-clock measurement of classical and Strassen-Winograd
//! `gemm` at `n = 4096` over GF(2^8) (`Gf2mWide<1, AES>`). Output is one
//! table row in the same format as
//! `benches/strassen_threshold_results.md` (line 80+), matching the
//! single-shot `std::time::Instant` methodology described in the file.
//!
//! Usage:
//!
//! ```bash
//! cargo run -p gf2-core --release --example strassen_gf2m8_4096 \
//!   --features rand
//! ```

use std::time::Instant;

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::winograd::gemm_winograd;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

struct StrassenGf2m8Cfg;
impl Gf2mWideConfig<1> for StrassenGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "StrassenGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, StrassenGf2m8Cfg>;

fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
        }
    }
    m
}

fn main() {
    let n = 4096;
    eprintln!("building {n}x{n} random GF(2^8) matrices...");
    let a = random_gf2m8(n, n, 0xCC ^ n as u64);
    let b = random_gf2m8(n, n, 0xDD ^ n as u64);

    eprintln!("running classical gemm (single shot)...");
    let t0 = Instant::now();
    let c_cl = gemm(&a, &b);
    let classical_ms = t0.elapsed().as_secs_f64() * 1000.0;
    std::hint::black_box(c_cl);

    eprintln!("running Strassen-Winograd gemm (single shot)...");
    let t1 = Instant::now();
    let c_wg = gemm_winograd(&a, &b);
    let winograd_ms = t1.elapsed().as_secs_f64() * 1000.0;
    std::hint::black_box(c_wg);

    let speedup = classical_ms / winograd_ms;
    println!("| {n:<5} | {classical_ms:>14.2} | {winograd_ms:>13.2} | {speedup:.3}x  |");
    println!(
        "\nclassical_ms={classical_ms:.2}  winograd_ms={winograd_ms:.2}  speedup={speedup:.3}x"
    );
}
