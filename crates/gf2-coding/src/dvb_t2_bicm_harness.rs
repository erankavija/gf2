//! DVB-T2 BICM-AWGN channel chain, FEC-encoder adapter and baseline-CSV
//! helpers shared by the simulation harnesses.

#![deny(unsafe_code)]

use crate::info_theory::ebn0_to_esn0;
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

/// One standard-normal sample by the cosine branch of the Box-Muller transform,
/// `sqrt(-2 ln u1) * cos(2π u2)`, for uniforms `u1`, `u2` in `[0, 1)`; `u1` is
/// clamped to at least `1e-15`.
#[inline]
pub fn box_muller_cos(u1: f64, u2: f64) -> f32 {
    let u1 = u1.max(1e-15);
    ((-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()) as f32
}

/// Code rate as a floating-point fraction.
///
/// Returns 1.0 for any code rate not in the DVB-T2 baseline set.
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
pub fn mod_str(m: DvbT2Modulation) -> &'static str {
    match m {
        DvbT2Modulation::Qam16 => "16qam",
        DvbT2Modulation::Qam64 => "64qam",
        _ => "unknown",
    }
}

/// [`BlockEncoder`] adapter for [`DvbT2Concat`]: `k` is the BBFRAME length
/// `k_bch` and `n` the FECFRAME length `n_ldpc`.
pub struct BicmFecEncoder {
    pub concat: DvbT2Concat,
}

impl BicmFecEncoder {
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

/// [`ChannelModel`] for the DVB-T2 BICM chain over AWGN: bit interleave, QAM
/// map, per-axis AWGN, soft demap, bit deinterleave. Es/N0 follows from
/// `eb_n0_db` and the code rate through [`ebn0_to_esn0`].
pub struct BicmAwgnChannel {
    pub interleaver: DvbT2BitInterleaver,
    /// Bits per QAM symbol (4 for 16-QAM, 6 for 64-QAM).
    pub bits_per_symbol: usize,
    spec: ModemSpec<f32>,
    demap: DemapMethod,
}

impl BicmAwgnChannel {
    /// Uses the Gray square QAM of order `2^bits_per_symbol`.
    ///
    /// # Panics
    ///
    /// Panics if `bits_per_symbol` is not one of `1, 2, 4, 6, 8`.
    pub fn new(
        interleaver: DvbT2BitInterleaver,
        bits_per_symbol: usize,
        demap: DemapMethod,
    ) -> Self {
        let order = 1usize << bits_per_symbol;
        let spec = ModemSpec::<f32>::gray_square_qam(order);
        Self {
            interleaver,
            bits_per_symbol,
            spec,
            demap,
        }
    }

    /// Runs the chain with caller-supplied noise and returns FECFRAME-order LLRs.
    ///
    /// `sigma` is the per-axis noise standard deviation and `noise_var` the
    /// per-symbol complex noise variance (`2 * sigma^2`) given to the demapper.
    /// `next_noise` yields standard-normal samples; it is called
    /// `bits.len() / bits_per_symbol` times for the I axis, then as many times
    /// for the Q axis.
    ///
    /// # Panics
    ///
    /// Panics if `bits.len()` differs from the interleaver's frame length.
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

        let interleaved = self.interleaver.interleave(bits);

        let interleaved_bits: Vec<bool> =
            (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
        let mut tx_i = vec![0.0_f32; num_symbols];
        let mut tx_q = vec![0.0_f32; num_symbols];
        mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);

        for s in tx_i.iter_mut() {
            *s += sigma * next_noise();
        }
        for s in tx_q.iter_mut() {
            *s += sigma * next_noise();
        }

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

        self.interleaver.deinterleave_llrs(&interleaved_llrs)
    }
}

impl ChannelModel for BicmAwgnChannel {
    fn batch_alignment(&self) -> usize {
        // The runner passes whole FECFRAMEs, whose length is a multiple of `bits_per_symbol`.
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
        let es_n0_db = ebn0_to_esn0(eb_n0_db, self.bits_per_symbol, rate);
        let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
        let sigma_sq = 1.0 / (2.0 * es_n0_lin);
        let sigma_f32 = (sigma_sq as f32).sqrt();
        let noise_var_f32 = (2.0 * sigma_sq) as f32; // N0 = 2 * sigma^2

        self.transmit_and_demodulate_with_noise(bits, sigma_f32, noise_var_f32, || {
            let u1 = rng.gen::<f64>();
            let u2 = rng.gen::<f64>();
            box_muller_cos(u1, u2)
        })
    }
}

/// A parsed row from the baseline measurement CSV.
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

/// Cells of the baseline matrix: 3 MODCODs × 3 SNR points × 3 decoder/demap pairs.
pub const BASELINE_MATRIX_CELL_COUNT: usize = 27;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;

    #[test]
    fn test_naming_all_6_modcods() {
        assert_eq!(rate_display(CodeRate::Rate1_2), "1/2");
        assert_eq!(rate_display(CodeRate::Rate2_3), "2/3");
        assert_eq!(rate_display(CodeRate::Rate3_4), "3/4");

        assert_eq!(rate_underscore(CodeRate::Rate1_2), "1_2");
        assert_eq!(rate_underscore(CodeRate::Rate2_3), "2_3");
        assert_eq!(rate_underscore(CodeRate::Rate3_4), "3_4");

        assert_eq!(rate_f64(CodeRate::Rate1_2), 0.5);
        assert!((rate_f64(CodeRate::Rate2_3) - 2.0 / 3.0).abs() < 1e-15);
        assert_eq!(rate_f64(CodeRate::Rate3_4), 0.75);

        assert_eq!(mod_str(DvbT2Modulation::Qam16), "16qam");
        assert_eq!(mod_str(DvbT2Modulation::Qam64), "64qam");
    }

    #[test]
    fn test_baseline_matrix_cell_count() {
        assert_eq!(
            BASELINE_MATRIX_CELL_COUNT, 27,
            "matrix must be 3 MODCODs × 3 SNR × 3 decoder/demap = 27 cells"
        );
    }

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
