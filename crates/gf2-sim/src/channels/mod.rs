//! CPU channel stages: AWGN, Rayleigh flat-fading, Rician flat-fading.
//!
//! Each stage is a [`Stage`](crate::Stage) impl operating on a
//! [`SymbolBatch`](crate::SymbolBatch) (in → out), drawing noise from a
//! per-stage [`ChannelScratch`](awgn::ChannelScratch) RNG that the Phase C
//! executor seeks per frame via the §3 word-position scheme (design doc §3/§5).
//!
//! # Public surface (design doc §1, line 532)
//!
//! ```
//! use gf2_sim::channels::{Awgn, Rayleigh, Rician};
//! let _ = Awgn::new(6.25, 4);
//! let _ = Rayleigh::new(6.25, 4);
//! let _ = Rician::new(6.25, 4, 2.0);
//! ```
//!
//! # Non-goals
//!
//! Frequency-selective/multipath fading, phase noise, frequency offset, and
//! GPU channel stages are out of scope for this task. GPU AWGN is Phase B
//! (`f6004add`).

use rand::Rng as _;
use rand_chacha::ChaCha20Rng;

use gf2_coding::dvb_t2_bicm_harness::box_muller_cos;

pub mod awgn;
pub mod rayleigh;
pub mod rician;

pub use awgn::Awgn;
pub use rayleigh::Rayleigh;
pub use rician::Rician;

/// Draws one standard-normal `N(0, 1)` sample from a [`ChaCha20Rng`].
///
/// This is the single source of truth for the per-axis Gaussian draw shared by
/// every channel stage. It draws two `f64` uniforms and feeds them to
/// [`box_muller_cos`] (the SSOT Box-Muller sample in `gf2-coding`), so it
/// consumes **exactly 4 ChaCha20 32-bit words** (one `f64` = 2 words). Routing
/// every per-symbol draw through this helper keeps the per-symbol word-count
/// contract identical across AWGN, Rayleigh, and Rician.
///
/// # Arguments
///
/// * `rng` — the per-worker noise RNG, positioned at the next draw.
///
/// # Returns
///
/// A single `N(0, 1)` sample.
///
/// # Complexity
///
/// O(1) — two uniform draws plus one Box-Muller evaluation.
#[inline]
pub(crate) fn draw_standard_normal(rng: &mut ChaCha20Rng) -> f32 {
    let u1: f64 = rng.random();
    let u2: f64 = rng.random();
    box_muller_cos(u1, u2)
}

/// Draws one circularly-symmetric complex Gaussian `CN(0, 1)` sample.
///
/// Returns `(re, im)` where each component is `N(0, 0.5)` so the complex
/// magnitude satisfies `E[|.|^2] = 0.5 + 0.5 = 1`. Implemented as two
/// [`draw_standard_normal`] calls each scaled by `1/sqrt(2)`, consuming
/// **exactly 8 ChaCha20 32-bit words** (two normals). Used for the unit-power
/// fading coefficient in [`Rayleigh`] and the scatter component in [`Rician`].
///
/// # Arguments
///
/// * `rng` — the per-worker noise RNG, positioned at the next draw.
///
/// # Returns
///
/// A `(re, im)` pair drawn from `CN(0, 1)` (per-component variance `1/2`).
///
/// # Complexity
///
/// O(1) — two standard-normal draws.
#[inline]
pub(crate) fn draw_cn01(rng: &mut ChaCha20Rng) -> (f32, f32) {
    let re = draw_standard_normal(rng) * std::f32::consts::FRAC_1_SQRT_2;
    let im = draw_standard_normal(rng) * std::f32::consts::FRAC_1_SQRT_2;
    (re, im)
}

/// Converts an Es/N0 (dB) to the per-axis AWGN noise standard deviation.
///
/// Returns `sigma = sqrt(1 / (2 * 10^(es_n0_db / 10)))` — the per-component
/// (real-axis) standard deviation applied to each of I and Q under the
/// unit-average-symbol-energy convention. The total complex noise variance is
/// `N0 = 2 * sigma^2`.
///
/// This is the single source of truth for the Es/N0 → sigma conversion shared
/// by the [`Awgn`], [`Rayleigh`], and [`Rician`] channel constructors.
/// (The [`DvbT2BicmFrameSim`](crate::frame_sim::DvbT2BicmFrameSim) frame
/// kernel delegates to the `f64`-input core [`es_n0_db_to_sigma_f64`] — its
/// Es/N0 surface is `f64`.)
///
/// # Arguments
///
/// * `es_n0_db` — channel Es/N0 in dB.
///
/// # Complexity
///
/// O(1).
#[inline]
#[must_use]
pub(crate) fn es_n0_db_to_sigma(es_n0_db: f32) -> f32 {
    es_n0_db_to_sigma_f64(f64::from(es_n0_db))
}

