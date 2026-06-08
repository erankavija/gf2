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

pub mod awgn;
pub mod rayleigh;
pub mod rician;

pub use awgn::Awgn;
pub use rayleigh::Rayleigh;
pub use rician::Rician;

/// Converts an Es/N0 (dB) to the per-axis AWGN noise standard deviation.
///
/// Returns `sigma = sqrt(1 / (2 * 10^(es_n0_db / 10)))` — the per-component
/// (real-axis) standard deviation applied to each of I and Q under the
/// unit-average-symbol-energy convention. The total complex noise variance is
/// `N0 = 2 * sigma^2`.
///
/// This is the single source of truth for the Es/N0 → sigma conversion shared
/// by the [`Awgn`], [`Rayleigh`], and [`Rician`] channel constructors.
///
/// (See also `frame_sim.rs`, which has an equivalent inline computation as part
/// of the separate, already-merged per-frame DVB-T2 kernel abstraction.)
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
    let es_n0_lin = 10.0_f64.powf(es_n0_db as f64 / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (sigma_sq as f32).sqrt()
}
