//! DVB-T2 bit interleaver: parity interleaving + column-twist interleaving.
//!
//! Implements the full two-stage bit interleaving process defined in
//! ETSI EN 302 755 v1.4.1, §6.1.3 ("Bit Interleaving").
//!
//! # Algorithm
//!
//! Given an FECFRAME of `N_ldpc` coded bits (the LDPC encoder output Λ),
//! the bit interleaver applies two sequential stages to produce V:
//!
//! **Stage 1 — parity interleaving (16-QAM and 64-QAM only).**
//! The `K_ldpc` information bits pass through unchanged.  The parity
//! bits are permuted by:
//!
//! ```text
//! u_i = λ_i                                     for 0 ≤ i < K_ldpc
//! u_{K_ldpc + 360·t + s} = λ_{K_ldpc + Q·s + t}  for 0 ≤ s < 360, 0 ≤ t < Q
//! ```
//!
//! where `Q = (N_ldpc − K_ldpc) / 360`.
//!
//! **Stage 2 — column-twist interleaving (16-QAM and 64-QAM only).**
//! The parity-interleaved bits U are serially written column-wise into
//! a matrix of `Nc` columns × `Nr` rows (the write start position of
//! column `c` is twisted by `tc[c]`), then read out row-wise:
//!
//! ```text
//! Write: u_i → column  c_i = i / Nr,  row  r_i = (i + tc[c_i]) mod Nr
//! Read:  v_j ← column  c_j = j mod Nc, row  r_j = j / Nc
//! ```
//!
//! Parameters `(Nc, Nr, tc[])` come from Tables 9 and 10 of the spec
//! (different from η_mod — see table below).
//!
//! For **QPSK** the entire §6.1.3 section does not apply; the bits pass
//! through without interleaving (Nc = 2, Nr = N/2, all twists zero).
//!
//! # Interleaver parameters (ETSI EN 302 755 v1.4.1, Tables 9 and 10)
//!
//! ```text
//! ┌─────────┬──────────────┬────┬───────┬──────────────────────────────────┐
//! │ Modul.  │ N_ldpc       │ Nc │  Nr   │ twist offsets tc[0..Nc-1]         │
//! ├─────────┼──────────────┼────┼───────┼──────────────────────────────────┤
//! │ QPSK    │ 64 800       │  2 │ 32400 │ 0, 0                              │
//! │ QPSK    │ 16 200       │  2 │  8100 │ 0, 0                              │
//! │ 16-QAM  │ 64 800       │  8 │  8100 │ 0, 0, 2, 4, 4, 5, 7, 7           │
//! │ 16-QAM  │ 16 200       │  8 │  2025 │ 0, 0, 0, 1, 7, 20, 20, 21        │
//! │ 64-QAM  │ 64 800       │ 12 │  5400 │ 0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9│
//! │ 64-QAM  │ 16 200       │ 12 │  1350 │ 0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7│
//! └─────────┴──────────────┴────┴───────┴──────────────────────────────────┘
//! ```
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
//! - ETSI EN 302 755 v1.4.1, §6.1.3, Table 9 (combined Normal+Short),
//!   Table 10 (column twisting parameters).

