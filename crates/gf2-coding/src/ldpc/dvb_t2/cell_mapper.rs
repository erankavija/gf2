//! DVB-T2 bit-to-cell-word demultiplexer and cell-word mapping
//! (`@/citation/Etsi2015` clauses 6.2.1 and 6.2.2).

use crate::bch::CodeRate;
use crate::ldpc::dvb_t2::bit_interleaver::{DvbT2Modcod, DvbT2Modulation};
use crate::ldpc::dvb_t2::params::FrameSize;
use crate::llr::Llr;
use crate::modem::ModemSpec;
use crate::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, FastGrayQamDemapper, GrayQamMapper,
};
use gf2_core::BitVec;

/// The rows of one demultiplexer parameter table of `@/citation/Etsi2015`
/// clause 6.2.1: entry `i` of a row is the output bit-number `e` of input
/// bit-number `di mod Nsubstreams = i`.
struct DemuxTable {
    qpsk: &'static [u8],
    qam16_normal: &'static [u8],
    qam16_short: &'static [u8],
    qam64_normal: &'static [u8],
    qam64_short: &'static [u8],
    qam256_normal: &'static [u8],
    qam256_short: &'static [u8],
}

/// `@/citation/Etsi2015` clause 6.2.1, Table 13(a): code rates 1/2, 3/4, 4/5
/// and 5/6. Its 16-QAM and 64-QAM rows apply to both frame lengths.
const TABLE_13A: DemuxTable = DemuxTable {
    qpsk: &[0, 1],
    qam16_normal: &[7, 1, 4, 2, 5, 3, 6, 0],
    qam16_short: &[7, 1, 4, 2, 5, 3, 6, 0],
    qam64_normal: &[11, 7, 3, 10, 6, 2, 9, 5, 1, 8, 4, 0],
    qam64_short: &[11, 7, 3, 10, 6, 2, 9, 5, 1, 8, 4, 0],
    qam256_normal: &[15, 1, 13, 3, 8, 11, 9, 5, 10, 6, 4, 7, 12, 2, 14, 0],
    qam256_short: &[7, 3, 1, 5, 2, 6, 4, 0],
};

/// `@/citation/Etsi2015` clause 6.2.1, Table 13(b): code rate 3/5.
const TABLE_13B: DemuxTable = DemuxTable {
    qpsk: &[0, 1],
    qam16_normal: &[0, 5, 1, 2, 4, 7, 3, 6],
    qam16_short: &[7, 1, 4, 2, 5, 3, 6, 0],
    qam64_normal: &[2, 7, 6, 9, 0, 3, 1, 8, 4, 11, 5, 10],
    qam64_short: &[11, 7, 3, 10, 6, 2, 9, 5, 1, 8, 4, 0],
    qam256_normal: &[2, 11, 3, 4, 0, 9, 1, 8, 10, 13, 7, 14, 6, 15, 5, 12],
    qam256_short: &[7, 3, 1, 5, 2, 6, 4, 0],
};

/// `@/citation/Etsi2015` clause 6.2.1, Table 13(c): code rate 2/3. Its
/// 16-QAM and 64-QAM rows apply to both frame lengths.
const TABLE_13C: DemuxTable = DemuxTable {
    qpsk: &[0, 1],
    qam16_normal: &[7, 1, 4, 2, 5, 3, 6, 0],
    qam16_short: &[7, 1, 4, 2, 5, 3, 6, 0],
    qam64_normal: &[11, 7, 3, 10, 6, 2, 9, 5, 1, 8, 4, 0],
    qam64_short: &[11, 7, 3, 10, 6, 2, 9, 5, 1, 8, 4, 0],
    qam256_normal: &[7, 2, 9, 0, 4, 6, 13, 3, 14, 10, 15, 5, 8, 12, 11, 1],
    qam256_short: &[7, 3, 1, 5, 2, 6, 4, 0],
};

