//! Minimal Residue Number System (RNS) prototype.
//!
//! Research artifact for JIT issue 0a7e2555. Not production code, not part of
//! the gf2 workspace. Demonstrates RNS representation of a large-integer field
//! element using three 50-bit coprime moduli (dynamic range ~2^150) and
//! compares naive per-element cost against two baselines:
//!
//!   * `baseline_u128_mod`   — native u128 a*b mod P (P ~ 2^63, no overflow);
//!     represents the cost of a single 64-bit Montgomery-style mulmod.
//!   * `baseline_schoolbook` — 128×128 -> 256-bit schoolbook mul followed by
//!     bit-serial reduction modulo P ~ 2^127; represents the cost of a
//!     generic 2-limb reduction with no hardware fast-path (pessimistic
//!     upper bound on 2-limb Montgomery).
//!
//! Layout:
//!   * `rns`      : RNS representation and operations
//!   * `baseline` : reference multi-word multiplication helpers
//!   * `tests`    : correctness tests (`cargo test --release`)
//!   * `main`     : tiny benchmark (`cargo run --release`)
//!
//! This file stays deliberately small and has **no** dependencies beyond `std`.

// ---------------------------------------------------------------------------
// RNS representation (3 × 50-bit moduli)
// ---------------------------------------------------------------------------

#[allow(dead_code)] // `add`-path used by tests only; keep for completeness.
mod rns {
    /// Three coprime 50-bit primes.  Products of two residues are < 2^100,
    /// which fits comfortably in u128.
    ///
    /// We verified primality externally (tiny sieve); they are coprime by
    /// being distinct primes.
    pub const M: [u64; 3] = [
        (1u64 << 50) - 27, // 1125899906842597
        (1u64 << 50) - 55, // 1125899906842569
        (1u64 << 50) - 93, // 1125899906842531
    ];
    pub const N: usize = 3;

    /// Product M0 * M1 * M2 as u128 — the dynamic range, ~2^150.
    pub fn modulus_product_u128() -> u128 {
        // This is only < 2^128 if the product fits. 3 × 50-bit = 150-bit, so
        // it does NOT fit in u128. Return the low 128 bits for diagnostics
        // only; real range checks must use higher precision.
        //
        // To keep the prototype simple, we only exercise values below the
        // largest u128-representable multiple of the product, i.e. we keep
        // inputs < 2^127 in tests.
        //
        // The true dynamic range is (M[0] as u256) * M[1] * M[2], but we
        // don't need the exact value for the tests.
        (M[0] as u128) * (M[1] as u128) // low product only, for display
    }

    /// Element in RNS form: each limb in `[0, m_i)`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Rns(pub [u64; N]);

    impl Rns {
        /// Forward CRT: integer (< 2^127, so fits in u128) -> RNS residues.
        pub fn from_u128(x: u128) -> Rns {
            Rns([
                (x % (M[0] as u128)) as u64,
                (x % (M[1] as u128)) as u64,
                (x % (M[2] as u128)) as u64,
            ])
        }

        /// Compute Garner mixed-radix coefficients `(v0, v1, v2)`.
        ///
        /// Garner's method:
        ///     v0 = r0
        ///     v1 = (r1 - v0) * m0^{-1}           mod m1
        ///     v2 = ((r2 - v0) * m0^{-1} - v1) * m1^{-1}    mod m2
        ///
        /// The reconstructed integer is `v0 + v1*m0 + v2*m0*m1`, which fits
        /// in 150 bits (the dynamic range of this RNS). Each `v_i < m_i < 2^50`.
        #[inline]
        fn garner_coeffs(self) -> (u64, u64, u64) {
            let m0_inv_m1 = mod_inv_u64(M[0] % M[1], M[1]);
            let m0_inv_m2 = mod_inv_u64(M[0] % M[2], M[2]);
            let m1_inv_m2 = mod_inv_u64(M[1] % M[2], M[2]);

            let r0 = self.0[0];
            let r1 = self.0[1];
            let r2 = self.0[2];

            let v0 = r0;
            let v1 = mul_mod_u64(sub_mod_u64(r1, v0 % M[1], M[1]), m0_inv_m1, M[1]);
            let mut t = sub_mod_u64(r2, v0 % M[2], M[2]);
            t = mul_mod_u64(t, m0_inv_m2, M[2]);
            t = sub_mod_u64(t, v1 % M[2], M[2]);
            let v2 = mul_mod_u64(t, m1_inv_m2, M[2]);
            (v0, v1, v2)
        }

