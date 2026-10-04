use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_core::BitVec;

fn main() {
    let code = LdpcCode::dvb_t2_normal(gf2_coding::CodeRate::Rate1_2);

    let n = code.n();
    let codeword = BitVec::zeros(n);

    let llrs: Vec<Llr> = (0..n)
        .map(|i| {
            if codeword.get(i) {
                Llr::new(-10.0f32)
            } else {
                Llr::new(10.0f32)
            }
        })
        .collect();

    let mut decoder = LdpcDecoder::new(code);
    for _ in 0..500 {
        let _ = decoder.decode_iterative(&llrs, 50);
    }
}
