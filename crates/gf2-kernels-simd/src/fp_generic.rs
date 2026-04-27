//! Generic AVX2 batch kernels for Montgomery-stored `Fp<P>` values.
//!
//! The specialised Fermat and Mersenne kernels remain the fastest paths for
//! their exact primes. This module provides a safe function-pointer bundle for
//! all other Montgomery-form primes with `P <= 2^63`: add/sub are lane-wise
//! modular corrections, while mul uses a 4-lane AVX2 Montgomery REDC template.

/// Lane-wise batch multiply for Montgomery-form `Fp<P>` storage words.
///
/// Computes `out[i] = REDC(a[i] * b[i])` for all elements. Inputs must be
/// internal Montgomery residues in `[0, modulus)`.
pub type FpGenericBatchMulFn = fn(&[u64], &[u64], u64, u64, &mut [u64]);

/// Lane-wise batch addition for Montgomery-form `Fp<P>` storage words.
pub type FpGenericBatchAddFn = fn(&[u64], &[u64], u64, &mut [u64]);

/// Lane-wise batch subtraction for Montgomery-form `Fp<P>` storage words.
pub type FpGenericBatchSubFn = fn(&[u64], &[u64], u64, &mut [u64]);

/// Bundle of generic Montgomery SIMD batch operations.
#[derive(Copy, Clone)]
pub struct FpGenericFns {
    /// Lane-wise Montgomery multiply.
    pub batch_mul_fn: FpGenericBatchMulFn,
    /// Lane-wise modular add.
    pub batch_add_fn: FpGenericBatchAddFn,
    /// Lane-wise modular subtract.
    pub batch_sub_fn: FpGenericBatchSubFn,
}

/// Detect and return the best available generic Montgomery SIMD bundle.
pub fn detect() -> Option<FpGenericFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<FpGenericFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(FpGenericFns {
            batch_mul_fn: batch_mul_safe,
            batch_add_fn: batch_add_safe,
            batch_sub_fn: batch_sub_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_mul_safe(a: &[u64], b: &[u64], modulus: u64, p_inv: u64, out: &mut [u64]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_generic::fp_montgomery_batch_mul(a, b, modulus, p_inv, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_add_safe(a: &[u64], b: &[u64], modulus: u64, out: &mut [u64]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_generic::fp_montgomery_batch_add(a, b, modulus, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn batch_sub_safe(a: &[u64], b: &[u64], modulus: u64, out: &mut [u64]) {
    // Safety: `detect_x86` only returns these pointers when AVX2 is available.
    unsafe { crate::x86::fp_generic::fp_montgomery_batch_sub(a, b, modulus, out) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: u64 = 2_147_483_629; // 2^31 - 19, prime and Montgomery-stored in gf2-core.
    const P_INV: u64 = {
        let mut inv: u64 = 1;
        let mut i = 0;
        while i < 6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(P.wrapping_mul(inv)));
            i += 1;
        }
        inv.wrapping_neg()
    };

    fn redc(t: u128) -> u64 {
        let m = (t as u64).wrapping_mul(P_INV);
        let u = ((t + m as u128 * P as u128) >> 64) as u64;
        if u >= P {
            u - P
        } else {
            u
        }
    }

    #[test]
    fn safe_wrappers_match_scalar_word_boundaries() {
        let fns = match detect() {
            Some(fns) => fns,
            None => return,
        };

        for &len in &[0usize, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257] {
            let a: Vec<u64> = (0..len as u64)
                .map(|i| (i.wrapping_mul(1_000_003) + 17) % P)
                .collect();
            let b: Vec<u64> = (0..len as u64)
                .map(|i| (i.wrapping_mul(2_000_033) + 23) % P)
                .collect();

            let mut add = vec![0u64; len];
            (fns.batch_add_fn)(&a, &b, P, &mut add);
            for i in 0..len {
                assert_eq!(add[i], ((a[i] as u128 + b[i] as u128) % P as u128) as u64);
            }

            let mut sub = vec![0u64; len];
            (fns.batch_sub_fn)(&a, &b, P, &mut sub);
            for i in 0..len {
                assert_eq!(
                    sub[i],
                    ((a[i] as u128 + P as u128 - b[i] as u128) % P as u128) as u64
                );
            }

            let mut mul = vec![0u64; len];
            (fns.batch_mul_fn)(&a, &b, P, P_INV, &mut mul);
            for i in 0..len {
                assert_eq!(
                    mul[i],
                    redc(a[i] as u128 * b[i] as u128),
                    "len={len}, i={i}"
                );
            }
        }
    }
}
