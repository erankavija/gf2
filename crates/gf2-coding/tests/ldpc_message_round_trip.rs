//! `decode_iterative` on the noiseless LLRs of an encoded message returns that
//! message, for every LDPC code family the crate constructs.

use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_coding::CodeRate;
use gf2_core::alg::rref::rref;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

const SEED: u64 = 0x95ca_83aa;
const CAP: usize = 50;

const RATES: [CodeRate; 6] = [
    CodeRate::Rate1_2,
    CodeRate::Rate3_5,
    CodeRate::Rate2_3,
    CodeRate::Rate3_4,
    CodeRate::Rate4_5,
    CodeRate::Rate5_6,
];

/// Encodes `messages` seeded random messages and asserts that `decoder`
/// converges on the noiseless LLRs of each codeword to the encoded message.
fn assert_decodes_the_encoded_message<E, D>(
    label: &str,
    encoder: &E,
    decoder: &mut D,
    messages: usize,
) where
    E: BlockEncoder,
    D: IterativeSoftDecoder,
{
    let mut rng = StdRng::seed_from_u64(SEED);
    for index in 0..messages {
        let message = BitVec::random(encoder.k(), &mut rng);
        let codeword = encoder.encode(&message);
        let llrs: Vec<Llr> = (0..codeword.len())
            .map(|i| Llr::new(if codeword.get(i) { -10.0 } else { 10.0 }))
            .collect();

        let result = decoder.decode_iterative(&llrs, CAP);
        assert!(result.converged, "{label}, message {index}: no convergence");
        assert_eq!(result.decoded_bits, message, "{label}, message {index}");
    }
}

fn assert_code_decodes_the_encoded_message(label: &str, code: LdpcCode, messages: usize) {
    let encoder = LdpcEncoder::new(code.clone());
    let mut decoder = LdpcDecoder::new(code);
    assert_decodes_the_encoded_message(label, &encoder, &mut decoder, messages);
}

fn is_full_rank(m: usize, n: usize, edges: &[(usize, usize)]) -> bool {
    let h = LdpcCode::from_edges(m, n, edges)
        .parity_check_matrix()
        .to_dense();
    rref(&h, false).rank == m
}

/// The first full-rank code `edges` yields over the seeds from [`SEED`] on.
fn first_full_rank(
    m: usize,
    n: usize,
    edges: impl Fn(&mut StdRng) -> Vec<(usize, usize)>,
) -> LdpcCode {
    (SEED..)
        .map(|seed| edges(&mut StdRng::seed_from_u64(seed)))
        .find(|edges| is_full_rank(m, n, edges))
        .map(|edges| LdpcCode::from_edges(m, n, &edges))
        .unwrap()
}

/// A full-rank `m × 2m` code of column weight three and row weight six: the
/// edge sockets of the checks are matched to those of the variables by a
/// seeded permutation, and a draw with a repeated edge is rejected.
fn regular_3_6(m: usize) -> LdpcCode {
    let n = 2 * m;
    first_full_rank(m, n, |rng| loop {
        let mut check_sockets: Vec<usize> = (0..6 * m).map(|socket| socket / 6).collect();
        check_sockets.shuffle(rng);
        let mut edges: Vec<(usize, usize)> = check_sockets
            .iter()
            .enumerate()
            .map(|(socket, &check)| (check, socket / 3))
            .collect();
        edges.sort_unstable();
        edges.dedup();
        if edges.len() == 6 * m {
            return edges;
        }
    })
}

/// A full-rank `m × n` code whose columns each hold three checks drawn
/// uniformly without replacement.
fn random_column_weight_3(m: usize, n: usize) -> LdpcCode {
    let checks: Vec<usize> = (0..m).collect();
    first_full_rank(m, n, |rng| {
        (0..n)
            .flat_map(|col| {
                checks
                    .choose_multiple(rng, 3)
                    .map(move |&check| (check, col))
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}

/// The (96, 48) quasi-cyclic code of the `LdpcBatchDecoder` contract tests.
fn quasi_cyclic_96_48() -> LdpcCode {
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![5, 0, 11, -1, 2, 9],
        vec![9, 13, 0, 6, -1, 4],
    ];
    LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16))
}

#[test]
fn quasi_cyclic_codes_decode_the_encoded_message() {
    assert_code_decodes_the_encoded_message("qc (96, 48)", quasi_cyclic_96_48(), 16);
}

#[test]
fn regular_and_random_codes_decode_the_encoded_message() {
    for m in [24, 96] {
        assert_code_decodes_the_encoded_message(
            &format!("regular (3, 6), m = {m}"),
            regular_3_6(m),
            16,
        );
    }
    for (m, n) in [(20, 48), (60, 200)] {
        assert_code_decodes_the_encoded_message(
            &format!("random column weight 3, {m} × {n}"),
            random_column_weight_3(m, n),
            16,
        );
    }
}

#[test]
fn dvb_t2_codes_decode_the_encoded_message() {
    for rate in RATES {
        assert_code_decodes_the_encoded_message(
            &format!("dvb-t2 short {rate:?}"),
            LdpcCode::dvb_t2_short(rate),
            2,
        );
        assert_code_decodes_the_encoded_message(
            &format!("dvb-t2 normal {rate:?}"),
            LdpcCode::dvb_t2_normal(rate),
            1,
        );
    }
}

#[test]
fn nr_5g_mother_codes_decode_the_encoded_message() {
    for base_graph in [1, 2] {
        for lifting in [2, 3, 5, 7, 16] {
            assert_code_decodes_the_encoded_message(
                &format!("nr bg{base_graph} z = {lifting}"),
                LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(base_graph, lifting)),
                4,
            );
        }
    }
}

#[test]
fn nr_5g_rate_matched_codes_decode_the_encoded_message() {
    for (base_graph, n, k) in [(1, 264, 88), (1, 600, 300), (2, 200, 40), (2, 520, 100)] {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, n, k);
        let mut decoder = Nr5gRateMatchedDecoder::new(code.clone());
        assert_decodes_the_encoded_message(
            &format!("nr rate-matched bg{base_graph} ({n}, {k})"),
            &code,
            &mut decoder,
            4,
        );
    }
}
