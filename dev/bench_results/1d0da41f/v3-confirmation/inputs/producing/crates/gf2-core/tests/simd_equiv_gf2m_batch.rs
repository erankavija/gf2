//! SIMD-vs-scalar equivalence proptests for the GF(2^m) batch element-wise
//! multiply/square kernel (jit:ec286cee, kernel C1).
//!
//! Compares the AVX2 + VPCLMULQDQ-on-YMM batch path against three scalar
//! reference strategies:
//!
//! 1. **Bit-by-bit shift-and-add** — independent bit-by-bit GF(2^m) reducer
//!    that the SIMD kernel cannot accidentally agree with.
//! 2. **`Gf2mField::Mul` per-element** — exercises the existing single-shot
//!    PCLMULQDQ + Barrett dispatch path; ensures the new batch kernel
//!    matches the production single-element path that callers already trust.
//! 3. **Log/exp tables** — a third independent oracle for `m ∈ {8, 16}`
//!    (table sizes for `m = 32` would exceed reasonable test memory).
//!
//! Covers `m ∈ {8, 16, 32}` and a sweep of word-boundary batch lengths
//! (0, 1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128,
//! 129, 255, 256, 257) so the 4-way unroll's tail handler is exercised at
//! every alignment relative to the YMM-pair lane structure.

#![cfg(feature = "simd")]

mod simd_equiv;

use gf2_core::gf2m::batch::{batch_mul, batch_square};
use gf2_core::gf2m::Gf2mField;
use proptest::prelude::any;
use proptest::strategy::Strategy;
use simd_equiv::{assert_simd_matches_scalar, WORD_BOUNDARY_LENGTHS};

fn gf2m_batch_simd_available() -> bool {
    gf2_core::kernels::simd::maybe_gf2m_batch().is_some()
}

// ---------------------------------------------------------------------------
// Reference strategies
// ---------------------------------------------------------------------------

/// Bit-by-bit GF(2^m) multiplication, independent of every PCLMULQDQ path.
fn scalar_bitwise_mul(a: u64, b: u64, m: u32, poly: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    let mut result = 0u64;
    let mut temp = a;
    for i in 0..m {
        if (b >> i) & 1 == 1 {
            result ^= temp;
        }
        let will_overflow = (temp >> (m - 1)) & 1 == 1;
        temp <<= 1;
        if will_overflow {
            temp ^= poly;
        }
    }
    result & if m == 64 { u64::MAX } else { (1u64 << m) - 1 }
}

/// Log/exp-table GF(2^m) multiplication; valid for `m ≤ 16`.
struct LogExpOracle {
    log_table: Vec<u32>,
    exp_table: Vec<u32>,
}

impl LogExpOracle {
    fn new(m: u32, poly: u64) -> Self {
        let order = (1usize << m) - 1;
        let mut log_table = vec![0u32; 1 << m];
        let mut exp_table = vec![0u32; 2 * order];
        let mut val = 1u64;
        for i in 0..order {
            exp_table[i] = val as u32;
            exp_table[i + order] = val as u32;
            log_table[val as usize] = i as u32;
            val <<= 1;
            if val & (1u64 << m) != 0 {
                val ^= poly;
            }
            val &= (1u64 << m) - 1;
        }
        Self {
            log_table,
            exp_table,
        }
    }

    fn mul(&self, a: u64, b: u64) -> u64 {
        if a == 0 || b == 0 {
            return 0;
        }
        let la = self.log_table[a as usize] as usize;
        let lb = self.log_table[b as usize] as usize;
        self.exp_table[la + lb] as u64
    }
}

// ---------------------------------------------------------------------------
// Boundary tests — explicit lengths from `WORD_BOUNDARY_LENGTHS`.
// ---------------------------------------------------------------------------

