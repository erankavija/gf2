//! gf2's arm of the GF(2^8) comparison: the byte-region API and the generic
//! `FieldVec`/`FieldMatrix` API over one-byte `Gf256` elements, on the cells
//! and inputs `ext/arm.c` shares, one line per cell and route. With
//! `--digests <path>...` it prints the SHA-256 of itself and of each path
//! instead.

use experiments_common::Sha256Digest;
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf256, Gf2mField, Gf2mWideConfig};
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

/// Timed batches per cell after one untimed batch.
const BATCHES: usize = 5;
const AXPY_LENGTHS: [usize; 4] = [4 << 10, 64 << 10, 1 << 20, 16 << 20];
const GEMM_SIZES: [usize; 3] = [64, 256, 1024];

/// GF(2^8) under 0x11D, the modulus of `Gf2mField::gf256` and of both
/// references.
struct Poly11d;
impl Gf2mWideConfig<1> for Poly11d {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1d];
}
type Element = Gf256<Poly11d>;

fn elements(bytes: &[u8]) -> FieldVec<Element> {
    bytes.iter().map(|&b| Element::new(b)).collect()
}

fn matrix(n: usize, bytes: &[u8]) -> FieldMatrix<Element> {
    FieldMatrix::from_rows(bytes.chunks(n).map(elements).collect())
}

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

/// Prints `sha256 <digest> <path>` for this executable and for each path in
/// `others`: the identity of the measured arms and the libraries they load.
fn print_digests(others: impl Iterator<Item = String>) -> std::io::Result<()> {
    let own = std::env::current_exe()?;
    let own_digest = Sha256Digest::of_file(Path::new("/proc/self/exe"))?;
    println!("sha256 {own_digest} {}", own.display());
    for path in others {
        println!("sha256 {} {path}", Sha256Digest::of_file(Path::new(&path))?);
    }
    Ok(())
}

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--digests") {
        print_digests(args).expect("the executables are readable");
        return;
    }
    let field = Gf2mField::gf256();
    for len in AXPY_LENGTHS {
        let mut rng = SplitMix(len as u64);
        let c = rng.byte();
        let x = rng.bytes(len);
        let mut y = rng.bytes(len);
        // Two equal updates cancel, so the digest is of one update of the
        // drawn `y`, taken before the timed repetitions.
        let mut once = y.clone();
        field.gf256_axpy(c, &x, &mut once);
        let digest = checksum(&once);
        let reps = axpy_reps(len);
        let seconds = best(|| {
            for _ in 0..reps {
                field.gf256_axpy(c, black_box(&x), &mut y);
            }
        });
        println!(
            "axpy\t{len}\tgf2 Gf2mField::gf256_axpy\t{:.2} GB/s\t{digest:016x}",
            (len * reps) as f64 / seconds / 1e9,
        );

        let mut rng = SplitMix(len as u64);
        let c = Element::new(rng.byte());
        let x = elements(&rng.bytes(len));
        let mut y = elements(&rng.bytes(len));
        let mut once = y.clone();
        once.axpy(&c, &x);
        let once: Vec<u8> = once.iter().map(|e| e.value()).collect();
        let digest = checksum(&once);
        let seconds = best(|| {
            for _ in 0..reps {
                y.axpy(&c, black_box(&x));
            }
        });
        println!(
            "axpy\t{len}\tgf2 FieldVec<Gf256>::axpy\t{:.2} GB/s\t{digest:016x}",
            (len * reps) as f64 / seconds / 1e9,
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

        let (a, b) = (matrix(n, &a), matrix(n, &b));
        let mut product = FieldMatrix::new(n, n, Element::zero());
        let seconds = best(|| {
            for _ in 0..reps {
                product = gemm(black_box(&a), &b);
            }
        }) / reps as f64;
        let c: Vec<u8> = (0..n * n)
            .map(|i| product.get(i / n, i % n).value())
            .collect();
        println!(
            "gemm\t{n}\tgf2 FieldMatrix<Gf256> gemm\t{:.3} ms {:.2} Gmul-add/s\t{:016x}",
            seconds * 1e3,
            (n * n * n) as f64 / seconds / 1e9,
            checksum(&c)
        );
    }
}
