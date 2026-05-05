//! GF(2^32) matmul smoke ground-truth emitter (`jit:b13799ac` R2).
//!
//! Companion to `benchmarks/reference/ntl_gf2pow32_smoke.cpp`. Walks the
//! single n=16 matmul cell the C++ smoke harness asserts against (a, b
//! seeded the same way the C++ harness seeds them via
//! `gf2_bench_derive_seed` + `splitmix64`), runs the **production**
//! gf2-core code path (`FieldMatrix<Gf2mWide<1, _>>::gemm` over the
//! Conway polynomial from
//! `crates/gf2-core/src/primitive_polys.rs::standard(32)`), and
//! serialises the input and the expected output to a binary file. The
//! C++ smoke loads that file at startup and asserts byte-equality
//! between (a) the input it builds locally and the input bytes recorded
//! here (L1 — seed-walk equivalence) and (b) NTL's `mat_GF2E::mul`
//! output and the gf2-core ground-truth output (L2 — direct
//! `reference ↔ gf2-core` equality per protocol § 6 criterion-3).
//!
//! Closes the criterion-3 contract that the prior transitive smoke
//! (NTL ↔ scalar + gf2-core ↔ scalar) did not literally satisfy.
//!
//! # File format (little-endian)
//!
//! ```text
//! magic   : 8 bytes ASCII "GF2P32M0"
//! n       : u32                  (matrix dimension; this emitter writes 16)
//! a_seed  : u64                  (master-derived seed for matrix A)
//! b_seed  : u64                  (master-derived seed for matrix B)
//! conway  : u64                  (full Conway polynomial bits including bit 32)
//! a_bytes : 4 * n * n bytes      (row-major u32 LE for A)
//! b_bytes : 4 * n * n bytes      (row-major u32 LE for B)
//! c_bytes : 4 * n * n bytes      (row-major u32 LE for C = A * B)
//! ```
//!
//! Element encoding follows the byte-level protocol in
//! `gf2pow32_constants.h`: each GF(2^32) element is a polynomial of
//! degree < 32 stored as a little-endian `u32`. The C++ smoke unpacks
//! via `GF2XFromBytes(buf, 4)` (NTL's wire format).
//!
//! # Seed derivation
//!
//! Mirrors `ntl_gf2pow32_smoke.cpp::main`:
//!
//! ```text
//! a_seed = derive_seed(MASTER, "matmul", 0, 0, 0) ^ (32 * 0x9E3779B97F4A7C15)
//! b_seed = a_seed ^ 0x1111_1111_1111_1111
//! ```
//!
//! # Output
//!
//! `--output <path>` (default `benchmarks/expected/gf2pow32_smoke_n16.bin`).

use std::fs::File;
use std::io::{BufWriter, Write};

use gf2_core::bench_seed::{derive_seed, splitmix64};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

const MAGIC: &[u8; 8] = b"GF2P32M0";
const N: usize = 16;
const MASTER: u64 = 0x6F73AC91D31E4A7Cu64;
const PHI: u64 = 0x9E3779B97F4A7C15u64;
const B_SEED_SALT: u64 = 0x1111_1111_1111_1111u64;

/// Conway-32 config, mirroring `tests/gf2pow32_matmul.rs::Gf2m32ConwayCfg`.
/// `MODULUS[0]` holds the low 32 bits of the polynomial; bit 32 is implicit
/// and equal to 1 per the `Gf2mWideConfig` contract. The drift-check test
/// `tests/gf2pow32_constant_drift.rs` keeps the C++ header byte-equal to
/// `PrimitivePolynomialDatabase::standard(32)`; this config and the test's
/// config independently dereference the same Rust SSOT at compile time.
const CONWAY_LOW32: u32 = 0x0000_8299;

struct Gf2m32ConwayCfg;
impl Gf2mWideConfig<1> for Gf2m32ConwayCfg {
    const M: usize = 32;
    const MODULUS: [u64; 1] = [CONWAY_LOW32 as u64];
    const NAME: &'static str = "Gf2m32Conway";
}

