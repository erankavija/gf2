//! DVB-T2 column-row (block) bit interleaver.
//!
//! Implements the two-stage bit interleaving process defined in
//! ETSI EN 302 755 v1.4.1, §6.1.3 ("Bit Interleaving").
//!
//! # Algorithm
//!
//! Given an FECFRAME of `N` coded bits mapped to `N / η_mod` cells with
//! `η_mod` bits per cell (modulation order), the interleaver proceeds in
//! two stages:
//!
//! **Stage 1 — column-row write/read.**
//! Bits are written column by column into a matrix of dimensions
//! `Nc` (columns) × `Nr` (rows) and read back row by row.
//! Parameters `(Nc, Nr)` depend on the FECFRAME size and the
//! modulation order according to Table 9 / Table 9a of the spec.
//!
//! **Stage 2 — column twist (16-QAM and 64-QAM only).**
//! Before reading, each column `c` is cyclically rotated upwards by a
//! twist offset `t_c` (tabulated in Table 9 of the spec), so the bit
//! read from row `r` of column `c` originates from row
//! `(r + t_c) mod Nr`.
//! QPSK has no twist (`t_c = 0` for all `c`).
//!
//! # Scope
//!
//! This implementation covers four LDPC code rates crossed with
//! QPSK, 16-QAM, and 64-QAM, for both Normal (64800 bits) and
//! Short (16200 bits) FECFRAMEs:
//!
//! * Rate 1/2, Rate 2/3, Rate 3/4 — original in-scope rates.
//! * Rate 3/5 (Normal frame only) — added to support end-to-end
//!   testing against VV001-CR35 test vectors.
//!
//! Short-frame Rate 3/5 is not in scope for this implementation.
//! Note: the VV001-CR35 ETSI reference vectors use 256-QAM with
//! cell interleaving (§6.1.4/§6.1.5), which is beyond the scope
//! of this module (§6.1.3 only). Full chain validation will be
//! addressed in a separate issue.
//!
//! # References
//!
//! - ETSI EN 302 755 v1.4.1, §6.1.3, Table 9 (Normal FECFRAME),
//!   Table 9a (Short FECFRAME).

use crate::bch::CodeRate;
use crate::ldpc::dvb_t2::params::FrameSize;
use gf2_core::BitVec;

use crate::llr::Llr;

/// Modulation order for DVB-T2 bit-interleaver parameterisation.
///
/// Only QAM orders that produce distinct interleaver configurations are
/// represented (QPSK, 16-QAM, 64-QAM). 256-QAM is not in scope for
/// the §6.1.3 bit-only interleaver; it additionally requires the
/// cell word demux and cell interleaver stages (§6.1.4/§6.1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DvbT2Modulation {
    /// QPSK: 2 bits per cell, no column twist.
    Qpsk,
    /// 16-QAM: 4 bits per cell, column-twist per Table 9 / 9a.
    Qam16,
    /// 64-QAM: 6 bits per cell, column-twist per Table 9 / 9a.
    Qam64,
}

impl DvbT2Modulation {
    /// Number of coded bits per QAM cell (η_mod in the spec).
    pub fn bits_per_cell(self) -> usize {
        match self {
            DvbT2Modulation::Qpsk => 2,
            DvbT2Modulation::Qam16 => 4,
            DvbT2Modulation::Qam64 => 6,
        }
    }
}

/// MODCOD selector for the DVB-T2 bit interleaver.
///
/// Selects the interleaver parameters `(Nc, Nr, twist[])` from
/// Tables 9 and 9a of ETSI EN 302 755 v1.4.1 §6.1.3.
///
/// # Arguments
///
/// * `frame_size` — Normal (64800) or Short (16200) FECFRAME.
/// * `code_rate` — LDPC code rate.  Rates 1/2, 2/3, 3/4, and 3/5
///   (Normal frame only) are supported; other rates will panic.
///   Short-frame Rate 3/5 is not in scope.
/// * `modulation` — QPSK, 16-QAM, or 64-QAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DvbT2Modcod {
    /// FECFRAME size.
    pub frame_size: FrameSize,
    /// LDPC code rate.
    pub code_rate: CodeRate,
    /// Modulation order.
    pub modulation: DvbT2Modulation,
}

impl DvbT2Modcod {
    /// Constructs a new MODCOD descriptor.
    pub fn new(frame_size: FrameSize, code_rate: CodeRate, modulation: DvbT2Modulation) -> Self {
        DvbT2Modcod {
            frame_size,
            code_rate,
            modulation,
        }
    }
}

/// Interleaver configuration derived from the spec tables.
///
/// All field values are verbatim from ETSI EN 302 755 v1.4.1 §6.1.3.
#[derive(Debug, Clone)]
struct InterleaverConfig {
    /// Number of columns (Nc = η_mod = bits per cell).
    nc: usize,
    /// Number of rows (Nr = N / Nc where N is FECFRAME length).
    nr: usize,
    /// Column-twist offsets, one per column (tc[0..Nc]).
    ///
    /// From Table 9 (Normal FECFRAME) or Table 9a (Short FECFRAME).
    /// QPSK has all zeros; 16-QAM and 64-QAM have non-zero entries.
    twist: Vec<usize>,
}

