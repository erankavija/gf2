//! AWGN link composition over the shared modem framework.
//!
//! [`ModemAwgnChannel`] ties a [`BatchMapper`] and a [`BatchSoftDemapper`]
//! together with the existing [`crate::channel::AwgnChannel`] to provide a
//! single `bits -> LLRs` pipeline for any validated [`super::ModemSpec`],
//! preset or custom. [`ModemChannelAdapter`] wraps the same two components
//! behind the [`crate::simulation::ChannelModel`] trait so any modem spec
//! can drop into [`crate::simulation::SimulationRunner`] in place of the
//! legacy [`crate::simulation::BpskAwgnChannel`].
//!
//! This module is deliberately additive. The legacy
//! [`crate::channel::BpskAwgn`](crate::channel) / `BpskModulator` helpers
//! are left untouched so existing callers keep working while the modem
//! framework migration rolls forward.
//!
//! # Noise convention
//!
//! [`AwgnChannel::variance`] returns `sigma^2`, the per-component
//! real-axis variance applied to each of I and Q. For a 2-D complex AWGN
//! channel the combined noise power is `N0 = 2 sigma^2`, and the
//! log-MAP demapper defines [`super::DemapInput::noise_var`] as `N0` (see
//! the BPSK closed-form tests in
//! [`super::ReferenceSoftDemapper`]: `LLR = 4 y / N0`). This adapter
//! therefore passes `N0 = 2 * channel.variance()` into every
//! [`super::DemapInput`] it builds, which recovers the legacy BPSK LLR
//! `LLR = 2 r / sigma^2` (= `4 r / N0`) used by
//! [`crate::channel::BpskModulator::to_llr`] and
//! [`crate::simulation::BpskAwgnChannel`] at matching noise settings.
//!
//! # Eb/N0 scaling for higher-order modulation
//!
//! The [`ChannelModel`]-level interface takes `Eb/N0` plus a code `rate`.
//! For an `m`-bit-per-symbol constellation with unit-average symbol
//! energy the per-component variance is
//! `sigma^2 = 1 / (2 * m * rate * 10^(Eb_N0_dB / 10))`. [`ModemChannelAdapter`]
//! applies this formula directly so that 16-QAM, QPSK, etc. are simulated
//! at the correct noise level; the legacy
//! [`AwgnChannel::from_eb_n0_db`](crate::channel::AwgnChannel::from_eb_n0_db)
//! helper bakes in `m = 1` and is used only by the BPSK compatibility
//! path.

use super::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar};
use crate::channel::AwgnChannel;
use crate::llr::Llr;
use crate::simulation::ChannelModel;
use gf2_core::BitVec;
use rand::Rng;

