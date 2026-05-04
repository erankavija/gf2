//! Integration test verifying gf2-core's `FieldMatrix<Gf2mWide<1, _>>` matmul
//! over GF(2^32) at n=16 against an independent scalar schoolbook reference.
//!
//! This Rust-side test pairs with the C++ `benchmarks/reference/ntl_gf2pow32_smoke.cpp`
//! oracle: both consume the same `gf2_bench_splitmix64` byte stream at the
//! same seed and both compare against an independent scalar reference defined
//! purely from the Conway-polynomial bits in
//! `crates/gf2-core/src/primitive_polys.rs::standard(32)`. If gf2-core's
//! `Gf2mWide<1, _>` multiplication, NTL's `mat_GF2E::mul`, and the scalar
//! reference agree at n=16, the protocol § 6 bitwise-equality contract is
//! satisfied transitively for the GF(2^32) matmul promotion (jit:b13799ac).
//!
//! Element encoding follows the byte-level protocol documented in
//! `ntl_gf2pow32_smoke.cpp`: each GF(2^32) element is a polynomial of degree
//! < 32 stored as a little-endian `u32`. `Gf2mWide<1, Cfg>::new([word])`
//! consumes the polynomial bits in the low 32 bits of the `u64` word.

use gf2_core::bench_seed::splitmix64;
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

/// GF(2^32) configuration using the Conway polynomial. The MODULUS holds
/// the low 32 bits of the polynomial; bit 32 is implicit and equal to 1
/// (per the `Gf2mWideConfig` contract). The Conway polynomial bits are
/// `0x1_0000_8299` (database) → low 32 bits `0x0000_8299`.
struct Gf2m32ConwayCfg;
impl Gf2mWideConfig<1> for Gf2m32ConwayCfg {
    const M: usize = 32;
    const MODULUS: [u64; 1] = [0x0000_8299];
    const NAME: &'static str = "Gf2m32Conway";
}

/// Scalar schoolbook GF(2^32) multiply, shift-and-reduce. Uses only the
/// polynomial bits below — independent of `Gf2mWide` arithmetic — to act
/// as the bitwise reference oracle for the matmul cross-check.
fn ref_gf2pow32_mul(a: u32, b: u32) -> u32 {
    // Low 32 bits of the Conway polynomial; the implicit leading 1 at bit 32
    // is the reduction trigger and not included in the XOR mask.
    const REDUCE_POLY: u32 = 0x0000_8299;
    let mut result: u32 = 0;
    let mut lhs = a;
    let mut rhs = b;
    for _ in 0..32 {
        if rhs & 1 != 0 {
            result ^= lhs;
        }
        let carry = lhs >> 31;
        lhs = lhs.wrapping_shl(1);
        if carry != 0 {
            lhs ^= REDUCE_POLY;
        }
        rhs >>= 1;
    }
    result
}

/// Fill an n×n row-major `Vec<u32>` with deterministic `splitmix64`-derived
/// GF(2^32) elements. Each element consumes one full SplitMix64 step. This
/// mirrors the `fill_uniform_u32` helper in `ntl_gf2pow32_smoke.cpp` so the
/// two harnesses see byte-identical inputs at the same master seed.
fn fill_uniform_u32(n: usize, seed: u64) -> Vec<u32> {
    let mut out = vec![0u32; n * n];
    let mut st = seed;
    for slot in out.iter_mut() {
        let draw = splitmix64(&mut st);
        *slot = (draw & 0xFFFF_FFFF) as u32;
    }
    out
}

/// Independent scalar reference n×n matmul. XOR is GF(2^32) addition.
fn scalar_matmul(a: &[u32], b: &[u32], n: usize) -> Vec<u32> {
    let mut c = vec![0u32; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut acc: u32 = 0;
            for k in 0..n {
                acc ^= ref_gf2pow32_mul(a[i * n + k], b[k * n + j]);
            }
            c[i * n + j] = acc;
        }
    }
    c
}