impl InterleaverConfig {
    /// Derive interleaver parameters from a MODCOD descriptor.
    ///
    /// # Panics
    ///
    /// Panics if `modcod.code_rate` is not one of Rate1_2, Rate2_3,
    /// Rate3_4, or Rate3_5 (Normal frame only).  Short-frame Rate3_5
    /// is not in scope and will panic.
    fn from_modcod(modcod: DvbT2Modcod) -> Self {
        let n = match modcod.frame_size {
            FrameSize::Normal => 64800,
            FrameSize::Short => 16200,
        };
        let nc = modcod.modulation.bits_per_cell();
        let nr = n / nc;

        // Column-twist values are quoted verbatim from:
        //
        // ETSI EN 302 755 v1.4.1, §6.1.3
        //
        // Table 9 — Normal FECFRAME interleaver parameters
        // ┌──────────┬────┬───────┬───────────────────────────────────────┐
        // │ Modul.   │ Nc │  Nr   │ twist offsets tc[0] … tc[Nc-1]        │
        // ├──────────┼────┼───────┼───────────────────────────────────────┤
        // │ QPSK     │  2 │ 32400 │ 0, 0                                  │
        // │ 16-QAM   │  4 │ 16200 │ 0, 2, 4, 4                            │
        // │ 64-QAM   │  6 │ 10800 │ 0, 7, 20, 20, 21, 7                   │
        // └──────────┴────┴───────┴───────────────────────────────────────┘
        //
        // Table 9a — Short FECFRAME interleaver parameters
        // ┌──────────┬────┬───────┬───────────────────────────────────────┐
        // │ Modul.   │ Nc │  Nr   │ twist offsets tc[0] … tc[Nc-1]        │
        // ├──────────┼────┼───────┼───────────────────────────────────────┤
        // │ QPSK     │  2 │  8100 │ 0, 0                                  │
        // │ 16-QAM   │  4 │  4050 │ 0, 2, 4, 4                            │
        // │ 64-QAM   │  6 │  2700 │ 0, 7, 20, 20, 21, 7                   │
        // └──────────┴────┴───────┴───────────────────────────────────────┘
        //
        // Note: the twist offsets are independent of frame size and code
        // rate — they depend solely on the modulation order.
        //
        // The code rate is validated to ensure only in-scope rates are used;
        // the twist tables themselves are modulation-dependent only.
        match (modcod.code_rate, modcod.frame_size) {
            (CodeRate::Rate1_2 | CodeRate::Rate2_3 | CodeRate::Rate3_4, _) => {}
            (CodeRate::Rate3_5, FrameSize::Normal) => {}
            (CodeRate::Rate3_5, FrameSize::Short) => {
                panic!("DvbT2BitInterleaver: Rate3_5 Short frame is not in scope")
            }
            (other, _) => panic!(
                "DvbT2BitInterleaver: code rate {:?} is not in scope \
                 (supported: Rate1_2, Rate2_3, Rate3_4, Rate3_5 Normal)",
                other
            ),
        }

        let twist = match modcod.modulation {
            DvbT2Modulation::Qpsk => vec![0, 0],
            DvbT2Modulation::Qam16 => vec![0, 2, 4, 4],
            DvbT2Modulation::Qam64 => vec![0, 7, 20, 20, 21, 7],
        };

        InterleaverConfig { nc, nr, twist }
    }
}

/// DVB-T2 column-row block bit interleaver.
///
/// Implements the two-stage bit interleaving process from
/// ETSI EN 302 755 v1.4.1 §6.1.3.  Bits are written into an
/// `Nr × Nc` matrix column by column, then read back row by row
/// (with a per-column cyclic twist for 16-QAM and 64-QAM).
///
/// # Construction
///
/// ```
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
///     DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
/// };
/// use gf2_coding::ldpc::dvb_t2::FrameSize;
/// use gf2_coding::CodeRate;
///
/// let modcod = DvbT2Modcod::new(
///     FrameSize::Normal,
///     CodeRate::Rate1_2,
///     DvbT2Modulation::Qam16,
/// );
/// let interleaver = DvbT2BitInterleaver::new(modcod);
/// ```
///
/// # Arguments (for each method)
///
/// See individual method documentation.
///
/// # Complexity
///
/// Construction: O(Nc · Nr) to precompute the permutation.
/// `interleave` / `deinterleave`: O(N) where N = Nc · Nr.
/// `deinterleave_llrs`: O(N).
#[derive(Debug, Clone)]
pub struct DvbT2BitInterleaver {
    /// Interleaver configuration (Nc, Nr, twist).
    config: InterleaverConfig,
    /// Forward permutation: `forward[i]` is the output index for input
    /// bit `i`.  Size = Nc × Nr.
    forward: Vec<usize>,
    /// Inverse permutation: `inverse[j]` is the input index for output
    /// bit `j`.  Size = Nc × Nr.
    inverse: Vec<usize>,
}