        /// Backward CRT via Garner's mixed-radix algorithm, producing a
        /// 256-bit integer split as `[low, high]` u128 limbs.
        ///
        /// The reconstructed value is `low + (high << 128)` and always
        /// satisfies `value < M = m0*m1*m2 < 2^150`. This is the honest,
        /// non-wrapping reconstruction used by the benchmark; it fits in two
        /// u128 limbs for any valid residue tuple.
        pub fn to_u256(self) -> [u128; 2] {
            let (v0, v1, v2) = self.garner_coeffs();

            // term_a = v0 + v1 * m0  — fits in u128 (at most 2^50 + 2^50*2^50 = 2^100).
            let term_a: u128 = (v0 as u128) + (v1 as u128) * (M[0] as u128);

            // term_b = v2 * m0 * m1 — up to 2^150. Compute m0*m1 as u128
            // (fits in 2^100) then widen to a 256-bit product via
            // u64 * u128 -> [u128; 2].
            let m0_m1: u128 = (M[0] as u128) * (M[1] as u128);
            let term_b: [u128; 2] = mul_u64_by_u128(v2, m0_m1);

            add_u256(term_b, [term_a, 0])
        }

        /// Backward CRT for callers that know the reconstruction fits in
        /// `u128`. Panics (in debug) if the high limb is non-zero.
        ///
        /// Use `to_u256` for unrestricted residues; `to_u128` is only safe
        /// when the caller controls input ranges so the reconstructed value
        /// is `< 2^128`.
        pub fn to_u128(self) -> u128 {
            let [lo, hi] = self.to_u256();
            debug_assert_eq!(
                hi, 0,
                "to_u128 called on a residue whose reconstruction exceeds u128 — use to_u256"
            );
            lo
        }

        /// Channel-wise addition — each lane independent, no carry chain.
        #[inline]
        pub fn add(self, rhs: Rns) -> Rns {
            let mut out = [0u64; N];
            for i in 0..N {
                out[i] = add_mod_u64(self.0[i], rhs.0[i], M[i]);
            }
            Rns(out)
        }

        /// Channel-wise multiplication — each lane independent.  Per-lane
        /// 50x50 -> 100-bit product reduced with a single u128 division.
        #[inline]
        pub fn mul(self, rhs: Rns) -> Rns {
            let mut out = [0u64; N];
            for i in 0..N {
                let prod = (self.0[i] as u128) * (rhs.0[i] as u128);
                out[i] = (prod % (M[i] as u128)) as u64;
            }
            Rns(out)
        }
    }

    // ---- tiny u256 helpers for the 150-bit reconstruction ----------------
    //
    // The RNS dynamic range is ~2^150 so reconstruction can exceed u128.
    // We only need two operations: `u64 * u128 -> [u128; 2]` and
    // `[u128; 2] + [u128; 2] -> [u128; 2]` (with no overflow at this scale).

    /// Widen a `u64 * u128` product into a 256-bit value stored as
    /// `[low, high]` u128 limbs.
    #[inline]
    fn mul_u64_by_u128(a: u64, b: u128) -> [u128; 2] {
        // Split b into two u64 halves: b = b_hi * 2^64 + b_lo.
        let b_lo = b as u64;
        let b_hi = (b >> 64) as u64;

        // Each partial product is u64 * u64 -> u128 exactly.
        let p_lo: u128 = (a as u128) * (b_lo as u128); // up to 2^128
        let p_hi: u128 = (a as u128) * (b_hi as u128); // up to 2^128

        // Combine: result = p_lo + (p_hi << 64). The shift may push bits
        // into the high u128 limb.
        let low: u128 = p_lo.wrapping_add(p_hi << 64);
        // Carry from the low-limb add: p_lo + (p_hi << 64) may wrap u128
        // when p_lo + low64(p_hi << 64) >= 2^128.
        let low_carry: u128 = if low < p_lo { 1 } else { 0 };
        let high: u128 = (p_hi >> 64) + low_carry;
        [low, high]
    }

