//! Shared DVB-T2 BICM-AWGN harness wiring.
//!
//! This module is the **single source of truth** for the DVB-T2 BICM-AWGN
//! channel building blocks used by both the campaign binary
//! (`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs`, migrated to the
//! gf2-sim pipeline by `bbf6b6ee`) and the standalone
//! baseline measurement harness
//! (`dev/benchmarks/gf2-sim/baseline_runner/src/main.rs`).
//!
//! # Provided items
//!
//! - [`esn0_to_ebn0`] / [`ebn0_to_esn0`] — SNR unit conversion.
//! - [`rate_f64`] — Code rate as a floating-point fraction.
//! - [`rate_display`] — Human-readable slash notation (`"1/2"`, `"2/3"`, …).
//! - [`rate_underscore`] — Filename-safe underscore notation (`"1_2"`, `"2_3"`, …).
//! - [`mod_str`] — Modulation string (`"16qam"`, `"64qam"`, …).
//! - [`BicmFecEncoder`] — [`BlockEncoder`] adapter for [`DvbT2Concat`].
//! - [`BicmAwgnChannel`] — [`ChannelModel`] for the full DVB-T2 BICM chain
//!   (bit-interleave → QAM-map → AWGN → QAM-soft-demap → bit-deinterleave).

#![deny(unsafe_code)]

use crate::ldpc::dvb_t2::bit_interleaver::DvbT2BitInterleaver;
use crate::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use crate::ldpc::dvb_t2::concat::DvbT2Concat;
use crate::llr::Llr;
use crate::modem::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
use crate::simulation::ChannelModel;
use crate::traits::BlockEncoder;
use crate::CodeRate;
use gf2_core::BitVec;
use rand::Rng;

// ---------------------------------------------------------------------------
// SNR conversion
// ---------------------------------------------------------------------------

