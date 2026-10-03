//! Modem framework: a validated constellation description ([`ModemSpec`])
//! and the batch traits ([`BatchMapper`], [`BatchSoftDemapper`],
//! [`BatchHardDemapper`]) that mapper and demapper backends implement.
//! [`ModemSpec::preferred_mapper`] and
//! [`ModemSpec::preferred_soft_demapper`] select a backend from the spec's
//! geometry.

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
