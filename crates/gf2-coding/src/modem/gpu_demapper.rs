//! HIP/ROCm GPU adapter for the shared [`BatchSoftDemapper`] interface.
//!
//! [`GpuGrayQamSoftDemapper`] wraps `gf2_kernels_hip::GpuGrayQamDemapper`
//! behind the same [`BatchSoftDemapper`] trait implemented by
//! [`super::ReferenceSoftDemapper`] and [`super::FastGrayQamDemapper`].
//! This is **prototype / research-grade** code gated behind the `hip`
//! Cargo feature; it backs the CPU/GPU crossover measurement tracked in
//! JIT issue `9c37ec8c` without adding any new public traits to the
//! modem surface.
//!
//! # Scope and limitations
//!
//! - Only the Gray-QAM presets validated by
//!   [`super::FastGrayQamDemapper::new`] are accepted (BPSK + orders
//!   `{4, 16, 64, 256}`).
//! - Only `f32` scalars are supported on device. The wider
//!   [`BatchSoftDemapper<S>`] trait is still generic, but this adapter
//!   implements it for `S = f32` only.
//! - The kernel implements the max-log variant only. The adapter
//!   advertises only [`super::DemapMethod::MaxLog`] via its
//!   [`super::ModemCapabilities`]; callers requesting
//!   [`super::DemapMethod::ExactLogMap`] are rejected by the standard
//!   [`BatchSoftDemapper`] input validator (the spec does not advertise
//!   that method). Callers that need log-MAP must keep using the CPU
//!   path. This matches the prototype's target: the crossover measurement
//!   compares the GPU max-log path against the CPU max-log path.
//! - The construction contract defers to
//!   [`super::FastGrayQamDemapper::new`] for preset validation: the
//!   adapter holds the CPU fast-path demapper as its numerical oracle
//!   source-of-truth for the PAM level table (SSOT) and construction
//!   panics with the same diagnostic on non-preset specs.

use crate::llr::Llr;

use super::{
    BatchSoftDemapper, DemapInput, FastGrayQamDemapper, ModemCapabilities, ModemSpec,
    ModemSpecBuilder, ModemView,
};

use gf2_kernels_hip::{GpuGrayQamDemapper, HipError};

