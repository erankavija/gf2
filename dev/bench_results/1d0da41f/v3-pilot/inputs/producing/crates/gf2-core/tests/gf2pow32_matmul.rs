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

/// Low 32 bits of the GF(2^32) Conway polynomial — the in-file SSOT for
/// this test module. Both `Gf2m32ConwayCfg::MODULUS[0]` and the scalar
/// reference multiplier below dereference this single constant so a
/// drift can only happen at one point. The cross-language SSOT against
/// `crates/gf2-core/src/primitive_polys.rs::standard(32)` and the C++
/// `benchmarks/reference/gf2pow32_constants.h::kGf2coreConwayM32` is
/// enforced by `tests/gf2pow32_constant_drift.rs` (which parses the
/// header at test time and `PrimitivePolynomialDatabase::standard(32)`
/// at runtime, and asserts both equal `(1u64 << 32) | u64(CONWAY_LOW32)`).
///
/// The implicit leading 1 at bit 32 is the reduction trigger and is not
/// part of this 32-bit mask.
const CONWAY_LOW32: u32 = 0x0000_8299;

/// GF(2^32) configuration using the Conway polynomial. The MODULUS holds
/// the low 32 bits of the polynomial; bit 32 is implicit and equal to 1
/// per the `Gf2mWideConfig` contract.
struct Gf2m32ConwayCfg;
impl Gf2mWideConfig<1> for Gf2m32ConwayCfg {
    const M: usize = 32;
    const MODULUS: [u64; 1] = [CONWAY_LOW32 as u64];
    const NAME: &'static str = "Gf2m32Conway";
}

/// Scalar schoolbook GF(2^32) multiply, shift-and-reduce. Uses only the
/// polynomial bits in `CONWAY_LOW32` — independent of `Gf2mWide`
/// arithmetic — to act as the in-Rust gf2-core ↔ scalar witness against
/// `Gf2mWide<1, _>::mul`. (The protocol § 6 candidate-vs-gf2-core direct
/// smoke at the C++ side is `benchmarks/reference/ntl_gf2pow32_smoke.cpp`,
/// which loads the gf2-core ground-truth file emitted by the
/// `gf2pow32_smoke_emit_expected` Cargo example.)
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
    // Master seed and tag derivation match `ntl_gf2pow32_smoke.cpp::main`
    // so the two harnesses build BYTE-IDENTICAL input matrices. The C++
    // smoke uses
    //   a_seed = gf2_bench_derive_seed(kMaster, "matmul", 0, 0, 0)
    //          ^ ((uint64_t)32) * 0x9E3779B97F4A7C15ULL
    //   b_seed = a_seed ^ 0x1111111111111111ULL
    // The Rust side mirrors this exactly via gf2_core::bench_seed::derive_seed.
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
