//! Shared modem data model.
//!
//! This module owns the validated constellation description consumed by
//! every downstream modem task: the batched mapper/demapper trait layer,
//! the exact log-MAP reference path, the Gray-QAM fast path, and the
//! bit-channel analysis collectors.
//!
//! # What this framework is
//!
//! The modem framework is a single, validated data model
//! ([`ModemSpec`]) plus two backend-agnostic traits
//! ([`BatchMapper`], [`BatchSoftDemapper`]) that decouple *what* a modem
//! is (constellation geometry + bit labelling) from *how* the framework
//! maps bits to symbols and symbols to LLRs. Every specialized backend
//! (reference log-MAP, Gray-QAM fast path, SIMD kernels, GPU adapters)
//! plugs into the same trait surface, so downstream code — AWGN / Rician
//! links, BER harnesses, bit-channel analysis — is written once against
//! the traits and works across all backends.
//!
//! # Preset workflow
//!
//! For the standard constellations you should use a preset:
//!
//! - [`ModemSpec::bpsk`] — 1 bit per symbol.
//! - [`ModemSpec::gray_square_qam`] — Gray-coded square QAM of order
//!   `2, 4, 16, 64, 256` (matches the DVB-T2 bit-to-cell mapping).
//!
//! The `*_with_scalar` variants give an `f64` spec for research workflows.
//!
//! ```
//! use gf2_coding::modem::{BatchMapper, BatchSoftDemapper, ModemSpec};
//!
//! let spec = ModemSpec::<f32>::gray_square_qam(16);
//! let mapper = spec.preferred_mapper();
//! let demapper = spec.preferred_soft_demapper();
//! assert_eq!(mapper.spec().bits_per_symbol(), 4);
//! assert_eq!(demapper.spec().bits_per_symbol(), 4);
//! ```
//!
//! See [`modem_gray_qam_preset`] for an end-to-end example that maps a
//! batch of random bits through `BpskAwgnChannel`-style AWGN and measures
//! the uncoded BER.
//!
//! [`modem_gray_qam_preset`]: https://github.com/openamateur/gf2/blob/main/crates/gf2-coding/examples/modem_gray_qam_preset.rs
//!
//! # Custom constellation workflow
//!
//! For research constellations — non-square QAM, irregular labellings,
//! 8-PSK, APSK — construct a spec through [`ModemSpecBuilder`]:
//!
//! ```
//! use gf2_coding::modem::{LabelWord, ModemSpec, ModemSpecBuilder, SymbolPoint};
//!
//! let spec: ModemSpec<f32> = ModemSpecBuilder::<f32>::new()
//!     .bits_per_symbol(2)
//!     .points(vec![
//!         SymbolPoint::new(1.0, 0.0),
//!         SymbolPoint::new(0.0, 1.0),
//!         SymbolPoint::new(-1.0, 0.0),
//!         SymbolPoint::new(0.0, -1.0),
//!     ])
//!     .labels(vec![
//!         LabelWord::new(0b00, 2),
//!         LabelWord::new(0b01, 2),
//!         LabelWord::new(0b11, 2),
//!         LabelWord::new(0b10, 2),
//!     ])
//!     .build();
//! assert_eq!(spec.num_symbols(), 4);
//! ```
//!
//! The builder normalizes the constellation to unit average symbol
//! energy by default, validates labels are a bijection, and panics with a
//! descriptive message on any invariant violation. Any spec built this
//! way is a first-class citizen: it plugs into every downstream path
//! described below. [`modem_custom_constellation`] walks through a
//! non-Gray 8-PSK example end-to-end.
//!
//! [`modem_custom_constellation`]: https://github.com/openamateur/gf2/blob/main/crates/gf2-coding/examples/modem_custom_constellation.rs
//!
//! # Shared API: `preferred_mapper` / `preferred_soft_demapper`
//!
//! Rather than constructing a backend by name, call
//! [`ModemSpec::preferred_mapper`] and
//! [`ModemSpec::preferred_soft_demapper`] on any validated spec. These
//! factories inspect the spec's geometry and return the fastest
//! correctness-equivalent backend available:
//!
//! - Gray square-QAM presets (and custom specs whose geometry matches
//!   the preset layout) route to the optimized [`GrayQamMapper`] and
//!   [`FastGrayQamDemapper`].
//! - Every other validated spec falls back transparently to
//!   [`ReferenceMapper`] and [`ReferenceSoftDemapper`], which implement
//!   the exact log-MAP / max-log formulas over an arbitrary constellation.
//!
//! New code should prefer the factories; direct backend construction is
//! reserved for advanced paths that need backend-specific APIs (GPU
//! adapters, SIMD scratch reuse).
//!
//! # Integration with channel and simulation primitives
//!
//! - [`ModemAwgnChannel`] glues any `(BatchMapper, BatchSoftDemapper,
//!   AwgnChannel)` triple into a `bits -> LLRs` pipeline. It is the
//!   canonical AWGN link for any modem spec.
//! - [`crate::simulation::BpskAwgnChannel`] is a ready-made
//!   [`crate::simulation::ChannelModel`] routed through the same BPSK
//!   preset for the legacy 1-D-noise path.
//! - [`ModemChannelAdapter`] wraps a mapper + demapper behind
//!   [`crate::simulation::ChannelModel`] and plugs into
//!   [`crate::simulation::SimulationRunner::run_uncoded_ber_with_channel`]
//!   (and the coded runners). It performs the `Eb/N0 → sigma²` conversion
//!   for any `bits_per_symbol` via [`awgn_link::unit_energy_sigma_sq_from_eb_n0_db`].
//! - [`crate::fading::QpskRicianChannelModel`] is the Rician-fading
//!   counterpart, built on the same shared mapper/demapper surface.
//!
//! See [`modem_simulation_harness`] for a `SimulationRunner` sweep driven
//! by a Gray-QAM preset and a Rician-fading channel model.
//!
//! [`modem_simulation_harness`]: https://github.com/openamateur/gf2/blob/main/crates/gf2-coding/examples/modem_simulation_harness.rs
//!
//! # Public surface summary
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
//! - Prefer the shared-API factories
//!   [`ModemSpec::preferred_mapper`] and
//!   [`ModemSpec::preferred_soft_demapper`] to obtain the best-available
//!   backend for a given spec; they return the optimized Gray-QAM
//!   backend for recognized preset layouts and transparently fall back
//!   to the reference path for every other validated spec.
//! - [`ModemAwgnChannel`] is a generic AWGN link adapter that composes
//!   any [`BatchMapper`] + [`BatchSoftDemapper`] over an
//!   [`crate::channel::AwgnChannel`]. It is the canonical AWGN link for
//!   any modem spec; the BPSK reference channel in
//!   [`crate::simulation::BpskAwgnChannel`] routes through the same
//!   `BPSK` preset for the `ChannelModel` consumers of the simulation
//!   harness.
//!
//! See `dev/active/c87c5043/c87c5043-constellation-data-model-plan.md` for the
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

pub mod analysis;
pub mod analysis_capture;
pub mod awgn_link;
mod bit_pack;
mod builder;
mod demapper;
mod fast_gray_qam_demapper;
mod gray_qam_mapper;
mod mapper;
mod presets;
mod ref_demapper;
mod ref_mapper;
mod scalar;
mod spec;
#[doc(hidden)]
pub mod test_oracle;
mod types;
mod view;

pub use analysis_capture::AnalysisCapture;
pub use awgn_link::{ModemAwgnChannel, ModemChannelAdapter};
#[doc(hidden)]
pub use bit_pack::unpack_label_msb_first;
pub use builder::ModemSpecBuilder;
pub use demapper::{BatchHardDemapper, BatchSoftDemapper, DemapInput};
pub use fast_gray_qam_demapper::FastGrayQamDemapper;
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

#[cfg(feature = "hip")]
pub mod gpu_demapper;
#[cfg(feature = "hip")]
pub use gpu_demapper::GpuGrayQamSoftDemapper;
