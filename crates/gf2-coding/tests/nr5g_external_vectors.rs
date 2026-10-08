//! External 5G NR LDPC conformance evidence (`@/citation/ThreeGpp2017`) from
//! `@/citation/Sionna2026`, with provenance in `data/ldpc/nr_5g/PROVENANCE.md`:
//! the compiled-in base-graph shift tables (Tables 5.3.2-2 and 5.3.2-3,
//! lifting sets `i_LS` = 0..7) equal the Sionna CSV tables bit for bit, and
//! the messages and rate-matched codewords of Sionna's encoder are reproduced
//! by the encoder and recovered by the single-frame and batch decoders.
//! BG2 rate coverage stops at 2/3: above a payload of 292 bits, clause 7.2.2
//! selects BG2 only for R <= 0.67.

use gf2_coding::ldpc::nr_5g::lifting::LIFTING_SIZE_SETS;
use gf2_coding::ldpc::nr_5g::{
    lifting_set_index, shift_table, Nr5gRateMatchedBatchDecoder, Nr5gRateMatchedCode,
    Nr5gRateMatchedDecoder,
};
use gf2_coding::ldpc::{BatchRoute, DecoderAlgorithm, LaneBackend, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BatchSoftDecoder, BlockEncoder, DecodeOutcome, IterativeSoftDecoder};
use gf2_coding::tuning::DecodeSelectors;
use gf2_core::BitVec;
use std::path::PathBuf;

/// Parses a Sionna base-graph CSV (format in `data/ldpc/nr_5g/PROVENANCE.md`)
/// into 8 dense per-`i_LS` matrices; entries absent from the file are -1.
fn load_reference(bg: u8) -> [Vec<Vec<i16>>; 8] {
    let (rows, cols, file, expected_edges) = match bg {
        1 => (46, 68, "5G_bg1.csv", 316),
        2 => (42, 52, "5G_bg2.csv", 197),
        _ => panic!("base graph must be 1 or 2, got {bg}"),
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/ldpc/nr_5g")
        .join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read reference file {}: {e}", path.display()));

    let mut mats: [Vec<Vec<i16>>; 8] = std::array::from_fn(|_| vec![vec![-1i16; cols]; rows]);
    let mut cur_row: Option<usize> = None;
    let mut edges = 0usize;
    for line in text.lines().skip(2) {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(';').collect();
        assert!(parts.len() >= 10, "malformed reference line: {line:?}");
        if !parts[0].trim().is_empty() {
            cur_row = Some(parts[0].trim().parse().expect("row index"));
        }
        let r = cur_row.expect("column entry before any row index");
        let c: usize = parts[1].trim().parse().expect("column index");
        assert!(r < rows && c < cols, "edge ({r},{c}) out of bounds");
        for (i_ls, mat) in mats.iter_mut().enumerate() {
            let v: i16 = parts[2 + i_ls].trim().parse().expect("shift value");
            assert!(v >= 0, "reference stores only connected edges, got {v}");
            assert_eq!(mat[r][c], -1, "duplicate edge ({r},{c})");
            mat[r][c] = v;
        }
        edges += 1;
    }
    assert_eq!(edges, expected_edges, "BG{bg} reference edge count");
    mats
}

/// Applies `V mod Z` to a raw shift table, preserving -1 (no connection).
fn reduce_mod_z(table: &[Vec<i16>], z: usize) -> Vec<Vec<i32>> {
    table
        .iter()
        .map(|row| {
            row.iter()
                .map(|&v| {
                    if v < 0 {
                        -1i32
                    } else {
                        (v as i32) % (z as i32)
                    }
                })
                .collect()
        })
        .collect()
}

fn check_bg_ils_against_reference(bg: u8, i_ls: usize) {
    let reference = load_reference(bg);

    assert_eq!(
        shift_table(bg, i_ls),
        reference[i_ls],
        "BG{bg} i_LS={i_ls}: raw shift table differs from the Sionna reference"
    );

    for &z in LIFTING_SIZE_SETS[i_ls] {
        let z = z as usize;
        let qc = QuasiCyclicLdpc::nr_5g(bg, z);
        let expected = reduce_mod_z(&reference[i_ls], z);
        assert_eq!(
            qc.base_matrix(),
            &expected[..],
            "BG{bg} Z={z} (i_LS={i_ls}): constructor base matrix differs from \
             the Sionna reference reduced mod Z"
        );
    }
}

#[test]
fn test_bg1_ils0_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 0);
}