use crate::bch::CodeRate;
use crate::ldpc::dvb_t2::params::{DvbParams, FrameSize};
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
/// Selects the interleaver parameters `(Nc, Nr, twist[], K_ldpc, Q_ldpc)` from
/// Tables 9 and 10 of ETSI EN 302 755 v1.4.1 §6.1.3.
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
/// All field values are verbatim from ETSI EN 302 755 v1.4.1 §6.1.3,
/// Tables 9 and 10.
#[derive(Debug, Clone)]
struct InterleaverConfig {
    /// Number of columns (Nc from Table 9 — NOT equal to η_mod).
    nc: usize,
    /// Number of rows (Nr = N_ldpc / Nc, from Table 9).
    nr: usize,
    /// Column-twist offsets, one per column (tc[0..Nc]), from Table 10.
    ///
    /// QPSK has all zeros; 16-QAM (Nc=8) and 64-QAM (Nc=12) have
    /// values from Table 10.
    twist: Vec<usize>,
    /// K_ldpc: number of information bits in the FECFRAME.
    /// Used to compute Q_ldpc for the parity interleaving stage.
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
    /// Rate3_4, or Rate3_5 (Normal frame only).  Short-frame Rate3_5
    /// is not in scope and will panic.
    fn from_modcod(modcod: DvbT2Modcod) -> Self {
        let n = match modcod.frame_size {
            FrameSize::Normal => 64800,
            FrameSize::Short => 16200,
        };

        // Validate code rate scope.
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

        // Nc and twist values from ETSI EN 302 755 v1.4.1, Tables 9 and 10.
        //
        // NOTE: Nc is NOT equal to η_mod (bits per cell). Per Table 9:
        //   QPSK:   Nc = 2  (η_mod = 2)
        //   16-QAM: Nc = 8  (η_mod = 4)
        //   64-QAM: Nc = 12 (η_mod = 6)
        //
        // Table 10: Column twisting parameter tc
        // ┌─────────┬────┬──────────────┬──────────────────────────────────────┐
        // │ Modul.  │ Nc │   N_ldpc     │ tc[0] … tc[Nc-1]                     │
        // ├─────────┼────┼──────────────┼──────────────────────────────────────┤
        // │ QPSK    │  2 │ 64800/16200  │ 0, 0                                 │
        // │ 16-QAM  │  8 │ 64 800       │ 0, 0, 2, 4, 4, 5, 7, 7               │
        // │ 16-QAM  │  8 │ 16 200       │ 0, 0, 0, 1, 7, 20, 20, 21            │
        // │ 64-QAM  │ 12 │ 64 800       │ 0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9  │
        // │ 64-QAM  │ 12 │ 16 200       │ 0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7  │
        // └─────────┴────┴──────────────┴──────────────────────────────────────┘
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

        // K_ldpc and Q_ldpc for parity interleaving.
        // Per §6.1.3: Q_ldpc = (N_ldpc − K_ldpc) / 360.
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
/// Implements the full two-stage bit interleaving process from
/// ETSI EN 302 755 v1.4.1 §6.1.3.
///
/// **Stage 1 — parity interleaving** (16-QAM and 64-QAM only):
/// Information bits are unchanged; parity bits at positions `K_ldpc..N_ldpc`
/// are permuted by `u_{K+360t+s} = λ_{K+Q·s+t}` (0 ≤ s < 360, 0 ≤ t < Q).
///
/// **Stage 2 — column-twist interleaving** (16-QAM and 64-QAM only):
/// Parity-interleaved bits U are written column-wise into an `Nc × Nr`
/// matrix (with per-column twist) and read row-wise to produce V.
///
/// Both stages are composed into a single precomputed permutation table
/// so `interleave` and `deinterleave` run in O(N) with no intermediate
/// allocation.
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
    /// Interleaver configuration (Nc, Nr, twist, K_ldpc, Q_ldpc).
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
    /// Precomputes forward and inverse permutation tables.  For QPSK,
    /// ETSI EN 302 755 v1.4.1 §6.1.3 does not apply: both tables are
    /// the identity permutation and bits pass through unchanged.  For
    /// 16-QAM and 64-QAM the tables compose the parity-interleaving
    /// stage (stage 1) and the column-twist stage (stage 2) from §6.1.3.
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

        // QPSK: §6.1.3 is scoped to "16-QAM, 64-QAM and 256-QAM" only.
        // The LDPC output Λ passes through unchanged — both permutation
        // tables are the identity.
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

        // Build the composed permutation in two steps.
        //
        // Step A: parity interleaving (§6.1.3 stage 1).
        //
        //   parity_perm[i] = i                          for 0 ≤ i < K_ldpc
        //   parity_perm[K + 360·t + s] = K + Q·s + t   for 0 ≤ s < 360, 0 ≤ t < Q
        //
        //   Q = (N − K) / 360.
        let mut parity_perm: Vec<usize> = (0..n).collect();
        if q > 0 {
            for s in 0..360usize {
                for t in 0..q {
                    parity_perm[k + 360 * t + s] = k + q * s + t;
                }
            }
        }

        // Step B: column-twist interleaving (§6.1.3 stage 2).
        //
        //   Write: u_i → col c_i = i / Nr, row r_i = (i + tc[c_i]) mod Nr
        //   Read:  v_j ← col c_j = j mod Nc, row r_j = j / Nc
        //
        //   It is easier to build the *inverse* directly:
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

        // Forward is the inverse of the inverse permutation.
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
    ///
    /// Note: 16-QAM has Nc=8 (not 4), 64-QAM has Nc=12 (not 6), per
    /// Table 9 of ETSI EN 302 755 v1.4.1.
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

    /// Interleaves a bit vector according to the DVB-T2 §6.1.3 algorithm.
    ///
    /// Applies the composed permutation that encodes both the parity
    /// interleaving stage and the column-twist stage.
    ///
    /// # Arguments
    ///
    /// * `bits` — Input [`BitVec`] of exactly `frame_bits()` bits
    ///   (the LDPC encoder output Λ).
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

    // Helper: all MODCOD combinations in scope (3 rates × 3 modulations
    // × 2 frame sizes = 18 configurations).
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
    ///   16-QAM: Nc=8, Nr=8100, N=64800, twist=[0,0,2,4,4,5,7,7]
    ///   64-QAM: Nc=12, Nr=5400, N=64800, twist=[0,0,2,2,3,4,4,5,5,7,8,9]
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
    /// Per ETSI EN 302 755 v1.4.1 §6.1.3 Table 9 (Short FECFRAME):
    ///   16-QAM: Nc=8, Nr=2025, N=16200, twist=[0,0,0,1,7,20,20,21]
    ///   64-QAM: Nc=12, Nr=1350, N=16200, twist=[0,0,0,2,2,2,3,3,3,6,7,7]
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

    /// Verify the Nc values from Table 9 of ETSI EN 302 755 v1.4.1.
    ///
    /// QPSK: Nc=2, 16-QAM: Nc=8, 64-QAM: Nc=12.
    /// These are frame-size-independent (Nc depends only on modulation).
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

    /// Verify the twist offsets from Table 10 of ETSI EN 302 755 v1.4.1.
    ///
    /// Table 10 (Column twisting parameter tc):
    ///   QPSK:               [0, 0]
    ///   16-QAM Normal:      [0, 0, 2, 4, 4, 5, 7, 7]
    ///   16-QAM Short:       [0, 0, 0, 1, 7, 20, 20, 21]
    ///   64-QAM Normal:      [0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9]
    ///   64-QAM Short:       [0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7]
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

        // 16-QAM Normal: Table 10 gives [0, 0, 2, 4, 4, 5, 7, 7].
        let m16n = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
        assert_eq!(
            DvbT2BitInterleaver::new(m16n).twist_offsets(),
            &[0usize, 0, 2, 4, 4, 5, 7, 7],
            "16-QAM Normal twist mismatch"
        );

        // 16-QAM Short: Table 10 gives [0, 0, 0, 1, 7, 20, 20, 21].
        let m16s = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
        assert_eq!(
            DvbT2BitInterleaver::new(m16s).twist_offsets(),
            &[0usize, 0, 0, 1, 7, 20, 20, 21],
            "16-QAM Short twist mismatch"
        );

        // 64-QAM Normal: Table 10 gives [0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9].
        let m64n = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
        assert_eq!(
            DvbT2BitInterleaver::new(m64n).twist_offsets(),
            &[0usize, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9],
            "64-QAM Normal twist mismatch"
        );

        // 64-QAM Short: Table 10 gives [0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7].
        let m64s = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
        assert_eq!(
            DvbT2BitInterleaver::new(m64s).twist_offsets(),
            &[0usize, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7],
            "64-QAM Short twist mismatch"
        );
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

    // --- QPSK identity (§6.1.3 out of scope for QPSK) ----------------------

    /// QPSK passes bits through unchanged: `interleave(bits) == bits`.
    ///
    /// ETSI EN 302 755 v1.4.1 §6.1.3 is titled "Bit Interleaving
    /// (for 16-QAM, 64-QAM and 256-QAM)" — QPSK is explicitly out of scope.
    /// For QPSK the LDPC output Λ passes through the §6.1.3 stage as-is
    /// (identity permutation); no parity interleaving and no column-twist
    /// interleaving are applied.
    #[test]
    fn test_qpsk_identity() {
        // Test both Normal and Short frames, two rates, with a pseudo-random input.
        for &fs in &[FrameSize::Normal, FrameSize::Short] {
            for &rate in &[CodeRate::Rate1_2, CodeRate::Rate3_4] {
                let modcod = DvbT2Modcod::new(fs, rate, DvbT2Modulation::Qpsk);
                let il = DvbT2BitInterleaver::new(modcod);
                let n = il.frame_bits();

                // LCG pseudo-random input.
                let mut state: u64 = 0xC0FFEE_DEADBEEF_u64;
                let input = bitvec_from_bools((0..n).map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (state >> 63) != 0
                }));

                // Forward: interleave must be identity for QPSK.
                let interleaved = il.interleave(&input);
                assert_eq!(
                    interleaved, input,
                    "QPSK interleave must be identity for {:?} {:?}",
                    fs, rate
                );

                // Inverse: deinterleave must also be identity.
                let deinterleaved = il.deinterleave(&input);
                assert_eq!(
                    deinterleaved, input,
                    "QPSK deinterleave must be identity for {:?} {:?}",
                    fs, rate
                );
            }
        }
    }

    // --- 64-QAM column-twist verification (spec Table 9/10) -----------------

    /// Verify the column-twist interleaver indices for 64-QAM Normal FECFRAME.
    ///
    /// Per ETSI EN 302 755 v1.4.1 §6.1.3, for 64-QAM Normal (Nc=12, Nr=5400):
    ///
    ///   v[0..11] = u[0, 5400, 16198, 21598, 26997, 32396, 37796,
    ///                43195, 48595, 53993, 59392, 64791]
    ///
    /// This tests only the column-twist stage (using QPSK to avoid parity
    /// interleaving, then manually verifying the formula with Rate1_2 64-QAM).
    ///
    /// For Rate1_2 64-QAM: Q = (64800-32400)/360 = 90, so parity interleaving
    /// DOES modify indices ≥ K=32400.  The first 12 outputs (j=0..11) read from
    /// rows 0 of each column:
    ///   v[j] source = col*Nr + (0 - tc[col] + Nr) % Nr  (for row_j=0)
    #[test]
    fn test_64qam_normal_col_twist_spec_example() {
        // Use Rate1_2 Normal 64-QAM.
        // K = 32400, Q = 90, N = 64800, Nc = 12, Nr = 5400
        // tc = [0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9]
        // For j in [0..12] (row=0):
        //   v[j] reads from col=j, row=(0-tc[j]+5400)%5400 of U.
        //   For col 0: row=(0-0+5400)%5400=0, src=0*5400+0=0 → U[0]=Λ[0]
        //   For col 1: row=(0-0+5400)%5400=0, src=1*5400+0=5400 → U[5400]=Λ[5400]
        //   For col 2: row=(0-2+5400)%5400=5398, src=2*5400+5398=16198 → U[16198]=Λ[16198]
        //     (all these indices < K=32400, so parity perm is identity there)
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam64);
        let il = DvbT2BitInterleaver::new(modcod);
        let nr = il.num_rows(); // 5400
        let nc = il.num_columns(); // 12
        let twist = il.twist_offsets().to_vec(); // [0,0,2,2,3,4,4,5,5,7,8,9]

        assert_eq!(nc, 12, "64-QAM Normal must have Nc=12");
        assert_eq!(nr, 5400, "64-QAM Normal must have Nr=5400");
        assert_eq!(twist, vec![0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9]);

        // Spec example values for j=0..11 (first row of output):
        let spec_example = [
            0usize, 5400, 16198, 21598, 26997, 32396, 37796, 43195, 48595, 53993, 59392, 64791,
        ];
        // NOTE: indices ≥ K=32400 pass through parity interleaving, so the
        // inverse[j] for indices with col ≥ 6 will differ from spec_example
        // because parity_perm maps them.  Only verify columns 0..5 (src < K).
        // For col 2: src = 16198 < 32400, so parity perm is identity. ✓
        for (j, &expected) in spec_example.iter().enumerate().take(6) {
            // cols 0..5 all have src < K=32400 for the j=col case
            assert_eq!(
                il.inverse[j], expected,
                "64-QAM inverse[{}] should be {} (spec example), got {}",
                j, expected, il.inverse[j]
            );
        }
    }

    // --- Word-boundary edge cases -------------------------------------------

    /// Roundtrip identity at word-boundary bit positions (0, 1, 63, 64, 65
    /// relative to a multiple of Nc) for QPSK, 16-QAM, 64-QAM Normal frames.
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
}