/// The Table 13 row of `modcod`; its length is `Nsubstreams` of Table 12.
fn demux_row(modcod: DvbT2Modcod) -> &'static [u8] {
    let table = match modcod.code_rate {
        CodeRate::Rate1_2 | CodeRate::Rate3_4 | CodeRate::Rate4_5 | CodeRate::Rate5_6 => &TABLE_13A,
        CodeRate::Rate3_5 => &TABLE_13B,
        CodeRate::Rate2_3 => &TABLE_13C,
    };
    match (modcod.modulation, modcod.frame_size) {
        (DvbT2Modulation::Qpsk, _) => table.qpsk,
        (DvbT2Modulation::Qam16, FrameSize::Normal) => table.qam16_normal,
        (DvbT2Modulation::Qam16, FrameSize::Short) => table.qam16_short,
        (DvbT2Modulation::Qam64, FrameSize::Normal) => table.qam64_normal,
        (DvbT2Modulation::Qam64, FrameSize::Short) => table.qam64_short,
        (DvbT2Modulation::Qam256, FrameSize::Normal) => table.qam256_normal,
        (DvbT2Modulation::Qam256, FrameSize::Short) => table.qam256_short,
    }
}

/// DVB-T2 bit-to-cell-word demultiplexer (`@/citation/Etsi2015` clause
/// 6.2.1) for one MODCOD.
///
/// Input bit `v_di` of the bit-interleaver output becomes bit
/// `b_{e,do}` with `do = di div Nsubstreams` and `e` from Table 13. A
/// demultiplexed word of `Nsubstreams` bits forms one cell word, or two
/// consecutive cell words of `η_mod = Nsubstreams / 2` bits each. Cell words
/// are stored flat: bit `y_{e,q}` sits at index `q · η_mod + e`, so `b_{e,do}`
/// sits at `do · Nsubstreams + e`.
#[derive(Debug, Clone)]
pub struct DvbT2CellDemux {
    row: &'static [u8],
    frame_bits: usize,
    bits_per_cell: usize,
}

impl DvbT2CellDemux {
    /// Selects the Table 13 row of `modcod`; every frame size, code rate and
    /// modulation has one.
    #[must_use]
    pub fn new(modcod: DvbT2Modcod) -> Self {
        let frame_bits = match modcod.frame_size {
            FrameSize::Normal => 64800,
            FrameSize::Short => 16200,
        };
        Self {
            row: demux_row(modcod),
            frame_bits,
            bits_per_cell: modcod.modulation.bits_per_cell(),
        }
    }

    /// Bits per FECFRAME, `N_ldpc`.
    #[must_use]
    pub fn frame_bits(&self) -> usize {
        self.frame_bits
    }

    /// Bits per cell word, `η_mod`.
    #[must_use]
    pub fn bits_per_cell(&self) -> usize {
        self.bits_per_cell
    }

    /// The flat cell-word index of input bit `di`.
    #[inline]
    fn cell_word_index(&self, di: usize) -> usize {
        let n_sub = self.row.len();
        di - di % n_sub + usize::from(self.row[di % n_sub])
    }

    /// Demultiplexes a bit-interleaved FECFRAME `v` into its flat cell words.
    ///
    /// # Panics
    ///
    /// Panics if `v.len() != frame_bits()`.
    #[must_use]
    pub fn demux(&self, v: &BitVec) -> BitVec {
        self.assert_frame(v.len(), "demux");
        let mut y = BitVec::zeros(self.frame_bits);
        for di in 0..self.frame_bits {
            if v.get(di) {
                y.set(self.cell_word_index(di), true);
            }
        }
        y
    }

    /// Multiplexes flat cell words `y` back into the bit-interleaved FECFRAME;
    /// the inverse of [`demux`](Self::demux).
    ///
    /// # Panics
    ///
    /// Panics if `y.len() != frame_bits()`.
    #[must_use]
    pub fn mux(&self, y: &BitVec) -> BitVec {
        self.assert_frame(y.len(), "mux");
        let mut v = BitVec::zeros(self.frame_bits);
        for di in 0..self.frame_bits {
            if y.get(self.cell_word_index(di)) {
                v.set(di, true);
            }
        }
        v
    }

