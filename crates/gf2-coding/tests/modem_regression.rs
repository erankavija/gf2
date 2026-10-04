//! Fixed-seed regression tests for the reference modem path, the Gray-QAM
//! fast path and the `BpskAwgnChannel` and `QpskRicianChannelModel` channel
//! models: round trip, fast-versus-reference LLR parity, banded BER, and the
//! `N0 = 2 sigma^2` noise convention.

use gf2_coding::channel::AwgnChannel;
use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
use gf2_coding::llr::Llr;
use gf2_coding::modem::awgn_link::{
    unit_energy_n0_from_eb_n0_db, unit_energy_sigma_sq_from_eb_n0_db,
};
use gf2_coding::modem::test_oracle::{bit_stream, Lcg};
use gf2_coding::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, GrayQamMapper,
    ModemSpec, ReferenceMapper, ReferenceSoftDemapper,
};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel};
use gf2_core::BitVec;
use rand::{rngs::StdRng, SeedableRng};

const PRESET_ORDERS: [usize; 5] = [2, 4, 16, 64, 256];

/// Box-Muller pair of unit-variance Gaussian samples from the shared `Lcg`,
/// so the noise is reproducible across platforms.
fn box_muller_pair_f64(rng: &mut Lcg) -> (f64, f64) {
    let u1 = (rng.next_u32() as f64 / u32::MAX as f64).max(1e-12);
    let u2 = rng.next_u32() as f64 / u32::MAX as f64;
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = std::f64::consts::TAU * u2;
    (r * theta.cos(), r * theta.sin())
}

fn box_muller_pair_f32(rng: &mut Lcg) -> (f32, f32) {
    let (a, b) = box_muller_pair_f64(rng);
    (a as f32, b as f32)
}

fn bits_to_bitvec(bits: &[bool]) -> BitVec {
    let mut bv = BitVec::zeros(bits.len());
    for (i, &b) in bits.iter().enumerate() {
        if b {
            bv.set(i, true);
        }
    }
    bv
}

fn check_round_trip_clean<M, D>(mapper: &M, demapper: &D, bits: &[bool], m: u8)
where
    M: BatchMapper<f64>,
    D: BatchSoftDemapper<f64>,
{
    let num_symbols = bits.len() / m as usize;
    let mut tx_i = vec![0.0_f64; num_symbols];
    let mut tx_q = vec![0.0_f64; num_symbols];
    mapper.map_bits(bits, &mut tx_i, &mut tx_q);

    // Strictly positive so the log-MAP path is well-defined.
    let nv = vec![1e-6_f64; num_symbols];
    let input = DemapInput::<f64> {
        rx_i: &tx_i,
        rx_q: &tx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &nv,
        method: DemapMethod::ExactLogMap,
    };
    let mut llrs = vec![Llr::new(0.0); num_symbols * m as usize];
    demapper.demap_llrs(input, &mut llrs);

    for (i, &expected) in bits.iter().enumerate() {
        let got = llrs[i].hard_decision();
        assert_eq!(
            got, expected,
            "round-trip bit mismatch at flat bit index {i}: got {got} want {expected}"
        );
    }
}

#[test]
fn test_round_trip_reference_path_all_presets() {
    let seeds: [(usize, u64); 5] = [
        (2, 0x4253_504B), // 'BSPK'
        (4, 0x5150_534B),
        (16, 0x3136_5141),
        (64, 0x3634_5141),
        (256, 0x3235_3651),
    ];
    for (order, seed) in seeds {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol();
        let bits = bit_stream(seed, 128 * m as usize);
        let mapper = ReferenceMapper::new(spec.clone());
        let demapper = ReferenceSoftDemapper::new(spec);
        check_round_trip_clean(&mapper, &demapper, &bits, m);
    }
}

#[test]
fn test_round_trip_fast_path_all_presets() {
    let seeds: [(usize, u64); 5] = [
        (2, 0x4653_5450),
        (4, 0x4653_5051),
        (16, 0x4653_4631),
        (64, 0x4653_4636),
        (256, 0x4653_3235),
    ];
    for (order, seed) in seeds {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol();
        let bits = bit_stream(seed, 128 * m as usize);
        let mapper: GrayQamMapper<f64> = GrayQamMapper::<f64>::from_preset_order_with_scalar(order);
        let demapper = FastGrayQamDemapper::new(spec);
        check_round_trip_clean(&mapper, &demapper, &bits, m);
    }
}

