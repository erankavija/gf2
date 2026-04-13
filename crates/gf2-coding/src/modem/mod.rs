//! Shared modem data model.
//!
//! This module owns the validated constellation description consumed by
//! every downstream modem task: the batched mapper/demapper trait layer,
//! the exact log-MAP reference path, the Gray-QAM fast path, and the
//! bit-channel analysis collectors.
//!
//! The public surface is intentionally narrow:
//!
//! - [`ModemScalar`] (sealed) and [`DefaultScalar`] select the coordinate
//!   scalar. Presets default to `f32`; `*_with_scalar` variants exist for
//!   `f64` research workflows.
//! - [`SymbolPoint`], [`LabelWord`], [`BitChannelId`],
//!   [`BitChannelSemantics`], [`Normalization`], [`DemapMethod`], and
//!   [`ModemCapabilities`] are the value vocabulary.
//! - [`ModemSpec`] is the sealed, validated spec. All construction goes
//!   through presets here and, in the future, general builders.
//! - [`ModemView`] is the borrowed read-only view handed to backends.
//!
//! See `dev/active/c87c5043-constellation-data-model-plan.md` for the
//! locked design decisions behind this surface.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::modem::{ModemSpec, BitChannelSemantics};
//!
//! let spec = ModemSpec::gray_square_qam(16);
//! assert_eq!(spec.num_symbols(), 16);
//! assert_eq!(spec.bits_per_symbol(), 4);
//!
//! let view = spec.view();
//! assert_eq!(view.bit_channel(0), BitChannelSemantics::IAxisPam(0));
//! assert_eq!(view.bit_channel(2), BitChannelSemantics::QAxisPam(0));
//! ```

mod presets;
mod scalar;
mod spec;
mod types;
mod view;

pub use scalar::{DefaultScalar, ModemScalar};
pub use spec::ModemSpec;
pub use types::{
    BitChannelId, BitChannelSemantics, DemapMethod, LabelWord, ModemCapabilities, Normalization,
    SymbolPoint,
};
pub use view::ModemView;
