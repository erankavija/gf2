//! Test/bench-only deterministic generators and shared harness configuration
//! across the crate's GPU suites and benchmark bins (feature = `test-support`).
//!
//! Centralises the **deterministic AWGN channel-LLR source**
//! ([`AwgnLlrSource`]) used by the GPU byte-identity tests and throughput
//! benches, so the SplitMix64 + Box-Muller + BPSK-LLR math exists exactly
//! once (review SSOT rule, issue `23d3525f` finding F3), and the
//! **external-comparison code selection** ([`ComparisonCode`]) shared by the
//! `export_alist` and `ldpc_bler_sweep` bins, so both sides of the aff3ct
//! comparison build the identical `H` from one construction site (same SSOT
//! rule, issue `18e69a1a`). The module mirrors
//! the `gf2-algebra::testutil` / `gf2-core::test-support` workspace pattern:
//! gated on `cfg(any(test, feature = "test-support"))`, auto-enabled for this
//! crate's own tests and benches via the self-dev-dependency in `Cargo.toml`.
//!
//! # Determinism contract
//!
//! The draw sequence is pinned: **two** `u64` draws per LLR sample (`u1` then
//! `u2`), SplitMix64 stream, Box-Muller **cosine** transform, channel LLR
//! `2·r/N0` computed in `f64` and narrowed to `f32` once. The byte-identity
//! suites' pinned outputs depend on this exact sequence — any change to the
//! expression order is a math change and will flip pinned hard decisions.
//!
//! # Deliberately distinct generators (do NOT fold here)
//!
//! * `gf2_coding::dvb_t2_bicm_harness::box_muller_cos` — the **production**
//!   shared-noise-realisation primitive with its own §5-pinned draw order
//!   (one noise stream shared verbatim between CPU and GPU chain arms);
//!   `tests/gpu_byte_identity.rs` feeds it from a raw SplitMix64 word stream.
//!   That is a different generator contract (harness draw order, not a
//!   channel-LLR source).
//! * The signed-unit IQ fillers in `tests/gpu_demap_byte_identity.rs` and
//!   `src/bin/gpu_demap_throughput.rs` — uniform IQ symbol streams (no
//!   Box-Muller, no LLR), a different contract.

use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::{CodeRate, LdpcCode, Llr};
use gf2_core::BitVec;

/// The two LDPC code configurations of the external-library comparison
/// harness (`dev/benchmarks/gf2-sim/comparison/`, issue `18e69a1a`).
///
/// This is the **single construction site** for the comparison codes: the
/// `export_alist` bin derives the AList `H` fed to aff3ct from
/// [`build`](Self::build), and the `ldpc_bler_sweep` bin decodes the code
/// returned by the same [`build`](Self::build) — so the "bit-identical `H`
/// on both sides" property of the comparison cannot drift between the two
/// bins (review SSOT rule).
///
/// # Examples
///
/// ```
/// use gf2_sim::testutil::ComparisonCode;
///
/// let code = ComparisonCode::parse("nr-bg1-r12").unwrap();
/// let ldpc = code.build();
/// // BG1 Z=384 mother code: N = 68*384, K = 22*384.
/// assert_eq!((ldpc.n(), ldpc.k()), (26112, 8448));
///
/// assert!(ComparisonCode::parse("bogus").is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonCode {
    /// DVB-T2 r1/2 Normal LDPC (ETSI EN 302 755): N = 64800, K = 32400.
    DvbT2R12,
    /// 5G NR BG1 mother code (Z = 384): N = 68·384 = 26112, K = 22·384 =
    /// 8448, rate ≈ 0.323. The mother code of
    /// `nr_5g_rate_matched(1, 16896, 8448)` — the comparison decodes it
    /// directly (no puncturing/shortening) so the exported AList and the
    /// decoded code are one and the same `H`.
    NrBg1R12,
}

