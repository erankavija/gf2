//! gf2's arm of the GF(2^8) comparison: the default byte-region routes on the
//! cells and inputs `ext/arm.c` shares, one line per cell.

use gf2_core::gf2m::Gf2mField;
use std::hint::black_box;
use std::time::Instant;

/// Timed batches per cell after one untimed batch.
const BATCHES: usize = 5;
const AXPY_LENGTHS: [usize; 4] = [4 << 10, 64 << 10, 1 << 20, 16 << 20];
const GEMM_SIZES: [usize; 3] = [64, 256, 1024];

/// The splitmix64 stream both arms draw their inputs from.
struct SplitMix(u64);

impl SplitMix {
    fn byte(&mut self) -> u8 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 56) as u8
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }
}

/// FNV-1a over the output, the identity both arms print.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Repetitions per batch, shared with `ext/arm.c`.
fn axpy_reps(len: usize) -> usize {
    ((1usize << 28) / len).max(1)
}

fn gemm_reps(n: usize) -> usize {
    match n {
        64 => 4096,
        256 => 64,
        _ => 2,
    }
}

/// Best batch time in seconds of `BATCHES` timed batches after one untimed.
fn best(mut batch: impl FnMut()) -> f64 {
    batch();
    (0..BATCHES)
        .map(|_| {
            let start = Instant::now();
            batch();
            start.elapsed().as_secs_f64()
        })
        .fold(f64::INFINITY, f64::min)
}

fn main() {
    let field = Gf2mField::gf256();
    for len in AXPY_LENGTHS {
        let mut rng = SplitMix(len as u64);
        let c = rng.byte();
        let x = rng.bytes(len);
        let mut y = rng.bytes(len);
        let reps = axpy_reps(len);
        let seconds = best(|| {
            for _ in 0..reps {
                field.gf256_axpy(c, black_box(&x), &mut y);
            }
        });
        println!(
            "axpy\t{len}\tgf2 Gf2mField::gf256_axpy\t{:.2} GB/s\t{:016x}",
            (len * reps) as f64 / seconds / 1e9,
            checksum(&y)
        );
    }
    for n in GEMM_SIZES {
        let mut rng = SplitMix(n as u64 ^ 0x6e6d);
        let a = rng.bytes(n * n);
        let b = rng.bytes(n * n);
        let mut c = vec![0u8; n * n];
        let reps = gemm_reps(n);
        let seconds = best(|| {
            for _ in 0..reps {
                field.gf256_gemm(n, n, n, black_box(&a), &b, &mut c);
            }
        }) / reps as f64;
        println!(
            "gemm\t{n}\tgf2 Gf2mField::gf256_gemm\t{:.3} ms {:.2} Gmul-add/s\t{:016x}",
            seconds * 1e3,
            (n * n * n) as f64 / seconds / 1e9,
            checksum(&c)
        );
    }
}
