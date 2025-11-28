use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use std::path::PathBuf;

fn main() {
    // Load LDPC code (normal rate 1/2)
    let code = LdpcCode::dvb_t2_normal(gf2_coding::CodeRate::Rate1_2);

    // Load encoding cache from data directory
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    let cache = EncodingCache::from_directory(&cache_dir)
        .expect("Failed to load cache - run: cargo run --release --bin generate_ldpc_cache");

    let encoder = LdpcEncoder::with_cache(code, &cache);

    // Create message
    let k = encoder.k();
    let message = BitVec::zeros(k);

    // Encode 1000 times to get good profiling data (takes ~10 seconds)
    for _ in 0..1000 {
        let _encoded = encoder.encode(&message);
    }
}