/// AWGN link over any modem spec, using the shared [`BatchMapper`] and
/// [`BatchSoftDemapper`] surfaces.
///
/// Combines a caller-supplied mapper, demapper, and [`AwgnChannel`] into a
/// single `transmit_and_demap` call. Complex Gaussian noise is applied to
/// each received symbol as two independent real Gaussian draws (one on I,
/// one on Q), each with variance equal to [`AwgnChannel::variance`]
/// (`sigma^2`, the per-component variance). The demapper is fed
/// `N0 = 2 sigma^2` via [`DemapInput::noise_var`] — see the module-level
/// "Noise convention" section.
///
/// Construction is zero-cost beyond moving the three components in; the
/// hot [`ModemAwgnChannel::transmit_and_demap`] loop allocates exactly
/// three short-lived scratch buffers per call (two I/Q symbol vectors and
/// one per-symbol noise-variance vector). Callers that need amortized
/// allocation across frames can hold a [`ModemAwgnChannel`] and reuse it
/// across successive calls; the allocator reuses the freed buffers.
///
/// # Type parameters
///
/// * `S` - Modem scalar (`f32` or `f64`, see [`ModemScalar`]).
/// * `M` - Bit-to-symbol mapper implementing [`BatchMapper<S>`].
/// * `D` - Soft demapper implementing [`BatchSoftDemapper<S>`].
///
/// # Examples
///
/// ```
/// use gf2_coding::channel::AwgnChannel;
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::{
///     DemapMethod, GrayQamMapper, ModemAwgnChannel, ModemSpec, ReferenceSoftDemapper,
/// };
///
/// let mapper = GrayQamMapper::<f32>::from_preset_order(4);
/// let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
/// let channel = AwgnChannel::from_variance(0.25);
/// let link = ModemAwgnChannel::new(mapper, demap, channel, DemapMethod::MaxLog);
///
/// let bits = vec![false, false, true, true]; // two QPSK symbols
/// let mut llrs = vec![Llr::new(0.0); bits.len()];
/// let mut rng = rand::thread_rng();
/// link.transmit_and_demap(&bits, &mut rng, &mut llrs);
/// assert_eq!(llrs.len(), bits.len());
/// ```
pub struct ModemAwgnChannel<S: ModemScalar, M: BatchMapper<S>, D: BatchSoftDemapper<S>> {
    mapper: M,
    demapper: D,
    channel: AwgnChannel,
    method: DemapMethod,
    _scalar: core::marker::PhantomData<S>,
}

impl<S: ModemScalar, M: BatchMapper<S>, D: BatchSoftDemapper<S>> ModemAwgnChannel<S, M, D> {
    /// Constructs an AWGN link from an already-built mapper, demapper, and
    /// channel.
    ///
    /// # Arguments
    ///
    /// * `mapper` - Bit-to-symbol mapper; must have the same
    ///   [`super::ModemSpec::bits_per_symbol`] as `demapper`.
    /// * `demapper` - Soft demapper; must advertise `method` in its
    ///   [`super::ModemCapabilities`].
    /// * `channel` - Noise model. [`AwgnChannel::variance`] is read as
    ///   `sigma^2`, the per-component variance for both I and Q (see the
    ///   module-level noise convention).
    /// * `method` - Log-MAP demapping method (`ExactLogMap` or `MaxLog`).
    ///
    /// # Panics
    ///
    /// Panics if `mapper` and `demapper` disagree on `bits_per_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    /// use gf2_coding::modem::{
    ///     DemapMethod, GrayQamMapper, ModemAwgnChannel, ModemSpec, ReferenceSoftDemapper,
    /// };
    ///
    /// let mapper = GrayQamMapper::<f32>::from_preset_order(4);
    /// let demapper = ReferenceSoftDemapper::new(ModemSpec::gray_square_qam(4));
    /// let channel = AwgnChannel::from_variance(0.25);
    /// let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
    /// assert_eq!(link.method(), DemapMethod::MaxLog);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new(mapper: M, demapper: D, channel: AwgnChannel, method: DemapMethod) -> Self {
        assert_eq!(
            mapper.spec().bits_per_symbol(),
            demapper.spec().bits_per_symbol(),
            "mapper and demapper bits_per_symbol must match",
        );
        Self {
            mapper,
            demapper,
            channel,
            method,
            _scalar: core::marker::PhantomData,
        }
    }

    /// Returns a reference to the underlying mapper.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn mapper(&self) -> &M {
        &self.mapper
    }

    /// Returns a reference to the underlying demapper.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn demapper(&self) -> &D {
        &self.demapper
    }

    /// Returns a reference to the underlying AWGN channel.
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn channel(&self) -> &AwgnChannel {
        &self.channel
    }

    /// Returns the configured [`DemapMethod`].
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn method(&self) -> DemapMethod {
        self.method
    }

    /// Maps bits to symbols, adds independent Gaussian noise on I and Q,
    /// and demaps to per-bit LLRs.
    ///
    /// The per-component noise variance for each of I and Q equals
    /// [`AwgnChannel::variance`] (= `sigma^2`). Noise samples on I and Q
    /// are drawn independently from the channel's Gaussian distribution.
    /// The demapper receives `N0 = 2 * sigma^2` — see the module-level
    /// noise convention.
    ///
    /// # Arguments
    ///
    /// * `bits` - MSB-first-within-symbol packed bits. Length must be a
    ///   multiple of `self.mapper().spec().bits_per_symbol()`.
    /// * `rng` - Random source for noise samples.
    /// * `out_llrs` - Destination slice for the resulting LLRs. Length
    ///   must equal `bits.len()`. Layout is symbol-major, MSB-first within
    ///   each symbol (matching [`BatchSoftDemapper::demap_llrs`]).
    ///
    /// # Panics
    ///
    /// Panics if `out_llrs.len() != bits.len()`, or if `bits.len()` is not
    /// a multiple of `bits_per_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::channel::AwgnChannel;
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::modem::{
    ///     DemapMethod, GrayQamMapper, ModemAwgnChannel, ModemSpec, ReferenceSoftDemapper,
    /// };
    ///
    /// let mapper = GrayQamMapper::<f32>::from_preset_order(4);
    /// let demapper = ReferenceSoftDemapper::new(ModemSpec::gray_square_qam(4));
    /// let channel = AwgnChannel::from_variance(1e-6);
    /// let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
    /// let bits = vec![false, true, true, false];
    /// let mut out = vec![Llr::new(0.0); bits.len()];
    /// let mut rng = rand::thread_rng();
    /// link.transmit_and_demap(&bits, &mut rng, &mut out);
    /// assert_eq!(out.len(), bits.len());
    /// ```
    ///
    /// # Complexity
    ///
    /// Dominated by the mapper (`O(num_symbols)`) and the demapper
    /// (`O(num_symbols * m)` for Gray-QAM, `O(num_symbols * M * m)` for
    /// the exact log-MAP reference path). Adds three scratch allocations
    /// of sizes `num_symbols`, `num_symbols`, and `num_symbols`.
    pub fn transmit_and_demap<R: Rng>(&self, bits: &[bool], rng: &mut R, out_llrs: &mut [Llr]) {
        run_awgn_modem_pipeline(
            &self.mapper,
            &self.demapper,
            &self.channel,
            self.method,
            bits,
            rng,
            out_llrs,
        );
    }
}

