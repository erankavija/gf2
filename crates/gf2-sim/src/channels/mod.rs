//! CPU channel stages: AWGN, Rayleigh flat-fading, Rician flat-fading. Each
//! stage maps a [`SymbolBatch`] to a [`SymbolBatch`], drawing from the RNG of
//! its [`ChannelScratch`](awgn::ChannelScratch).

use rand::Rng as _;
use rand_chacha::ChaCha20Rng;

use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;

use crate::batch::SymbolBatch;
use crate::parallel::{DEBUG_ASSERT_WORD_MARGIN, FRAME_STRIDE};

pub mod awgn;
pub mod rayleigh;
pub mod rician;

pub use awgn::Awgn;
pub use rayleigh::Rayleigh;
pub use rician::Rician;

/// ChaCha20 32-bit words one [`draw_standard_normal`] call consumes.
const WORDS_PER_NORMAL: u128 = 4;

/// The largest symbol count of a frame that draws `normals_per_symbol`
/// [`draw_standard_normal`] samples per symbol within the per-frame budget of
/// `FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN` words.
pub(crate) const fn max_frame_symbols(normals_per_symbol: u128) -> usize {
    ((FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN) / (normals_per_symbol * WORDS_PER_NORMAL)) as usize
}

/// Panics unless every frame of `batch` has at most `max_symbols` symbols.
/// `channel` names the stage in the message.
pub(crate) fn assert_frame_budget(channel: &str, batch: &SymbolBatch, max_symbols: usize) {
    for (f, (i_frame, q_frame)) in batch.i.iter().zip(batch.q.iter()).enumerate() {
        let symbols = i_frame.len().max(q_frame.len());
        assert!(
            symbols <= max_symbols,
            "{channel} frame {f} has {symbols} symbols; a frame holds at most {max_symbols} \
             symbols, which draw FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN = {} ChaCha20 words",
            FRAME_STRIDE - DEBUG_ASSERT_WORD_MARGIN
        );
    }
}

/// Draws one `N(0, 1)` sample: two `f64` uniforms fed to [`box_muller_cos`],
/// consuming exactly [`WORDS_PER_NORMAL`] ChaCha20 32-bit words.
#[inline]
pub(crate) fn draw_standard_normal(rng: &mut ChaCha20Rng) -> f32 {
    let u1: f64 = rng.random();
    let u2: f64 = rng.random();
    box_muller_cos(u1, u2)
}

/// Draws one `CN(0, 1)` sample `(re, im)`, each component `N(0, 1/2)`,
/// consuming exactly 8 ChaCha20 32-bit words.
#[inline]
pub(crate) fn draw_cn01(rng: &mut ChaCha20Rng) -> (f32, f32) {
    let re = draw_standard_normal(rng) * std::f32::consts::FRAC_1_SQRT_2;
    let im = draw_standard_normal(rng) * std::f32::consts::FRAC_1_SQRT_2;
    (re, im)
}

/// Converts an Es/N0 (dB) to the per-axis AWGN noise standard deviation
/// `sigma = sqrt(1 / (2 * 10^(es_n0_db / 10)))` under the
/// unit-average-symbol-energy convention; `N0 = 2 * sigma^2`.
#[inline]
#[must_use]
pub(crate) fn es_n0_db_to_sigma(es_n0_db: f32) -> f32 {
    es_n0_db_to_sigma_f64(f64::from(es_n0_db))
}

/// The `f64`-input core of [`es_n0_db_to_sigma`].
///
/// Paired with [`es_n0_db_to_n0_f64`]: deriving sigma and `N0` from the same
/// `f64` Es/N0 keeps the injected noise and the demapper's `N0` at one SNR.
#[inline]
#[must_use]
pub(crate) fn es_n0_db_to_sigma_f64(es_n0_db: f64) -> f32 {
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (sigma_sq as f32).sqrt()
}

/// Converts an Es/N0 (dB) to the total complex AWGN noise variance
/// `N0 = 2 * sigma^2` with `sigma^2 = 1 / (2 * 10^(Es/N0 / 10))`.
///
/// The value is computed in `f64` and rounded to `f32` once; squaring the
/// already-rounded `f32` sigma can differ from it by an ULP.
#[inline]
#[must_use]
pub fn es_n0_db_to_n0(es_n0_db: f32) -> f32 {
    es_n0_db_to_n0_f64(f64::from(es_n0_db))
}

/// The `f64`-input core of [`es_n0_db_to_n0`], for
/// [`DvbT2BicmFrameSim`](crate::frame_sim::DvbT2BicmFrameSim), whose Es/N0 is
/// `f64`.
#[inline]
#[must_use]
pub(crate) fn es_n0_db_to_n0_f64(es_n0_db: f64) -> f32 {
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (2.0 * sigma_sq) as f32
}