impl ComparisonCode {
    /// Parses the harness CLI name (`dvb-t2-r12` / `nr-bg1-r12`).
    ///
    /// # Arguments
    ///
    /// * `s` — the `--code` CLI value.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::testutil::ComparisonCode;
    ///
    /// assert_eq!(
    ///     ComparisonCode::parse("dvb-t2-r12").unwrap(),
    ///     ComparisonCode::DvbT2R12,
    /// );
    /// assert!(ComparisonCode::parse("dvb-t2").is_err());
    /// ```
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "dvb-t2-r12" => Ok(Self::DvbT2R12),
            "nr-bg1-r12" => Ok(Self::NrBg1R12),
            other => Err(format!(
                "unknown --code '{other}' (expected 'dvb-t2-r12' or 'nr-bg1-r12')"
            )),
        }
    }

    /// Builds the `LdpcCode` for this configuration — the one whose
    /// parity-check matrix both comparison bins share.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::testutil::ComparisonCode;
    ///
    /// let ldpc = ComparisonCode::NrBg1R12.build();
    /// assert_eq!((ldpc.n(), ldpc.k()), (26112, 8448));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(nnz(`H`)) — table-driven sparse `H` construction (no encoder cache
    /// is built).
    #[must_use]
    pub fn build(self) -> LdpcCode {
        match self {
            Self::DvbT2R12 => LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
            Self::NrBg1R12 => {
                // nr_5g_rate_matched(1, 16896, 8448) selects Z = 384 for BG1;
                // the comparison uses its full (un-rate-matched) mother code.
                let rm = QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448);
                rm.mother_code().clone()
            }
        }
    }
}

/// A self-contained deterministic AWGN channel-LLR source: a SplitMix64
/// stream feeds a Box-Muller cosine transform to produce N(0, 1) noise,
/// added to a BPSK-mapped codeword (bit `b` → `1 - 2b`, i.e. `+1` for 0,
/// `-1` for 1). The channel LLR is `2·r/N0` with `N0 = 2·sigma²` (the
/// standard AWGN-BPSK LLR).
///
/// Only used to manufacture varied, reproducible LLR inputs for the GPU
/// byte-identity tests and throughput benches; the exact distribution is
/// irrelevant to those comparisons (both decode arms see the identical
/// LLRs) — only that the stream is deterministic per seed.
///
/// # Examples
///
/// ```
/// use gf2_sim::testutil::AwgnLlrSource;
/// use gf2_core::BitVec;
///
/// // Same seed -> bit-identical frames.
/// let mut a = AwgnLlrSource::new(42);
/// let mut b = AwgnLlrSource::new(42);
/// let cw = BitVec::zeros(8);
/// assert_eq!(a.frame_for_codeword(&cw, 0.8), b.frame_for_codeword(&cw, 0.8));
///
/// // The all-zero convenience draws the same stream as the explicit
/// // all-zero codeword (BPSK maps bit 0 -> +1 either way).
/// let mut c = AwgnLlrSource::new(42);
/// let mut d = AwgnLlrSource::new(42);
/// assert_eq!(c.frame_all_zero(8, 0.8), d.frame_for_codeword(&cw, 0.8));
/// ```
pub struct AwgnLlrSource {
    state: u64,
}

impl AwgnLlrSource {
    /// Creates a source whose SplitMix64 stream starts at `seed`.
    ///
    /// # Arguments
    ///
    /// * `seed` — the SplitMix64 starting state; equal seeds reproduce
    ///   bit-identical LLR streams.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::testutil::AwgnLlrSource;
    ///
    /// let mut src = AwgnLlrSource::new(0xA930_BE7F);
    /// let frame = src.frame_all_zero(4, 0.65);
    /// assert_eq!(frame.len(), 4);
    /// ```
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// One SplitMix64 step.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Top 53 bits / 2^53 ∈ [0, 1).
    fn next_uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    /// One N(0, 1) draw via the Box-Muller cosine transform (two uniform
    /// draws per sample: `u1` then `u2` — the pinned draw order).
    fn next_normal(&mut self) -> f64 {
        let mut u1 = self.next_uniform();
        let u2 = self.next_uniform();
        if u1 < 1e-15 {
            u1 = 1e-15;
        }
        let r = (-2.0 * u1.ln()).sqrt();
        r * (std::f64::consts::TAU * u2).cos()
    }

    /// One LLR sample for BPSK symbol `s` (`+1.0` or `-1.0`) at noise std
    /// `sigma`: `r = s + N(0, sigma)`, LLR `= 2·r/N0`.
    ///
    /// The expression tree (`(2.0 * r / n0) as f32`, noise = `normal * sigma`)
    /// is pinned — see the module-level determinism contract.
    fn llr_sample(&mut self, s: f64, sigma: f64, n0: f64) -> Llr {
        let noise = self.next_normal() * sigma;
        let r = s + noise;
        Llr::new((2.0 * r / n0) as f32)
    }