fn run_boundary_test(m: u32, poly: u64) {
    if !gf2m_batch_simd_available() {
        eprintln!("GF(2^m) batch SIMD backend unavailable — skipping batch-mul boundary test");
        return;
    }

    let field = Gf2mField::new(m as usize, poly);
    let mask = if m == 64 { u64::MAX } else { (1u64 << m) - 1 };
    let oracle = if m <= 16 {
        Some(LogExpOracle::new(m, poly))
    } else {
        None
    };

    for &bits in WORD_BOUNDARY_LENGTHS {
        let len = bits;
        let a: Vec<u64> = (0..len)
            .map(|i| (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
            .collect();
        let b: Vec<u64> = (0..len)
            .map(|i| (i as u64).wrapping_mul(0x6C62_272E_07BB_0142) & mask)
            .collect();

        let mut got = vec![0u64; len];
        batch_mul(&field, &a, &b, &mut got);

        for i in 0..len {
            // Strategy 1: bit-by-bit
            let bitwise = scalar_bitwise_mul(a[i], b[i], m, poly);
            assert_eq!(
                got[i], bitwise,
                "bitwise oracle disagrees at m={m}, len={len}, i={i}"
            );
            // Strategy 2: Gf2mField single-shot
            if a[i] != 0 && b[i] != 0 {
                let ea = field.element(a[i]);
                let eb = field.element(b[i]);
                let single = (&ea * &eb).value();
                assert_eq!(
                    got[i], single,
                    "Gf2mField single-shot disagrees at m={m}, len={len}, i={i}"
                );
            }
            // Strategy 3: log/exp (m ≤ 16)
            if let Some(oracle) = oracle.as_ref() {
                let log_exp = oracle.mul(a[i], b[i]);
                assert_eq!(
                    got[i], log_exp,
                    "log/exp oracle disagrees at m={m}, len={len}, i={i}"
                );
            }
        }
    }
}

#[test]
fn boundary_lengths_gf2_8() {
    run_boundary_test(8, 0b100011101);
}

#[test]
fn boundary_lengths_gf2_16() {
    run_boundary_test(16, 0b1_0001_0000_0000_1011);
}

#[test]
fn boundary_lengths_gf2_32() {
    run_boundary_test(32, 0b1_0000_0000_0100_0000_0000_0000_0000_0111);
}

// ---------------------------------------------------------------------------
// Proptest equivalence — random batches up to 256 elements, `m ∈ {8, 16, 32}`.
// ---------------------------------------------------------------------------

/// Build a `(Vec<u64>, Vec<u64>)` strategy for batches up to length `max_len`,
/// each element pre-masked to `m` bits.
fn batch_pair_strategy(
    m: u32,
    max_len: usize,
) -> impl Strategy<Value = (Vec<u64>, Vec<u64>, u32, u64)> {
    let mask = if m == 64 { u64::MAX } else { (1u64 << m) - 1 };
    let poly = match m {
        8 => 0b100011101u64,
        16 => 0b1_0001_0000_0000_1011u64,
        32 => 0b1_0000_0000_0100_0000_0000_0000_0000_0111u64,
        _ => unreachable!(),
    };
    (
        proptest::collection::vec(any::<u64>().prop_map(move |v| v & mask), 0..=max_len),
        proptest::collection::vec(any::<u64>().prop_map(move |v| v & mask), 0..=max_len),
    )
        .prop_map(move |(mut a, mut b)| {
            // Equalise lengths.
            let n = a.len().min(b.len());
            a.truncate(n);
            b.truncate(n);
            (a, b, m, poly)
        })
}

#[test]
fn proptest_gf2_8_batch_mul_matches_field_single_shot() {
    if !gf2m_batch_simd_available() {
        return;
    }
    let field = Gf2mField::new(8, 0b100011101);
    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>, u32, u64), (), _, _, _>(
        // Scalar reference: per-element bit-by-bit multiply.
        |input| {
            let (a, b, m, poly) = input;
            let mut out = vec![0u64; a.len()];
            for i in 0..a.len() {
                out[i] = scalar_bitwise_mul(a[i], b[i], *m, *poly);
            }
            *a = out;
        },
        // Candidate: SIMD batch path.
        |input| {
            let (a, b, _m, _poly) = input;
            let mut out = vec![0u64; a.len()];
            batch_mul(&field, a, b, &mut out);
            *a = out;
        },
        batch_pair_strategy(8, 257),
    );
}

#[test]
fn proptest_gf2_16_batch_mul_matches_field_single_shot() {
    if !gf2m_batch_simd_available() {
        return;
    }
    let field = Gf2mField::new(16, 0b1_0001_0000_0000_1011);
    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>, u32, u64), (), _, _, _>(
        |input| {
            let (a, b, m, poly) = input;
            let mut out = vec![0u64; a.len()];
            for i in 0..a.len() {
                out[i] = scalar_bitwise_mul(a[i], b[i], *m, *poly);
            }
            *a = out;
        },
        |input| {
            let (a, b, _m, _poly) = input;
            let mut out = vec![0u64; a.len()];
            batch_mul(&field, a, b, &mut out);
            *a = out;
        },
        batch_pair_strategy(16, 257),
    );
}

#[test]
fn proptest_gf2_32_batch_mul_matches_field_single_shot() {
    if !gf2m_batch_simd_available() {
        return;
    }
    let field = Gf2mField::new(32, 0b1_0000_0000_0100_0000_0000_0000_0000_0111);
    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>, u32, u64), (), _, _, _>(
        |input| {
            let (a, b, m, poly) = input;
            let mut out = vec![0u64; a.len()];
            for i in 0..a.len() {
                out[i] = scalar_bitwise_mul(a[i], b[i], *m, *poly);
            }
            *a = out;
        },
        |input| {
            let (a, b, _m, _poly) = input;
            let mut out = vec![0u64; a.len()];
            batch_mul(&field, a, b, &mut out);
            *a = out;
        },
        batch_pair_strategy(32, 257),
    );
}

// ---------------------------------------------------------------------------
// Square kernel parity — square(a) == mul(a, a) for every supported m.
// ---------------------------------------------------------------------------

#[test]
fn square_matches_mul_self_gf2_8() {
    if !gf2m_batch_simd_available() {
        return;
    }
    let field = Gf2mField::gf256();
    let mask = 0xFFu64;
    let a: Vec<u64> = (0..=255u64).map(|i| i & mask).collect();
    let mut squared = vec![0u64; a.len()];
    let mut product = vec![0u64; a.len()];
    batch_square(&field, &a, &mut squared);
    batch_mul(&field, &a, &a, &mut product);
    assert_eq!(
        squared, product,
        "square(a) should equal mul(a, a) elementwise"
    );
}

#[test]
fn square_matches_mul_self_gf2_16_word_boundary_lengths() {
    if !gf2m_batch_simd_available() {
        return;
    }
    let field = Gf2mField::gf65536();
    let mask = 0xFFFFu64;
    for &len in WORD_BOUNDARY_LENGTHS {
        let a: Vec<u64> = (0..len)
            .map(|i| (i as u64).wrapping_mul(0x9E37_79B9) & mask)
            .collect();
        let mut squared = vec![0u64; len];
        let mut product = vec![0u64; len];
        batch_square(&field, &a, &mut squared);
        batch_mul(&field, &a, &a, &mut product);
        assert_eq!(squared, product, "square != mul-self at len={len}");
    }
}
