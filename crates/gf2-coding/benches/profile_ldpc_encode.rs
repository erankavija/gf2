use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use std::path::PathBuf;

fn main() {
    let code = LdpcCode::dvb_t2_normal(gf2_coding::CodeRate::Rate1_2);

    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    let cache = EncodingCache::from_directory(&cache_dir)
        .expect("Failed to load cache - run: cargo run --release --bin generate_ldpc_cache");

    let encoder = LdpcEncoder::with_cache(code, &cache);

    let k = encoder.k();
    let message = BitVec::zeros(k);

    for _ in 0..1000 {
        let _encoded = encoder.encode(&message);
    }
}