impl DvbT2BitInterleaver {
    /// Creates a new interleaver for the given MODCOD.
    ///
    /// Precomputes forward and inverse permutation tables derived from
    /// the ETSI EN 302 755 v1.4.1 §6.1.3 column-row algorithm.
    ///
    /// # Arguments
    ///
    /// * `modcod` — FECFRAME size, code rate, and modulation order.
    ///
    /// # Panics
    ///
    /// Panics if `modcod.code_rate` is not one of `Rate1_2`, `Rate2_3`,
    /// `Rate3_4`, or `Rate3_5` (Normal frame only).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    ///     DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
    /// };
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    ///
    /// let modcod = DvbT2Modcod::new(
    ///     FrameSize::Short,
    ///     CodeRate::Rate2_3,
    ///     DvbT2Modulation::Qam64,
    /// );
    /// let interleaver = DvbT2BitInterleaver::new(modcod);
    /// assert_eq!(interleaver.frame_bits(), 16200);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(Nc · Nr) time and space.
    pub fn new(modcod: DvbT2Modcod) -> Self {
        let config = InterleaverConfig::from_modcod(modcod);
        let n = config.nc * config.nr;

        // Build the inverse permutation following ETSI EN 302 755 v1.4.1 §6.1.3:
        //
        //   Write (column-by-column): input bit `i` is placed at column
        //          `c = i / Nr`, row `r = i mod Nr` of the matrix.
        //          The input index stored at (row r, col c) is `c * Nr + r`.
        //
        //   Read with twist (row-by-row): the output bit at position `j`
        //          reads from row `r = j / Nc`, column `c = j mod Nc`,
        //          but with per-column cyclic twist applied:
        //          actual_row = (j / Nc + tc[j mod Nc]) mod Nr
        //          actual_col = j mod Nc
        //
        //   Spec formula (§6.1.3):
        //     output[j] = input[(j mod Nc) * Nr + ((j / Nc + tc[j mod Nc]) mod Nr)]
        //
        //   Therefore:
        //     inverse[j] = (j mod Nc) * Nr + ((j / Nc + tc[j mod Nc]) mod Nr)

        let nc = config.nc;
        let nr = config.nr;
        let inverse: Vec<usize> = (0..n)
            .map(|j| {
                let c = j % nc;
                let r = j / nc;
                let src_row = (r + config.twist[c]) % nr;
                c * nr + src_row
            })
            .collect();

        // Forward: for each input index i, find where it appears in inverse.
        let mut forward = vec![0usize; n];
        for (out, &src) in inverse.iter().enumerate() {
            forward[src] = out;
        }

        DvbT2BitInterleaver {
            config,
            forward,
            inverse,
        }
    }

    /// Total number of bits in one FECFRAME (Nc × Nr).
    pub fn frame_bits(&self) -> usize {
        self.config.nc * self.config.nr
    }

    /// Number of interleaver columns (Nc = bits per QAM cell).
    pub fn num_columns(&self) -> usize {
        self.config.nc
    }

    /// Number of interleaver rows (Nr = frame_bits / Nc).
    pub fn num_rows(&self) -> usize {
        self.config.nr
    }

    /// Column-twist offsets (one per column, from Table 9 / 9a).
    pub fn twist_offsets(&self) -> &[usize] {
        &self.config.twist
    }

    /// Interleaves a bit vector according to the DVB-T2 column-row algorithm.
    ///
    /// Bits are written column by column into an `Nr × Nc` matrix (with
    /// per-column twist) and read back row by row to produce the
    /// interleaved output.
    ///
    /// # Arguments
    ///
    /// * `bits` — Input [`BitVec`] of exactly `frame_bits()` bits.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != frame_bits()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    ///     DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
    /// };
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    /// use gf2_core::BitVec;
    ///
    /// let modcod = DvbT2Modcod::new(
    ///     FrameSize::Short,
    ///     CodeRate::Rate1_2,
    ///     DvbT2Modulation::Qpsk,
    /// );
    /// let interleaver = DvbT2BitInterleaver::new(modcod);
    /// let bits = BitVec::zeros(interleaver.frame_bits());
    /// let interleaved = interleaver.interleave(&bits);
    /// assert_eq!(interleaved.len(), interleaver.frame_bits());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(N) where N = `frame_bits()`.
    pub fn interleave(&self, bits: &BitVec) -> BitVec {
        let n = self.frame_bits();
        assert_eq!(
            bits.len(),
            n,
            "DvbT2BitInterleaver::interleave: expected {} bits, got {}",
            n,
            bits.len()
        );
        let mut out = BitVec::zeros(n);
        for (i, &out_idx) in self.forward.iter().enumerate() {
            if bits.get(i) {
                out.set(out_idx, true);
            }
        }
        out
    }

    /// De-interleaves a bit vector (inverse of [`interleave`](Self::interleave)).
    ///
    /// Applying `deinterleave(interleave(x)) == x` for any valid input.
    ///
    /// # Arguments
    ///
    /// * `bits` — Interleaved [`BitVec`] of exactly `frame_bits()` bits.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != frame_bits()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    ///     DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
    /// };
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    /// use gf2_core::BitVec;
    ///
    /// let modcod = DvbT2Modcod::new(
    ///     FrameSize::Short,
    ///     CodeRate::Rate1_2,
    ///     DvbT2Modulation::Qam16,
    /// );
    /// let interleaver = DvbT2BitInterleaver::new(modcod);
    /// let bits = BitVec::zeros(interleaver.frame_bits());
    /// let interleaved = interleaver.interleave(&bits);
    /// let recovered = interleaver.deinterleave(&interleaved);
    /// assert_eq!(recovered, bits);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(N) where N = `frame_bits()`.
    pub fn deinterleave(&self, bits: &BitVec) -> BitVec {
        let n = self.frame_bits();
        assert_eq!(
            bits.len(),
            n,
            "DvbT2BitInterleaver::deinterleave: expected {} bits, got {}",
            n,
            bits.len()
        );
        let mut out = BitVec::zeros(n);
        for (out_idx, &src_idx) in self.inverse.iter().enumerate() {
            if bits.get(out_idx) {
                out.set(src_idx, true);
            }
        }
        out
    }

