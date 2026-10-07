//! Correctness gate for the operation-equivalent xdsopl PCTITL adapter.
//!
//! Three independent checks run before any timed cell, in the order the
//! measurement contract requires evidence: canonical bit order and tail
//! padding through the conversion helpers, the permutation itself over every
//! position of every surveyed MODCOD, and the two timed arms' whole-consumer
//! outputs on identical seeded frames.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2BitInterleaver;
use gf2_coding::Llr;
use gf2_core::BitVec;
use survey_gf2_side::{
    bitvec_from_words, frame_bits, modcod_for_name, pack_bits, seeded_word_banks, unpack_words,
    xdsopl_forward, MODCODS,
};

/// Canonical little-endian bit order survives unpack/pack at the word
/// boundaries the engineering contract names, with zero tail padding.
fn validate_packing() {
    let known_words = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210];
    let bitvec = BitVec::from_words(known_words.to_vec(), 128);
    let bits = unpack_words(bitvec.words(), bitvec.len());
    let round_trip = pack_bits(&bits);
    assert_eq!(
        round_trip, known_words,
        "canonical bit packing changed bytes"
    );

    for length in [0_usize, 1, 63, 64, 65, 127, 128] {
        let words = seeded_word_banks(0xfeed_face_u64 + length as u64, 1, length)
            .pop()
            .expect("one bank");
        let bits = unpack_words(&words, length);
        assert_eq!(bits.len(), length, "unpack produced the wrong bit count");
        let packed = pack_bits(&bits);
        assert_eq!(
            packed, words,
            "round trip changed the packed words at length {length}"
        );
        if let Some(&tail) = words.last() {
            let tail_bits = length % 64;
            if tail_bits != 0 {
                assert_eq!(
                    tail >> tail_bits,
                    0,
                    "tail padding is nonzero at length {length}"
                );
            }
        }
    }
    println!("PASS canonical bit order: 128/128 bits round-trip, lengths 0/1/63/64/65/127/128 preserved with zero tail padding");
}

/// Every output position of gf2's permutation equals xdsopl's for this MODCOD.
fn validate_permutation(name: &str) {
    let bits = frame_bits(name);
    let interleaver = DvbT2BitInterleaver::new(modcod_for_name(name));
    assert_eq!(
        interleaver.frame_bits(),
        bits,
        "{name}: gf2 frame length disagrees with the survey table"
    );
    let tagged: Vec<Llr> = (0..bits).map(|index| Llr::new(index as f32)).collect();
    let forward: Vec<usize> = interleaver
        .deinterleave_llrs(&tagged)
        .into_iter()
        .map(|llr| llr.value() as usize)
        .collect();
    let mut gf2_inverse = vec![0_usize; bits];
    for (input_index, &output_index) in forward.iter().enumerate() {
        gf2_inverse[output_index] = input_index;
    }

    let identity: Vec<i32> = (0..bits).map(|index| index as i32).collect();
    let mut xdsopl_out = vec![0_i32; bits];
    xdsopl_forward(name, &identity, &mut xdsopl_out);
    for index in 0..bits {
        if gf2_inverse[index] != xdsopl_out[index] as usize {
            eprintln!(
                "FAIL {name}: first mismatch at index {index}: gf2_inverse={} xdsopl_out={}",
                gf2_inverse[index], xdsopl_out[index]
            );
            std::process::exit(1);
        }
    }
    println!("PASS {name}: {bits}/{bits} positions match");
}

/// The two timed arms produce identical output on identical seeded frames.
///
/// This is the whole-consumer path each arm times: gf2 interleaves its native
/// packed `BitVec`; the external arm unpacks to one `int32` per bit, calls
/// `PCTITL::fwd` and packs the result back.
fn validate_timed_arms(name: &str, seed: u64) {
    let bits = frame_bits(name);
    let interleaver = DvbT2BitInterleaver::new(modcod_for_name(name));
    let banks = seeded_word_banks(seed, 2, bits);
    for (index, words) in banks.iter().enumerate() {
        let gf2_out = interleaver.interleave(&bitvec_from_words(words, bits));
        let unpacked = unpack_words(words, bits);
        let mut external = vec![0_i32; bits];
        xdsopl_forward(name, &unpacked, &mut external);
        let packed = pack_bits(&external);
        if packed != gf2_out.words() {
            let first = packed
                .iter()
                .zip(gf2_out.words())
                .position(|(left, right)| left != right)
                .unwrap_or(0);
            eprintln!(
                "FAIL {name}: arm outputs differ on bank {index} at word {first}: \
                 external={:#018x} gf2={:#018x}",
                packed[first],
                gf2_out.words()[first]
            );
            std::process::exit(1);
        }
    }
    println!(
        "PASS {name}: {} banks of {bits} bits, both timed arms agree bit-for-bit",
        banks.len()
    );
}

fn main() {
    validate_packing();
    for name in MODCODS {
        validate_permutation(name);
    }
    for (index, name) in MODCODS.iter().enumerate() {
        validate_timed_arms(name, 101 + index as u64);
    }
}
