//! AWGN link composition over the shared modem framework.
//!
//! [`ModemAwgnChannel`] ties a [`BatchMapper`] and a [`BatchSoftDemapper`]
//! together with the existing [`crate::channel::AwgnChannel`] to provide a
//! single `bits -> LLRs` pipeline for any validated [`super::ModemSpec`],
//! preset or custom. It is the integration entry point for story
//! `92186a40`: downstream task `bf865220` will route
//! [`crate::simulation::SimulationRunner`] through this adapter, and tasks
//! `0cafa5f5` / `b3bb774a` will rebuild BPSK/QPSK compatibility wrappers
//! on top of it.
//!
//! This module is deliberately additive. The legacy
//! [`crate::channel::BpskAwgn`](crate::channel) / `BpskModulator` helpers
//! are left untouched so existing callers keep working while the modem
//! framework migration rolls forward.
//!
//! # Noise convention
//!
//! The existing [`AwgnChannel`] samples real-valued Gaussian noise with
//! variance `channel.variance()`. For a complex AWGN channel with
//! independent Gaussian noise on each of I and Q, `channel.variance()` is
//! interpreted as the **per-component** variance (`N0 / 2`), which is the
//! value the log-MAP demapper expects in
//! [`super::DemapInput::noise_var`]. This matches the legacy BPSK LLR
//! formula `LLR = 2 r / sigma^2` used by [`crate::channel::BpskModulator`]
//! (see `crates/gf2-coding/src/channel.rs:100` and `channel.rs:156`).

use super::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar};
use crate::channel::AwgnChannel;
use crate::llr::Llr;
use rand::Rng;

/// AWGN link over any modem spec, using the shared [`BatchMapper`] and
/// [`BatchSoftDemapper`] surfaces.
///
/// Combines a caller-supplied mapper, demapper, and [`AwgnChannel`] into a
/// single `transmit_and_demap` call. Complex Gaussian noise is applied to
/// each received symbol as two independent real Gaussian draws (one on I,
/// one on Q), each with variance equal to [`AwgnChannel::variance`]. The
/// per-component variance is then passed through to the demapper as
/// [`DemapInput::noise_var`].
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
///     BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, GrayQamMapper,
///     ModemAwgnChannel, ModemScalar, ModemView,
/// };
///
/// struct StubDemap<'a>(gf2_coding::modem::ModemSpec<f32>, std::marker::PhantomData<&'a ()>);
/// impl<'a> BatchSoftDemapper<f32> for StubDemap<'a> {
///     fn spec(&self) -> ModemView<'_, f32> { self.0.view() }
///     fn demap_llrs(&self, input: DemapInput<'_, f32>, out: &mut [Llr]) {
///         for v in out.iter_mut() { *v = Llr::new(0.0); }
///         let _ = input;
///     }
/// }
///
/// let mapper = GrayQamMapper::<f32>::from_preset_order(4);
/// let demap = StubDemap(gf2_coding::modem::ModemSpec::gray_square_qam(4), std::marker::PhantomData);
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
    /// * `channel` - Noise model. [`AwgnChannel::variance`] is used as the
    ///   per-component variance for both I and Q (see module docs).
    /// * `method` - Log-MAP demapping method (`ExactLogMap` or `MaxLog`).
    ///
    /// # Panics
    ///
    /// Panics if `mapper` and `demapper` disagree on `bits_per_symbol`.
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
    /// [`AwgnChannel::variance`]. Noise samples on I and Q are drawn
    /// independently from the channel's Gaussian distribution.
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
    /// # Complexity
    ///
    /// Dominated by the mapper (`O(num_symbols)`) and the demapper
    /// (`O(num_symbols * m)` for Gray-QAM, `O(num_symbols * M * m)` for
    /// the exact log-MAP reference path). Adds three scratch allocations
    /// of sizes `num_symbols`, `num_symbols`, and `num_symbols`.
    pub fn transmit_and_demap<R: Rng>(&self, bits: &[bool], rng: &mut R, out_llrs: &mut [Llr]) {
        let m = self.mapper.spec().bits_per_symbol() as usize;
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

        // Map bits to symbols.
        let mut tx_i = vec![S::zero(); num_symbols];
        let mut tx_q = vec![S::zero(); num_symbols];
        self.mapper.map_bits(bits, &mut tx_i, &mut tx_q);

        // Add independent Gaussian noise on I and Q (per-component
        // variance = channel.variance()).
        for sample in tx_i.iter_mut() {
            let noisy = self.channel.transmit(sample.to_f64(), rng);
            *sample = S::from_f64(noisy);
        }
        for sample in tx_q.iter_mut() {
            let noisy = self.channel.transmit(sample.to_f64(), rng);
            *sample = S::from_f64(noisy);
        }

        // Per-symbol noise variance (constant across the batch for AWGN).
        let per_comp_var = S::from_f64(self.channel.variance());
        let noise_var = vec![per_comp_var; num_symbols];

        let input = DemapInput::<S> {
            rx_i: &tx_i,
            rx_q: &tx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &noise_var,
            method: self.method,
        };
        self.demapper.demap_llrs(input, out_llrs);
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