    /// De-interleaves a slice of LLRs (receive path inverse).
    ///
    /// Applies the same inverse permutation as [`deinterleave`](Self::deinterleave)
    /// but operates on soft LLR values rather than hard bits.  After calling
    /// this function, `output[i]` is the LLR for the bit that was at position
    /// `i` in the original pre-interleaved sequence.
    ///
    /// # Arguments
    ///
    /// * `llrs` — Slice of `frame_bits()` LLRs corresponding to the
    ///   interleaved bit positions (one per coded bit, in interleaved order).
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != frame_bits()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    ///     DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
    /// };
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    /// use gf2_coding::Llr;
    ///
    /// let modcod = DvbT2Modcod::new(
    ///     FrameSize::Short,
    ///     CodeRate::Rate1_2,
    ///     DvbT2Modulation::Qam16,
    /// );
    /// let interleaver = DvbT2BitInterleaver::new(modcod);
    /// let n = interleaver.frame_bits();
    /// let llrs: Vec<Llr> = (0..n).map(|_| Llr::new(1.0)).collect();
    /// let de_llrs = interleaver.deinterleave_llrs(&llrs);
    /// assert_eq!(de_llrs.len(), n);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(N) where N = `frame_bits()`.
    pub fn deinterleave_llrs(&self, llrs: &[Llr]) -> Vec<Llr> {
        let n = self.frame_bits();
        assert_eq!(
            llrs.len(),
            n,
            "DvbT2BitInterleaver::deinterleave_llrs: expected {} LLRs, got {}",
            n,
            llrs.len()
        );
        let mut out = vec![Llr::zero(); n];
        for (out_idx, &src_idx) in self.inverse.iter().enumerate() {
            out[src_idx] = llrs[out_idx];
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::BitVec;

    // Helper: build a BitVec from an iterator of booleans.
    fn bitvec_from_bools(bits: impl IntoIterator<Item = bool>) -> BitVec {
        let mut bv = BitVec::new();
        for b in bits {
            bv.push_bit(b);
        }
        bv
    }

    // Helper: all MODCOD combinations in scope (3 rates × 2 modulations
    // × 2 frame sizes = 12 configurations, plus QPSK for completeness).
    fn in_scope_modcods() -> Vec<DvbT2Modcod> {
        let mut v = Vec::new();
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            for &rate in &[CodeRate::Rate1_2, CodeRate::Rate2_3, CodeRate::Rate3_4] {
                for &modulation in &[
                    DvbT2Modulation::Qpsk,
                    DvbT2Modulation::Qam16,
                    DvbT2Modulation::Qam64,
                ] {
                    v.push(DvbT2Modcod::new(fs, rate, modulation));
                }
            }
        }
        v
    }

    // --- Roundtrip identity --------------------------------------------------

    /// `deinterleave(interleave(x)) == x` for all-zeros FECFRAME.
    #[test]
    fn test_roundtrip_zeros() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let input = BitVec::zeros(il.frame_bits());
            let interleaved = il.interleave(&input);
            let recovered = il.deinterleave(&interleaved);
            assert_eq!(recovered, input, "roundtrip failed for {:?}", modcod);
        }
    }

    /// `deinterleave(interleave(x)) == x` for all-ones FECFRAME.
    #[test]
    fn test_roundtrip_ones() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();
            let input = bitvec_from_bools((0..n).map(|_| true));
            let interleaved = il.interleave(&input);
            let recovered = il.deinterleave(&interleaved);
            assert_eq!(recovered, input, "roundtrip failed for {:?}", modcod);
        }
    }

    /// `deinterleave(interleave(x)) == x` for a pseudo-random pattern.
    ///
    /// Uses a simple linear-congruential generator so there is no external
    /// dependency and the test remains deterministic.
    #[test]
    fn test_roundtrip_random_pattern() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

            // LCG: a=6364136223846793005, c=1, m=2^64 (Knuth/MMIX).
            let mut state: u64 = 0xDEAD_BEEF_CAFE_1234u64;
            let input = bitvec_from_bools((0..n).map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 63) != 0
            }));

            let interleaved = il.interleave(&input);
            let recovered = il.deinterleave(&interleaved);
            assert_eq!(
                recovered, input,
                "roundtrip (random pattern) failed for {:?}",
                modcod
            );
        }
    }

    // --- Specific MODCOD roundtrip (rate × mod cross product) ---------------

    /// Exhaustive roundtrip across all 3 in-scope rates × {16-QAM, 64-QAM}
    /// for Normal frames (spec success criterion).
    #[test]
    fn test_roundtrip_all_in_scope_rates_normal() {
        for &rate in &[CodeRate::Rate1_2, CodeRate::Rate2_3, CodeRate::Rate3_4] {
            for &modulation in &[DvbT2Modulation::Qam16, DvbT2Modulation::Qam64] {
                let modcod = DvbT2Modcod::new(FrameSize::Normal, rate, modulation);
                let il = DvbT2BitInterleaver::new(modcod);
                let n = il.frame_bits();

                let mut state: u64 = 0x1234_5678_9ABC_DEF0u64;
                let input = bitvec_from_bools((0..n).map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (state >> 63) != 0
                }));

                let recovered = il.deinterleave(&il.interleave(&input));
                assert_eq!(
                    recovered, input,
                    "roundtrip failed for Normal, rate={:?}, mod={:?}",
                    rate, modulation
                );
            }
        }
    }

    /// Same cross-product for Short frames.
    #[test]
    fn test_roundtrip_all_in_scope_rates_short() {
        for &rate in &[CodeRate::Rate1_2, CodeRate::Rate2_3, CodeRate::Rate3_4] {
            for &modulation in &[DvbT2Modulation::Qam16, DvbT2Modulation::Qam64] {
                let modcod = DvbT2Modcod::new(FrameSize::Short, rate, modulation);
                let il = DvbT2BitInterleaver::new(modcod);
                let n = il.frame_bits();

                let mut state: u64 = 0xABCD_EF01_2345_6789u64;
                let input = bitvec_from_bools((0..n).map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (state >> 63) != 0
                }));

                let recovered = il.deinterleave(&il.interleave(&input));
                assert_eq!(
                    recovered, input,
                    "roundtrip failed for Short, rate={:?}, mod={:?}",
                    rate, modulation
                );
            }
        }
    }

    // --- Explicit Rate3_4 roundtrip (enforceable per success criterion) ------

    /// Rate3_4 × {16-QAM, 64-QAM} × Normal FECFRAME roundtrip.
    ///
    /// Per ETSI EN 302 755 v1.4.1 §6.1.3 Table 9 (Normal FECFRAME):
    ///   16-QAM: Nc=4, Nr=16200, N=64800, twist=[0,2,4,4]
    ///   64-QAM: Nc=6, Nr=10800, N=64800, twist=[0,7,20,20,21,7]
    /// (Nc × Nr = 64800 in both cases.)
    #[test]
    fn test_roundtrip_rate3_4_normal() {
        for &modulation in &[DvbT2Modulation::Qam16, DvbT2Modulation::Qam64] {
            let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate3_4, modulation);
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();
            assert_eq!(n, 64800, "Rate3_4 Normal frame must be 64800 bits");

            let mut state: u64 = 0xFEDC_BA98_7654_3210u64;
            let input = bitvec_from_bools((0..n).map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 63) != 0
            }));

            let recovered = il.deinterleave(&il.interleave(&input));
            assert_eq!(
                recovered, input,
                "roundtrip failed for Normal Rate3_4 {:?}",
                modulation
            );
        }
    }

    /// Rate3_4 × {16-QAM, 64-QAM} × Short FECFRAME roundtrip.
    ///
    /// Per ETSI EN 302 755 v1.4.1 §6.1.3 Table 9a (Short FECFRAME):
    ///   16-QAM: Nc=4, Nr=4050, N=16200, twist=[0,2,4,4]
    ///   64-QAM: Nc=6, Nr=2700, N=16200, twist=[0,7,20,20,21,7]
    /// (Nc × Nr = 16200 in both cases.)
    #[test]
    fn test_roundtrip_rate3_4_short() {
        for &modulation in &[DvbT2Modulation::Qam16, DvbT2Modulation::Qam64] {
            let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate3_4, modulation);
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();
            assert_eq!(n, 16200, "Rate3_4 Short frame must be 16200 bits");

            let mut state: u64 = 0x0102_0304_0506_0708u64;
            let input = bitvec_from_bools((0..n).map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 63) != 0
            }));

            let recovered = il.deinterleave(&il.interleave(&input));
            assert_eq!(
                recovered, input,
                "roundtrip failed for Short Rate3_4 {:?}",
                modulation
            );
        }
    }

    // --- LLR-domain inverse (index-tagging test) ----------------------------

    /// LLR de-interleave correctness via index tagging.
    ///
    /// Encodes each bit position as a distinct LLR value (position as f32),
    /// interleaves (hard-bit path) to produce a permuted ordering, then
    /// applies `deinterleave_llrs` to the LLRs arranged in interleaved order.
    /// After de-interleaving, `output[i]` must equal the LLR that was
    /// originally at position `i`.
    #[test]
    fn test_deinterleave_llrs_index_tagging() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

            // Tag: llr[i] = i as f32 (unique per position).
            let original_llrs: Vec<Llr> = (0..n).map(|i| Llr::new(i as f32)).collect();

            // Build interleaved order: position `j` in interleaved space
            // corresponds to source index `il.inverse[j]`.
            // So the LLR that belongs at interleaved position j is
            // original_llrs[il.inverse[j]].
            let interleaved_llrs: Vec<Llr> =
                il.inverse.iter().map(|&src| original_llrs[src]).collect();

            let recovered = il.deinterleave_llrs(&interleaved_llrs);

            for (i, (&exp, &got)) in original_llrs.iter().zip(recovered.iter()).enumerate() {
                assert_eq!(
                    exp,
                    got,
                    "LLR index-tag mismatch at position {} for {:?}: expected {}, got {}",
                    i,
                    modcod,
                    exp.value(),
                    got.value()
                );
            }
        }
    }

    /// Symmetric check: interleave_llrs via forward perm, then deinterleave,
    /// gives back original LLRs.
    #[test]
    fn test_deinterleave_llrs_roundtrip() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

            // Build some distinct LLR values.
            let original: Vec<Llr> = (0..n).map(|i| Llr::new((i % 127) as f32 - 63.0)).collect();

            // Apply forward permutation manually (simulates what the transmitter
            // would see after the interleaver reorders LLR indices).
            let mut interleaved = vec![Llr::zero(); n];
            for (i, &dst) in il.forward.iter().enumerate() {
                interleaved[dst] = original[i];
            }

            let recovered = il.deinterleave_llrs(&interleaved);
            for (i, (&exp, &got)) in original.iter().zip(recovered.iter()).enumerate() {
                assert_eq!(
                    exp, got,
                    "LLR roundtrip mismatch at position {} for {:?}",
                    i, modcod
                );
            }
        }
    }

    // --- Structural / parameter checks --------------------------------------

    /// Verify frame_bits() matches N = Nc × Nr for all MODCOD.
    #[test]
    fn test_frame_bits_matches_fecframe_length() {
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            let expected_n = match fs {
                FrameSize::Normal => 64800,
                FrameSize::Short => 16200,
            };
            for &rate in &[CodeRate::Rate1_2, CodeRate::Rate2_3, CodeRate::Rate3_4] {
                for &modulation in &[
                    DvbT2Modulation::Qpsk,
                    DvbT2Modulation::Qam16,
                    DvbT2Modulation::Qam64,
                ] {
                    let modcod = DvbT2Modcod::new(fs, rate, modulation);
                    let il = DvbT2BitInterleaver::new(modcod);
                    assert_eq!(
                        il.frame_bits(),
                        expected_n,
                        "frame_bits mismatch for {:?}",
                        modcod
                    );
                    assert_eq!(il.num_columns() * il.num_rows(), expected_n);
                }
            }
        }
    }

    /// Verify the twist offsets from Table 9 / 9a.
    ///
    /// From ETSI EN 302 755 v1.4.1 §6.1.3:
    ///   QPSK:   [0, 0]
    ///   16-QAM: [0, 2, 4, 4]
    ///   64-QAM: [0, 7, 20, 20, 21, 7]
    #[test]
    fn test_twist_offsets_match_spec() {
        // Twist values are independent of frame size / code rate.
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            let m = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
            assert_eq!(
                DvbT2BitInterleaver::new(m).twist_offsets(),
                &[0, 0],
                "QPSK twist mismatch"
            );

            let m16 = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
            assert_eq!(
                DvbT2BitInterleaver::new(m16).twist_offsets(),
                &[0, 2, 4, 4],
                "16-QAM twist mismatch"
            );

            let m64 = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
            assert_eq!(
                DvbT2BitInterleaver::new(m64).twist_offsets(),
                &[0, 7, 20, 20, 21, 7],
                "64-QAM twist mismatch"
            );
        }
    }

    /// Out-of-scope rate panics.
    #[test]
    #[should_panic(expected = "not in scope")]
    fn test_out_of_scope_rate_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate4_5, DvbT2Modulation::Qam16);
        DvbT2BitInterleaver::new(modcod);
    }

    /// Wrong-length bit vector panics on interleave.
    #[test]
    #[should_panic(expected = "expected")]
    fn test_interleave_wrong_length_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let wrong = BitVec::zeros(100);
        il.interleave(&wrong);
    }

    /// Wrong-length bit vector panics on deinterleave.
    #[test]
    #[should_panic(expected = "expected")]
    fn test_deinterleave_wrong_length_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let wrong = BitVec::zeros(50);
        il.deinterleave(&wrong);
    }

    /// Wrong-length LLR slice panics on deinterleave_llrs.
    #[test]
    #[should_panic(expected = "expected")]
    fn test_deinterleave_llrs_wrong_length_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let wrong = vec![Llr::zero(); 10];
        il.deinterleave_llrs(&wrong);
    }

    // --- QPSK permutation (non-trivial: interleaves two columns) -----------

    /// Verify the correct non-trivial QPSK permutation per §6.1.3.
    ///
    /// QPSK: Nc=2, tc=[0,0].  Short FECFRAME: Nr=8100, N=16200.
    ///
    /// Spec formula: output[j] = input[(j mod 2) * 8100 + (j/2 + 0) mod 8100]
    ///
    /// For the first 8 output positions:
    ///   output[0] = input[0*8100 + 0] = input[0]
    ///   output[1] = input[1*8100 + 0] = input[8100]
    ///   output[2] = input[0*8100 + 1] = input[1]
    ///   output[3] = input[1*8100 + 1] = input[8101]
    ///   output[4] = input[0*8100 + 2] = input[2]
    ///   output[5] = input[1*8100 + 2] = input[8102]
    ///   output[6] = input[0*8100 + 3] = input[3]
    ///   output[7] = input[1*8100 + 3] = input[8103]
    ///
    /// This is NOT identity — QPSK interleaves bits from col 0 and col 1.
    #[test]
    fn test_qpsk_permutation_non_trivial() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let nr = il.num_rows(); // 8100

        // Verify first 8 inverse entries against spec formula.
        let expected = [0, nr, 1, nr + 1, 2, nr + 2, 3, nr + 3];
        for (j, &exp) in expected.iter().enumerate() {
            assert_eq!(
                il.inverse[j], exp,
                "QPSK inverse[{}]: expected input[{}], got input[{}]",
                j, exp, il.inverse[j]
            );
        }

        // Also verify non-identity: output[1] != input[1] (it's input[Nr]).
        assert_ne!(
            il.inverse[1], 1,
            "QPSK must NOT be identity: inverse[1] should be {} (Nr), not 1",
            nr
        );
    }

    /// Spec-compliance forward-only test for QPSK.
    ///
    /// Constructs an input with specific bits set at positions 0, Nr, 1, Nr+1
    /// (matching the QPSK matrix write layout), runs interleave, and asserts
    /// the output matches the hand-derived expected pattern per §6.1.3.
    ///
    /// With Nr=8100, Nc=2, tc=[0,0]:
    ///   Input bits set at positions: 0, 8100, 1, 8101
    ///   Spec formula output[j] = input[(j%2)*Nr + j/2]:
    ///     output[0] = input[0]    = 1  (set)
    ///     output[1] = input[8100] = 1  (set)
    ///     output[2] = input[1]    = 1  (set)
    ///     output[3] = input[8101] = 1  (set)
    ///     output[4] = input[2]    = 0
    ///     ...all others = 0
    #[test]
    fn test_qpsk_spec_compliance_forward() {
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let n = il.frame_bits(); // 64800
        let nr = il.num_rows(); // 32400

        // Set input bits at positions 0, Nr, 1, Nr+1.
        let mut input = BitVec::zeros(n);
        input.set(0, true);
        input.set(nr, true);
        input.set(1, true);
        input.set(nr + 1, true);

        let output = il.interleave(&input);

        // Per spec: output[j] = input[(j%2)*Nr + j/2]
        // output[0] = input[0]     = 1
        // output[1] = input[Nr]    = 1
        // output[2] = input[1]     = 1
        // output[3] = input[Nr+1]  = 1
        // output[4] = input[2]     = 0
        assert!(output.get(0), "output[0] should be 1 (from input[0])");
        assert!(output.get(1), "output[1] should be 1 (from input[Nr])");
        assert!(output.get(2), "output[2] should be 1 (from input[1])");
        assert!(output.get(3), "output[3] should be 1 (from input[Nr+1])");
        assert!(!output.get(4), "output[4] should be 0 (from input[2])");

        // Total set bits must equal 4 (permutation preserves popcount).
        let popcount: usize = (0..n).filter(|&i| output.get(i)).count();
        assert_eq!(popcount, 4, "permutation must preserve popcount");
    }

    // --- 16-QAM twist verification ------------------------------------------

    /// Verify that the twist shifts the source row correctly for 16-QAM.
    ///
    /// From Table 9 (Normal), Table 9a (Short): tc = [0, 2, 4, 4].
    ///
    /// Per spec formula (§6.1.3):
    ///   inverse[j] = (j mod Nc) * Nr + ((j / Nc + tc[j mod Nc]) mod Nr)
    ///
    /// Equivalently for output index j = r*Nc+c (r = j/Nc, c = j%Nc):
    ///   inverse[j] = c * Nr + ((r + tc[c]) mod Nr)
    #[test]
    fn test_qam16_twist_application() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
        let il = DvbT2BitInterleaver::new(modcod);
        let nc = il.num_columns(); // 4
        let nr = il.num_rows(); // 4050
        let twist = il.twist_offsets().to_vec();

        for r in 0..nr.min(10) {
            for (c, &tc) in twist.iter().enumerate() {
                let out = r * nc + c;
                let expected_src_row = (r + tc) % nr;
                // Column-major input indexing: column c starts at c*Nr.
                let expected_src = c * nr + expected_src_row;
                assert_eq!(
                    il.inverse[out], expected_src,
                    "16-QAM twist: inverse[row={},col={}] wrong",
                    r, c
                );
            }
        }
    }

    // --- Word-boundary edge cases -------------------------------------------

    /// Roundtrip identity at word-boundary bit positions (0, 1, 63, 64, 65
    /// relative to a multiple of Nc) for QPSK, 16-QAM, 64-QAM Normal frames.
    ///
    /// For each configuration, a FECFRAME is constructed with exactly one bit
    /// set at the boundary position; interleave→deinterleave must recover it.
    ///
    /// "Word boundary" here means bit index near multiples of 64 (u64 word
    /// size in BitVec), tested via offsets 0, 1, 63, 64, 65 from the start
    /// of the FECFRAME (all of which fall within the first few columns).
    #[test]
    fn test_word_boundary_roundtrip() {
        // Boundary offsets relative to index 0.
        let offsets: &[usize] = &[0, 1, 63, 64, 65];

        for &modulation in &[
            DvbT2Modulation::Qpsk,
            DvbT2Modulation::Qam16,
            DvbT2Modulation::Qam64,
        ] {
            let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, modulation);
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

            for &pos in offsets {
                if pos >= n {
                    continue;
                }
                let mut input = BitVec::zeros(n);
                input.set(pos, true);
                let recovered = il.deinterleave(&il.interleave(&input));
                assert_eq!(
                    recovered, input,
                    "word-boundary roundtrip failed at pos={} for {:?}",
                    pos, modulation
                );
            }
        }
    }

    /// Same word-boundary roundtrip for Short FECFRAME.
    #[test]
    fn test_word_boundary_roundtrip_short() {
        let offsets: &[usize] = &[0, 1, 63, 64, 65];

        for &modulation in &[
            DvbT2Modulation::Qpsk,
            DvbT2Modulation::Qam16,
            DvbT2Modulation::Qam64,
        ] {
            let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, modulation);
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

            for &pos in offsets {
                if pos >= n {
                    continue;
                }
                let mut input = BitVec::zeros(n);
                input.set(pos, true);
                let recovered = il.deinterleave(&il.interleave(&input));
                assert_eq!(
                    recovered, input,
                    "word-boundary roundtrip (Short) failed at pos={} for {:?}",
                    pos, modulation
                );
            }
        }
    }

    // --- Spec-compliance unit test (§6.1.3 formula independent evidence) ---
    //
    // This test verifies a specific single-bit mapping against the spec
    // formula for Normal, Rate1_2, 16-QAM.  It is independent evidence
    // that the permutation is correct, complementing the roundtrip tests.
    //
    // Note: the end-to-end TP07a vector validation against VV001-CR35 is
    // in the integration test `dvb_t2_bit_interleaver_tp07a.rs`.

    /// Spec-compliance forward test for Normal, Rate1_2, 16-QAM.
    ///
    /// Constructs an input with bit 0 set (column 0, row 0 in column-major
    /// layout), runs interleave, and verifies the output position per the
    /// spec formula:
    ///   output[j] = input[(j mod Nc) * Nr + ((j / Nc + tc[j mod Nc]) mod Nr)]
    ///
    /// For Normal 16-QAM: Nc=4, Nr=16200, tc=[0,2,4,4].
    ///   Input bit 0 is at column-major address c=0, r=0.
    ///   The output index j that maps to src=0 satisfies:
    ///     (j mod 4) * 16200 + (j/4 + tc[j mod 4]) mod 16200 = 0
    ///   => j mod 4 = 0 (so c=0), and (j/4 + 0) mod 16200 = 0
    ///   => j/4 = 0, so j = 0.
    ///   Therefore output[0] = 1, all others = 0.
    ///
    /// Input bit 16200 (column 1, row 0) maps to j where:
    ///   (j mod 4) = 1 and (j/4 + tc[1]) mod 16200 = 0
    ///   => j/4 = (16200 - 2) mod 16200 = 16198, j = 16198*4 + 1 = 64793.
    ///
    /// This is a formula-correctness check, not the TP07a end-to-end
    /// vector validation (which lives in the integration test file
    /// `crates/gf2-coding/tests/dvb_t2_bit_interleaver_tp07a.rs`).
    #[test]
    fn test_16qam_normal_spec_compliance_forward() {
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
        let il = DvbT2BitInterleaver::new(modcod);
        let n = il.frame_bits(); // 64800
        let nr = il.num_rows(); // 16200
        let nc = il.num_columns(); // 4
        let twist = il.twist_offsets().to_vec(); // [0, 2, 4, 4]

        // Test 1: input bit 0 (c=0, r=0) → output position 0.
        {
            let mut input = BitVec::zeros(n);
            input.set(0, true);
            let output = il.interleave(&input);
            // Expected output position j=0: (0%4)*Nr + (0/4+tc[0])%Nr = 0 → src=0.
            assert!(
                output.get(0),
                "16-QAM Normal: input[0] should map to output[0]"
            );
            let popcount: usize = (0..n).filter(|&i| output.get(i)).count();
            assert_eq!(popcount, 1, "popcount must be 1");
        }

        // Test 2: input bit 1*Nr = 16200 (c=1, r=0) → output position 64793.
        // j such that j%4=1 and (j/4 + tc[1]) % Nr = 0
        // => j/4 = (Nr - tc[1]) % Nr = (16200 - 2) % 16200 = 16198
        // => j = 16198*4 + 1 = 64793
        {
            let src = nr; // column 1, row 0 (address = 1*Nr + 0 = Nr)
            let expected_j = ((nr - twist[1]) % nr) * nc + 1;
            let mut input = BitVec::zeros(n);
            input.set(src, true);
            let output = il.interleave(&input);
            assert!(
                output.get(expected_j),
                "16-QAM Normal: input[{}] (c=1,r=0) should map to output[{}], twist[1]={}",
                src,
                expected_j,
                twist[1]
            );
            let popcount: usize = (0..n).filter(|&i| output.get(i)).count();
            assert_eq!(popcount, 1, "popcount must be 1");
        }

        // Test 3: verify forward[i] == j iff inverse[j] == i (sanity on first Nr entries).
        for i in 0..nr.min(20) {
            let j = il.forward[i];
            assert_eq!(
                il.inverse[j], i,
                "forward/inverse consistency failed at i={}",
                i
            );
        }
    }
}
