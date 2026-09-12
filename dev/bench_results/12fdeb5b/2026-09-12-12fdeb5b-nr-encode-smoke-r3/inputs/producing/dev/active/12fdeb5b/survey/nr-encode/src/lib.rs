//! Shared pieces of the 5G NR rate-matched encoder comparison.
//!
//! The compared operation is the whole consumer: `target_k` information bits
//! in, `target_n` rate-matched codeword bits out. gf2 exposes it as
//! `<Nr5gRateMatchedCode as BlockEncoder>::encode`, which fuses 3GPP TS
//! 38.212 Section 5.4.2.1 bit selection into the encode as a gather. AFF3CT
//! exposes the two halves separately as `Encoder::encode` on
//! `Encoder_LDPC_QC_fast` and `Puncturer::puncture` on `Puncturer_5G`.
//! srsRAN exposes them as `ldpc_encoder::encode` and
//! `ldpc_rate_matcher::rate_match`.
//!
//! The consumer representation is gf2's: `&BitVec` in, `BitVec` out. Each
//! external adapter converts into its project's representation and back
//! inside its own timed call, so the comparison charges every conversion to
//! the arm that needs it. AFF3CT works on one `int32_t` per bit; srsRAN works
//! on bits packed most-significant-bit first inside `uint8_t` words, while
//! gf2's `BitVec` packs least-significant-bit first inside `u64` words, so
//! the srsRAN adapter's conversion is a per-byte bit reversal.

use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_core::BitVec;
use tuning_campaign_support::abtest::SplitMix64;

/// One encoder configuration: gf2's base graph and target lengths, and the
/// redundancy version the arms select from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub name: &'static str,
    pub base_graph: u8,
    pub target_n: usize,
    pub target_k: usize,
    /// Redundancy version, as TS 38.212 Table 5.4.2.1-2 indexes it.
    pub redundancy_version: u8,
}

const fn configuration(
    name: &'static str,
    base_graph: u8,
    target_n: usize,
    target_k: usize,
) -> Configuration {
    Configuration {
        name,
        base_graph,
        target_n,
        target_k,
        redundancy_version: 0,
    }
}

const fn with_rv(mut configuration: Configuration, redundancy_version: u8) -> Configuration {
    configuration.redundancy_version = redundancy_version;
    configuration
}

/// Every configuration the validation record covers, in report order.
///
/// The grid spans both base graphs, the rates 1/5 to 5/6 that TS 38.212
/// Section 7.2.2 assigns them, lifting sizes from the smallest a base graph
/// admits to the largest, codes with and without filler bits, and the
/// redundancy versions 1 to 3 that only one arm implements.
pub const CONFIGURATIONS: [Configuration; 18] = [
    // Base graph 2: short blocks, low rates, every lifting set boundary the
    // grid reaches.
    configuration("bg2-n256-k121", 2, 256, 121),
    configuration("bg2-n512-k200", 2, 512, 200),
    configuration("bg2-n1024-k400", 2, 1024, 400),
    configuration("bg2-n1200-k600", 2, 1200, 600),
    configuration("bg2-n1440-k720", 2, 1440, 720),
    configuration("bg2-n3840-k1280", 2, 3840, 1280),
    configuration("bg2-n100-k40", 2, 100, 40),
    configuration("bg2-n5000-k2500", 2, 5000, 2500),
    // Base graph 1: long blocks, rates 1/3 to 5/6.
    configuration("bg1-n1024-k512", 1, 1024, 512),
    configuration("bg1-n2048-k1024", 1, 2048, 1024),
    configuration("bg1-n4096-k2048", 1, 4096, 2048),
    configuration("bg1-n1200-k900", 1, 1200, 900),
    configuration("bg1-n400-k308", 1, 400, 308),
    configuration("bg1-n1320-k1056", 1, 1320, 1056),
    configuration("bg1-n2560-k2048", 1, 2560, 2048),
    configuration("bg1-n8448-k4224", 1, 8448, 4224),
    // Redundancy versions other than 0.
    with_rv(configuration("bg1-n2560-k2048-rv1", 1, 2560, 2048), 1),
    with_rv(configuration("bg2-n1024-k400-rv2", 2, 1024, 400), 2),
];

/// Looks up a configuration by name.
///
/// # Panics
///
/// Panics when `name` is not in [`CONFIGURATIONS`].
pub fn configuration_named(name: &str) -> Configuration {
    CONFIGURATIONS
        .iter()
        .copied()
        .find(|candidate| candidate.name == name)
        .unwrap_or_else(|| panic!("unknown NR encoder configuration {name:?}"))
}

