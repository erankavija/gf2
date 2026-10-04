//! Verifies the `FieldMatrix<Gf2mWide<1, _>>` product over GF(2^32) at n = 16
//! against an independent scalar schoolbook reference. Each element is a
//! polynomial of degree < 32 stored in the low 32 bits of the `u64` word.

use gf2_core::bench_seed::splitmix64;
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

/// Low 32 bits of the GF(2^32) Conway polynomial (`@/citation/Lubeck2024`);
/// the leading 1 at bit 32 is implicit.
const CONWAY_LOW32: u32 = 0x0000_8299;

/// GF(2^32) under the Conway polynomial.
struct Gf2m32ConwayCfg;
impl Gf2mWideConfig<1> for Gf2m32ConwayCfg {
    const M: usize = 32;
    const MODULUS: [u64; 1] = [CONWAY_LOW32 as u64];
    const NAME: &'static str = "Gf2m32Conway";
}

/// Scalar shift-and-reduce GF(2^32) multiply from the bits of `CONWAY_LOW32`,
/// independent of `Gf2mWide` arithmetic.
fn ref_gf2pow32_mul(a: u32, b: u32) -> u32 {
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
            lhs ^= CONWAY_LOW32;
        }
        rhs >>= 1;
    }
    result
}

/// An n×n row-major matrix of SplitMix64-derived elements, one step per
/// element.
fn fill_uniform_u32(n: usize, seed: u64) -> Vec<u32> {
    let mut out = vec![0u32; n * n];
    let mut st = seed;
    for slot in out.iter_mut() {
        let draw = splitmix64(&mut st);
        *slot = (draw & 0xFFFF_FFFF) as u32;
    }
    out
}

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
    let db = PrimitivePolynomialDatabase::standard(32).expect("m=32 entry exists");
    assert_eq!(
        db, 0x1_0000_8299,
        "database returned wrong Conway polynomial — primitive_polys.rs drift"
    );
    assert_eq!(Gf2m32ConwayCfg::MODULUS[0], db & 0xFFFF_FFFF);
}

#[test]
fn test_gf2pow32_fieldmatrix_gemm_matches_scalar_reference() {
    let n: usize = 16;
    // Seeds match the `gf2pow32_smoke_emit_expected` example.
    use gf2_core::bench_seed::derive_seed;
    const K_MASTER: u64 = 0x6F73AC91D31E4A7Cu64;
    const PHI: u64 = 0x9E3779B97F4A7C15u64;
    let a_seed: u64 = derive_seed(K_MASTER, "matmul", 0, 0, 0) ^ (32u64).wrapping_mul(PHI);
    let b_seed: u64 = a_seed ^ 0x1111_1111_1111_1111u64;

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
    // Fixed vectors, so a logic error in `ref_gf2pow32_mul` cannot agree with
    // the same error in `Gf2mWide` multiplication.
    assert_eq!(ref_gf2pow32_mul(1, 0xDEAD_BEEF), 0xDEAD_BEEF);
    assert_eq!(ref_gf2pow32_mul(0xDEAD_BEEF, 1), 0xDEAD_BEEF);
    assert_eq!(ref_gf2pow32_mul(0, 0xCAFE_BABE), 0);
    let x: u32 = 0x0000_0002;
    let x_squared = ref_gf2pow32_mul(x, x);
    // x^2 fits in 32 bits (degree 2), so no reduction needed.
    assert_eq!(x_squared, 0x0000_0004);
    let a: u32 = 0x1234_5678;
    let b: u32 = 0x9ABC_DEF0;
    assert_eq!(ref_gf2pow32_mul(a, b), ref_gf2pow32_mul(b, a));
}
