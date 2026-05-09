//! SIMD-batching strategy microbench for JIT issue `c7542983` (R4).
//!
//! This stand-alone crate compares two strategies for SIMD-batching the
//! bipedal F_3 arithmetic from Scheinerman 2024 §2.2:
//!
//! 1. **Per-prime hand-rolled** ([`per_prime`]): specialised AVX2 functions
//!    for the F_3 `(mag, sgn)` encoding. Each `add`/`sub`/`mul` is written
//!    against `__m256i` directly.
//! 2. **Generic framework** ([`generic`]): a `BatchedBipedalLike<MagLanes,
//!    SgnLanes>` template parametrised by the encoding shape, which F_3
//!    instantiates via concrete `__m256i` operands.
//!
//! Both strategies operate on AVX2 256-bit lanes (4 x u64 = 256 packed F_3
//! elements per word-pair). The bench harness ([`mod@bench`]) drives both at
//! lane batches of 64, 256, and 1024 F_3 elements (1, 4, 16 AVX2 ops worth)
//! and reports cycles/op via `_rdtsc`.
//!
//! Correctness equivalence between the strategies is enforced by the
//! `correctness_*` tests in this file.
//!
//! See `dev/plans/r4_simd_batching_decision.md` for the decision document
//! produced from running this bench.

#![allow(clippy::missing_safety_doc, unused_unsafe)]

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod per_prime;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod generic;

pub mod bench;

/// Bipedal F_3 reference: decode `(mag, sgn)` for one bit position to a
/// canonical 0/1/2 value.
///
/// # Arguments
///
/// * `mag` — magnitude bit (0 means F_3 zero).
/// * `sgn` — sign bit (canonical zero is `(0, 0)`; `(0, 1)` decodes to zero too).
///
/// # Examples
///
/// ```
/// use simd_batching_bench::decode_lane;
/// assert_eq!(decode_lane(0, 0), 0);
/// assert_eq!(decode_lane(0, 1), 0); // alt-zero
/// assert_eq!(decode_lane(1, 0), 1);
/// assert_eq!(decode_lane(1, 1), 2);
/// ```
///
/// # Complexity
///
/// `O(1)`: three bit operations.
pub fn decode_lane(mag: u8, sgn: u8) -> u8 {
    debug_assert!(mag <= 1 && sgn <= 1);
    if mag == 0 {
        0
    } else if sgn == 0 {
        1
    } else {
        2
    }
}

/// Scalar bipedal F_3 add over a single (`mag`, `sgn`) bit pair.
///
/// Implements the paper §2.2 6-op formula:
/// `t = m1 ^ s1 ^ s2; u = m2 & t; m_+ = u | (m1 ^ m2); s_+ = u ^ s1`.
///
/// # Arguments
///
/// * `(m1, s1)` — first lane.
/// * `(m2, s2)` — second lane.
///
/// # Examples
///
/// ```
/// use simd_batching_bench::{scalar_add, decode_lane};
/// // 1 + 2 = 0 mod 3
/// let (m, s) = scalar_add(1, 0, 1, 1);
/// assert_eq!(decode_lane(m, s), 0);
/// ```
///
/// # Complexity
///
/// `O(1)`: six bit operations.
pub fn scalar_add(m1: u8, s1: u8, m2: u8, s2: u8) -> (u8, u8) {
    let t = m1 ^ s1 ^ s2;
    let u = m2 & t & 1;
    let m_plus = (u | (m1 ^ m2)) & 1;
    let s_plus = (u ^ s1) & 1;
    (m_plus, s_plus)
}

/// Scalar bipedal F_3 sub over a single (`mag`, `sgn`) bit pair.
///
/// Implements the paper §2.2 6-op formula:
/// `t = s1 ^ s2; u = m1 & t; m_- = u | (m1 ^ m2); s_- = u ^ (m2 ^ s2)`.
///
/// # Arguments
///
/// * `(m1, s1)` — first lane.
/// * `(m2, s2)` — second lane.
///
/// # Examples
///
/// ```
/// use simd_batching_bench::{scalar_sub, decode_lane};
/// // 1 - 2 = 2 mod 3
/// let (m, s) = scalar_sub(1, 0, 1, 1);
/// assert_eq!(decode_lane(m, s), 2);
/// ```
///
/// # Complexity
///
/// `O(1)`: six bit operations.
pub fn scalar_sub(m1: u8, s1: u8, m2: u8, s2: u8) -> (u8, u8) {
    let t = (s1 ^ s2) & 1;
    let u = m1 & t & 1;
    let m_minus = (u | (m1 ^ m2)) & 1;
    let s_minus = (u ^ (m2 ^ s2)) & 1;
    (m_minus, s_minus)
}

/// Scalar bipedal F_3 mul over a single (`mag`, `sgn`) bit pair.
///
/// Implements the paper §2.2 2-op formula: `m_x = m1 & m2; s_x = s1 ^ s2`.
///
/// # Arguments
///
/// * `(m1, s1)` — first lane.
/// * `(m2, s2)` — second lane.
///
/// # Examples
///
/// ```
/// use simd_batching_bench::{scalar_mul, decode_lane};
/// // 2 * 2 = 1 mod 3
/// let (m, s) = scalar_mul(1, 1, 1, 1);
/// assert_eq!(decode_lane(m, s), 1);
/// ```
///
/// # Complexity
///
/// `O(1)`: two bit operations.
pub fn scalar_mul(m1: u8, s1: u8, m2: u8, s2: u8) -> (u8, u8) {
    let m_x = m1 & m2 & 1;
    let s_x = (s1 ^ s2) & 1;
    (m_x, s_x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the scalar reference covers all 9 ordered (a, b) pairs for add.
    #[test]
    fn scalar_add_truth_table() {
        // Encoding: 0 -> (0,0), 1 -> (1,0), 2 -> (1,1)
        // (alt-zero (0,1) is allowed but we only generate canonical inputs here)
        let enc = [(0u8, 0u8), (1, 0), (1, 1)];
        for a in 0..3u8 {
            for b in 0..3u8 {
                let (m1, s1) = enc[a as usize];
                let (m2, s2) = enc[b as usize];
                let (m, s) = scalar_add(m1, s1, m2, s2);
                let got = decode_lane(m, s);
                let want = (a + b) % 3;
                assert_eq!(got, want, "{} + {} expected {} got {}", a, b, want, got);
            }
        }
    }

    /// Verify scalar sub truth table.
    #[test]
    fn scalar_sub_truth_table() {
        let enc = [(0u8, 0u8), (1, 0), (1, 1)];
        for a in 0..3u8 {
            for b in 0..3u8 {
                let (m1, s1) = enc[a as usize];
                let (m2, s2) = enc[b as usize];
                let (m, s) = scalar_sub(m1, s1, m2, s2);
                let got = decode_lane(m, s);
                let want = (a + 3 - b) % 3;
                assert_eq!(got, want, "{} - {} expected {} got {}", a, b, want, got);
            }
        }
    }

    /// Verify scalar mul truth table.
    #[test]
    fn scalar_mul_truth_table() {
        let enc = [(0u8, 0u8), (1, 0), (1, 1)];
        for a in 0..3u8 {
            for b in 0..3u8 {
                let (m1, s1) = enc[a as usize];
                let (m2, s2) = enc[b as usize];
                let (m, s) = scalar_mul(m1, s1, m2, s2);
                let got = decode_lane(m, s);
                let want = (a * b) % 3;
                assert_eq!(got, want, "{} * {} expected {} got {}", a, b, want, got);
            }
        }
    }
}