/// Convert Es/N0 (dB) to Eb/N0 (dB).
///
/// # Arguments
///
/// - `es_n0_db`: Symbol energy per noise power spectral density, in dB.
/// - `bits_per_symbol`: Number of bits per QAM symbol (e.g. 4 for 16-QAM).
/// - `code_rate`: Code rate as a fraction (e.g. 0.5 for rate-1/2).
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::esn0_to_ebn0;
/// // 16-QAM rate 1/2: offset = 10*log10(4 * 0.5) = 10*log10(2) ≈ 3.0103 dB
/// let eb_n0 = esn0_to_ebn0(6.0, 4, 0.5);
/// let expected = 6.0 - 10.0_f64 * 2.0_f64.log10();
/// assert!((eb_n0 - expected).abs() < 1e-10);
/// ```
pub fn esn0_to_ebn0(es_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    es_n0_db - 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

/// Convert Eb/N0 (dB) to Es/N0 (dB).
///
/// # Arguments
///
/// - `eb_n0_db`: Bit energy per noise power spectral density, in dB.
/// - `bits_per_symbol`: Number of bits per QAM symbol (e.g. 4 for 16-QAM).
/// - `code_rate`: Code rate as a fraction (e.g. 0.5 for rate-1/2).
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::ebn0_to_esn0;
/// // 16-QAM rate 1/2: offset = 10*log10(4 * 0.5) = 10*log10(2) ≈ 3.0103 dB
/// let es_n0 = ebn0_to_esn0(3.0, 4, 0.5);
/// let expected = 3.0 + 10.0_f64 * 2.0_f64.log10();
/// assert!((es_n0 - expected).abs() < 1e-10);
/// ```
pub fn ebn0_to_esn0(eb_n0_db: f64, bits_per_symbol: usize, code_rate: f64) -> f64 {
    eb_n0_db + 10.0 * (bits_per_symbol as f64 * code_rate).log10()
}

// ---------------------------------------------------------------------------
// Box-Muller noise
// ---------------------------------------------------------------------------

/// One standard-normal sample via the cosine branch of the Box-Muller transform.
///
/// This is the **single source of truth** for the per-axis AWGN noise sample
/// used by the DVB-T2 BICM chain (both [`BicmAwgnChannel`] and the `gf2-sim`
/// parallel frame kernel). The caller supplies the two uniforms; this function
/// applies the `u1.max(1e-15)` clamp internally (to avoid `ln(0)`) and returns
/// `((-2*ln(u1)).sqrt() * cos(2*pi*u2)) as f32`. Keeping the formula here lets
/// each call site choose its own RNG (e.g. `rand 0.8` vs `rand_chacha 0.9`)
/// while sharing identical noise math.
///
/// # Arguments
///
/// - `u1`: First uniform in `[0, 1)` (clamped to `>= 1e-15` internally).
/// - `u2`: Second uniform in `[0, 1)`.
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;
/// // u2 = 0 → cos(0) = 1, so the sample is +sqrt(-2 ln u1).
/// let n = box_muller_cos(0.5_f64.exp().recip(), 0.0);
/// // u1 = e^-0.5 → -2 ln u1 = 1 → sqrt = 1.
/// assert!((n - 1.0).abs() < 1e-6);
/// ```
#[inline]
pub fn box_muller_cos(u1: f64, u2: f64) -> f32 {
    let u1 = u1.max(1e-15);
    ((-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()) as f32
}

// ---------------------------------------------------------------------------
// Naming helpers
// ---------------------------------------------------------------------------

/// Code rate as a floating-point fraction.
///
/// Returns 1.0 for any code rate not in the DVB-T2 baseline set.
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::rate_f64;
/// use gf2_coding::CodeRate;
/// assert_eq!(rate_f64(CodeRate::Rate1_2), 0.5);
/// assert!((rate_f64(CodeRate::Rate2_3) - 2.0/3.0).abs() < 1e-15);
/// assert_eq!(rate_f64(CodeRate::Rate3_4), 0.75);
/// ```
pub fn rate_f64(r: CodeRate) -> f64 {
    match r {
        CodeRate::Rate1_2 => 0.5,
        CodeRate::Rate2_3 => 2.0 / 3.0,
        CodeRate::Rate3_4 => 0.75,
        _ => 1.0,
    }
}

/// Human-readable slash notation for a code rate (`"1/2"`, `"2/3"`, `"3/4"`).
///
/// Returns `"?"` for unrecognised rates.
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::rate_display;
/// use gf2_coding::CodeRate;
/// assert_eq!(rate_display(CodeRate::Rate1_2), "1/2");
/// assert_eq!(rate_display(CodeRate::Rate2_3), "2/3");
/// assert_eq!(rate_display(CodeRate::Rate3_4), "3/4");
/// ```
pub fn rate_display(r: CodeRate) -> &'static str {
    match r {
        CodeRate::Rate1_2 => "1/2",
        CodeRate::Rate2_3 => "2/3",
        CodeRate::Rate3_4 => "3/4",
        _ => "?",
    }
}

/// Filename-safe underscore notation for a code rate (`"1_2"`, `"2_3"`, `"3_4"`).
///
/// Returns `"unknown"` for unrecognised rates.
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::rate_underscore;
/// use gf2_coding::CodeRate;
/// assert_eq!(rate_underscore(CodeRate::Rate1_2), "1_2");
/// assert_eq!(rate_underscore(CodeRate::Rate2_3), "2_3");
/// assert_eq!(rate_underscore(CodeRate::Rate3_4), "3_4");
/// ```
pub fn rate_underscore(r: CodeRate) -> &'static str {
    match r {
        CodeRate::Rate1_2 => "1_2",
        CodeRate::Rate2_3 => "2_3",
        CodeRate::Rate3_4 => "3_4",
        _ => "unknown",
    }
}