/// HIP/ROCm GPU soft demapper for Gray square-QAM and BPSK presets.
///
/// Implements [`BatchSoftDemapper<f32>`] for the same Gray-square-QAM +
/// BPSK preset family as [`super::FastGrayQamDemapper`], running the
/// max-log kernel on device. See the module docs for the scope and
/// limitations; the construction-time validation is delegated to the CPU
/// fast path so this adapter and the CPU path accept the exact same set
/// of specs.
///
/// # Method advertisement
///
/// The GPU kernel implements only [`super::DemapMethod::MaxLog`]. The
/// adapter therefore advertises a narrowed [`super::ModemCapabilities`]
/// with `supports_max_log = true` and `supports_exact_log_map = false`
/// via [`BatchSoftDemapper::spec`]. Callers requesting
/// [`super::DemapMethod::ExactLogMap`] are rejected by the standard
/// pre-flight validator with the canonical
/// `"spec does not advertise ExactLogMap support"` panic — no
/// special-case rejection lives in the adapter itself. Callers that
/// need log-MAP must stay on [`FastGrayQamDemapper`].
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::{
///     BatchSoftDemapper, DemapInput, DemapMethod, GpuGrayQamSoftDemapper, ModemSpec,
/// };
///
/// let spec = ModemSpec::<f32>::gray_square_qam(16);
/// let demapper = GpuGrayQamSoftDemapper::new(spec, 1024).unwrap();
///
/// let rx_i = [0.3_f32; 8];
/// let rx_q = [-0.4_f32; 8];
/// let nv = [0.1_f32; 8];
/// let input = DemapInput::<f32> {
///     rx_i: &rx_i,
///     rx_q: &rx_q,
///     gain_i: None,
///     gain_q: None,
///     noise_var: &nv,
///     method: DemapMethod::MaxLog,
/// };
/// let mut out = vec![Llr::new(0.0); 8 * 4];
/// demapper.demap_llrs(input, &mut out);
/// ```
pub struct GpuGrayQamSoftDemapper {
    /// Narrowed [`ModemSpec`] that mirrors the CPU fast path's spec but
    /// advertises only [`super::DemapMethod::MaxLog`] via its
    /// capabilities. Returned by [`BatchSoftDemapper::spec`] so the
    /// shared [`super::demapper::validate_demap_input`] choke point
    /// correctly rejects `ExactLogMap` requests without the adapter
    /// needing its own special case. The underlying
    /// [`FastGrayQamDemapper`] instance is consumed at construction time
    /// solely for preset validation and the SSOT PAM-level readout; the
    /// adapter does not retain it post-construction.
    gpu_spec: ModemSpec<f32>,
    gpu: GpuGrayQamDemapper,
}

impl GpuGrayQamSoftDemapper {
    /// Constructs a new GPU demapper for a Gray-square-QAM or BPSK preset
    /// and pre-allocates device buffers for up to `max_batch` symbols.
    ///
    /// The `spec` is validated via [`FastGrayQamDemapper::new`], so the
    /// same diagnostic error messages apply. The PAM level table is
    /// read from the CPU fast path (SSOT) and uploaded to device once.
    /// A narrowed internal [`ModemSpec`] that mirrors `spec` but
    /// advertises only [`super::DemapMethod::MaxLog`] is also built;
    /// it is the view returned from [`BatchSoftDemapper::spec`] so that
    /// generic callers querying capabilities see an honest picture of
    /// what the adapter can execute.
    ///
    /// # Arguments
    ///
    /// * `spec` - A Gray-square-QAM preset or BPSK preset.
    /// * `max_batch` - Maximum number of received symbols per
    ///   [`Self::demap_llrs`] call. Device allocations are sized for
    ///   this bound.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on any device allocation / upload failure.
    ///
    /// # Panics
    ///
    /// Panics with the diagnostic from [`FastGrayQamDemapper::new`] when
    /// the spec is not a supported preset.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::modem::{GpuGrayQamSoftDemapper, ModemSpec};
    ///
    /// let demapper = GpuGrayQamSoftDemapper::new(
    ///     ModemSpec::<f32>::gray_square_qam(64),
    ///     4096,
    /// )
    /// .unwrap();
    /// assert_eq!(demapper.max_batch(), 4096);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`max_batch`) device allocation plus one H→D copy of `axis_len`
    /// f32 values.
    pub fn new(spec: ModemSpec<f32>, max_batch: usize) -> Result<Self, HipError> {
        let cpu = FastGrayQamDemapper::<f32>::new(spec);
        let m = cpu.spec().bits_per_symbol();
        let is_bpsk = m == 1;
        // SSOT: read the validated PAM levels from the CPU fast path and
        // narrow to f32 for device upload. The CPU side owns the table
        // in f64 for numerical-headroom parity with its internal
        // kernels; the GPU adapter is f32-only by design.
        let pam_levels_f32: Vec<f32> = cpu.pam_levels().iter().map(|&v| v as f32).collect();
        let gpu = GpuGrayQamDemapper::new(&pam_levels_f32, m, is_bpsk, max_batch)?;

        // Build a narrowed ModemSpec mirroring the input spec but with
        // the capability flags tightened to advertise only MaxLog. The
        // CPU fast path has already validated the preset, so reconstructing
        // via the public ModemSpecBuilder on the same points/labels/etc. is
        // guaranteed to succeed; we preserve the existing per-bit analysis
        // slice so downstream `ModemView::bit_channel_analysis` calls keep
        // returning the preset's analytic metadata.
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
    ///
    /// # Complexity
    ///
    /// O(1).
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
    /// The per-symbol `noise_var` is the total complex AWGN variance
    /// `N0 = 2 sigma^2`, exactly as for every other backend.
    ///
    /// This adapter advertises only [`super::DemapMethod::MaxLog`] in its
    /// [`super::ModemCapabilities`]; callers requesting
    /// [`super::DemapMethod::ExactLogMap`] are rejected by the standard
    /// [`super::demapper::validate_demap_input`] pre-flight with the
    /// canonical "method not advertised" panic — the adapter does not
    /// carry its own special-case check.
    ///
    /// # Panics
    ///
    /// Panics via the shared pre-flight validator when `input` or
    /// `out_llrs.len()` violates the [`BatchSoftDemapper`] contract, when
    /// the selected method is not advertised by
    /// [`BatchSoftDemapper::spec`] (this adapter only advertises
    /// [`super::DemapMethod::MaxLog`], so `ExactLogMap` panics here),
    /// when `num_symbols > max_batch`, or when the underlying GPU call
    /// returns an error (surfaced as a panic because the prototype
    /// adapter has no infallible error channel through the trait).
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