/// gf2's rate-matched code for one configuration.
pub fn gf2_code(configuration: Configuration) -> Nr5gRateMatchedCode {
    QuasiCyclicLdpc::nr_5g_rate_matched(
        configuration.base_graph,
        configuration.target_n,
        configuration.target_k,
    )
}

/// Expands one seed into `banks` messages of `len` bits.
pub fn seeded_messages(seed: u64, banks: usize, len: usize) -> Vec<BitVec> {
    let mut mixer = SplitMix64::new(seed);
    (0..banks)
        .map(|_| {
            let mut message = BitVec::zeros(len);
            for index in 0..len {
                message.set(index, mixer.next_u64() & 1 == 1);
            }
            message
        })
        .collect()
}

/// Packs `bits` into `len_bits` bits with the most significant bit of each
/// byte first, the layout srsRAN's `bit_buffer` uses, zero-padding the
/// positions from `bits.len()` to `len_bits`.
///
/// # Panics
///
/// Panics when `bits` is longer than `len_bits`.
pub fn to_msb_first_bytes(bits: &BitVec, len_bits: usize) -> Vec<u8> {
    assert!(
        bits.len() <= len_bits,
        "cannot pack {} bits into {len_bits}",
        bits.len()
    );
    let mut bytes = bits.to_bytes_le();
    for byte in bytes.iter_mut() {
        *byte = byte.reverse_bits();
    }
    bytes.resize(len_bits.div_ceil(8), 0);
    bytes
}

/// Reads `len_bits` bits packed most significant bit first from `bytes`.
///
/// # Panics
///
/// Panics when `bytes` is shorter than `len_bits` bits need.
pub fn from_msb_first_bytes(bytes: &[u8], len_bits: usize) -> BitVec {
    assert!(
        bytes.len() >= len_bits.div_ceil(8),
        "{} bytes cannot hold {len_bits} bits",
        bytes.len()
    );
    let reversed: Vec<u8> = bytes.iter().map(|byte| byte.reverse_bits()).collect();
    let mut bits = BitVec::from_bytes_le(&reversed);
    bits.resize(len_bits, false);
    bits
}

/// The positions at which two bit vectors of equal length agree.
///
/// # Panics
///
/// Panics when the lengths differ.
pub fn identical_bits(left: &BitVec, right: &BitVec) -> usize {
    assert_eq!(left.len(), right.len(), "lengths must match");
    (0..left.len())
        .filter(|&index| left.get(index) == right.get(index))
        .count()
}

#[cfg(feature = "aff3ct")]
pub mod aff3ct;
#[cfg(feature = "srsran")]
pub mod srsran;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_messages_are_deterministic_and_seed_selected() {
        let first = seeded_messages(7, 2, 65);
        assert_eq!(first, seeded_messages(7, 2, 65));
        assert_ne!(first[0], first[1], "banks draw distinct messages");
        assert_ne!(
            first,
            seeded_messages(8, 2, 65),
            "the seed selects the stream"
        );
    }

    #[test]
    fn byte_order_conversion_round_trips_at_word_boundaries() {
        for len in [0usize, 1, 7, 8, 9, 63, 64, 65, 121, 256] {
            let bits = seeded_messages(11, 1, len).pop().expect("one bank");
            let bytes = to_msb_first_bytes(&bits, len);
            assert_eq!(bytes.len(), len.div_ceil(8));
            assert_eq!(
                from_msb_first_bytes(&bytes, len),
                bits,
                "round trip at {len}"
            );
        }
    }

    #[test]
    fn packing_into_a_longer_buffer_zero_fills_the_tail() {
        let bits = seeded_messages(13, 1, 12).pop().expect("one bank");
        let bytes = to_msb_first_bytes(&bits, 64);
        assert_eq!(bytes.len(), 8);
        let read_back = from_msb_first_bytes(&bytes, 64);
        assert_eq!(read_back.len(), 64);
        for index in 12..64 {
            assert!(!read_back.get(index), "position {index} is zero filled");
        }
    }

    #[test]
    fn msb_first_packing_puts_bit_zero_in_the_top_bit() {
        let mut bits = BitVec::zeros(8);
        bits.set(0, true);
        assert_eq!(to_msb_first_bytes(&bits, 8), vec![0x80]);
    }

    #[test]
    fn configuration_names_are_unique() {
        for (index, configuration) in CONFIGURATIONS.iter().enumerate() {
            assert_eq!(configuration_named(configuration.name), *configuration);
            assert!(CONFIGURATIONS[..index]
                .iter()
                .all(|earlier| earlier.name != configuration.name));
        }
    }
}