/// Modulation string used in filenames and CSV columns (`"16qam"`, `"64qam"`).
///
/// Returns `"unknown"` for unrecognised modulations.
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::mod_str;
/// use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
/// assert_eq!(mod_str(DvbT2Modulation::Qam16), "16qam");
/// assert_eq!(mod_str(DvbT2Modulation::Qam64), "64qam");
/// ```
pub fn mod_str(m: DvbT2Modulation) -> &'static str {
    match m {
        DvbT2Modulation::Qam16 => "16qam",
        DvbT2Modulation::Qam64 => "64qam",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// BICM FEC encoder wrapper
// ---------------------------------------------------------------------------

/// [`BlockEncoder`] adapter for [`DvbT2Concat`].
///
/// Wraps the concatenated BCH+LDPC codec so it can be passed to
/// [`SimulationRunner::run_with_decoder`] as a `&dyn BlockEncoder`.
///
/// - `k()` returns `k_bch` (BBFRAME bit count).
/// - `n()` returns `n_ldpc` (FECFRAME bit count).
/// - `encode()` performs BCH outer coding followed by LDPC inner coding.
pub struct BicmFecEncoder {
    /// The underlying concatenated BCH+LDPC codec.
    pub concat: DvbT2Concat,
}

impl BicmFecEncoder {
    /// Create a new [`BicmFecEncoder`] from an already-configured [`DvbT2Concat`].
    pub fn new(concat: DvbT2Concat) -> Self {
        Self { concat }
    }
}

impl BlockEncoder for BicmFecEncoder {
    fn k(&self) -> usize {
        self.concat.k_bch()
    }

    fn n(&self) -> usize {
        self.concat.n_ldpc()
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        self.concat.encode(message)
    }
}

// ---------------------------------------------------------------------------
// BICM AWGN channel model
// ---------------------------------------------------------------------------

/// [`ChannelModel`] for the full DVB-T2 BICM chain over AWGN.
///
/// [`transmit_and_demodulate`][BicmAwgnChannel::transmit_and_demodulate]
/// receives `n_ldpc` FECFRAME bits (encoder output) and performs:
///
/// 1. Bit interleaving (FECFRAME order → interleaved order).
/// 2. QAM mapping (interleaved bits → I/Q symbols).
/// 3. AWGN noise injection (Box-Muller on each I/Q axis independently).
/// 4. QAM soft demapping → interleaved LLRs.
/// 5. Bit deinterleaving (interleaved LLRs → FECFRAME-order LLRs).
///
/// The caller ([`SimulationRunner`][crate::simulation::SimulationRunner]) passes
/// `eb_n0_db` and the code rate; Es/N0 is computed internally via
/// [`ebn0_to_esn0`].
pub struct BicmAwgnChannel {
    /// DVB-T2 bit interleaver / deinterleaver.
    pub interleaver: DvbT2BitInterleaver,
    /// Bits per QAM symbol (4 for 16-QAM, 6 for 64-QAM).
    pub bits_per_symbol: usize,
    spec: ModemSpec<f32>,
    demap: DemapMethod,
}

impl BicmAwgnChannel {
    /// Create a new [`BicmAwgnChannel`].
    ///
    /// # Arguments
    ///
    /// - `interleaver`: Pre-configured [`DvbT2BitInterleaver`] for the MODCOD.
    /// - `bits_per_symbol`: 2 for QPSK, 4 for 16-QAM, 6 for 64-QAM.
    /// - `demap`: Soft-demapping method ([`DemapMethod::MaxLog`] or
    ///   [`DemapMethod::ExactLogMap`]).
    pub fn new(
        interleaver: DvbT2BitInterleaver,
        bits_per_symbol: usize,
        demap: DemapMethod,
    ) -> Self {
        // Gray-square-QAM constellation order = 2^bits_per_symbol: QPSK→4,
        // 16-QAM→16, 64-QAM→64. (The earlier `if ==4 {16} else {64}` mis-mapped
        // QPSK to order 64, panicking on the symbol-count mismatch.)
        let order = 1usize << bits_per_symbol;
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        Self {
            interleaver,
            bits_per_symbol,
            spec,
            demap,
        }
    }

    /// Runs the canonical DVB-T2 BICM-AWGN transmit/demod chain with a
    /// caller-supplied noise generator.
    ///
    /// This is the **single source of truth** for the chain math
    /// (bit-interleave → QAM-map → per-axis AWGN → QAM-soft-demap →
    /// bit-deinterleave). The `eb_n0`-driven [`ChannelModel`] implementation and
    /// the `gf2-sim` parallel frame kernel both call this method; they differ
    /// only in how they draw noise samples (which RNG / version), which is why
    /// `sigma` / `noise_var` and the noise generator are passed in rather than
    /// derived from an SNR here.
    ///
    /// # Arguments
    ///
    /// - `bits`: FECFRAME codeword (`n_ldpc` bits).
    /// - `sigma`: Per-axis noise standard deviation (`sqrt(sigma_sq)`); each
    ///   noise sample is scaled by this before being added to the symbol axis.
    /// - `noise_var`: Per-symbol total complex noise variance (`N0 = 2*sigma_sq`)
    ///   passed to the soft demapper.
    /// - `next_noise`: Standard-normal sample generator. **Draw contract:** it is
    ///   called `num_symbols` times for the I axis first, then `num_symbols`
    ///   times for the Q axis, in that order, where
    ///   `num_symbols = bits.len() / bits_per_symbol`.
    ///
    /// # Returns
    ///
    /// FECFRAME-order soft LLRs (`n_ldpc` values).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::dvb_t2_bicm_harness::{BicmAwgnChannel, box_muller_cos};
    /// use gf2_coding::ldpc::dvb_t2::bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation};
    /// use gf2_coding::ldpc::dvb_t2::FrameSize;
    /// use gf2_coding::modem::DemapMethod;
    /// use gf2_coding::CodeRate;
    /// use gf2_core::BitVec;
    /// use rand::Rng;
    ///
    /// let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate1_2, DvbT2Modulation::Qam16);
    /// let il = DvbT2BitInterleaver::new(modcod);
    /// let channel = BicmAwgnChannel::new(il, 4, DemapMethod::ExactLogMap);
    /// let codeword = BitVec::zeros(64800);
    /// let mut rng = rand::thread_rng();
    /// let llrs = channel.transmit_and_demodulate_with_noise(&codeword, 0.5, 0.5, || {
    ///     let u1 = rng.gen::<f64>();
    ///     let u2 = rng.gen::<f64>();
    ///     box_muller_cos(u1, u2)
    /// });
    /// assert_eq!(llrs.len(), 64800);
    /// ```
    pub fn transmit_and_demodulate_with_noise(
        &self,
        bits: &BitVec,
        sigma: f32,
        noise_var: f32,
        mut next_noise: impl FnMut() -> f32,
    ) -> Vec<Llr> {
        let n_ldpc = bits.len();
        let num_symbols = n_ldpc / self.bits_per_symbol;

        let mapper = self.spec.preferred_mapper();
        let demapper = self.spec.preferred_soft_demapper();

        // 1. Bit interleave: FECFRAME order → interleaved order.
        let interleaved = self.interleaver.interleave(bits);

        // 2. QAM map: interleaved bits → I/Q symbols.
        let interleaved_bits: Vec<bool> =
            (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
        let mut tx_i = vec![0.0_f32; num_symbols];
        let mut tx_q = vec![0.0_f32; num_symbols];
        mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

        // 3. AWGN: independent noise on the I axis (all symbols) then the Q axis
        //    (all symbols), per the documented draw contract.
        for s in tx_i.iter_mut() {
            *s += sigma * next_noise();
        }
        for s in tx_q.iter_mut() {
            *s += sigma * next_noise();
        }

        // 4. QAM soft demap → interleaved LLRs.
        let noise_var_buf = vec![noise_var; num_symbols];
        let mut interleaved_llrs = vec![Llr::new(0.0); n_ldpc];
        demapper.demap_llrs(
            DemapInput {
                rx_i: &tx_i,
                rx_q: &tx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &noise_var_buf,
                method: self.demap,
            },
            &mut interleaved_llrs,
        );

        // 5. Bit deinterleave LLRs → FECFRAME order.
        self.interleaver.deinterleave_llrs(&interleaved_llrs)
    }
}

impl ChannelModel for BicmAwgnChannel {
    fn batch_alignment(&self) -> usize {
        // FECFRAME length is always divisible by bits_per_symbol for DVB-T2.
        // Return 1 since the runner passes the full n_ldpc-bit codeword.
        1
    }

    fn demap_method(&self) -> DemapMethod {
        self.demap
    }

    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        // Convert Eb/N0 → Es/N0 → per-component noise variance.
        // Es/N0 = Eb/N0 + 10*log10(m * r)
        // sigma^2 = 1 / (2 * 10^(Es_N0/10))
        let es_n0_db = ebn0_to_esn0(eb_n0_db, self.bits_per_symbol, rate);
        let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        let sigma_f32 = (sigma_sq as f32).sqrt();
        let noise_var_f32 = (2.0 * sigma_sq) as f32; // N0 = 2 * sigma^2

        // Delegate to the canonical chain; draw u1 then u2 per sample so the
        // byte stream is unchanged from the prior inline implementation.
        self.transmit_and_demodulate_with_noise(bits, sigma_f32, noise_var_f32, || {
            let u1 = rng.gen::<f64>();
            let u2 = rng.gen::<f64>();
            box_muller_cos(u1, u2)
        })
    }
}

// ---------------------------------------------------------------------------
// Baseline-runner matrix and CSV helpers
// ---------------------------------------------------------------------------

/// A parsed row from the baseline measurement CSV.
///
/// Used by the standalone baseline runner to write and diff throughput
/// receipts. Exposed here so the logic can be covered by `cargo test -p
/// gf2-coding`.
#[derive(Debug, Clone)]
pub struct BaselineCellResult {
    pub rate: String,
    pub modulation: String,
    pub es_n0_db: f64,
    pub decoder: String,
    pub demap: String,
    pub frames: usize,
    pub wall_seconds: f64,
    pub frames_per_sec: f64,
    pub mean_iters: f64,
    pub ber: f64,
    pub fer: f64,
    pub commit_sha: String,
    pub date: String,
}

impl BaselineCellResult {
    /// CSV header line matching the columns written by the baseline runner.
    pub fn csv_header() -> &'static str {
        "rate,modulation,es_n0_db,decoder,demap,frames,wall_seconds,frames_per_sec,mean_iters,ber,fer,commit_sha,date"
    }

    /// Render this result as a CSV row (no trailing newline).
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{:.2},{},{},{},{:.3},{:.4},{:.3},{:.6},{:.6},{},{}",
            self.rate,
            self.modulation,
            self.es_n0_db,
            self.decoder,
            self.demap,
            self.frames,
            self.wall_seconds,
            self.frames_per_sec,
            self.mean_iters,
            self.ber,
            self.fer,
            self.commit_sha,
            self.date,
        )
    }
}

