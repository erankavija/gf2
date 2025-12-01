use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_core::BitVec;

fn main() {
    // Load LDPC code
    let code = LdpcCode::dvb_t2_normal(gf2_coding::CodeRate::Rate1_2);

    // Create codeword (all zeros = valid codeword)
    let n = code.n();
    let codeword = BitVec::zeros(n);

    // Convert to LLRs (assuming BPSK with high SNR)
    let llrs: Vec<Llr> = (0..n)
        .map(|i| {
            if codeword.get(i) {
                Llr::new(-10.0f32)
            } else {
                Llr::new(10.0f32)
            }
        })
        .collect();

    // Decode 500 times (slower than encoding, takes ~15 seconds)
    let mut decoder = LdpcDecoder::new(code);
    for _ in 0..500 {
        let _ = decoder.decode_iterative(&llrs, 50);
    }
}
