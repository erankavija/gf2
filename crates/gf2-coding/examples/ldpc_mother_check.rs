//! Check if the MOTHER code (no rate matching) decodes properly
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder, QuasiCyclicLdpc};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel};
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn main() {
    // BG2, Z=13: mother code
    let qc = QuasiCyclicLdpc::nr_5g(2, 13);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    println!(
        "Mother code: n={}, k={}, m={}",
        code.n(),
        code.k(),
        code.m()
    );

    let rate = code.k() as f64 / code.n() as f64;
    println!("Rate: {:.3}", rate);

    let channel = BpskAwgnChannel;

    for &eb_n0_db in &[1.0, 2.0, 3.0, 4.0] {
        let mut decoder = LdpcDecoder::new(code.clone());
        let mut rng = StdRng::seed_from_u64(42);
        let mut frame_errors = 0;
        let k = code.k();
        let num_frames = 500;

        for _ in 0..num_frames {
            let msg = BitVec::random(k, &mut rng);
            let encoder = LdpcEncoder::new(code.clone());
            let cw = encoder.encode(&msg);

            let llrs = channel.transmit_and_demodulate(&cw, eb_n0_db, rate, &mut rng);

            let result = decoder.decode_iterative(&llrs, 50);
            let has_error = (0..k).any(|i| result.decoded_bits.get(i) != msg.get(i));
            if has_error {
                frame_errors += 1;
            }
        }

        println!(
            "Mother @{:.1} dB: BLER={:.4e} ({}/{})",
            eb_n0_db,
            frame_errors as f64 / num_frames as f64,
            frame_errors,
            num_frames
        );
    }
}