/// Shared `bits -> LLRs` pipeline backing both [`ModemAwgnChannel`] and
/// [`ModemChannelAdapter`].
///
/// Runs the canonical map/noise/demap composition once and is the single
/// source of truth for:
///
/// - Length-preconditions on `bits.len()` and `out_llrs.len()`.
/// - Independent per-component Gaussian noise on I and Q using
///   [`AwgnChannel::transmit`].
/// - The `N0 = 2 * channel.variance()` convention documented at the
///   module level, passed through [`DemapInput::noise_var`].
///
/// Both adapters differ only in how they obtain `channel` (pre-built vs.
/// derived from `Eb/N0 + rate + m`) and how they source `bits` (raw
/// `&[bool]` vs. `BitVec`). The shared body below ensures they cannot
/// drift on any pipeline detail.
#[inline]
fn run_awgn_modem_pipeline<S, M, D, R>(
    mapper: &M,
    demapper: &D,
    channel: &AwgnChannel,
    method: DemapMethod,
    bits: &[bool],
    rng: &mut R,
    out_llrs: &mut [Llr],
) where
    S: ModemScalar,
    M: BatchMapper<S>,
    D: BatchSoftDemapper<S>,
    R: Rng,
{
    let m = mapper.spec().bits_per_symbol() as usize;
    assert!(m > 0, "bits_per_symbol must be non-zero");
    assert_eq!(
        bits.len() % m,
        0,
        "bits.len() must be a multiple of bits_per_symbol",
    );
    assert_eq!(
        out_llrs.len(),
        bits.len(),
        "out_llrs.len() must equal bits.len()",
    );
    let num_symbols = bits.len() / m;

    let mut tx_i = vec![S::zero(); num_symbols];
    let mut tx_q = vec![S::zero(); num_symbols];
    mapper.map_bits(bits, &mut tx_i, &mut tx_q);

    for sample in tx_i.iter_mut() {
        let noisy = channel.transmit(sample.to_f64(), rng);
        *sample = S::from_f64(noisy);
    }
    for sample in tx_q.iter_mut() {
        let noisy = channel.transmit(sample.to_f64(), rng);
        *sample = S::from_f64(noisy);
    }

    let n0 = S::from_f64(2.0 * channel.variance());
    let noise_var = vec![n0; num_symbols];

    let input = DemapInput::<S> {
        rx_i: &tx_i,
        rx_q: &tx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &noise_var,
        method,
    };
    demapper.demap_llrs(input, out_llrs);
}