    /// One frame of channel LLRs for the **all-zero codeword** (every BPSK
    /// symbol `+1`) at noise std `sigma`.
    ///
    /// Draws exactly `2·n` `u64`s from the stream (two per sample), identical
    /// to [`frame_for_codeword`](Self::frame_for_codeword) over
    /// `BitVec::zeros(n)`.
    ///
    /// # Arguments
    ///
    /// * `n` — the frame length (number of LLRs).
    /// * `sigma` — the AWGN noise standard deviation (`N0 = 2·sigma²`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::testutil::AwgnLlrSource;
    ///
    /// let mut src = AwgnLlrSource::new(7);
    /// let frame = src.frame_all_zero(16, 0.8);
    /// assert_eq!(frame.len(), 16);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`n`) — two SplitMix64 draws and one Box-Muller transform per LLR.
    #[must_use]
    pub fn frame_all_zero(&mut self, n: usize, sigma: f64) -> Vec<Llr> {
        let n0 = 2.0 * sigma * sigma;
        (0..n).map(|_| self.llr_sample(1.0, sigma, n0)).collect()
    }

    /// One frame of channel LLRs over a transmitted codeword `cw` at noise
    /// std `sigma`. BPSK: bit `b` → `1 - 2b` (`+1` for 0, `-1` for 1).
    ///
    /// # Arguments
    ///
    /// * `cw` — the transmitted codeword; the frame has `cw.len()` LLRs.
    /// * `sigma` — the AWGN noise standard deviation (`N0 = 2·sigma²`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::testutil::AwgnLlrSource;
    /// use gf2_core::BitVec;
    ///
    /// let mut cw = BitVec::zeros(4);
    /// cw.set(1, true);
    /// let mut src = AwgnLlrSource::new(3);
    /// let frame = src.frame_for_codeword(&cw, 0.7);
    /// assert_eq!(frame.len(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`cw.len()`) — two SplitMix64 draws and one Box-Muller transform per
    /// LLR.
    #[must_use]
    pub fn frame_for_codeword(&mut self, cw: &BitVec, sigma: f64) -> Vec<Llr> {
        let n0 = 2.0 * sigma * sigma;
        (0..cw.len())
            .map(|i| {
                let s = if cw.get(i) { -1.0 } else { 1.0 };
                self.llr_sample(s, sigma, n0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned SplitMix64 stream: first outputs from seed 0 must match the
    /// reference sequence (guards against an accidental constant/step change,
    /// which would silently re-pin every byte-identity suite's inputs).
    #[test]
    fn test_splitmix64_reference_stream() {
        let mut src = AwgnLlrSource::new(0);
        // SplitMix64(seed = 0) reference outputs (Vigna's splitmix64.c).
        assert_eq!(src.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(src.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(src.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn test_same_seed_bit_identical() {
        let mut a = AwgnLlrSource::new(0xDEAD_BEEF);
        let mut b = AwgnLlrSource::new(0xDEAD_BEEF);
        let fa = a.frame_all_zero(64, 0.95);
        let fb = b.frame_all_zero(64, 0.95);
        assert_eq!(fa, fb);
    }

    /// `frame_all_zero(n, sigma)` and `frame_for_codeword(zeros(n), sigma)`
    /// must be the SAME stream (the all-zero convenience is not a separate
    /// generator).
    #[test]
    fn test_all_zero_equals_zero_codeword() {
        let mut a = AwgnLlrSource::new(0x1234);
        let mut b = AwgnLlrSource::new(0x1234);
        let fa = a.frame_all_zero(33, 0.8);
        let fb = b.frame_for_codeword(&BitVec::zeros(33), 0.8);
        assert_eq!(fa, fb);
    }

    /// A set bit flips the BPSK sign: with the same stream position the LLR
    /// for bit 1 is `2(-1 + noise)/N0` vs `2(1 + noise)/N0` for bit 0 — they
    /// must differ by exactly `4/N0` in f64 before the f32 narrowing, so
    /// check the sign relation on a strong-signal draw.
    #[test]
    fn test_codeword_bit_flips_sign() {
        let n = 16;
        let mut ones = BitVec::zeros(n);
        for i in 0..n {
            ones.set(i, true);
        }
        let mut a = AwgnLlrSource::new(99);
        let mut b = AwgnLlrSource::new(99);
        // Tiny sigma: noise is negligible, so signs are determined by the bit.
        let fz = a.frame_all_zero(n, 1e-3);
        let fo = b.frame_for_codeword(&ones, 1e-3);
        for i in 0..n {
            assert!(fz[i].value() > 0.0, "bit 0 -> positive LLR at {i}");
            assert!(fo[i].value() < 0.0, "bit 1 -> negative LLR at {i}");
        }
    }
}