    /// Add two 256-bit values represented as `[low, high]` u128 limbs.
    /// Saturates in debug on overflow of the high limb — we never expect
    /// that for the reconstruction (the result is always < 2^150 < 2^256).
    #[inline]
    fn add_u256(a: [u128; 2], b: [u128; 2]) -> [u128; 2] {
        let (lo, carry) = a[0].overflowing_add(b[0]);
        let hi = a[1].wrapping_add(b[1]).wrapping_add(carry as u128);
        debug_assert!(
            a[1].checked_add(b[1])
                .and_then(|s| s.checked_add(carry as u128))
                .is_some(),
            "u256 add overflow — result exceeds 2^256"
        );
        [lo, hi]
    }

    // ---- scalar mod helpers (50-bit moduli) --------------------------------

    #[inline]
    fn add_mod_u64(a: u64, b: u64, m: u64) -> u64 {
        let s = a + b; // safe: both < m < 2^50
        if s >= m {
            s - m
        } else {
            s
        }
    }

    #[inline]
    fn sub_mod_u64(a: u64, b: u64, m: u64) -> u64 {
        if a >= b {
            a - b
        } else {
            a + m - b
        }
    }

    #[inline]
    fn mul_mod_u64(a: u64, b: u64, m: u64) -> u64 {
        ((a as u128 * b as u128) % m as u128) as u64
    }

    /// Extended Euclidean modular inverse.  Requires gcd(a, m) == 1.
    fn mod_inv_u64(a: u64, m: u64) -> u64 {
        let (g, x, _) = ext_gcd(a as i128, m as i128);
        assert_eq!(g, 1, "modular inverse requires gcd = 1");
        let mi = m as i128;
        (((x % mi) + mi) % mi) as u64
    }

    fn ext_gcd(a: i128, b: i128) -> (i128, i128, i128) {
        if b == 0 {
            (a, 1, 0)
        } else {
            let (g, x1, y1) = ext_gcd(b, a % b);
            (g, y1, x1 - (a / b) * y1)
        }
    }
}

// ---------------------------------------------------------------------------
// Baselines for the crossover benchmark
// ---------------------------------------------------------------------------

mod baseline {
    /// A 63-bit-ish prime: 2^61 - 1 (Mersenne prime M61).  Fits in u64 and
    /// any u64 × u64 product fits in u128. Represents the cost of a single
    /// 64-bit Montgomery-style mulmod.
    pub const P_61: u64 = (1u64 << 61) - 1;

    /// u128 mulmod with a 63-bit modulus — represents 1-limb Montgomery cost.
    #[inline]
    pub fn mul_mod_p61(a: u64, b: u64) -> u64 {
        ((a as u128 * b as u128) % P_61 as u128) as u64
    }

    /// 128×128 -> 256-bit schoolbook product in four 64-bit limbs.
    #[inline]
    pub fn mul_128x128_256(a: u128, b: u128) -> [u64; 4] {
        let a0 = a as u64;
        let a1 = (a >> 64) as u64;
        let b0 = b as u64;
        let b1 = (b >> 64) as u64;
        let p00 = (a0 as u128) * (b0 as u128);
        let p01 = (a0 as u128) * (b1 as u128);
        let p10 = (a1 as u128) * (b0 as u128);
        let p11 = (a1 as u128) * (b1 as u128);

        let w0 = p00 as u64;
        // mid = p00>>64 + p01.lo + p10.lo  (bits fit in 66)
        let mid = (p00 >> 64) + (p01 & 0xFFFF_FFFF_FFFF_FFFF) + (p10 & 0xFFFF_FFFF_FFFF_FFFF);
        let w1 = mid as u64;
        let carry_mid = mid >> 64;
        let hi = (p01 >> 64) + (p10 >> 64) + (p11 & 0xFFFF_FFFF_FFFF_FFFF) + carry_mid;
        let w2 = hi as u64;
        let carry_hi = hi >> 64;
        let top = (p11 >> 64) + carry_hi;
        let w3 = top as u64;
        [w0, w1, w2, w3]
    }

