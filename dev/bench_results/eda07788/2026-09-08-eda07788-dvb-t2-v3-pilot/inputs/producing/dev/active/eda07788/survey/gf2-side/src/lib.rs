//! Shared helpers for the DVB-T2 external-baseline survey.
//!
//! One MODCOD name selects both sides of the comparison: gf2's
//! [`DvbT2BitInterleaver`] configuration and the matching xdsopl `PCTITL`
//! template instantiation in `../xdsopl-shim/xdsopl_shim.cpp`. Keeping the
//! mapping here means the validator and both timed arms agree by construction.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{DvbT2Modcod, DvbT2Modulation};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

/// MODCOD names this survey measures, in the order the addenda declare them.
pub const MODCODS: [&str; 4] = [
    "qam16-r12-normal",
    "qam64-r12-normal",
    "qam16-r12-short",
    "qam64-r12-short",
];

/// FECFRAME length in bits for one MODCOD name.
pub fn frame_bits(modcod: &str) -> usize {
    match modcod {
        "qam16-r12-normal" | "qam64-r12-normal" => 64_800,
        "qam16-r12-short" | "qam64-r12-short" => 16_200,
        other => panic!("unrecognized DVB-T2 MODCOD: {other}"),
    }
}

/// gf2-side MODCOD descriptor for one MODCOD name.
pub fn modcod_for_name(name: &str) -> DvbT2Modcod {
    let (frame_size, modulation) = match name {
        "qam16-r12-normal" => (FrameSize::Normal, DvbT2Modulation::Qam16),
        "qam64-r12-normal" => (FrameSize::Normal, DvbT2Modulation::Qam64),
        "qam16-r12-short" => (FrameSize::Short, DvbT2Modulation::Qam16),
        "qam64-r12-short" => (FrameSize::Short, DvbT2Modulation::Qam64),
        other => panic!("unrecognized DVB-T2 MODCOD: {other}"),
    };
    DvbT2Modcod::new(frame_size, CodeRate::Rate1_2, modulation)
}

/// Expands one seed into canonical little-endian packed fixture banks.
///
/// The tail bits above `bits` are zeroed so every bank satisfies the canonical
/// zero-tail-padding invariant of a `BitVec` of that length.
pub fn seeded_word_banks(seed: u64, banks: usize, bits: usize) -> Vec<Vec<u64>> {
    let words = bits.div_ceil(64);
    let mut mixer = tuning_campaign_support::abtest::SplitMix64::new(seed);
    (0..banks)
        .map(|_| {
            let mut bank: Vec<u64> = (0..words).map(|_| mixer.next_u64()).collect();
            let tail_bits = bits % 64;
            if tail_bits != 0 {
                bank[words - 1] &= (1_u64 << tail_bits) - 1;
            }
            bank
        })
        .collect()
}

/// Unpacks canonical little-endian words into one 0/1 `i32` per bit.
pub fn unpack_words(words: &[u64], bits: usize) -> Vec<i32> {
    assert!(words.len() >= bits.div_ceil(64));
    (0..bits)
        .map(|index| ((words[index >> 6] >> (index & 63)) & 1) as i32)
        .collect()
}

/// Packs 0/1 values using the canonical little-endian bit numbering.
pub fn pack_bits(bits: &[i32]) -> Vec<u64> {
    let mut words = vec![0_u64; bits.len().div_ceil(64)];
    for (index, &bit) in bits.iter().enumerate() {
        assert!(bit == 0 || bit == 1, "non-bit value {bit} at index {index}");
        if bit == 1 {
            words[index >> 6] |= 1_u64 << (index & 63);
        }
    }
    words
}

/// Builds a `BitVec` from canonical packed words without changing bit order.
pub fn bitvec_from_words(words: &[u64], bits: usize) -> BitVec {
    BitVec::from_words(words.to_vec(), bits)
}

/// Calls the selected xdsopl `PCTITL` forward operation.
pub fn xdsopl_forward(modcod: &str, input: &[i32], output: &mut [i32]) {
    let bits = frame_bits(modcod);
    assert_eq!(input.len(), bits);
    assert_eq!(output.len(), bits);
    unsafe {
        match modcod {
            "qam16-r12-normal" => {
                xdsopl_pctitl_qam16_r12_normal_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            "qam64-r12-normal" => {
                xdsopl_pctitl_qam64_r12_normal_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            "qam16-r12-short" => {
                xdsopl_pctitl_qam16_r12_short_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            "qam64-r12-short" => {
                xdsopl_pctitl_qam64_r12_short_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            other => panic!("unrecognized DVB-T2 MODCOD: {other}"),
        }
    }
}

unsafe extern "C" {
    fn xdsopl_pctitl_qam16_r12_normal_fwd(input: *const i32, output: *mut i32);
    fn xdsopl_pctitl_qam64_r12_normal_fwd(input: *const i32, output: *mut i32);
    fn xdsopl_pctitl_qam16_r12_short_fwd(input: *const i32, output: *mut i32);
    fn xdsopl_pctitl_qam64_r12_short_fwd(input: *const i32, output: *mut i32);
}
