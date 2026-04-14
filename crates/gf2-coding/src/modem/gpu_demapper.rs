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
//! - The kernel implements the max-log variant. When
//!   [`super::DemapMethod::ExactLogMap`] is requested the adapter still
//!   runs the kernel but the returned LLRs are the max-log
//!   approximation; callers that need log-MAP must keep using the CPU
//!   path. This matches the prototype's target: the crossover
//!   measurement compares the GPU max-log path against the CPU max-log
//!   path.
//! - The construction contract defers to
//!   [`super::FastGrayQamDemapper::new`] for preset validation: the
//!   adapter holds the CPU fast-path demapper as its numerical oracle
//!   source-of-truth for the PAM level table (SSOT) and construction
//!   panics with the same diagnostic on non-preset specs.

use crate::llr::Llr;

use super::{BatchSoftDemapper, DemapInput, FastGrayQamDemapper, ModemSpec, ModemView};

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
    /// Holds the validated CPU fast-path demapper as the single source
    /// of truth for preset metadata (bits-per-symbol, BPSK flag, PAM
    /// level table). The GPU backend never rederives any of these.
    cpu: FastGrayQamDemapper<f32>,
    gpu: GpuGrayQamDemapper,
}

impl GpuGrayQamSoftDemapper {
    /// Constructs a new GPU demapper for a Gray-square-QAM or BPSK preset
    /// and pre-allocates device buffers for up to `max_batch` symbols.
    ///
    /// The `spec` is validated via [`FastGrayQamDemapper::new`], so the
    /// same diagnostic error messages apply. The PAM level table is
    /// read from the CPU fast path (SSOT) and uploaded to device once.
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
        Ok(Self { cpu, gpu })
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
        self.cpu.spec()
    }

    /// Runs the GPU max-log Gray-QAM demap kernel and writes LLRs into
    /// `out_llrs`.
    ///
    /// The per-symbol `noise_var` is the total complex AWGN variance
    /// `N0 = 2 sigma^2`, exactly as for every other backend. When
    /// [`super::DemapMethod::ExactLogMap`] is selected the kernel still
    /// runs (max-log approximation); see the module-level note on this
    /// design choice.
    ///
    /// # Panics
    ///
    /// Panics via the shared pre-flight validator when `input` or
    /// `out_llrs.len()` violates the [`BatchSoftDemapper`] contract, when
    /// `num_symbols > max_batch`, or when the underlying GPU call
    /// returns an error (surfaced as a panic because the prototype
    /// adapter has no infallible error channel through the trait).
    fn demap_llrs(&self, input: DemapInput<'_, f32>, out_llrs: &mut [Llr]) {
        let view = self.cpu.spec();
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