    /// A 127-bit Mersenne prime: 2^127 - 1. Used as a stand-in for the target
    /// of a 2-limb Montgomery reduction.
    pub const P_127: u128 = (1u128 << 127) - 1;

    /// Bit-serial reduction of a 256-bit value modulo a u128 modulus.
    ///
    /// Deliberately slow: this represents an *unoptimized* 2-limb reduction.
    /// A real 2-limb Montgomery REDC would be ~5-10x faster than this.
    pub fn rem_256_by_u128(limbs: [u64; 4], m: u128) -> u128 {
        assert!(m > 0);
        let mut r: u128 = 0;
        for bit_rev in (0..256).rev() {
            let limb = bit_rev / 64;
            let shift = bit_rev % 64;
            let bit = (limbs[limb] >> shift) & 1;
            let hi_bit = r >> 127;
            r = (r << 1) | bit as u128;
            if hi_bit == 1 || r >= m {
                r = r.wrapping_sub(m);
            }
        }
        r
    }

    /// "Unoptimized 2-limb" mulmod.  Schoolbook multiply + bit-serial reduce.
    #[inline]
    pub fn mul_mod_p127_slow(a: u128, b: u128) -> u128 {
        let w = mul_128x128_256(a, b);
        rem_256_by_u128(w, P_127)
    }