#[allow(clippy::too_many_arguments)]
fn parity_f64(
    spec: ModemSpec<f64>,
    rng_seed: u64,
    batch: usize,
    eb_n0_db: f64,
    rate: f64,
    method: DemapMethod,
    tol: f32,
    tag: &str,
) -> f32 {
    let m = spec.bits_per_symbol() as usize;
    let fast = FastGrayQamDemapper::new(spec.clone());
    let reference = ReferenceSoftDemapper::new(spec.clone());

    let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db);
    let n0 = 2.0 * sigma_sq;
    let std = sigma_sq.sqrt();

    let mapper = ReferenceMapper::new(spec);
    let bits = bit_stream(rng_seed.wrapping_add(1), batch * m);
    let mut tx_i = vec![0.0_f64; batch];
    let mut tx_q = vec![0.0_f64; batch];
    mapper.map_bits(&bits, &mut tx_i, &mut tx_q);

    let mut rng = Lcg::new(rng_seed);
    let mut rx_i = vec![0.0_f64; batch];
    let mut rx_q = vec![0.0_f64; batch];
    for k in 0..batch {
        let (n_i, n_q) = box_muller_pair_f64(&mut rng);
        rx_i[k] = tx_i[k] + std * n_i;
        rx_q[k] = tx_q[k] + std * n_q;
    }

    let nv = vec![n0; batch];
    let input = DemapInput::<f64> {
        rx_i: &rx_i,
        rx_q: &rx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &nv,
        method,
    };
    let mut out_fast = vec![Llr::new(0.0); batch * m];
    let mut out_ref = vec![Llr::new(0.0); batch * m];
    fast.demap_llrs(input, &mut out_fast);
    reference.demap_llrs(input, &mut out_ref);

    let mut max_abs = 0.0_f32;
    for (f, r) in out_fast.iter().zip(out_ref.iter()) {
        let d = (f.value() - r.value()).abs();
        if d > max_abs {
            max_abs = d;
        }
    }
    assert!(
        max_abs <= tol,
        "fast vs reference parity [{tag}]: max abs diff {max_abs} > tol {tol}"
    );
    max_abs
}

#[test]
fn test_fast_ref_parity_low_snr_all_presets_f64() {
    for &order in &PRESET_ORDERS {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        for method in [DemapMethod::ExactLogMap, DemapMethod::MaxLog] {
            let _ = parity_f64(
                spec.clone(),
                0xF00D_0000_u64.wrapping_add(order as u64),
                64,
                0.0,
                1.0,
                method,
                1e-4,
                &format!("f64 low-SNR order={order} method={method:?}"),
            );
        }
    }
}

#[test]
fn test_fast_ref_parity_high_snr_all_presets_f64() {
    for &order in &PRESET_ORDERS {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        for method in [DemapMethod::ExactLogMap, DemapMethod::MaxLog] {
            let _ = parity_f64(
                spec.clone(),
                0xBEEF_0000_u64.wrapping_add(order as u64),
                64,
                10.0,
                1.0,
                method,
                // f32 LLR storage quantizes the large 10 dB magnitudes.
                1e-2,
                &format!("f64 high-SNR order={order} method={method:?}"),
            );
        }
    }
}

#[test]
fn test_fast_ref_parity_f32_awgn_all_presets() {
    // f32 rx samples carry their own quantization, hence the 1e-2 tolerance.
    for &order in &PRESET_ORDERS {
        let spec: ModemSpec<f32> = ModemSpec::<f32>::gray_square_qam(order);
        let m = spec.bits_per_symbol() as usize;
        let fast = FastGrayQamDemapper::new(spec.clone());
        let reference = ReferenceSoftDemapper::new(spec.clone());

        let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(m, 1.0, 3.0);
        let n0 = (2.0 * sigma_sq) as f32;
        let std = sigma_sq.sqrt() as f32;

        let mapper = ReferenceMapper::new(spec);
        let bits = bit_stream(0xC0FE_0000u64.wrapping_add(order as u64), 64 * m);
        let mut tx_i = vec![0.0_f32; 64];
        let mut tx_q = vec![0.0_f32; 64];
        mapper.map_bits(&bits, &mut tx_i, &mut tx_q);

        let mut rng = Lcg::new(0xF001_0000_u64.wrapping_add(order as u64));
        let mut rx_i = vec![0.0_f32; 64];
        let mut rx_q = vec![0.0_f32; 64];
        for k in 0..64 {
            let (n_i, n_q) = box_muller_pair_f32(&mut rng);
            rx_i[k] = tx_i[k] + std * n_i;
            rx_q[k] = tx_q[k] + std * n_q;
        }

        let nv = vec![n0; 64];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out_fast = vec![Llr::new(0.0); 64 * m];
        let mut out_ref = vec![Llr::new(0.0); 64 * m];
        fast.demap_llrs(input, &mut out_fast);
        reference.demap_llrs(input, &mut out_ref);

        let mut max_abs = 0.0_f32;
        for (f, r) in out_fast.iter().zip(out_ref.iter()) {
            max_abs = max_abs.max((f.value() - r.value()).abs());
        }
        assert!(
            max_abs <= 1e-2,
            "f32 fast vs reference parity order={order}: max abs diff {max_abs}"
        );
    }
}

