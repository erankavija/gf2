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
//! - [`ModemSpec`] is the sealed, validated spec. Construction goes
//!   through presets ([`ModemSpec::bpsk`], [`ModemSpec::gray_square_qam`])
//!   or the public [`ModemSpecBuilder`] for custom constellations.
//! - [`ModemView`] is the borrowed read-only view handed to backends.
//! - [`BatchMapper`], [`BatchSoftDemapper`], and [`BatchHardDemapper`] are
//!   the backend-agnostic batch interfaces implemented by every modem
//!   backend (scalar reference path, Gray-QAM fast path, SIMD kernels, and
//!   any future GPU backend). [`DemapInput`] is the shared per-batch input
//!   struct consumed by both demapper variants.
//! - [`ReferenceMapper`] is the correctness-first [`BatchMapper`]
//!   implementation for any validated [`ModemSpec`] (including custom
//!   research constellations built via [`ModemSpecBuilder`]).
//! - [`ReferenceSoftDemapper`] is the correctness-first
//!   [`BatchSoftDemapper`] implementation for any validated
//!   [`ModemSpec`]; it evaluates the exact log-MAP or max-log formula
//!   over every constellation point.
//! - [`GrayQamMapper`] is the scalar Gray-square-QAM fast-path
//!   implementation of [`BatchMapper`], built directly from a preset
//!   [`ModemSpec`].
//! - [`ModemAwgnChannel`] is a generic AWGN link adapter that composes
//!   any [`BatchMapper`] + [`BatchSoftDemapper`] over an
//!   [`crate::channel::AwgnChannel`]. It replaces the BPSK-only
//!   `channel::BpskAwgn` path for modem-framework workflows while
//!   leaving the legacy surface in place (migration is tracked in
//!   issues `bf865220`, `0cafa5f5`, `b3bb774a`, `5fd315c0`).
//!
//! See `dev/active/c87c5043-constellation-data-model-plan.md` for the
//! locked design decisions behind this surface.
//!
//! # Noise and normalization contract
//!
//! Every batched demapper in this module receives per-symbol noise via
//! [`DemapInput::noise_var`]. That field carries the **total per-symbol
//! complex AWGN noise variance** `N0 = 2 sigma^2`: for real AWGN with
//! independent Gaussian noise of variance `sigma^2` on each of I and Q,
//! callers must pass `2 * sigma^2`. This matches the log-MAP LLR formula
//! `LLR = log(p(y | bit = 0) / p(y | bit = 1))` with per-point squared
//! distances scaled by `1 / N0` (equivalently `1 / (2 sigma^2)`). The
//! authoritative discussion of how this composes with
//! [`crate::channel::AwgnChannel::variance`] (which returns `sigma^2`)
//! lives in the `awgn_link` module docs (see [`ModemAwgnChannel`]);
//! modem backends read the value supplied through
//! [`DemapInput::noise_var`] and never re-derive it.
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

mod awgn_link;
mod bit_pack;
mod builder;
mod demapper;
mod gray_qam_mapper;
mod mapper;
mod presets;
mod ref_demapper;
mod ref_mapper;
mod scalar;
mod spec;
mod types;
mod view;

pub use awgn_link::{ModemAwgnChannel, ModemChannelAdapter};
#[doc(hidden)]
pub use bit_pack::unpack_label_msb_first;
pub use builder::ModemSpecBuilder;
pub use demapper::{BatchHardDemapper, BatchSoftDemapper, DemapInput};
pub use gray_qam_mapper::GrayQamMapper;
pub use mapper::BatchMapper;
pub use ref_demapper::ReferenceSoftDemapper;
pub use ref_mapper::ReferenceMapper;
pub use scalar::{DefaultScalar, ModemScalar};
pub use spec::ModemSpec;
pub use types::{
    BitChannelAnalysis, BitChannelId, BitChannelSemantics, DemapMethod, LabelWord,
    ModemCapabilities, Normalization, SymbolPoint,
};
pub use view::ModemView;