#[test]
fn test_bg1_ils1_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 1);
}

#[test]
fn test_bg1_ils2_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 2);
}

#[test]
fn test_bg1_ils3_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 3);
}

#[test]
fn test_bg1_ils4_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 4);
}

#[test]
fn test_bg1_ils5_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 5);
}

#[test]
fn test_bg1_ils6_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 6);
}

#[test]
fn test_bg1_ils7_tables_match_external_reference() {
    check_bg_ils_against_reference(1, 7);
}

#[test]
fn test_bg2_ils0_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 0);
}

#[test]
fn test_bg2_ils1_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 1);
}

#[test]
fn test_bg2_ils2_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 2);
}

#[test]
fn test_bg2_ils3_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 3);
}

#[test]
fn test_bg2_ils4_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 4);
}

#[test]
fn test_bg2_ils5_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 5);
}

#[test]
fn test_bg2_ils6_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 6);
}

#[test]
fn test_bg2_ils7_tables_match_external_reference() {
    check_bg_ils_against_reference(2, 7);
}

fn check_wrong_ils_guard_z208(bg: u8) {
    let z = 208usize;
    assert_eq!(lifting_set_index(z as u16), Some(6), "Z=208 must be i_LS=6");

    let reference = load_reference(bg);
    let correct = reduce_mod_z(&reference[6], z);

    let qc = QuasiCyclicLdpc::nr_5g(bg, z);
    assert_eq!(
        qc.base_matrix(),
        &correct[..],
        "BG{bg} Z=208: correct i_LS=6 table must match the external reference"
    );

    for wrong_ils in (0..8).filter(|&w| w != 6) {
        let wrong = reduce_mod_z(&shift_table(bg, wrong_ils), z);
        assert_ne!(
            wrong, correct,
            "BG{bg} Z=208: the WRONG i_LS={wrong_ils} table must differ from \
             the i_LS=6 external reference — per-i_LS tables may have been \
             collapsed (the ~2 dB BLER trap)"
        );
    }
}

#[test]
fn test_bg1_wrong_ils_table_for_z208_differs_from_reference() {
    check_wrong_ils_guard_z208(1);
}

#[test]
fn test_bg2_wrong_ils_table_for_z208_differs_from_reference() {
    check_wrong_ils_guard_z208(2);
}

#[test]
fn test_per_ils_tables_pairwise_distinct() {
    for bg in [1u8, 2u8] {
        for i in 0..8 {
            for j in (i + 1)..8 {
                assert_ne!(
                    shift_table(bg, i),
                    shift_table(bg, j),
                    "BG{bg}: raw shift tables for i_LS={i} and i_LS={j} must \
                     be distinct — per-i_LS tables may have been collapsed"
                );
            }
        }
    }
}

fn deterministic_message(k: usize) -> BitVec {
    stepped_message(k, 3)
}

/// The `k`-bit message with every `step`-th bit set, from bit zero.
fn stepped_message(k: usize, step: usize) -> BitVec {
    let mut msg = BitVec::zeros(k);
    for i in (0..k).step_by(step) {
        msg.set(i, true);
    }
    msg
}

fn noiseless_llrs(codeword: &BitVec) -> Vec<Llr> {
    (0..codeword.len())
        .map(|i| Llr::new(if codeword.get(i) { -4.0 } else { 4.0 }))
        .collect()
}

/// The frames Sionna's encoder produced for one rate-matched configuration.
struct ExternalVectors {
    lifting_size: usize,
    filler_bits: usize,
    messages: Vec<BitVec>,
    codewords: Vec<BitVec>,
}

/// Parses a hex bit string of `data/ldpc/nr_5g/sionna_ldpc5g_vectors.txt`
/// (format in `PROVENANCE.md` beside it) into its first `len` bits.
fn parse_hex_bits(hex: &str, len: usize) -> BitVec {
    assert_eq!(hex.len(), len.div_ceil(4), "hex length for {len} bits");
    let mut bits = BitVec::zeros(len);
    for (j, c) in hex.chars().enumerate() {
        let digit = c.to_digit(16).expect("hex digit");
        for b in 0..4 {
            let set = digit & (8 >> b) != 0;
            if 4 * j + b < len {
                bits.set(4 * j + b, set);
            } else {
                assert!(!set, "padding bits must be zero");
            }
        }
    }
    bits
}

