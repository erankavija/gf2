//! DVB-T2 bit interleaver: parity interleaving + column-twist interleaving
//! (`@/citation/Etsi2015` §6.1.3, Tables 9 and 10).

use crate::bch::CodeRate;
use crate::ldpc::dvb_t2::params::{DvbParams, FrameSize};
use gf2_core::BitVec;

use crate::llr::Llr;

/// Modulation order for DVB-T2 bit-interleaver parameterisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DvbT2Modulation {
    /// QPSK: 2 bits per cell, no §6.1.3 interleaving applied.
    Qpsk,
    /// 16-QAM: 4 bits per cell, parity interleave + column-twist per
    /// Tables 9 and 10 (Nc=8).
    Qam16,
    /// 64-QAM: 6 bits per cell, parity interleave + column-twist per
    /// Tables 9 and 10 (Nc=12).
    Qam64,
}

impl DvbT2Modulation {
    /// Number of coded bits per QAM cell (η_mod in the spec).
    ///
    /// Note: η_mod differs from the interleaver column count `Nc`.
    /// For 16-QAM η_mod = 4 but Nc = 8; for 64-QAM η_mod = 6 but Nc = 12.
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
/// Selects the interleaver parameters `(Nc, Nr, twist[], K_ldpc, Q_ldpc)`.
/// [`DvbT2BitInterleaver::new`] panics on a code rate other than 1/2, 2/3,
/// 3/4, or 3/5 with the Normal frame.
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

/// Interleaver configuration from `@/citation/Etsi2015` §6.1.3, Tables 9
/// and 10.
#[derive(Debug, Clone)]
struct InterleaverConfig {
    /// Number of columns (Nc from Table 9 — NOT equal to η_mod).
    nc: usize,
    /// Number of rows (Nr = N_ldpc / Nc, from Table 9).
    nr: usize,
    /// Column-twist offsets, one per column (tc[0..Nc]), from Table 10;
    /// all zeros for QPSK.
    twist: Vec<usize>,
    /// K_ldpc: number of information bits in the FECFRAME (`N_ldpc` for QPSK).
    k_ldpc: usize,
    /// Q_ldpc = (N_ldpc − K_ldpc) / 360.
    /// Zero for QPSK (no parity interleaving).
    q_ldpc: usize,
}

impl InterleaverConfig {
    /// Derive interleaver parameters from a MODCOD descriptor.
    ///
    /// # Panics
    ///
    /// Panics if `modcod.code_rate` is not one of Rate1_2, Rate2_3,
    /// Rate3_4, or Rate3_5 (Normal frame only).
    fn from_modcod(modcod: DvbT2Modcod) -> Self {
        let n = match modcod.frame_size {
            FrameSize::Normal => 64800,
            FrameSize::Short => 16200,
        };

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

        // Tables 9 and 10; Nc differs from η_mod (bits per cell).
        let (nc, twist): (usize, Vec<usize>) = match (modcod.modulation, modcod.frame_size) {
            (DvbT2Modulation::Qpsk, _) => (2, vec![0, 0]),
            (DvbT2Modulation::Qam16, FrameSize::Normal) => (8, vec![0, 0, 2, 4, 4, 5, 7, 7]),
            (DvbT2Modulation::Qam16, FrameSize::Short) => (8, vec![0, 0, 0, 1, 7, 20, 20, 21]),
            (DvbT2Modulation::Qam64, FrameSize::Normal) => {
                (12, vec![0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9])
            }
            (DvbT2Modulation::Qam64, FrameSize::Short) => {
                (12, vec![0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7])
            }
        };
        let nr = n / nc;

        // For QPSK, §6.1.3 does not apply: Q_ldpc = 0 (no parity interleaving).
        let (k_ldpc, q_ldpc) = if modcod.modulation == DvbT2Modulation::Qpsk {
            (n, 0)
        } else {
            let dvb_params = DvbParams::for_code(modcod.frame_size, modcod.code_rate);
            let k = dvb_params.k;
            let q = (n - k) / 360;
            (k, q)
        };

        InterleaverConfig {
            nc,
            nr,
            twist,
            k_ldpc,
            q_ldpc,
        }
    }
}

/// DVB-T2 bit interleaver (§6.1.3): parity interleaving + column-twist.
///
/// Given an FECFRAME of `N_ldpc` coded bits (the LDPC encoder output Λ),
/// two sequential stages produce V. Both apply to 16-QAM and 64-QAM only;
/// for QPSK the bits pass through unchanged.
///
/// **Stage 1 — parity interleaving.**
/// The `K_ldpc` information bits pass through unchanged.  The parity
/// bits are permuted by:
///
/// ```text
/// u_i = λ_i                                     for 0 ≤ i < K_ldpc
/// u_{K_ldpc + 360·t + s} = λ_{K_ldpc + Q·s + t}  for 0 ≤ s < 360, 0 ≤ t < Q
/// ```
///
/// where `Q = (N_ldpc − K_ldpc) / 360`.
///
/// **Stage 2 — column-twist interleaving.**
/// The parity-interleaved bits U are serially written column-wise into
/// a matrix of `Nc` columns × `Nr` rows (the write start position of
/// column `c` is twisted by `tc[c]`), then read out row-wise:
///
/// ```text
/// Write: u_i → column  c_i = i / Nr,  row  r_i = (i + tc[c_i]) mod Nr
/// Read:  v_j ← column  c_j = j mod Nc, row  r_j = j / Nc
/// ```
///
/// Both stages are composed into one precomputed permutation table, so
/// `interleave`, `deinterleave` and `deinterleave_llrs` run in O(N) with
/// N = Nc · Nr.
#[derive(Debug, Clone)]
pub struct DvbT2BitInterleaver {
    config: InterleaverConfig,
    /// Forward permutation: `forward[i]` is the output index for input
    /// bit `i`.  Size = Nc × Nr.
    forward: Vec<usize>,
    /// Inverse permutation: `inverse[j]` is the input index for output
    /// bit `j`.  Size = Nc × Nr.
    inverse: Vec<usize>,
}

impl DvbT2BitInterleaver {
    /// Precomputes the forward and inverse permutation tables for `modcod`
    /// in O(Nc · Nr) time and space; both are the identity for QPSK.
    ///
    /// # Panics
    ///
    /// Panics if `modcod.code_rate` is not one of `Rate1_2`, `Rate2_3`,
    /// `Rate3_4`, or `Rate3_5` (Normal frame only).
    pub fn new(modcod: DvbT2Modcod) -> Self {
        let config = InterleaverConfig::from_modcod(modcod);
        let n = config.nc * config.nr;

        // §6.1.3 covers "16-QAM, 64-QAM and 256-QAM" only.
        if modcod.modulation == DvbT2Modulation::Qpsk {
            let identity: Vec<usize> = (0..n).collect();
            return DvbT2BitInterleaver {
                config,
                forward: identity.clone(),
                inverse: identity,
            };
        }

        let nc = config.nc;
        let nr = config.nr;
        let k = config.k_ldpc;
        let q = config.q_ldpc;

        let mut parity_perm: Vec<usize> = (0..n).collect();
        if q > 0 {
            for s in 0..360usize {
                for t in 0..q {
                    parity_perm[k + 360 * t + s] = k + q * s + t;
                }
            }
        }

        // The inverse is built directly:
        //
        //   inv_col_twist[j] = (j mod Nc) * Nr + ((j / Nc - tc[j mod Nc] + Nr) % Nr)
        //   inverse[j] = parity_perm[ inv_col_twist[j] ]
        let inverse: Vec<usize> = (0..n)
            .map(|j| {
                let c = j % nc;
                let r = j / nc;
                let src_row = (r + nr - config.twist[c] % nr) % nr;
                let col_twist_src = c * nr + src_row;
                parity_perm[col_twist_src]
            })
            .collect();

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

    /// Number of interleaver columns (Nc from Table 9 — NOT η_mod).
    pub fn num_columns(&self) -> usize {
        self.config.nc
    }

    /// Number of interleaver rows (Nr = frame_bits / Nc, from Table 9).
    pub fn num_rows(&self) -> usize {
        self.config.nr
    }

    /// Column-twist offsets (one per column, from Table 10).
    pub fn twist_offsets(&self) -> &[usize] {
        &self.config.twist
    }

    /// Interleaves `bits` (the LDPC encoder output Λ) according to the DVB-T2
    /// §6.1.3 algorithm.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len() != frame_bits()`.
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
    /// # Panics
    ///
    /// Panics if `bits.len() != frame_bits()`.
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

    /// De-interleaves LLRs with the inverse permutation of
    /// [`deinterleave`](Self::deinterleave): `output[i]` is the LLR of the bit
    /// at position `i` of the pre-interleaved sequence.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != frame_bits()`.
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

    fn bitvec_from_bools(bits: impl IntoIterator<Item = bool>) -> BitVec {
        let mut bv = BitVec::new();
        for b in bits {
            bv.push_bit(b);
        }
        bv
    }

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

    #[test]
    fn test_deinterleave_llrs_index_tagging() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

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

    #[test]
    fn test_deinterleave_llrs_roundtrip() {
        for modcod in in_scope_modcods() {
            let il = DvbT2BitInterleaver::new(modcod);
            let n = il.frame_bits();

            let original: Vec<Llr> = (0..n).map(|i| Llr::new((i % 127) as f32 - 63.0)).collect();

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

    /// Nc values from Table 9 of `@/citation/Etsi2015`; Nc depends only on
    /// the modulation.
    #[test]
    fn test_nc_matches_spec_table9() {
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            let qpsk = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
            assert_eq!(
                DvbT2BitInterleaver::new(qpsk).num_columns(),
                2,
                "QPSK must have Nc=2"
            );

            let qam16 = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
            assert_eq!(
                DvbT2BitInterleaver::new(qam16).num_columns(),
                8,
                "16-QAM must have Nc=8 (not η_mod=4)"
            );

            let qam64 = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
            assert_eq!(
                DvbT2BitInterleaver::new(qam64).num_columns(),
                12,
                "64-QAM must have Nc=12 (not η_mod=6)"
            );
        }
    }

    /// Twist offsets from Table 10 of `@/citation/Etsi2015`.
    #[test]
    fn test_twist_offsets_match_spec() {
        // QPSK twist is independent of frame size and code rate.
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            let m = DvbT2Modcod::new(fs, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
            assert_eq!(
                DvbT2BitInterleaver::new(m).twist_offsets(),
                &[0usize, 0],
                "QPSK twist mismatch"
            );
        }

        let m16n = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
        assert_eq!(
            DvbT2BitInterleaver::new(m16n).twist_offsets(),
            &[0usize, 0, 2, 4, 4, 5, 7, 7],
            "16-QAM Normal twist mismatch"
        );

        let m16s = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
        assert_eq!(
            DvbT2BitInterleaver::new(m16s).twist_offsets(),
            &[0usize, 0, 0, 1, 7, 20, 20, 21],
            "16-QAM Short twist mismatch"
        );

        let m64n = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
        assert_eq!(
            DvbT2BitInterleaver::new(m64n).twist_offsets(),
            &[0usize, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9],
            "64-QAM Normal twist mismatch"
        );

        let m64s = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
        assert_eq!(
            DvbT2BitInterleaver::new(m64s).twist_offsets(),
            &[0usize, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7],
            "64-QAM Short twist mismatch"
        );
    }

    #[test]
    #[should_panic(expected = "not in scope")]
    fn test_out_of_scope_rate_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate4_5, DvbT2Modulation::Qam16);
        DvbT2BitInterleaver::new(modcod);
    }