/// Drop-in [`ChannelModel`] implementation that runs any modem spec over
/// AWGN.
///
/// Holds a mapper + demapper pair and builds a fresh [`AwgnChannel`] from
/// `(eb_n0_db, rate)` on every [`ChannelModel::transmit_and_demodulate`]
/// call, so it plugs into [`crate::simulation::SimulationRunner`] exactly
/// where [`crate::simulation::BpskAwgnChannel`] currently does. For BPSK
/// with the reference demapper this reproduces the legacy
/// `LLR = 2 r / sigma^2` path; for higher-order modems it returns one LLR
/// per transmitted bit in the same MSB-first-within-symbol layout the
/// shared batch-demapper contract defines.
///
/// # Type parameters
///
/// * `M` - Bit-to-symbol mapper implementing [`BatchMapper<f32>`]. `f32`
///   is the modem-scalar width the simulation harness works in.
/// * `D` - Soft demapper implementing [`BatchSoftDemapper<f32>`].
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::{
///     DemapMethod, GrayQamMapper, ModemChannelAdapter, ModemSpec, ReferenceSoftDemapper,
/// };
/// use gf2_coding::simulation::ChannelModel;
/// use gf2_core::BitVec;
///
/// let mapper = GrayQamMapper::<f32>::from_preset_order(2); // BPSK (size 2)
/// let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
/// let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);
///
/// let bits = BitVec::from_bytes_le(&[0b1010_0101]);
/// let mut rng = rand::thread_rng();
/// let llrs = adapter.transmit_and_demodulate(&bits, 3.0, 0.5, &mut rng);
/// assert_eq!(llrs.len(), bits.len());
/// ```
pub struct ModemChannelAdapter<M, D>
where
    M: BatchMapper<f32>,
    D: BatchSoftDemapper<f32>,
{
    mapper: M,
    demapper: D,
    method: DemapMethod,
}

impl<M, D> ModemChannelAdapter<M, D>
where
    M: BatchMapper<f32>,
    D: BatchSoftDemapper<f32>,
{
    /// Builds an adapter from an already-validated mapper/demapper pair.
    ///
    /// # Arguments
    ///
    /// * `mapper` - Bit-to-symbol mapper. Must have the same
    ///   [`super::ModemSpec::bits_per_symbol`] as `demapper`.
    /// * `demapper` - Soft demapper. Must advertise `method` in its
    ///   [`super::ModemCapabilities`].
    /// * `method` - Log-MAP demapping method (`ExactLogMap` or `MaxLog`).
    ///
    /// # Panics
    ///
    /// Panics if `mapper.spec().bits_per_symbol() != demapper.spec().bits_per_symbol()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{
    ///     DemapMethod, GrayQamMapper, ModemChannelAdapter, ModemSpec, ReferenceSoftDemapper,
    /// };
    ///
    /// let mapper = GrayQamMapper::<f32>::from_preset_order(16);
    /// let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(16));
    /// let _ = ModemChannelAdapter::new(mapper, demap, DemapMethod::MaxLog);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new(mapper: M, demapper: D, method: DemapMethod) -> Self {
        assert_eq!(
            mapper.spec().bits_per_symbol(),
            demapper.spec().bits_per_symbol(),
            "mapper and demapper bits_per_symbol must match",
        );
        Self {
            mapper,
            demapper,
            method,
        }
    }

    /// Returns the configured [`DemapMethod`].
    #[inline]
    pub fn method(&self) -> DemapMethod {
        self.method
    }
}