/// Parse a baseline CSV string (including the header line) into a list of
/// [`BaselineCellResult`] rows.
///
/// Lines that cannot be parsed are silently skipped.
///
/// # Examples
///
/// ```
/// use gf2_coding::dvb_t2_bicm_harness::parse_baseline_csv;
/// let csv = "\
/// rate,modulation,es_n0_db,decoder,demap,frames,wall_seconds,frames_per_sec,mean_iters,ber,fer,commit_sha,date\n\
/// 1/2,16qam,6.25,SumProduct,ExactLogMap,200,123.456,1.6216,32.100,0.000500,0.050000,abc1234567,2026-06-07\n";
/// let rows = parse_baseline_csv(csv);
/// assert_eq!(rows.len(), 1);
/// assert_eq!(rows[0].rate, "1/2");
/// assert!((rows[0].frames_per_sec - 1.6216).abs() < 1e-4);
/// ```
pub fn parse_baseline_csv(content: &str) -> Vec<BaselineCellResult> {
    let mut out = Vec::new();
    for line in content.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 13 {
            continue;
        }
        let Ok(es_n0_db) = f[2].parse::<f64>() else {
            continue;
        };
        let Ok(frames) = f[5].parse::<usize>() else {
            continue;
        };
        let Ok(wall_seconds) = f[6].parse::<f64>() else {
            continue;
        };
        let Ok(frames_per_sec) = f[7].parse::<f64>() else {
            continue;
        };
        let Ok(mean_iters) = f[8].parse::<f64>() else {
            continue;
        };
        let Ok(ber) = f[9].parse::<f64>() else {
            continue;
        };
        let Ok(fer) = f[10].parse::<f64>() else {
            continue;
        };
        out.push(BaselineCellResult {
            rate: f[0].to_string(),
            modulation: f[1].to_string(),
            es_n0_db,
            decoder: f[3].to_string(),
            demap: f[4].to_string(),
            frames,
            wall_seconds,
            frames_per_sec,
            mean_iters,
            ber,
            fer,
            commit_sha: f[11].to_string(),
            date: f[12].to_string(),
        });
    }
    out
}

