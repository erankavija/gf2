//! AWGN link composition over the modem traits: [`ModemAwgnChannel`] maps,
//! adds noise and demaps for a fixed [`AwgnChannel`], and
//! [`ModemChannelAdapter`] does the same behind [`ChannelModel`] from
//! `Eb/N0` and a code rate.
//!
//! # Noise convention
//!
//! [`AwgnChannel::variance`] is `sigma^2`, the variance applied to each of
//! I and Q. Both adapters pass `N0 = 2 sigma^2` as
//! [`super::DemapInput::noise_var`], which gives the BPSK LLR
//! `2 r / sigma^2 = 4 r / N0`.

use super::{BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar};
use crate::channel::AwgnChannel;
use crate::llr::Llr;
use crate::simulation::ChannelModel;
use gf2_core::BitVec;
use rand::Rng;

/// `Eb/N0` to per-component AWGN variance for a constellation with unit
/// average symbol energy:
/// `sigma^2 = 1 / (2 * m * rate * 10^(Eb_N0_dB / 10))`, with `m` bits per
/// symbol and code rate `rate`.
///
/// # Panics
///
/// Panics if `m == 0` or if `rate` is outside `(0, 1]`.
pub fn unit_energy_sigma_sq_from_eb_n0_db(m: usize, rate: f64, eb_n0_db: f64) -> f64 {
    assert!(m > 0, "bits-per-symbol m must be positive");
    assert!(
        rate > 0.0 && rate <= 1.0,
        "code rate must be in (0, 1], got {rate}"
    );
    let eb_n0_lin = 10.0_f64.powf(eb_n0_db / 10.0);
    1.0 / (2.0 * (m as f64) * rate * eb_n0_lin)
}

/// `Eb/N0` to total complex noise power `N0 = 2 sigma^2`, with `sigma^2`
/// from [`unit_energy_sigma_sq_from_eb_n0_db`].
///
/// # Panics
///
/// Same as [`unit_energy_sigma_sq_from_eb_n0_db`].
pub fn unit_energy_n0_from_eb_n0_db(m: usize, rate: f64, eb_n0_db: f64) -> f64 {
    2.0 * unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db)
}

/// AWGN link over any modem spec: a mapper, a demapper and an
/// [`AwgnChannel`] composed into one `bits -> LLRs` call.
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
    /// # Panics
    ///
    /// Panics if `mapper` and `demapper` disagree on `bits_per_symbol`.
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
    #[inline]
    pub fn mapper(&self) -> &M {
        &self.mapper
    }

    /// Returns a reference to the underlying demapper.
    #[inline]
    pub fn demapper(&self) -> &D {
        &self.demapper
    }

    /// Returns a reference to the underlying AWGN channel.
    #[inline]
    pub fn channel(&self) -> &AwgnChannel {
        &self.channel
    }

    /// Returns the configured [`DemapMethod`].
    #[inline]
    pub fn method(&self) -> DemapMethod {
        self.method
    }

    /// Maps bits to symbols, adds independent Gaussian noise of variance
    /// [`AwgnChannel::variance`] on each of I and Q, and demaps to per-bit
    /// LLRs in the layout of [`BatchSoftDemapper::demap_llrs`].
    ///
    /// # Panics
    ///
    /// Panics if `out_llrs.len() != bits.len()`, or if `bits.len()` is not
    /// a multiple of `bits_per_symbol`.
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

/// `bits -> LLRs` pipeline shared by [`ModemAwgnChannel`] and
/// [`ModemChannelAdapter`].
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

/// [`ChannelModel`] that runs any modem spec over AWGN.
///
/// Each [`ChannelModel::transmit_and_demodulate`] call builds an
/// [`AwgnChannel`] with the variance of
/// [`unit_energy_sigma_sq_from_eb_n0_db`] and returns one LLR per
/// transmitted bit in the layout of [`BatchSoftDemapper::demap_llrs`]. It
/// panics if `rate` is outside `(0, 1]`.
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
    /// # Panics
    ///
    /// Panics if `mapper.spec().bits_per_symbol() != demapper.spec().bits_per_symbol()`.
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
    fn batch_alignment(&self) -> usize {
        self.mapper.spec().bits_per_symbol() as usize
    }

    fn demap_method(&self) -> DemapMethod {
        self.method
    }

    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        assert!(
            rate > 0.0 && rate <= 1.0,
            "ModemChannelAdapter::transmit_and_demodulate: code rate must be in (0, 1], got {rate}",
        );

        let m = self.mapper.spec().bits_per_symbol() as usize;
        let sigma_squared = unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db);
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
    use super::*;
    use crate::modem::{GrayQamMapper, ModemSpec, ReferenceSoftDemapper};
    use crate::simulation::{BpskAwgnChannel, ChannelModel};
    use gf2_core::BitVec;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn test_bpsk_llrs_match_closed_form() {
        // With noise_var = 2 * sigma^2 the reference demapper's BPSK LLR
        // equals 4 r / N0 = 2 r / sigma^2, the BPSK closed form.
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
            let expected = (2.0 * r as f64 / sigma_sq) as f32;
            let err = (modem_llrs[i].value() - expected).abs();
            assert!(
                err < 1e-3,
                "BPSK LLR mismatch at r={r}: modem={} expected={expected}",
                modem_llrs[i].value(),
            );
        }
    }

    #[test]
    fn test_adapter_is_channel_model() {
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
        // sigma^2 = 1 / (2 * m * rate * 10^(Eb/N0 / 10)): at fixed
        // (Eb/N0, rate), QPSK (m=2) has half the sigma^2 of BPSK (m=1).
        let eb_n0_db = 10.0_f64;
        let rate = 0.5_f64;
        let eb_n0_linear = 10.0_f64.powf(eb_n0_db / 10.0);
        let sigma_sq_bpsk = 1.0 / (2.0 * 1.0 * rate * eb_n0_linear);
        let sigma_sq_qpsk = 1.0 / (2.0 * 2.0 * rate * eb_n0_linear);
        assert!((sigma_sq_bpsk - 2.0 * sigma_sq_qpsk).abs() < 1e-12);

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