fn load_external_vectors(bg: u8, target_n: usize, target_k: usize) -> ExternalVectors {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/nr_5g/sionna_ldpc5g_vectors.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read vector file {}: {e}", path.display()));
    let field = |fields: &[&str], key: &str| -> usize {
        fields
            .iter()
            .find_map(|f| f.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or_else(|| panic!("config line lacks {key}"))
            .parse()
            .expect("config value")
    };

    let mut found: Option<ExternalVectors> = None;
    let mut in_config = false;
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["config", rest @ ..] => {
                in_config = (field(rest, "bg"), field(rest, "n"), field(rest, "k"))
                    == (bg as usize, target_n, target_k);
                if in_config {
                    assert!(found.is_none(), "duplicate configuration");
                    found = Some(ExternalVectors {
                        lifting_size: field(rest, "z"),
                        filler_bits: field(rest, "k_filler"),
                        messages: Vec::new(),
                        codewords: Vec::new(),
                    });
                }
            }
            ["frame", message, codeword] => {
                if in_config {
                    let vectors = found.as_mut().expect("frame before its configuration");
                    vectors.messages.push(parse_hex_bits(message, target_k));
                    vectors.codewords.push(parse_hex_bits(codeword, target_n));
                }
            }
            _ => panic!("malformed vector line: {line:?}"),
        }
    }
    found.unwrap_or_else(|| panic!("no external vectors for BG{bg} (n={target_n}, k={target_k})"))
}

fn batch_decoder(
    rm: &Nr5gRateMatchedCode,
    selectors: DecodeSelectors,
) -> Nr5gRateMatchedBatchDecoder {
    Nr5gRateMatchedBatchDecoder::with_selectors(
        rm.clone(),
        DecoderAlgorithm::NormalizedMinSum(0.75),
        50,
        selectors,
    )
}

/// The number of frames in one wave of the batch decoder.
fn wave_frames(rm: &Nr5gRateMatchedCode) -> usize {
    batch_decoder(rm, DecodeSelectors::CONSERVATIVE).batch_granularity()
}

/// Decodes noiseless `codewords` through [`Nr5gRateMatchedBatchDecoder`] on the
/// lane route and on the single-frame route and expects `messages`.
fn check_batch_decodes(
    rm: &Nr5gRateMatchedCode,
    codewords: &[BitVec],
    messages: &[BitVec],
    label: &str,
) {
    let frames: Vec<Vec<Llr>> = codewords.iter().map(noiseless_llrs).collect();
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();

    let routes = [
        (
            DecodeSelectors::try_new(1, usize::MAX, 1).unwrap(),
            BatchRoute::Lane(LaneBackend::detected()),
        ),
        (DecodeSelectors::CONSERVATIVE, BatchRoute::SingleFrame),
    ];
    for (selectors, route) in routes {
        let mut decoder = batch_decoder(rm, selectors);
        assert_eq!(decoder.batch_route(frames.len()), route, "{label}");
        let mut decoded = vec![BitVec::new(); frames.len()];
        let mut outcomes = vec![DecodeOutcome::default(); frames.len()];
        decoder.decode_batch(&views, &mut decoded, &mut outcomes);
        for (f, outcome) in outcomes.iter().enumerate() {
            assert!(
                outcome.syndrome_check_passed,
                "{label}, {route:?}: noiseless batch decode of frame {f} must converge"
            );
        }
        assert_eq!(
            decoded, messages,
            "{label}, {route:?}: noiseless batch decode must be bit-exact"
        );
    }
}