fn fill_uniform_u32(n: usize, seed: u64) -> Vec<u32> {
    let mut out = vec![0u32; n * n];
    let mut st = seed;
    for slot in out.iter_mut() {
        let draw = splitmix64(&mut st);
        *slot = (draw & 0xFFFF_FFFF) as u32;
    }
    out
}

fn fieldmatrix_from_u32(src: &[u32], n: usize) -> FieldMatrix<Gf2mWide<1, Gf2m32ConwayCfg>> {
    let mut m = FieldMatrix::<Gf2mWide<1, Gf2m32ConwayCfg>>::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            let elem = Gf2mWide::<1, Gf2m32ConwayCfg>::new([src[i * n + j] as u64]);
            m.set(i, j, elem);
        }
    }
    m
}

fn matrix_to_u32(m: &FieldMatrix<Gf2mWide<1, Gf2m32ConwayCfg>>, n: usize) -> Vec<u32> {
    let mut out = vec![0u32; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = m.get(i, j).words()[0] as u32;
        }
    }
    out
}

fn write_u32_le_block<W: Write>(w: &mut W, data: &[u32]) -> std::io::Result<()> {
    for v in data {
        w.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}

fn parse_args() -> String {
    let mut args = std::env::args().skip(1);
    let mut out = String::from("benchmarks/expected/gf2pow32_smoke_n16.bin");
    while let Some(a) = args.next() {
        match a.as_str() {
            "--output" => {
                out = args.next().expect("--output requires a path argument");
            }
            other if other.starts_with("--") => {
                panic!("unknown flag: {other}");
            }
            _ => {}
        }
    }
    out
}

fn main() -> std::io::Result<()> {
    let path = parse_args();

    // Sanity: the database SSOT must match the embedded constant.
    let db = PrimitivePolynomialDatabase::standard(32)
        .expect("primitive_polys.rs::standard(32) must return Some — Rust SSOT");
    let db_low32 = (db & 0xFFFF_FFFF) as u32;
    assert_eq!(
        db_low32, CONWAY_LOW32,
        "Conway-32 SSOT drift: database low32=0x{db_low32:08x} embedded=0x{CONWAY_LOW32:08x}"
    );

    // Seed derivation mirrors `ntl_gf2pow32_smoke.cpp::main`:
    //   a_seed = gf2_bench_derive_seed(kMaster, "matmul", 0, 0, 0)
    //          ^ ((uint64_t)32 * 0x9E3779B97F4A7C15ULL)
    //   b_seed = a_seed ^ 0x1111_1111_1111_1111
    let a_seed = derive_seed(MASTER, "matmul", 0, 0, 0) ^ (32u64).wrapping_mul(PHI);
    let b_seed = a_seed ^ B_SEED_SALT;

    let a_bytes = fill_uniform_u32(N, a_seed);
    let b_bytes = fill_uniform_u32(N, b_seed);
    let a = fieldmatrix_from_u32(&a_bytes, N);
    let b = fieldmatrix_from_u32(&b_bytes, N);
    let c = gemm(&a, &b);
    let c_bytes = matrix_to_u32(&c, N);

    let f = File::create(&path)?;
    let mut w = BufWriter::new(f);
    w.write_all(MAGIC)?;
    w.write_all(&(N as u32).to_le_bytes())?;
    w.write_all(&a_seed.to_le_bytes())?;
    w.write_all(&b_seed.to_le_bytes())?;
    w.write_all(&db.to_le_bytes())?;
    write_u32_le_block(&mut w, &a_bytes)?;
    write_u32_le_block(&mut w, &b_bytes)?;
    write_u32_le_block(&mut w, &c_bytes)?;
    w.flush()?;

    eprintln!(
        "[gf2pow32_smoke_emit_expected] wrote {path} \
         (n={N}, a_seed=0x{a_seed:016x}, b_seed=0x{b_seed:016x}, conway=0x{db:x})"
    );
    Ok(())
}