    fn assert_frame(&self, len: usize, op: &str) {
        assert_eq!(
            len, self.frame_bits,
            "DvbT2CellDemux::{op}: expected {} bits, got {len}",
            self.frame_bits
        );
    }
}

/// DVB-T2 cell mapper for one MODCOD: the demultiplexer of
/// [`DvbT2CellDemux`] followed by the cell-word mapping of
/// `@/citation/Etsi2015` clause 6.2.2, Tables 14(b) to 14(i), normalized to
/// unit average energy per Table 15. The soft demapper inverts both.
///
/// Within a cell word the even-indexed bits `y_0, y_2, …` select the real
/// part and the odd-indexed bits `y_1, y_3, …` the imaginary part, each as a
/// Gray label whose first bit is the most significant. The mapper composes
/// the demultiplexer and this split into one permutation onto the label
/// layout of [`ModemSpec::gray_square_qam`], built once at construction, and
/// maps and demaps through [`GrayQamMapper`] and [`FastGrayQamDemapper`].
pub struct DvbT2CellMapper {
    demux: DvbT2CellDemux,
    /// `label_src[k]` is the bit-interleaver output index of label bit `k`
    /// in the [`ModemSpec::gray_square_qam`] layout.
    label_src: Vec<u32>,
    mapper: GrayQamMapper<f32>,
    demapper: FastGrayQamDemapper<f32>,
}

