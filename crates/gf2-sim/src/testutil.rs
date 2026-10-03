//! Deterministic LLR generation and comparison-code selection for tests and
//! benches, compiled under `cfg(test)` or the `test-support` feature.

use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::{CodeRate, LdpcCode, Llr};
use gf2_core::BitVec;

/// The LDPC codes of the comparison against AFF3CT
/// (`@/citation/Cassagne2019`).
///
/// The `export_alist` bin exports the `H` of [`build`](Self::build) and the
/// `ldpc_bler_sweep` bin decodes the same code, so both sides share one
/// parity-check matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonCode {
    /// DVB-T2 r1/2 Normal LDPC (`@/citation/Etsi2015`): N = 64800, K = 32400.
    DvbT2R12,
    /// 5G NR BG1 mother code (Z = 384): N = 68·384 = 26112, K = 22·384 =
    /// 8448, decoded without puncturing or shortening.
    NrBg1R12,
}

impl ComparisonCode {
    /// Parses the harness CLI name (`dvb-t2-r12` / `nr-bg1-r12`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "dvb-t2-r12" => Ok(Self::DvbT2R12),
            "nr-bg1-r12" => Ok(Self::NrBg1R12),
            other => Err(format!(
                "unknown --code '{other}' (expected 'dvb-t2-r12' or 'nr-bg1-r12')"
            )),
        }
    }

    /// Builds the `LdpcCode` for this configuration.
    ///
    /// # Complexity
    ///
    /// That of the selected code-family constructor. The NR path calls
    /// [`QuasiCyclicLdpc::nr_5g_rate_matched`], including its encoder-data
    /// preparation, before retaining the mother code.
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
pub struct AwgnLlrSource {
    state: u64,
}

impl AwgnLlrSource {
    /// Creates a source whose SplitMix64 stream starts at `seed`.
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
    /// draws per sample: `u1` then `u2`).
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
    /// fixes the `f32` output bits.
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
    #[must_use]
    pub fn frame_all_zero(&mut self, n: usize, sigma: f64) -> Vec<Llr> {
        let n0 = 2.0 * sigma * sigma;
        (0..n).map(|_| self.llr_sample(1.0, sigma, n0)).collect()
    }

    /// One frame of channel LLRs over a transmitted codeword `cw` at noise
    /// std `sigma`. BPSK: bit `b` → `1 - 2b` (`+1` for 0, `-1` for 1).
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

    #[test]
    fn test_splitmix64_reference_stream() {
        let mut src = AwgnLlrSource::new(0);
        // SplitMix64(seed = 0) reference outputs (`@/citation/Vigna2015`).
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

    #[test]
    fn test_all_zero_equals_zero_codeword() {
        let mut a = AwgnLlrSource::new(0x1234);
        let mut b = AwgnLlrSource::new(0x1234);
        let fa = a.frame_all_zero(33, 0.8);
        let fb = b.frame_for_codeword(&BitVec::zeros(33), 0.8);
        assert_eq!(fa, fb);
    }

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
