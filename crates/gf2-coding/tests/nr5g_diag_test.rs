use gf2_coding::channel::{AwgnChannel, BpskModulator};
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;

#[test]
fn diag_bler_simulation_mother_code() {
    // Reproduce the bug: simulate BLER using LdpcEncoder + LdpcDecoder on mother code
    let qc = QuasiCyclicLdpc::nr_5g(2, 13);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let encoder = LdpcEncoder::new(code.clone());

    let n = code.n();
    let k = code.k();
    let rate = k as f64 / n as f64;

    eprintln!("Mother code: n={}, k={}, rate={:.3}", n, k, rate);

    let mut rng = StdRng::seed_from_u64(42);

    for &eb_n0_db in &[3.0, 4.0] {
        let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
        let sigma_sq = channel.variance();

        let num_frames = 100;
        let mut frame_errors = 0;
        let mut convergence_failures = 0;

        for _ in 0..num_frames {
            let msg = BitVec::random(k, &mut rng);
            let cw = encoder.encode(&msg);

            let symbols: Vec<f64> = (0..n).map(|i| BpskModulator::modulate(cw.get(i))).collect();
            let received = channel.transmit_symbols(&symbols, &mut rng);
            let llrs: Vec<Llr> = received
                .iter()
                .map(|&r| BpskModulator::to_llr(r, sigma_sq))
                .collect();

            let mut decoder = LdpcDecoder::new(code.clone());
            let result = decoder.decode_iterative(&llrs, 50);

            if !result.converged {
                convergence_failures += 1;
            }

            let has_error = (0..k).any(|i| result.decoded_bits.get(i) != msg.get(i));
            if has_error {
                frame_errors += 1;
            }
        }

        let bler = frame_errors as f64 / num_frames as f64;
        eprintln!(
            "  @{:.1}dB: BLER={:.3}, convergence_failures={}/{}",
            eb_n0_db, bler, convergence_failures, num_frames
        );

        if eb_n0_db >= 4.0 {
            assert!(
                bler < 1.0,
                "BUG CONFIRMED: BLER=1.0 at {}dB - all frames have errors",
                eb_n0_db
            );
        }
    }
}