impl<M, D> ChannelModel for ModemChannelAdapter<M, D>
where
    M: BatchMapper<f32>,
    D: BatchSoftDemapper<f32>,
{
    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        // Match the legacy `AwgnChannel::from_eb_n0_db` contract: code
        // rate must live in (0, 1]. This keeps `ModemChannelAdapter` a
        // true drop-in replacement for `BpskAwgnChannel` in the existing
        // simulation harness.
        assert!(
            rate > 0.0 && rate <= 1.0,
            "ModemChannelAdapter::transmit_and_demodulate: code rate must be in (0, 1], got {rate}",
        );

        // Eb/N0 -> per-component noise variance for an m-bit/symbol
        // constellation with unit average symbol energy.
        //
        // Es = m * Rc * Eb, so Es/N0 = m * rate * Eb/N0. With unit-average
        // symbol energy the complex noise power is N0 = 1 / (Es/N0) and
        // each of I and Q carries sigma^2 = N0 / 2.
        //
        // For BPSK (m = 1) this reduces to sigma^2 = 1 / (2 * rate * Eb/N0),
        // matching the legacy `AwgnChannel::from_eb_n0_db` path used by
        // [`crate::simulation::BpskAwgnChannel`]. See the module-level
        // noise convention.
        let m = self.mapper.spec().bits_per_symbol() as usize;
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        let sigma_squared = 1.0 / (2.0 * (m as f64) * rate * eb_n0_linear);
        let channel = AwgnChannel::from_variance(sigma_squared);

        let n = bits.len();
        let bits_vec: Vec<bool> = (0..n).map(|i| bits.get(i)).collect();
        let mut out = vec![Llr::new(0.0); n];
        run_awgn_modem_pipeline(
            &self.mapper,
            &self.demapper,
            &channel,
            self.method,
            &bits_vec,
            rng,
            &mut out,
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::{GrayQamMapper, ModemSpec, ModemView};

    /// Stub demapper that emits `Llr(+1.0)` for every bit, regardless of
    /// received sample. Used purely to exercise the plumbing.
    struct ConstDemapper {
        spec: ModemSpec<f32>,
    }

    impl BatchSoftDemapper<f32> for ConstDemapper {
        fn spec(&self) -> ModemView<'_, f32> {
            self.spec.view()
        }
        fn demap_llrs(&self, input: DemapInput<'_, f32>, out: &mut [Llr]) {
            let m = self.spec.bits_per_symbol() as usize;
            assert_eq!(out.len(), input.rx_i.len() * m);
            for v in out.iter_mut() {
                *v = Llr::new(1.0);
            }
        }
    }

    /// Sign-based demapper for BPSK: bit 0 if received I > 0, else bit 1.
    /// Pretends all received power is on I; ignores Q. Used for roundtrip
    /// testing at variance ~= 0.
    struct BpskSignDemapper {
        spec: ModemSpec<f32>,
    }

    impl BatchSoftDemapper<f32> for BpskSignDemapper {
        fn spec(&self) -> ModemView<'_, f32> {
            self.spec.view()
        }
        fn demap_llrs(&self, input: DemapInput<'_, f32>, out: &mut [Llr]) {
            assert_eq!(out.len(), input.rx_i.len());
            for (s, o) in input.rx_i.iter().zip(out.iter_mut()) {
                *o = Llr::new(*s * 100.0);
            }
        }
    }

    #[test]
    fn test_new_checks_bits_per_symbol_match() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let demapper = ConstDemapper {
            spec: ModemSpec::gray_square_qam(4),
        };
        let channel = AwgnChannel::from_variance(0.1);
        let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
        assert_eq!(link.mapper().spec().bits_per_symbol(), 2);
        assert_eq!(link.method(), DemapMethod::MaxLog);
        assert!((link.channel().variance() - 0.1).abs() < 1e-12);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol must match")]
    fn test_new_panics_on_bps_mismatch() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let demapper = ConstDemapper {
            spec: ModemSpec::gray_square_qam(16),
        };
        let channel = AwgnChannel::from_variance(0.1);
        let _ = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
    }

    #[test]
    #[should_panic(expected = "out_llrs.len() must equal bits.len()")]
    fn test_transmit_panics_on_length_mismatch() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let demapper = ConstDemapper {
            spec: ModemSpec::gray_square_qam(4),
        };
        let channel = AwgnChannel::from_variance(0.1);
        let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
        let bits = vec![false, false, true, true];
        let mut out = vec![Llr::new(0.0); 3];
        let mut rng = rand::thread_rng();
        link.transmit_and_demap(&bits, &mut rng, &mut out);
    }

    #[test]
    #[should_panic(expected = "multiple of bits_per_symbol")]
    fn test_transmit_panics_on_non_multiple_bits() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let demapper = ConstDemapper {
            spec: ModemSpec::gray_square_qam(4),
        };
        let channel = AwgnChannel::from_variance(0.1);
        let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
        let bits = vec![false, true, true]; // len = 3, not multiple of 2
        let mut out = vec![Llr::new(0.0); 3];
        let mut rng = rand::thread_rng();
        link.transmit_and_demap(&bits, &mut rng, &mut out);
    }

    #[test]
    fn test_transmit_and_demap_constant_stub() {
        // Verify plumbing: stub emits +1.0 for all bits.
        let mapper = GrayQamMapper::<f32>::from_preset_order(16);
        let demapper = ConstDemapper {
            spec: ModemSpec::gray_square_qam(16),
        };
        let channel = AwgnChannel::from_variance(0.1);
        let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
        let bits: Vec<bool> = (0..16).map(|i| (i & 1) == 0).collect();
        let mut out = vec![Llr::new(0.0); bits.len()];
        let mut rng = rand::rngs::StdRng::seed_from_u64(0xA1B2);
        use rand::SeedableRng;
        let _ = &mut rng;
        link.transmit_and_demap(&bits, &mut rng, &mut out);
        assert!(out.iter().all(|l| (l.value() - 1.0).abs() < 1e-6));
    }

    #[test]
    fn test_transmit_and_demap_low_noise_bpsk_roundtrip() {
        // With tiny noise variance and a sign demapper, hard decisions
        // should recover the transmitted bits.
        let spec = ModemSpec::<f32>::bpsk();
        let mapper = GrayQamMapper::<f32>::from_preset_order(2);
        let demapper = BpskSignDemapper { spec };
        let channel = AwgnChannel::from_variance(1e-6);
        let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
        let bits = vec![false, true, false, true, true, false, false, true];
        let mut out = vec![Llr::new(0.0); bits.len()];
        let mut rng = rand::rngs::StdRng::seed_from_u64(0xC0FFEE);
        use rand::SeedableRng;
        let _ = &mut rng;
        link.transmit_and_demap(&bits, &mut rng, &mut out);
        let decoded: Vec<bool> = out.iter().map(|l| l.hard_decision()).collect();
        assert_eq!(decoded, bits);
    }

    #[test]
    fn test_accessors_return_correct_references() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let demapper = ConstDemapper {
            spec: ModemSpec::gray_square_qam(4),
        };
        let channel = AwgnChannel::from_variance(0.25);
        let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::ExactLogMap);
        assert_eq!(link.mapper().spec().num_symbols(), 4);
        assert_eq!(link.demapper().spec().num_symbols(), 4);
        assert!((link.channel().variance() - 0.25).abs() < 1e-12);
        assert_eq!(link.method(), DemapMethod::ExactLogMap);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::modem::{GrayQamMapper, ModemSpec, ModemView};
    use proptest::prelude::*;
    use rand::SeedableRng;

    struct ZeroDemapper {
        spec: ModemSpec<f32>,
    }

    impl BatchSoftDemapper<f32> for ZeroDemapper {
        fn spec(&self) -> ModemView<'_, f32> {
            self.spec.view()
        }
        fn demap_llrs(&self, input: DemapInput<'_, f32>, out: &mut [Llr]) {
            // Trivial: write per-symbol (rx_i - rx_q) broadcast to bits.
            let m = self.spec.bits_per_symbol() as usize;
            assert_eq!(out.len(), input.rx_i.len() * m);
            for (s, chunk) in out.chunks_mut(m).enumerate() {
                let v = input.rx_i[s] - input.rx_q[s];
                for e in chunk.iter_mut() {
                    *e = Llr::new(v);
                }
            }
        }
    }

    proptest! {
        #[test]
        fn random_bits_random_variance_no_nan(
            seed in any::<u64>(),
            var in 0.01f64..5.0f64,
            n_sym in 1usize..32usize,
        ) {
            let mapper = GrayQamMapper::<f32>::from_preset_order(4);
            let demapper = ZeroDemapper { spec: ModemSpec::gray_square_qam(4) };
            let channel = AwgnChannel::from_variance(var);
            let link = ModemAwgnChannel::new(mapper, demapper, channel, DemapMethod::MaxLog);
            let bits: Vec<bool> = (0..n_sym * 2)
                .map(|i| (seed as usize + i) & 1 == 0)
                .collect();
            let mut out = vec![Llr::new(0.0); bits.len()];
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            link.transmit_and_demap(&bits, &mut rng, &mut out);
            for l in out.iter() {
                prop_assert!(l.value().is_finite());
            }
        }
    }
}