/// Holds the encoder, the single-frame decoder and the batch decoder to the
/// Sionna vectors of one configuration, which span one full wave and an
/// incomplete final wave.
fn check_external_vectors(rm: &Nr5gRateMatchedCode, label: &str) {
    let params = rm.params();
    let vectors = load_external_vectors(params.base_graph, rm.n(), rm.k());
    assert_eq!(
        vectors.lifting_size, params.lifting_factor,
        "{label}: lifting size differs from Sionna's"
    );
    assert_eq!(
        vectors.filler_bits, params.num_shortened,
        "{label}: filler count differs from Sionna's"
    );
    let (frames, wave) = (vectors.messages.len(), wave_frames(rm));
    assert!(
        frames > wave && frames % wave != 0,
        "{label}: the vectors must span a full and an incomplete wave"
    );

    for (f, (msg, codeword)) in vectors.messages.iter().zip(&vectors.codewords).enumerate() {
        assert_eq!(
            &rm.encode(msg),
            codeword,
            "{label}: encoder output differs from Sionna's codeword of frame {f}"
        );
        let mut decoder = Nr5gRateMatchedDecoder::new(rm.clone());
        let result = decoder.decode_iterative(&noiseless_llrs(codeword), 50);
        assert!(
            result.syndrome_check_passed,
            "{label}: noiseless decode of Sionna frame {f} must converge"
        );
        assert_eq!(
            &result.decoded_bits, msg,
            "{label}: Sionna frame {f} must decode to its message"
        );
    }
    check_batch_decodes(rm, &vectors.codewords, &vectors.messages, label);
}

/// Round-trips library-encoded frames through the batch decoder; the first
/// message is [`deterministic_message`].
fn check_batch_roundtrip(rm: &Nr5gRateMatchedCode, label: &str) {
    let messages: Vec<BitVec> = (0..wave_frames(rm) + 1)
        .map(|f| stepped_message(rm.k(), 3 + f))
        .collect();
    let codewords: Vec<BitVec> = messages.iter().map(|msg| rm.encode(msg)).collect();
    check_batch_decodes(rm, &codewords, &messages, label);
}

fn check_rate_tuple(bg: u8, target_n: usize, target_k: usize, expect_z: usize, expect_ils: usize) {
    let rm = QuasiCyclicLdpc::nr_5g_rate_matched(bg, target_n, target_k);
    let params = rm.params().clone();
    assert_eq!(params.base_graph, bg);
    assert_eq!(
        params.lifting_factor, expect_z,
        "BG{bg} (n={target_n}, k={target_k}): expected lifting size Z={expect_z}"
    );
    assert_eq!(
        lifting_set_index(expect_z as u16),
        Some(expect_ils),
        "Z={expect_z} must belong to lifting set i_LS={expect_ils}"
    );
    assert_eq!(rm.mother_code().n(), params.nb * expect_z);

    let reference = load_reference(bg);
    let expected = reduce_mod_z(&reference[expect_ils], expect_z);
    let qc = QuasiCyclicLdpc::nr_5g(bg, expect_z);
    assert_eq!(
        qc.base_matrix(),
        &expected[..],
        "BG{bg} Z={expect_z} (i_LS={expect_ils}): mother base matrix differs \
         from the Sionna reference reduced mod Z"
    );

    let msg = deterministic_message(target_k);
    let codeword = rm.encode(&msg);
    assert_eq!(codeword.len(), target_n);
    let llrs = noiseless_llrs(&codeword);
    let label = format!("BG{bg} (n={target_n}, k={target_k})");
    check_external_vectors(&rm, &label);
    check_batch_roundtrip(&rm, &label);
    let mut decoder = Nr5gRateMatchedDecoder::new(rm);
    let result = decoder.decode_iterative(&llrs, 50);
    assert!(
        result.syndrome_check_passed,
        "BG{bg} (n={target_n}, k={target_k}): noiseless decode must converge"
    );
    assert_eq!(
        result.decoded_bits, msg,
        "BG{bg} (n={target_n}, k={target_k}): noiseless roundtrip must be bit-exact"
    );
}

#[test]
fn test_rate_1_3_bg1_z24_ils1() {
    check_rate_tuple(1, 1584, 528, 24, 1);
}

#[test]
fn test_rate_1_2_bg1_z20_ils2() {
    check_rate_tuple(1, 880, 440, 20, 2);
}

#[test]
fn test_rate_2_3_bg1_z28_ils3() {
    check_rate_tuple(1, 924, 616, 28, 3);
}

#[test]
fn test_rate_5_6_bg1_z30_ils7() {
    check_rate_tuple(1, 792, 660, 30, 7);
}

#[test]
fn test_rate_1_3_bg2_z26_ils6() {
    check_rate_tuple(2, 600, 200, 26, 6);
}

#[test]
fn test_rate_1_2_bg2_z44_ils5() {
    check_rate_tuple(2, 704, 352, 44, 5);
}

#[test]
fn test_rate_2_3_bg2_z48_ils1() {
    check_rate_tuple(2, 540, 360, 48, 1);
}