fn fieldmatrix_from_u32_slice(src: &[u32], n: usize) -> FieldMatrix<Gf2mWide<1, Gf2m32ConwayCfg>> {
    let mut m = FieldMatrix::<Gf2mWide<1, Gf2m32ConwayCfg>>::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            let elem = Gf2mWide::<1, Gf2m32ConwayCfg>::new([src[i * n + j] as u64]);
            m.set(i, j, elem);
        }
    }
    m
}

#[test]
fn test_gf2pow32_conway_constant_matches_database() {
    // Sanity check: the database returns the bits the test config relies on.
    let db = PrimitivePolynomialDatabase::standard(32).expect("m=32 entry exists");
    assert_eq!(
        db, 0x1_0000_8299,
        "database returned wrong Conway polynomial — primitive_polys.rs drift"
    );
    // The MODULUS constant in `Gf2m32ConwayCfg` must be the low 32 bits of
    // the database value (the leading bit is implicit per the
    // `Gf2mWideConfig` contract).
    assert_eq!(Gf2m32ConwayCfg::MODULUS[0], db & 0xFFFF_FFFF);
}

#[test]
fn test_gf2pow32_fieldmatrix_gemm_matches_scalar_reference() {
    // n=16 mirrors the protocol § 6 smoke contract; the C++ harness uses
    // the same size for the NTL `mat_GF2E` cross-check.
    let n: usize = 16;
    // Master seed and tag derivation match `ntl_gf2pow32_smoke.cpp` so a
    // future merged-stream comparison can be done byte-for-byte. We use
    // simple constants here rather than `derive_seed` because the C++
    // harness already exercises the `derive_seed` path; the gf2-core test
    // only needs to verify that gf2-core's matmul agrees with the scalar
    // reference on whatever input we hand it.
    let a_seed: u64 = 0x0123_4567_89AB_CDEF;
    let b_seed: u64 = 0xFEDC_BA98_7654_3210;

    let a_bytes = fill_uniform_u32(n, a_seed);
    let b_bytes = fill_uniform_u32(n, b_seed);

    let a = fieldmatrix_from_u32_slice(&a_bytes, n);
    let b = fieldmatrix_from_u32_slice(&b_bytes, n);
    let c = gemm(&a, &b);

    let c_ref = scalar_matmul(&a_bytes, &b_bytes, n);

    for i in 0..n {
        for j in 0..n {
            let got = c.get(i, j);
            let got_word = got.words()[0] as u32;
            let want = c_ref[i * n + j];
            assert_eq!(
                got_word, want,
                "GF(2^32) matmul mismatch at ({i}, {j}): \
                 gf2_core=0x{got_word:08x} ref=0x{want:08x}"
            );
        }
    }
}

#[test]
fn test_gf2pow32_ref_mul_self_check_known_vectors() {
    // Spot-check the scalar reference on a handful of small vectors so a
    // wholesale logic error in `ref_gf2pow32_mul` does not silently agree
    // with a wholesale logic error in `Gf2mWide`'s multiplication.
    //
    // 1 * x = x for any x.
    assert_eq!(ref_gf2pow32_mul(1, 0xDEAD_BEEF), 0xDEAD_BEEF);
    assert_eq!(ref_gf2pow32_mul(0xDEAD_BEEF, 1), 0xDEAD_BEEF);
    // 0 * x = 0.
    assert_eq!(ref_gf2pow32_mul(0, 0xCAFE_BABE), 0);
    // x * x squaring: well-defined and commutative.
    let x: u32 = 0x0000_0002; // The element `x` itself.
    let x_squared = ref_gf2pow32_mul(x, x);
    // x^2 fits in 32 bits (degree 2), so no reduction needed.
    assert_eq!(x_squared, 0x0000_0004);
    // Commutativity check.
    let a: u32 = 0x1234_5678;
    let b: u32 = 0x9ABC_DEF0;
    assert_eq!(ref_gf2pow32_mul(a, b), ref_gf2pow32_mul(b, a));
}