/// The `f64`-input core of [`es_n0_db_to_sigma`] — the SSOT arithmetic.
///
/// Paired with [`es_n0_db_to_n0_f64`]: BOTH derive from the same full-precision
/// `f64` `es_n0_db`, so a frame kernel built `from_eb_n0` (a generally
/// non-`f32`-representable Es/N0) injects channel noise and demaps with the
/// SAME rounded SNR — deriving sigma through the narrowing `f32` helper while
/// N0 takes the `f64` path would make the two correspond to *different*
/// rounded SNRs, violating the "demap with the true channel N0" contract. For
/// any `f32`-representable input the `f32` wrapper agrees bit-for-bit
/// (widening is exact).
#[inline]
#[must_use]
pub(crate) fn es_n0_db_to_sigma_f64(es_n0_db: f64) -> f32 {
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (sigma_sq as f32).sqrt()
}

/// Converts an Es/N0 (dB) to the total complex AWGN noise variance `N0`.
///
/// Returns `N0 = 2 * sigma^2` with `sigma^2 = 1 / (2 * 10^(Es/N0 / 10))` —
/// the per-symbol total complex noise variance a soft demapper must assume to
/// be physically consistent with an AWGN channel at `es_n0_db` (the sibling
/// of [`es_n0_db_to_sigma`], which returns the per-axis standard deviation
/// the channel injects).
///
/// # The once-rounded contract
///
/// The computation is performed entirely in `f64` and rounded to `f32`
/// **exactly once**, at the end. This is the single source of truth for the
/// Es/N0 → N0 conversion: the `81d05bab` preset originally squared the
/// already-rounded `f32` sigma (`2.0 * sigma * sigma`), which rounds *twice*
/// and can differ from this value by an ULP — perturbing every demapped LLR
/// and breaking the chain-vs-SSOT byte-identity (`de160fc5`, design doc
/// §11). Every consumer (the preset's channel→demapper N0 coupling, the
/// [`DvbT2BicmFrameSim`](crate::frame_sim::DvbT2BicmFrameSim) frame kernel
/// via [`es_n0_db_to_n0_f64`], tests, and examples) must derive N0 through
/// this helper rather than re-deriving the formula inline.
///
/// # Arguments
///
/// * `es_n0_db` — channel Es/N0 in dB.
///
/// # Examples
///
/// ```
/// use gf2_sim::channels::es_n0_db_to_n0;
///
/// // 0 dB: Es/N0 = 1, so N0 = 2 * (1/2) = 1.
/// assert_eq!(es_n0_db_to_n0(0.0), 1.0);
///
/// // The once-rounded f64 derivation, bit-for-bit.
/// let es_n0_db = 6.0_f32;
/// let sigma_sq = 1.0_f64 / (2.0 * 10.0_f64.powf(f64::from(es_n0_db) / 10.0));
/// assert_eq!(es_n0_db_to_n0(es_n0_db).to_bits(), ((2.0 * sigma_sq) as f32).to_bits());
/// ```
///
/// # Complexity
///
/// O(1).
#[inline]
#[must_use]
pub fn es_n0_db_to_n0(es_n0_db: f32) -> f32 {
    es_n0_db_to_n0_f64(f64::from(es_n0_db))
}

/// The `f64`-input core of [`es_n0_db_to_n0`] — the SSOT arithmetic.
///
/// [`DvbT2BicmFrameSim`](crate::frame_sim::DvbT2BicmFrameSim)'s Es/N0 surface
/// is `f64` (its `from_eb_n0` constructor derives a generally
/// non-`f32`-representable Es/N0), so the frame kernel delegates here
/// directly; narrowing through the public `f32` helper would change its
/// `noise_var` on that path. For any `f32`-representable input the two
/// functions agree bit-for-bit (the public helper is a widening wrapper).
#[inline]
#[must_use]
pub(crate) fn es_n0_db_to_n0_f64(es_n0_db: f64) -> f32 {
    let es_n0_lin = 10.0_f64.powf(es_n0_db / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (2.0 * sigma_sq) as f32
}
