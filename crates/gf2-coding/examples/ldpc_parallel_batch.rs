//! Parallel LDPC batch decoding: a `PooledBatchDecoder` over `LdpcBatchDecoder`.

use gf2_coding::ldpc::{
    DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode, LdpcEncoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::pooled::PooledBatchDecoder;
use gf2_coding::traits::{BatchSoftDecoder, BlockEncoder, DecodeOutcome};
use gf2_core::BitVec;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![5, 0, 11, -1, 2, 9],
        vec![9, 13, 0, 6, -1, 4],
    ];
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16));
    let encoder = LdpcEncoder::new(code.clone());
    let messages: Vec<BitVec> = (0..64)
        .map(|seed| BitVec::random_seeded(encoder.k(), seed))
        .collect();
    let frames: Vec<Vec<Llr>> = messages
        .iter()
        .map(|message| {
            let codeword = encoder.encode(message);
            (0..codeword.len())
                .map(|i| Llr::new(if codeword.get(i) { -2.0 } else { 2.0 }))
                .collect()
        })
        .collect();

    // One inner decoder per worker of the pool current at construction.
    let rayon_pool = rayon::ThreadPoolBuilder::new().num_threads(8).build()?;
    let mut decoder = rayon_pool.install(|| {
        PooledBatchDecoder::new(|| {
            LdpcBatchDecoder::new(
                code.clone(),
                DecoderConfig::default(),
                50,
                DecodedWord::Message,
            )
        })
    });

    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    let mut decoded = vec![BitVec::new(); views.len()];
    let mut outcomes = vec![DecodeOutcome::default(); views.len()];
    rayon_pool.install(|| decoder.decode_batch(&views, &mut decoded, &mut outcomes));

    assert_eq!(decoded, messages);
    assert!(outcomes.iter().all(|outcome| outcome.converged));
    Ok(())
}