#[test]
fn test_bpsk_awgn_channel_ber_locked() {
    let channel = BpskAwgnChannel;
    let n_bits = 2048;
    let mut rng = StdRng::seed_from_u64(0xB9_B95C_5EED_0003u64);
    let tx_bits = bit_stream(0xDEAD_5EED_BEEF_5EEDu64, n_bits);
    let tx_bv = bits_to_bitvec(&tx_bits);

    let llrs = channel.transmit_and_demodulate(&tx_bv, 3.0, 1.0, &mut rng);
    assert_eq!(llrs.len(), n_bits, "LLR stream length must match bits");

    let mut errors = 0usize;
    for (i, llr) in llrs.iter().enumerate() {
        if llr.hard_decision() != tx_bits[i] {
            errors += 1;
        }
    }
    // The theoretical uncoded BPSK BER at 3 dB Eb/N0 is about 0.023.
    let ber = errors as f64 / n_bits as f64;
    assert!(
        (0.005..=0.06).contains(&ber),
        "BpskAwgnChannel BER at 3 dB = {ber} ({errors}/{n_bits}) outside expected band"
    );
    assert!(ber < 0.1, "BpskAwgnChannel BER at 3 dB too high: {ber}");
}

#[test]
fn test_qpsk_rician_channel_locked() {
    // A coefficient-swapped or open-loop adapter drives the 10 dB BER toward
    // 0.5; a dropped noise step drives the -2 dB error count to 0.
    let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
    let n_bits = 1024; // exact `frame_bits()` for fig8.
    let tx_bits = bit_stream(0xF1_0FAD_EF16_0008u64, n_bits);
    let tx_bv = bits_to_bitvec(&tx_bits);

    {
        let mut rng = StdRng::seed_from_u64(0xFADE_5EED);
        let llrs = channel.transmit_and_demodulate(&tx_bv, 10.0, 1.0, &mut rng);
        assert_eq!(llrs.len(), n_bits);
        let errors = llrs
            .iter()
            .zip(tx_bits.iter())
            .filter(|(l, &b)| l.hard_decision() != b)
            .count();
        let ber = errors as f64 / n_bits as f64;
        assert!(
            ber < 0.25,
            "QpskRicianChannelModel BER at 10 dB = {ber} too high ({errors}/{n_bits})"
        );
    }

    {
        let mut rng = StdRng::seed_from_u64(0xD15E_A5ED);
        let mut total_errors = 0usize;
        let mut total_bits = 0usize;
        for frame_seed in 0..4u64 {
            let tx = bit_stream(0xD15E_A5ED_u64.wrapping_add(frame_seed), n_bits);
            let tx_bv = bits_to_bitvec(&tx);
            let llrs = channel.transmit_and_demodulate(&tx_bv, -2.0, 1.0, &mut rng);
            assert_eq!(llrs.len(), n_bits);
            total_errors += llrs
                .iter()
                .zip(tx.iter())
                .filter(|(l, &b)| l.hard_decision() != b)
                .count();
            total_bits += n_bits;
        }
        assert!(
            total_errors > 0,
            "QpskRicianChannelModel at -2 dB Eb/N0 produced zero bit errors over {total_bits} bits; \
             this indicates the adapter's noise step has silently collapsed to 0"
        );
    }
}