    #[test]
    #[should_panic(expected = "expected")]
    fn test_interleave_wrong_length_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let wrong = BitVec::zeros(100);
        il.interleave(&wrong);
    }

    #[test]
    #[should_panic(expected = "expected")]
    fn test_deinterleave_wrong_length_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let wrong = BitVec::zeros(50);
        il.deinterleave(&wrong);
    }

    #[test]
    #[should_panic(expected = "expected")]
    fn test_deinterleave_llrs_wrong_length_panics() {
        let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qpsk);
        let il = DvbT2BitInterleaver::new(modcod);
        let wrong = vec![Llr::zero(); 10];
        il.deinterleave_llrs(&wrong);
    }

    /// `@/citation/Etsi2015` §6.1.3 is titled "Bit Interleaving
    /// (for 16-QAM, 64-QAM and 256-QAM)", so QPSK passes Λ through unchanged.
    #[test]
    fn test_qpsk_identity() {
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            for &rate in &[CodeRate::Rate1_2, CodeRate::Rate3_4] {
                let modcod = DvbT2Modcod::new(fs, rate, DvbT2Modulation::Qpsk);
                let il = DvbT2BitInterleaver::new(modcod);
                let n = il.frame_bits();

                let mut state: u64 = 0xC0FFEE_DEADBEEF_u64;
                let input = bitvec_from_bools((0..n).map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (state >> 63) != 0
                }));

                let interleaved = il.interleave(&input);
                assert_eq!(
                    interleaved, input,
                    "QPSK interleave must be identity for {:?} {:?}",
                    fs, rate
                );

                let deinterleaved = il.deinterleave(&input);
                assert_eq!(
                    deinterleaved, input,
                    "QPSK deinterleave must be identity for {:?} {:?}",
                    fs, rate
                );
            }
        }
    }

    /// Column-twist source indices for 64-QAM Normal FECFRAME.
    ///
    /// Per `@/citation/Etsi2015` §6.1.3, for 64-QAM Normal (Nc=12, Nr=5400):
    ///
    ///   v[0..11] = u[0, 5400, 16198, 21598, 26997, 32396, 37796,
    ///                43195, 48595, 53993, 59392, 64791]
    ///
    /// Rate 1/2 has K=32400, so parity interleaving remaps the sources of
    /// columns 6..11; only columns 0..5 (source < K) are compared.
    #[test]
    fn test_64qam_normal_col_twist_spec_example() {
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
        let il = DvbT2BitInterleaver::new(modcod);
        let nr = il.num_rows();
        let nc = il.num_columns();
        let twist = il.twist_offsets().to_vec();

        assert_eq!(nc, 12, "64-QAM Normal must have Nc=12");
        assert_eq!(nr, 5400, "64-QAM Normal must have Nr=5400");
        assert_eq!(twist, vec![0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9]);

        let spec_example = [
            0usize, 5400, 16198, 21598, 26997, 32396, 37796, 43195, 48595, 53993, 59392, 64791,
        ];
        for (j, &expected) in spec_example.iter().enumerate().take(6) {
            assert_eq!(
                il.inverse[j], expected,
                "64-QAM inverse[{}] should be {} (spec example), got {}",
                j, expected, il.inverse[j]
            );
        }
    }

    /// Roundtrip identity at word-boundary bit positions (0, 1, 63, 64, 65
    /// relative to a multiple of Nc) for QPSK, 16-QAM, 64-QAM Normal frames.
    #[test]
    fn test_word_boundary_roundtrip() {
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
}