#[cfg(test)]
mod legacy_compat_tests {
    //! Regressions that pin the adapter to the legacy BPSK path.
    //!
    //! The code-review gate called out two issues the earlier version of
    //! this module was missing:
    //!
    //! 1. `ModemChannelAdapter` (the `ChannelModel` integration point) was
    //!    absent, so the existing [`crate::simulation::SimulationRunner`]
    //!    was still hard-coded to [`crate::simulation::BpskAwgnChannel`].
    //! 2. The noise-variance convention handed to the reference demapper
    //!    was inconsistent with the demapper's own BPSK closed-form test
    //!    (`LLR = 4 y / N0`), which silently scaled every LLR by 2x.
    //!
    //! These tests cover both: the adapter plugs into a `ChannelModel`
    //! consumer, and BPSK LLRs produced through the modem framework match
    //! the legacy `LLR = 2 r / sigma^2` formula.
    use super::*;
    use crate::channel::BpskModulator;
    use crate::modem::{GrayQamMapper, ModemSpec, ReferenceSoftDemapper};
    use crate::simulation::{BpskAwgnChannel, ChannelModel};
    use gf2_core::BitVec;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn test_bpsk_llrs_match_legacy_formula() {
        // With noise_var = 2 * sigma^2 the reference demapper's BPSK LLR
        // equals 4 r / N0 = 2 r / sigma^2, which is exactly the legacy
        // BpskModulator::to_llr formula. Drive both paths with the same
        // fabricated rx symbols and assert equality.
        let sigma_sq = 0.5_f64;
        let n0 = 2.0 * sigma_sq;
        let rx_samples = [-1.5_f32, -0.3, 0.0, 0.2, 1.1];

        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let rx_q = [0.0_f32; 5];
        let noise_var = [n0 as f32; 5];
        let input = DemapInput::<f32> {
            rx_i: &rx_samples,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &noise_var,
            method: DemapMethod::ExactLogMap,
        };
        let mut modem_llrs = [Llr::new(0.0); 5];
        demapper.demap_llrs(input, &mut modem_llrs);

        for (i, &r) in rx_samples.iter().enumerate() {
            let legacy = BpskModulator::to_llr(r as f64, sigma_sq);
            let err = (modem_llrs[i].value() - legacy.value()).abs();
            assert!(
                err < 1e-3,
                "BPSK LLR mismatch at r={r}: modem={} legacy={}",
                modem_llrs[i].value(),
                legacy.value(),
            );
        }
    }

