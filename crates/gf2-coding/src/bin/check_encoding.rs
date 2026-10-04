use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;

fn main() {
    let cache = EncodingCache::from_directory(std::path::Path::new("data/ldpc/dvb_t2")).unwrap();

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate3_5);
    let encoder = LdpcEncoder::with_cache(code.clone(), &cache);

    let msg = gf2_core::BitVec::zeros(encoder.k());
    let cw = encoder.encode(&msg);

    let is_valid = code.is_valid_codeword(&cw);

    println!("Message: {} zeros", msg.len());
    println!("Codeword: {} bits", cw.len());
    println!("Valid codeword: {}", is_valid);

    if !is_valid {
        println!("\n⚠️  WARNING: Encoder produced INVALID codeword!");
        println!("The systematic encoder may not be working correctly.");
    }
}
