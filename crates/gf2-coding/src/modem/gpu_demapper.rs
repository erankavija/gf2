//! HIP/ROCm GPU adapter for the shared [`BatchSoftDemapper`] interface,
//! gated behind the `hip` Cargo feature. [`GpuGrayQamSoftDemapper`] accepts
//! the specs accepted by [`super::FastGrayQamDemapper::new`], runs on `f32`
//! only, and implements [`super::DemapMethod::MaxLog`] only.

use crate::llr::Llr;

use super::{
    BatchSoftDemapper, DemapInput, FastGrayQamDemapper, ModemCapabilities, ModemSpec,
    ModemSpecBuilder, ModemView,
};

use gf2_kernels_hip::{GpuGrayQamDemapper, HipError};

/// HIP/ROCm GPU soft demapper for Gray square-QAM and BPSK presets.
///
/// Runs the max-log kernel on device. [`BatchSoftDemapper::spec`]
/// advertises `supports_max_log = true` and
/// `supports_exact_log_map = false`, so a
/// [`super::DemapMethod::ExactLogMap`] request panics in the shared input
/// validation.
pub struct GpuGrayQamSoftDemapper {
    /// Copy of the input spec whose capabilities advertise only `MaxLog`.
    gpu_spec: ModemSpec<f32>,
    gpu: GpuGrayQamDemapper,
}

impl GpuGrayQamSoftDemapper {
    /// Constructs a GPU demapper and pre-allocates device buffers for up to
    /// `max_batch` symbols per [`Self::demap_llrs`] call.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on any device allocation / upload failure.
    ///
    /// # Panics
    ///
    /// Panics with the diagnostic from [`FastGrayQamDemapper::new`] when
    /// the spec is not a supported preset.
    pub fn new(spec: ModemSpec<f32>, max_batch: usize) -> Result<Self, HipError> {
        let cpu = FastGrayQamDemapper::<f32>::new(spec);
        let m = cpu.spec().bits_per_symbol();
        let is_bpsk = m == 1;
        // The CPU fast path holds the levels in f64; the device kernel takes f32.
        let pam_levels_f32: Vec<f32> = cpu.pam_levels().iter().map(|&v| v as f32).collect();
        let gpu = GpuGrayQamDemapper::new(&pam_levels_f32, m, is_bpsk, max_batch)?;

        let view = cpu.spec();
        let input_caps = view.capabilities();
        let narrowed_caps = ModemCapabilities {
            supports_exact_log_map: false,
            supports_max_log: true,
            analysis: input_caps.analysis,
        };
        let gpu_spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(view.bits_per_symbol())
            .points(view.points().to_vec())
            .labels(view.labels().to_vec())
            .bit_channels(view.bit_channels().to_vec())
            .normalization(view.normalization())
            .capabilities(narrowed_caps)
            .build();

        Ok(Self { gpu_spec, gpu })
    }

    /// Returns the maximum batch size this demapper was constructed for.
    #[inline]
    pub fn max_batch(&self) -> usize {
        self.gpu.max_batch()
    }
}

impl BatchSoftDemapper<f32> for GpuGrayQamSoftDemapper {
    fn spec(&self) -> ModemView<'_, f32> {
        self.gpu_spec.view()
    }

    /// Runs the GPU max-log Gray-QAM demap kernel and writes LLRs into
    /// `out_llrs`.
    ///
    /// # Panics
    ///
    /// Panics when `input` or `out_llrs.len()` violates the
    /// [`BatchSoftDemapper`] contract, when `input.method` is
    /// [`super::DemapMethod::ExactLogMap`], when `num_symbols > max_batch`,
    /// or when the GPU call returns an error.
    fn demap_llrs(&self, input: DemapInput<'_, f32>, out_llrs: &mut [Llr]) {
        let view = self.gpu_spec.view();
        let num_symbols = super::demapper::validate_demap_input(
            "GpuGrayQamSoftDemapper::demap_llrs",
            &view,
            &input,
            out_llrs.len(),
        );
        if num_symbols == 0 {
            return;
        }
        assert!(
            num_symbols <= self.gpu.max_batch(),
            "GpuGrayQamSoftDemapper::demap_llrs: num_symbols {num_symbols} > max_batch {}",
            self.gpu.max_batch()
        );

        let flat_llrs = self
            .gpu
            .demap_batch(
                input.rx_i,
                input.rx_q,
                input.gain_i,
                input.gain_q,
                input.noise_var,
            )
            .expect("GpuGrayQamSoftDemapper::demap_llrs: HIP kernel launch failed");

        let m = view.bits_per_symbol() as usize;
        let expected = num_symbols * m;
        assert_eq!(
            flat_llrs.len(),
            expected,
            "GpuGrayQamSoftDemapper::demap_llrs: kernel returned {} LLRs, expected {expected}",
            flat_llrs.len()
        );
        for (dst, src) in out_llrs.iter_mut().zip(flat_llrs.iter()) {
            *dst = Llr::new(*src);
        }
    }
}