impl DvbT2CellMapper {
    /// Builds the mapper for `modcod`.
    ///
    /// # Complexity
    ///
    /// O(`N_ldpc`) time and space for the label permutation.
    #[must_use]
    pub fn new(modcod: DvbT2Modcod) -> Self {
        let demux = DvbT2CellDemux::new(modcod);
        let (n, eta) = (demux.frame_bits(), demux.bits_per_cell());
        let mut word_src = vec![0u32; n];
        for di in 0..n {
            word_src[demux.cell_word_index(di)] = di as u32;
        }
        let half = eta / 2;
        let label_src = (0..n)
            .map(|k| {
                let (cell, j) = (k - k % eta, k % eta);
                let e = if j < half { 2 * j } else { 2 * (j - half) + 1 };
                word_src[cell + e]
            })
            .collect();
        let order = 1usize << eta;
        Self {
            demux,
            label_src,
            mapper: GrayQamMapper::from_preset_order(order),
            demapper: FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(order)),
        }
    }

    /// The demultiplexer this mapper applies first.
    #[must_use]
    pub fn demux(&self) -> &DvbT2CellDemux {
        &self.demux
    }

    /// Bits per cell, `η_mod`.
    #[must_use]
    pub fn bits_per_cell(&self) -> usize {
        self.demux.bits_per_cell()
    }

    /// Cells per FECFRAME, `N_ldpc / η_mod` (Table 11).
    #[must_use]
    pub fn num_cells(&self) -> usize {
        self.demux.frame_bits() / self.demux.bits_per_cell()
    }

    /// Maps a bit-interleaved FECFRAME `v` to its cells.
    ///
    /// # Panics
    ///
    /// Panics if `v.len()` differs from `N_ldpc` or an output length from
    /// [`num_cells`](Self::num_cells).
    pub fn map(&self, v: &BitVec, out_i: &mut [f32], out_q: &mut [f32]) {
        self.demux.assert_frame(v.len(), "map");
        let labels: Vec<bool> = self
            .label_src
            .iter()
            .map(|&src| v.get(src as usize))
            .collect();
        self.mapper.map_bits(&labels, out_i, out_q);
    }

    /// Maps flat cell words `y` (the [`DvbT2CellDemux::demux`] output) to
    /// their cells: clause 6.2.2 alone.
    ///
    /// # Panics
    ///
    /// As [`map`](Self::map).
    pub fn map_cell_words(&self, y: &BitVec, out_i: &mut [f32], out_q: &mut [f32]) {
        self.map(&self.demux.mux(y), out_i, out_q);
    }

    /// Soft-demaps the cells of one FECFRAME into `out`, one LLR per
    /// bit-interleaver output bit in its order.
    ///
    /// # Panics
    ///
    /// Panics if `input.rx_i.len()` differs from
    /// [`num_cells`](Self::num_cells), `out.len()` from `N_ldpc`, or as
    /// [`FastGrayQamDemapper::demap_llrs`] does.
    pub fn demap_llrs(&self, input: DemapInput<'_, f32>, out: &mut [Llr]) {
        assert_eq!(
            input.rx_i.len(),
            self.num_cells(),
            "DvbT2CellMapper::demap_llrs: expected {} cells",
            self.num_cells()
        );
        self.demux.assert_frame(out.len(), "demap_llrs");
        let mut labels = vec![Llr::zero(); out.len()];
        self.demapper.demap_llrs(input, &mut labels);
        for (&src, llr) in self.label_src.iter().zip(labels) {
            out[src as usize] = llr;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::DemapMethod;

    const MODULATIONS: [DvbT2Modulation; 4] = [
        DvbT2Modulation::Qpsk,
        DvbT2Modulation::Qam16,
        DvbT2Modulation::Qam64,
        DvbT2Modulation::Qam256,
    ];

    const RATES: [CodeRate; 6] = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];

    fn bitvec(bits: impl IntoIterator<Item = bool>) -> BitVec {
        let mut v = BitVec::new();
        bits.into_iter().for_each(|b| v.push_bit(b));
        v
    }

    fn all_modcods() -> impl Iterator<Item = DvbT2Modcod> {
        [FrameSize::Normal, FrameSize::Short]
            .into_iter()
            .flat_map(|fs| {
                RATES.into_iter().flat_map(move |rate| {
                    MODULATIONS
                        .into_iter()
                        .map(move |m| DvbT2Modcod::new(fs, rate, m))
                })
            })
    }

    /// Table 12: `Nsubstreams` is `2 η_mod`, except `η_mod` for QPSK and
    /// short-frame 256-QAM; every Table 13 row is a permutation of it.
    #[test]
    fn test_every_row_is_a_permutation_of_table_12_width() {
        for modcod in all_modcods() {
            let row = demux_row(modcod);
            let eta = modcod.modulation.bits_per_cell();
            let single = modcod.modulation == DvbT2Modulation::Qpsk
                || (modcod.modulation == DvbT2Modulation::Qam256
                    && modcod.frame_size == FrameSize::Short);
            assert_eq!(row.len(), if single { eta } else { 2 * eta }, "{modcod:?}");
            let mut sorted = row.to_vec();
            sorted.sort_unstable();
            assert!(
                sorted.iter().enumerate().all(|(i, &e)| usize::from(e) == i),
                "{modcod:?}"
            );
        }
    }

    /// Demultiplexing then multiplexing is the identity, and mapping then
    /// hard-deciding the noise-free soft demap returns the input, for every
    /// frame size, code rate and modulation.
    #[test]
    fn test_round_trips_for_every_modcod() {
        for (seed, modcod) in all_modcods().enumerate() {
            let mapper = DvbT2CellMapper::new(modcod);
            let n = mapper.demux().frame_bits();
            let v = BitVec::random_seeded(n, seed as u64);

            let y = mapper.demux().demux(&v);
            assert_eq!(mapper.demux().mux(&y), v, "{modcod:?}");

            let cells = mapper.num_cells();
            let (mut i, mut q) = (vec![0.0; cells], vec![0.0; cells]);
            mapper.map(&v, &mut i, &mut q);
            let mut llrs = vec![Llr::zero(); n];
            mapper.demap_llrs(
                DemapInput {
                    rx_i: &i,
                    rx_q: &q,
                    gain_i: None,
                    gain_q: None,
                    noise_var: &vec![1e-3; cells],
                    method: DemapMethod::MaxLog,
                },
                &mut llrs,
            );
            let decided = bitvec(llrs.iter().map(|l| l.hard_decision()));
            assert_eq!(decided, v, "{modcod:?}");
        }
    }

    /// Re/Im levels of `@/citation/Etsi2015` Tables 14(b) to 14(i) before
    /// normalization: level `k` from the most negative, indexed by the Gray
    /// label `(y_0, y_2, …)` or `(y_1, y_3, …)` read most significant first.
    fn table_14_level(label: &[bool]) -> i32 {
        let rows: &[&[u8]] = match label.len() {
            // Table 14(b)/(c): y = 1, 0.
            1 => &[&[1], &[0]],
            // Table 14(d)/(e).
            2 => &[&[1, 0], &[1, 1], &[0, 1], &[0, 0]],
            // Table 14(f)/(g).
            3 => &[
                &[1, 0, 0],
                &[1, 0, 1],
                &[1, 1, 1],
                &[1, 1, 0],
                &[0, 1, 0],
                &[0, 1, 1],
                &[0, 0, 1],
                &[0, 0, 0],
            ],
            // Table 14(h)/(i).
            4 => &[
                &[1, 0, 0, 0],
                &[1, 0, 0, 1],
                &[1, 0, 1, 1],
                &[1, 0, 1, 0],
                &[1, 1, 1, 0],
                &[1, 1, 1, 1],
                &[1, 1, 0, 1],
                &[1, 1, 0, 0],
                &[0, 1, 0, 0],
                &[0, 1, 0, 1],
                &[0, 1, 1, 1],
                &[0, 1, 1, 0],
                &[0, 0, 1, 0],
                &[0, 0, 1, 1],
                &[0, 0, 0, 1],
                &[0, 0, 0, 0],
            ],
            _ => unreachable!(),
        };
        let k = rows
            .iter()
            .position(|row| row.iter().zip(label).all(|(&b, &l)| (b == 1) == l))
            .expect("every label is in the table");
        2 * k as i32 + 1 - rows.len() as i32
    }

    /// Every cell word maps to its Table 14 point scaled by the Table 15
    /// normalization `1 / sqrt(2 (M - 1) / 3)`.
    #[test]
    fn test_cell_words_map_to_table_14_points() {
        for modulation in MODULATIONS {
            let modcod = DvbT2Modcod::new(FrameSize::Short, CodeRate::Rate1_2, modulation);
            let mapper = DvbT2CellMapper::new(modcod);
            let eta = mapper.bits_per_cell();
            let cells = mapper.num_cells();
            let scale = (2.0 * ((1u32 << eta) - 1) as f64 / 3.0).sqrt();
            let word = |q: usize| q % (1 << eta);
            let y = bitvec((0..cells * eta).map(|k| word(k / eta) >> (k % eta) & 1 == 1));
            let (mut i, mut q) = (vec![0.0; cells], vec![0.0; cells]);
            mapper.map_cell_words(&y, &mut i, &mut q);
            for c in 0..cells {
                let bits: Vec<bool> = (0..eta).map(|e| word(c) >> e & 1 == 1).collect();
                let even: Vec<bool> = bits.iter().copied().step_by(2).collect();
                let odd: Vec<bool> = bits.iter().copied().skip(1).step_by(2).collect();
                let re = f64::from(table_14_level(&even)) / scale;
                let im = f64::from(table_14_level(&odd)) / scale;
                assert!(
                    (f64::from(i[c]) - re).abs() < 1e-5 && (f64::from(q[c]) - im).abs() < 1e-5,
                    "{modulation:?} word {:#x}: ({}, {}) vs ({re}, {im})",
                    word(c),
                    i[c],
                    q[c]
                );
            }
        }
    }
}