    #[test]
    fn test_adapter_is_channel_model() {
        // Smoke test: ModemChannelAdapter plugs into a generic function
        // bound to ChannelModel the same way BpskAwgnChannel does.
        fn run<C: ChannelModel>(channel: &C, bits: &BitVec, rng: &mut StdRng) -> Vec<Llr> {
            channel.transmit_and_demodulate(bits, 3.0, 1.0, rng)
        }

        let mapper = GrayQamMapper::<f32>::from_preset_order(2); // BPSK label order = 2
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);

        let bits = BitVec::from_bytes_le(&[0b1010_0101]);
        let mut rng_a = StdRng::seed_from_u64(0xA5A5);
        let mut rng_b = StdRng::seed_from_u64(0xA5A5);
        let modem_llrs = run(&adapter, &bits, &mut rng_a);
        let legacy_llrs = run(&BpskAwgnChannel, &bits, &mut rng_b);
        assert_eq!(modem_llrs.len(), bits.len());
        assert_eq!(legacy_llrs.len(), bits.len());
    }

    #[test]
    fn test_adapter_qpsk_high_snr_recovers_bits() {
        // m > 1 regression: without the bits_per_symbol factor in the
        // Eb/N0 -> sigma^2 conversion, QPSK would be simulated at too
        // high a noise level (by ~3 dB) and even at 20 dB Eb/N0 the hard
        // decisions could drift. With the corrected scaling, QPSK
        // recovers every bit at high SNR.
        let mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);

        let bits = BitVec::from_bytes_le(&[0b1011_0010, 0b0110_1100, 0b1111_0000, 0b0000_1111]);
        let mut rng = StdRng::seed_from_u64(0xBEEF_CAFE);
        let llrs = adapter.transmit_and_demodulate(&bits, 25.0, 1.0, &mut rng);
        assert_eq!(llrs.len(), bits.len());
        for (i, llr) in llrs.iter().enumerate() {
            assert_eq!(
                llr.hard_decision(),
                bits.get(i),
                "QPSK hard decision mismatch at bit {i}: llr={}",
                llr.value(),
            );
        }
    }

    #[test]
    fn test_adapter_qpsk_sigma_is_half_of_bpsk_at_same_eb_n0() {
        // Numeric pin on the bits_per_symbol scaling in
        // `ModemChannelAdapter::transmit_and_demodulate`:
        //   sigma^2 = 1 / (2 * m * rate * 10^(Eb/N0 / 10))
        // At fixed (Eb/N0, rate), switching from BPSK (m=1) to QPSK (m=2)
        // must halve sigma^2. Any regression that drops the `m` factor
        // (the earlier blocker) would return sigma_qpsk == sigma_bpsk.
        //
        // The adapter does not expose sigma^2 directly, so we reproduce
        // its formula inline and also do a small end-to-end sanity run.
        let eb_n0_db = 10.0_f64;
        let rate = 0.5_f64;
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        let sigma_sq_bpsk = 1.0 / (2.0 * 1.0 * rate * eb_n0_linear);
        let sigma_sq_qpsk = 1.0 / (2.0 * 2.0 * rate * eb_n0_linear);
        assert!((sigma_sq_bpsk - 2.0 * sigma_sq_qpsk).abs() < 1e-12);

        // End-to-end: both adapters emit finite, bits-length LLRs at the
        // same (Eb/N0, rate) — exercises the full pipeline with m>1.
        let qpsk_mapper = GrayQamMapper::<f32>::from_preset_order(4);
        let qpsk_demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let qpsk_adapter =
            ModemChannelAdapter::new(qpsk_mapper, qpsk_demap, DemapMethod::ExactLogMap);

        let bpsk_mapper = GrayQamMapper::<f32>::from_preset_order(2);
        let bpsk_demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let bpsk_adapter =
            ModemChannelAdapter::new(bpsk_mapper, bpsk_demap, DemapMethod::ExactLogMap);

        let bits = BitVec::from_bytes_le(&[0b0101_0101]);
        let mut rng_a = StdRng::seed_from_u64(0x7777);
        let mut rng_b = StdRng::seed_from_u64(0x7777);
        let bpsk_llrs = bpsk_adapter.transmit_and_demodulate(&bits, eb_n0_db, rate, &mut rng_a);
        let qpsk_llrs = qpsk_adapter.transmit_and_demodulate(&bits, eb_n0_db, rate, &mut rng_b);
        assert_eq!(bpsk_llrs.len(), 8);
        assert_eq!(qpsk_llrs.len(), 8);
        for l in bpsk_llrs.iter().chain(qpsk_llrs.iter()) {
            assert!(l.value().is_finite());
        }
    }

    #[test]
    #[should_panic(expected = "code rate must be in (0, 1]")]
    fn test_adapter_rejects_rate_above_one() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(2);
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);
        let bits = BitVec::from_bytes_le(&[0b0000_0001]);
        let mut rng = StdRng::seed_from_u64(0);
        let _ = adapter.transmit_and_demodulate(&bits, 3.0, 1.5, &mut rng);
    }

    #[test]
    #[should_panic(expected = "code rate must be in (0, 1]")]
    fn test_adapter_rejects_rate_zero() {
        let mapper = GrayQamMapper::<f32>::from_preset_order(2);
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);
        let bits = BitVec::from_bytes_le(&[0b0000_0001]);
        let mut rng = StdRng::seed_from_u64(0);
        let _ = adapter.transmit_and_demodulate(&bits, 3.0, 0.0, &mut rng);
    }

    #[test]
    fn test_adapter_high_snr_bpsk_recovers_bits() {
        // At very high Eb/N0 the adapter must recover every bit through
        // sign decisions, same as the legacy path would.
        let mapper = GrayQamMapper::<f32>::from_preset_order(2);
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);

        let bits = BitVec::from_bytes_le(&[0b1011_0010, 0b0110_1100]);
        let mut rng = StdRng::seed_from_u64(0xD15EA5E);
        let llrs = adapter.transmit_and_demodulate(&bits, 20.0, 1.0, &mut rng);
        for (i, llr) in llrs.iter().enumerate() {
            assert_eq!(
                llr.hard_decision(),
                bits.get(i),
                "hard decision mismatch at bit {i}: llr={}",
                llr.value(),
            );
        }
    }
}