#[test]
fn test_noise_convention_bpsk_closed_form() {
    // BPSK with points {+1, -1} and noise_var = N0 = 2 sigma^2 has
    // LLR = 2 r / sigma^2 = 4 r / N0.
    let spec: ModemSpec<f64> = ModemSpec::<f64>::bpsk_with_scalar();
    let demapper = ReferenceSoftDemapper::new(spec);

    let rx_samples = [-1.5, -0.5, -0.1, 0.0, 0.1, 0.5, 1.5];
    let sigma_sqs = [0.1_f64, 0.25, 0.5, 1.0];
    for &sigma_sq in &sigma_sqs {
        let n0 = 2.0 * sigma_sq;
        for &y in &rx_samples {
            let rx_i = vec![y];
            let rx_q = vec![0.0_f64];
            let nv = vec![n0];
            let input = DemapInput::<f64> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            };
            let mut out = [Llr::new(0.0)];
            demapper.demap_llrs(input, &mut out);
            let expected = 2.0 * y / sigma_sq;
            let got = out[0].value() as f64;
            assert!(
                (got - expected).abs() <= 1e-3_f64.max(expected.abs() * 1e-3),
                "BPSK closed form: y={y} sigma^2={sigma_sq} got {got} want {expected}"
            );
        }
    }

    let sigma_sq = unit_energy_sigma_sq_from_eb_n0_db(1, 1.0, 6.0);
    let n0_helper = unit_energy_n0_from_eb_n0_db(1, 1.0, 6.0);
    assert!((n0_helper - 2.0 * sigma_sq).abs() < 1e-15);
}

#[test]
fn test_noise_convention_qpsk_projection() {
    // Gray QPSK at unit average symbol energy has per-axis amplitude
    // s = 1/sqrt(2) and the MSB is the I-axis sign bit. Max-log LLR on bit 0:
    //   ((y_i + s)^2 - (y_i - s)^2) / N0 = 4 * s * y_i / N0
    let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(4);
    let demapper = FastGrayQamDemapper::new(spec);
    let s = 1.0_f64 / 2.0_f64.sqrt();

    let rx_pairs: [(f64, f64); 5] = [
        (0.0, 0.0),
        (0.3, -0.2),
        (-0.7, 0.9),
        (1.0, -1.0),
        (-1.1, 0.05),
    ];
    let sigma_sqs = [0.1_f64, 0.5, 1.0];
    for &sigma_sq in &sigma_sqs {
        let n0 = 2.0 * sigma_sq;
        for &(y_i, y_q) in &rx_pairs {
            let rx_i_v = vec![y_i];
            let rx_q_v = vec![y_q];
            let nv = vec![n0];
            let input = DemapInput::<f64> {
                rx_i: &rx_i_v,
                rx_q: &rx_q_v,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::MaxLog,
            };
            let mut out = [Llr::new(0.0), Llr::new(0.0)];
            demapper.demap_llrs(input, &mut out);
            let expected_msb = 4.0 * s * y_i / n0;
            let expected_lsb = 4.0 * s * y_q / n0;
            let got_msb = out[0].value() as f64;
            let got_lsb = out[1].value() as f64;
            assert!(
                (got_msb - expected_msb).abs() <= 1e-2_f64.max(expected_msb.abs() * 1e-3),
                "QPSK MSB projection: y=({y_i},{y_q}) sigma^2={sigma_sq} got {got_msb} want {expected_msb}"
            );
            assert!(
                (got_lsb - expected_lsb).abs() <= 1e-2_f64.max(expected_lsb.abs() * 1e-3),
                "QPSK LSB projection: y=({y_i},{y_q}) sigma^2={sigma_sq} got {got_lsb} want {expected_lsb}"
            );
        }
    }
}

#[test]
fn test_awgn_channel_helper_matches_unit_energy_helper() {
    for m in [1usize, 2, 4, 6, 8] {
        for &rate in &[0.5_f64, 1.0] {
            for &ebn0 in &[0.0, 3.0, 6.0, 10.0] {
                let ch = AwgnChannel::from_eb_n0_db(ebn0, rate);
                let expected = unit_energy_sigma_sq_from_eb_n0_db(1, rate, ebn0);
                // `AwgnChannel::from_eb_n0_db` is fixed to m = 1; the loop
                // over `m` exercises the helpers at higher m.
                assert!(
                    (ch.variance() - expected).abs() < 1e-12,
                    "AwgnChannel::from_eb_n0_db rate={rate} ebn0={ebn0}: variance {} != expected {}",
                    ch.variance(),
                    expected
                );
                let sigma_sq_m = unit_energy_sigma_sq_from_eb_n0_db(m, rate, ebn0);
                let n0_m = unit_energy_n0_from_eb_n0_db(m, rate, ebn0);
                assert!(
                    (n0_m - 2.0 * sigma_sq_m).abs() < 1e-15,
                    "N0/sigma^2 helper mismatch m={m} rate={rate}"
                );
            }
        }
    }
}