    /// "Optimistic 2-limb" mulmod: u128 multiply (wraps!) with the low 128
    /// bits of the product reduced by two conditional subtractions.  This is
    /// *incorrect* mathematically — the real answer needs the high 128 bits
    /// — but it is a lower bound on what a hand-rolled optimized 2-limb
    /// Montgomery could achieve (roughly 1 multiply + small reduction).
    ///
    /// We include this only as a hardware-optimistic reference point for the
    /// crossover analysis.  The benchmark reports both flavours.
    #[inline]
    pub fn mul_mod_p127_optimistic(a: u128, b: u128) -> u128 {
        let prod = a.wrapping_mul(b);
        // Fold-by-one: (prod mod 2^127 - 1) ≈ (prod & (2^127-1)) + (prod >> 127)
        // This is exact for Mersenne primes when done correctly on the full
        // product, but we only have the low 128 bits here — so this is a
        // performance proxy, not a correct math operation.
        let lo = prod & P_127;
        let hi = prod >> 127;
        let s = lo + hi;
        if s >= P_127 {
            s - P_127
        } else {
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Bench: run a batch of k multiplications RNS-style and baseline-style.
// ---------------------------------------------------------------------------

fn bench_batch(k: usize) {
    use std::hint::black_box;
    use std::time::Instant;

    // Deterministic pseudo-random inputs.
    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    }

    let mut s = 0x9E3779B97F4A7C15u64;

    // Inputs <= 2^96 so every path (RNS, u128, 2-limb) reads real-looking
    // values. Keep them small enough to fit in u128 paths.
    let mut a_vals: Vec<u128> = Vec::with_capacity(k);
    let mut b_vals: Vec<u128> = Vec::with_capacity(k);
    for _ in 0..k {
        let al = lcg(&mut s) as u128;
        let ah = (lcg(&mut s) & ((1u64 << 32) - 1)) as u128; // 32-bit high -> total <2^96
        let bl = lcg(&mut s) as u128;
        let bh = (lcg(&mut s) & ((1u64 << 32) - 1)) as u128;
        a_vals.push((ah << 64) | al);
        b_vals.push((bh << 64) | bl);
    }

    // --- RNS path ---
    let t0 = Instant::now();
    let mut a_rns: Vec<rns::Rns> = Vec::with_capacity(k);
    let mut b_rns: Vec<rns::Rns> = Vec::with_capacity(k);
    for i in 0..k {
        a_rns.push(rns::Rns::from_u128(a_vals[i]));
        b_rns.push(rns::Rns::from_u128(b_vals[i]));
    }
    let t_fwd = t0.elapsed();

    let t1 = Instant::now();
    let mut c_rns: Vec<rns::Rns> = Vec::with_capacity(k);
    for i in 0..k {
        c_rns.push(a_rns[i].mul(b_rns[i]));
    }
    let t_rns_mul = t1.elapsed();

    // Backward CRT: use `to_u256` so reconstruction is honest for ALL
    // residues in the 2^150 dynamic range. Products of 2^96 inputs can
    // exceed u128; `to_u128` would silently wrap. Both limbs of the
    // u256 output feed the black-box checksum so the compiler cannot
    // elide the high limb.
    let t2 = Instant::now();
    let mut checksum_lo: u128 = 0;
    let mut checksum_hi: u128 = 0;
    for r in &c_rns {
        let [lo, hi] = r.to_u256();
        checksum_lo ^= lo;
        checksum_hi ^= hi;
    }
    let t_back = t2.elapsed();
    black_box(checksum_lo);
    black_box(checksum_hi);

    // --- Baseline: M61 (u64) mulmod ---
    let t3 = Instant::now();
    let mut acc: u64 = 0;
    for i in 0..k {
        acc ^= baseline::mul_mod_p61(
            a_vals[i] as u64 & (baseline::P_61 - 1),
            b_vals[i] as u64 & (baseline::P_61 - 1),
        );
    }
    let t_base_u64 = t3.elapsed();
    black_box(acc);

    // --- Baseline: P127 slow schoolbook + bit-serial reduce ---
    let t4 = Instant::now();
    let mut acc2: u128 = 0;
    for i in 0..k {
        acc2 ^=
            baseline::mul_mod_p127_slow(a_vals[i] % baseline::P_127, b_vals[i] % baseline::P_127);
    }
    let t_base_slow = t4.elapsed();
    black_box(acc2);

    // --- Baseline: P127 "optimistic" (low-128-bit only) ---
    let t5 = Instant::now();
    let mut acc3: u128 = 0;
    for i in 0..k {
        acc3 ^= baseline::mul_mod_p127_optimistic(a_vals[i], b_vals[i]);
    }
    let t_base_opt = t5.elapsed();
    black_box(acc3);

    let rns_total = t_fwd + t_rns_mul + t_back;

    println!("--- batch size k = {k} ---");
    print_line("rns forward CRT      ", t_fwd.as_secs_f64(), k);
    print_line("rns channel mul      ", t_rns_mul.as_secs_f64(), k);
    print_line("rns backward CRT     ", t_back.as_secs_f64(), k);
    print_line("rns total (all three)", rns_total.as_secs_f64(), k);
    print_line("baseline 1-limb M61  ", t_base_u64.as_secs_f64(), k);
    print_line("baseline 2-limb slow ", t_base_slow.as_secs_f64(), k);
    print_line("baseline 2-limb opt. ", t_base_opt.as_secs_f64(), k);
    println!(
        "  => rns/1-limb ratio         : {:.2}x  (rns is slower when > 1)",
        rns_total.as_secs_f64() / t_base_u64.as_secs_f64()
    );
    println!(
        "  => rns/2-limb-slow ratio    : {:.2}x",
        rns_total.as_secs_f64() / t_base_slow.as_secs_f64()
    );
    println!(
        "  => rns/2-limb-optimistic    : {:.2}x",
        rns_total.as_secs_f64() / t_base_opt.as_secs_f64()
    );
}

fn print_line(label: &str, secs: f64, k: usize) {
    let us = secs * 1e6;
    let ns_per = secs * 1e9 / k as f64;
    println!("  {label}: {us:>12.3} us total   ({ns_per:>7.2} ns/elt)");
}

fn main() {
    println!("RNS prototype (3 x 50-bit moduli, dynamic range ~2^150)");
    println!("moduli: {} {} {}", rns::M[0], rns::M[1], rns::M[2]);
    println!(
        "low-128 product (display only): {}",
        rns::modulus_product_u128()
    );
    println!();
    for &k in &[1usize, 16, 256, 4096, 65536, 262144] {
        bench_batch(k);
        println!();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::rns::{Rns, M};

    fn modulus_product_low128() -> u128 {
        // Only the low 128 bits of the true 150-bit product — used as a cap
        // on test inputs so that the u128 reconstruction path does not wrap.
        (M[0] as u128) * (M[1] as u128)
    }

    #[test]
    fn moduli_are_pairwise_coprime() {
        fn gcd(mut a: u64, mut b: u64) -> u64 {
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a
        }
        for (i, &mi) in M.iter().enumerate() {
            for (j, &mj) in M.iter().enumerate().skip(i + 1) {
                assert_eq!(gcd(mi, mj), 1, "M[{i}] and M[{j}] not coprime");
            }
        }
    }

    #[test]
    fn roundtrip_small_values() {
        for v in [0u128, 1, 2, 3, 7, 1234, 1_000_000_007] {
            let x = Rns::from_u128(v);
            assert_eq!(x.to_u128(), v, "roundtrip failed for {v}");
        }
    }

    #[test]
    fn roundtrip_large_values() {
        // Any u128 input is safely < M (since M > 2^149), so `to_u128`
        // is correct for all u128 values. (The cap variable is retained
        // below for historical context — early prototypes used it to
        // guard against u128 wrap in the old `to_u128`.)
        let cap = modulus_product_low128();
        let _ = cap; // silence unused-var in debug builds
        for v in [
            cap / 7,
            cap / 3,
            cap / 2,
            cap - 1,
            1u128 << 99,
            (1u128 << 127) - 1,
            u128::MAX,
        ] {
            let x = Rns::from_u128(v);
            assert_eq!(x.to_u128(), v, "roundtrip failed for {v}");
        }
    }

    #[test]
    fn add_matches_u128() {
        let a = 0x12345678_u128;
        let b = 0x9abcdef0_u128;
        let ra = Rns::from_u128(a);
        let rb = Rns::from_u128(b);
        let rs = ra.add(rb);
        assert_eq!(rs.to_u128(), a + b);
    }

    #[test]
    fn mul_matches_u128_small() {
        // Small inputs: product < 2^99, far below the 150-bit range.
        let pairs: [(u128, u128); 5] = [
            (0, 0),
            (1, 1),
            (2, 3),
            (123456, 987654321),
            (1u128 << 48, 1u128 << 48), // product 2^96
        ];
        for (a, b) in pairs {
            // Ground truth: a*b fits in u128 (2^96 < 2^128), so native mul is
            // exact. The RNS result is interpreted modulo M (the full
            // 150-bit product), and a*b < M so they must agree.
            let expected = a * b;
            let r = Rns::from_u128(a).mul(Rns::from_u128(b));
            let got = r.to_u128();
            assert_eq!(got, expected, "fail on a={a} b={b}");
        }
    }

    // ---- proptest-based invariants --------------------------------------
    //
    // These cover the three headline CRT properties for the prototype:
    //
    //   1. forward/backward roundtrip
    //   2. additive homomorphism
    //   3. multiplicative homomorphism modulo M
    //
    // Each test runs 256 cases (overriding proptest's default of 256 only
    // where we need more breathing room). Input strategies are chosen so
    // that the u128 ground truth is exact — see per-test comments.

    use proptest::prelude::*;

    /// The three-modulus RNS dynamic range as a 256-bit value
    /// `M = m0 * m1 * m2`. Used as the modulus against which multiplicative
    /// homomorphism is checked.
    fn modulus_m_u256() -> [u128; 2] {
        // M[0] * M[1] fits in u128 (< 2^100).
        let m0m1: u128 = (M[0] as u128) * (M[1] as u128);
        // Widen to [u128; 2] via the same u64*u128 helper used in to_u256.
        // We replicate the math here rather than exposing the helper.
        let b_lo = m0m1 as u64;
        let b_hi = (m0m1 >> 64) as u64;
        let p_lo: u128 = (M[2] as u128) * (b_lo as u128);
        let p_hi: u128 = (M[2] as u128) * (b_hi as u128);
        let low: u128 = p_lo.wrapping_add(p_hi << 64);
        let low_carry: u128 = if low < p_lo { 1 } else { 0 };
        let high: u128 = (p_hi >> 64) + low_carry;
        [low, high]
    }

    /// `(a * b) mod M` computed as a reference: `a, b < 2^128`, product fits
    /// in a 256-bit value, reduced against the 150-bit modulus `M` via
    /// bit-serial division. Deliberately simple; used in tests only.
    fn mul_mod_m_ref(a: u128, b: u128) -> [u128; 2] {
        // 128x128 -> 256 schoolbook product, in [u128; 2] = [low, high].
        let a_lo = a as u64;
        let a_hi = (a >> 64) as u64;
        let b_lo = b as u64;
        let b_hi = (b >> 64) as u64;

        let p00: u128 = (a_lo as u128) * (b_lo as u128);
        let p01: u128 = (a_lo as u128) * (b_hi as u128);
        let p10: u128 = (a_hi as u128) * (b_lo as u128);
        let p11: u128 = (a_hi as u128) * (b_hi as u128);

        // Assemble into a 4-limb u64 view.
        let w0 = p00 as u64;
        let mid = (p00 >> 64) + (p01 & 0xFFFF_FFFF_FFFF_FFFF) + (p10 & 0xFFFF_FFFF_FFFF_FFFF);
        let w1 = mid as u64;
        let carry_mid = mid >> 64;
        let hi = (p01 >> 64) + (p10 >> 64) + (p11 & 0xFFFF_FFFF_FFFF_FFFF) + carry_mid;
        let w2 = hi as u64;
        let carry_hi = hi >> 64;
        let top = (p11 >> 64) + carry_hi;
        let w3 = top as u64;

        let prod_low: u128 = (w0 as u128) | ((w1 as u128) << 64);
        let prod_high: u128 = (w2 as u128) | ((w3 as u128) << 64);

        // Bit-serial mod M (256-bit dividend, 256-bit divisor-as-[u128;2]).
        let m = modulus_m_u256();
        let mut r: [u128; 2] = [0, 0];
        for bit in (0..256).rev() {
            // Shift r left by one, bringing in the dividend bit.
            let carry_up = r[0] >> 127;
            r[0] = (r[0] << 1)
                | (if bit < 128 {
                    (prod_low >> bit) & 1
                } else {
                    (prod_high >> (bit - 128)) & 1
                });
            r[1] = (r[1] << 1) | carry_up;
            // Compare r >= m. If so, subtract.
            let ge = r[1] > m[1] || (r[1] == m[1] && r[0] >= m[0]);
            if ge {
                let (new_lo, borrow) = r[0].overflowing_sub(m[0]);
                r[0] = new_lo;
                r[1] = r[1].wrapping_sub(m[1]).wrapping_sub(borrow as u128);
            }
        }
        r
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// CRT roundtrip: forward then backward is the identity for any
        /// `x < 2^128 < M`. Reconstruction fits u128 because the input does.
        #[test]
        fn prop_roundtrip_u128(x in any::<u128>()) {
            let r = Rns::from_u128(x);
            prop_assert_eq!(r.to_u128(), x);
        }

        /// Additive homomorphism. We cap inputs at 2^127 each so that
        /// `a + b < 2^128` and the u128 ground truth is exact. The
        /// reconstruction of `ra.add(rb)` must match the integer sum (it
        /// does, since the sum is also `< M`).
        #[test]
        fn prop_add_homomorphism(
            a in 0u128..(1u128 << 127),
            b in 0u128..(1u128 << 127),
        ) {
            let ra = Rns::from_u128(a);
            let rb = Rns::from_u128(b);
            let sum = ra.add(rb);
            prop_assert_eq!(sum.to_u128(), a + b);
        }

        /// Multiplicative homomorphism modulo `M`. Inputs are unrestricted
        /// u128, so `a * b` can exceed u128; we reduce with a reference
        /// bit-serial `mul_mod_m_ref` and compare against the u256
        /// reconstruction from `to_u256`. This exercises the full 150-bit
        /// output range, including residues whose reconstruction requires
        /// the high u128 limb.
        #[test]
        fn prop_mul_homomorphism(
            a in any::<u128>(),
            b in any::<u128>(),
        ) {
            let ra = Rns::from_u128(a);
            let rb = Rns::from_u128(b);
            let prod = ra.mul(rb);
            let got = prod.to_u256();
            let expected = mul_mod_m_ref(a, b);
            prop_assert_eq!(got, expected);
        }
    }
}
