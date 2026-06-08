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