/// Number of cells in the standard baseline measurement matrix.
///
/// The matrix covers 3 MODCODs × 3 SNR points × 3 decoder/demap pairs = 27
/// cells.  This constant is exposed so downstream callers and tests can assert
/// against it without re-deriving the product.
pub const BASELINE_MATRIX_CELL_COUNT: usize = 27;

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;

    // --- SNR conversion ---

    #[test]
    fn test_snr_roundtrip_16qam_rate1_2() {
        // 16-QAM, rate 1/2: 10*log10(4 * 0.5) = 10*log10(2) ≈ 3.0103 dB offset
        let bits_per_symbol = 4;
        let rate = 0.5;
        let original_esn0 = 6.25_f64;
        let ebn0 = esn0_to_ebn0(original_esn0, bits_per_symbol, rate);
        let recovered = ebn0_to_esn0(ebn0, bits_per_symbol, rate);
        assert!(
            (recovered - original_esn0).abs() < 1e-12,
            "round-trip failed: {original_esn0} -> {ebn0} -> {recovered}"
        );
    }

    #[test]
    fn test_snr_roundtrip_64qam_rate2_3() {
        let bits_per_symbol = 6;
        let rate = 2.0 / 3.0;
        let original_esn0 = 13.5_f64;
        let ebn0 = esn0_to_ebn0(original_esn0, bits_per_symbol, rate);
        let recovered = ebn0_to_esn0(ebn0, bits_per_symbol, rate);
        assert!(
            (recovered - original_esn0).abs() < 1e-12,
            "round-trip failed: {original_esn0} -> {ebn0} -> {recovered}"
        );
    }

    #[test]
    fn test_esn0_to_ebn0_known_value() {
        // 16-QAM rate 1/2: offset = 10*log10(4*0.5) = 10*log10(2) ≈ 3.0103 dB
        let ebn0 = esn0_to_ebn0(6.0, 4, 0.5);
        let expected = 6.0 - 10.0 * 2.0_f64.log10();
        assert!((ebn0 - expected).abs() < 1e-12);
    }

    #[test]
    fn test_ebn0_to_esn0_known_value() {
        let esn0 = ebn0_to_esn0(3.0, 4, 0.5);
        let expected = 3.0 + 10.0 * 2.0_f64.log10();
        assert!((esn0 - expected).abs() < 1e-12);
    }

    // --- Naming helpers: all 6 baseline MODCODs ---

    #[test]
    fn test_naming_all_6_modcods() {
        // rate_display
        assert_eq!(rate_display(CodeRate::Rate1_2), "1/2");
        assert_eq!(rate_display(CodeRate::Rate2_3), "2/3");
        assert_eq!(rate_display(CodeRate::Rate3_4), "3/4");

        // rate_underscore
        assert_eq!(rate_underscore(CodeRate::Rate1_2), "1_2");
        assert_eq!(rate_underscore(CodeRate::Rate2_3), "2_3");
        assert_eq!(rate_underscore(CodeRate::Rate3_4), "3_4");

        // rate_f64
        assert_eq!(rate_f64(CodeRate::Rate1_2), 0.5);
        assert!((rate_f64(CodeRate::Rate2_3) - 2.0 / 3.0).abs() < 1e-15);
        assert_eq!(rate_f64(CodeRate::Rate3_4), 0.75);

        // mod_str
        assert_eq!(mod_str(DvbT2Modulation::Qam16), "16qam");
        assert_eq!(mod_str(DvbT2Modulation::Qam64), "64qam");
    }

    // --- Matrix cell count ---

    #[test]
    fn test_baseline_matrix_cell_count() {
        // 3 MODCODs × 3 SNR points × 3 decoder/demap pairs = 27 cells.
        assert_eq!(
            BASELINE_MATRIX_CELL_COUNT, 27,
            "matrix must be 3 MODCODs × 3 SNR × 3 decoder/demap = 27 cells"
        );
    }

    // --- CSV parse and delta helpers ---

    #[test]
    fn test_parse_baseline_csv_single_row() {
        let csv = "\
rate,modulation,es_n0_db,decoder,demap,frames,wall_seconds,frames_per_sec,mean_iters,ber,fer,commit_sha,date\n\
1/2,16qam,6.25,SumProduct,ExactLogMap,200,123.456,1.6216,32.100,0.000500,0.050000,abc1234567,2026-06-07\n";
        let rows = parse_baseline_csv(csv);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.rate, "1/2");
        assert_eq!(r.modulation, "16qam");
        assert!((r.es_n0_db - 6.25).abs() < 1e-10);
        assert_eq!(r.decoder, "SumProduct");
        assert_eq!(r.demap, "ExactLogMap");
        assert_eq!(r.frames, 200);
        assert!((r.frames_per_sec - 1.6216).abs() < 1e-4);
        assert!((r.mean_iters - 32.1).abs() < 1e-4);
        assert!((r.ber - 0.0005).abs() < 1e-8);
        assert!((r.fer - 0.05).abs() < 1e-8);
        assert_eq!(r.commit_sha, "abc1234567");
        assert_eq!(r.date, "2026-06-07");
    }

    #[test]
    fn test_parse_baseline_csv_skips_malformed_rows() {
        let csv = "\
rate,modulation,es_n0_db,decoder,demap,frames,wall_seconds,frames_per_sec,mean_iters,ber,fer,commit_sha,date\n\
bad,row\n\
1/2,16qam,6.25,SumProduct,ExactLogMap,200,123.456,1.6216,32.100,0.000500,0.050000,abc1234567,2026-06-07\n";
        let rows = parse_baseline_csv(csv);
        assert_eq!(rows.len(), 1, "malformed row should be skipped");
    }

    #[test]
    fn test_parse_baseline_csv_delta_match() {
        // Simulate a delta comparison: new result vs baseline.
        let csv_new = "\
rate,modulation,es_n0_db,decoder,demap,frames,wall_seconds,frames_per_sec,mean_iters,ber,fer,commit_sha,date\n\
1/2,16qam,6.25,SumProduct,ExactLogMap,200,110.0,1.818,30.0,0.000400,0.040000,new123,2026-06-08\n";
        let csv_old = "\
rate,modulation,es_n0_db,decoder,demap,frames,wall_seconds,frames_per_sec,mean_iters,ber,fer,commit_sha,date\n\
1/2,16qam,6.25,SumProduct,ExactLogMap,200,123.456,1.6216,32.100,0.000500,0.050000,abc1234567,2026-06-07\n";
        let new_rows = parse_baseline_csv(csv_new);
        let old_rows = parse_baseline_csv(csv_old);
        assert_eq!(new_rows.len(), 1);
        assert_eq!(old_rows.len(), 1);

        // Find the matching baseline row (same rate, mod, snr, decoder, demap).
        let r = &new_rows[0];
        let bline = old_rows.iter().find(|b| {
            b.rate == r.rate
                && b.modulation == r.modulation
                && (b.es_n0_db - r.es_n0_db).abs() < 0.01
                && b.decoder == r.decoder
                && b.demap == r.demap
        });
        assert!(bline.is_some(), "matching baseline row not found");
        let delta_pct = (r.frames_per_sec - bline.unwrap().frames_per_sec)
            / bline.unwrap().frames_per_sec
            * 100.0;
        // new is faster: 1.818 vs 1.6216 ≈ +12%
        assert!(delta_pct > 0.0, "new should be faster in this fixture");
    }

    #[test]
    fn test_baseline_cell_result_csv_roundtrip() {
        let original = BaselineCellResult {
            rate: "1/2".to_string(),
            modulation: "16qam".to_string(),
            es_n0_db: 6.25,
            decoder: "SumProduct".to_string(),
            demap: "ExactLogMap".to_string(),
            frames: 200,
            wall_seconds: 123.456,
            frames_per_sec: 1.6216,
            mean_iters: 32.1,
            ber: 0.0005,
            fer: 0.05,
            commit_sha: "abc1234567".to_string(),
            date: "2026-06-07".to_string(),
        };
        let csv = format!(
            "{}\n{}\n",
            BaselineCellResult::csv_header(),
            original.to_csv_row()
        );
        let parsed = parse_baseline_csv(&csv);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].rate, original.rate);
        assert!((parsed[0].es_n0_db - original.es_n0_db).abs() < 0.01);
        assert!((parsed[0].frames_per_sec - original.frames_per_sec).abs() < 0.001);
    }
}
