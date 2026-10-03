//! MSB-first bit-label packing shared by the modem backends and tests.

/// Length-checks a mapper batch and returns the number of symbols.
///
/// # Panics
///
/// Panics, naming `mapper_name`, if `bits_len` is not a multiple of
/// `bits_per_symbol` or an output length differs from the symbol count.
#[inline]
pub(crate) fn check_batch_lengths(
    mapper_name: &str,
    bits_per_symbol: u8,
    bits_len: usize,
    out_i_len: usize,
    out_q_len: usize,
) -> usize {
    let m = bits_per_symbol as usize;
    assert!(
        bits_len.is_multiple_of(m),
        "{mapper_name}: bits length {bits_len} is not a multiple of bits_per_symbol {m}"
    );
    let num_symbols = bits_len / m;
    assert!(
        out_i_len == num_symbols,
        "{mapper_name}: out_i length {out_i_len} does not match expected {num_symbols}"
    );
    assert!(
        out_q_len == num_symbols,
        "{mapper_name}: out_q length {out_q_len} does not match expected {num_symbols}"
    );
    num_symbols
}

/// Assembles an MSB-first `u16` label; `symbol_bits[0]` is the most
/// significant bit of the label.
#[inline]
pub(crate) fn pack_label_msb_first(symbol_bits: &[bool]) -> u16 {
    let mut label: u16 = 0;
    for &b in symbol_bits {
        label = (label << 1) | u16::from(b);
    }
    label
}

/// Returns bit `bit_idx` of a `bits_per_symbol`-bit label as `0` or `1`,
/// where `bit_idx = 0` is the MSB.
#[inline]
pub(crate) fn bit_at_msb_first(label: u16, bit_idx: u8, bits_per_symbol: u8) -> u16 {
    let shift = bits_per_symbol - 1 - bit_idx;
    (label >> shift) & 1
}

/// Explodes a `u16` label into an MSB-first `Vec<bool>` of length `m`;
/// inverse of `pack_label_msb_first`.
#[inline]
pub fn unpack_label_msb_first(label: u16, m: u8) -> Vec<bool> {
    (0..m).map(|k| bit_at_msb_first(label, k, m) == 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_label_roundtrip() {
        for m in 1u8..=8 {
            for v in 0u16..(1 << m) {
                let bits = unpack_label_msb_first(v, m);
                assert_eq!(bits.len(), m as usize);
                assert_eq!(pack_label_msb_first(&bits), v);
            }
        }
    }

    #[test]
    fn test_pack_label_msb_ordering() {
        let bits = vec![true, false, false, false];
        assert_eq!(pack_label_msb_first(&bits), 0b1000);
    }

    #[test]
    fn test_bit_at_msb_first_matches_unpack() {
        for m in 1u8..=8 {
            for v in 0u16..(1 << m) {
                let unpacked = unpack_label_msb_first(v, m);
                for k in 0..m {
                    assert_eq!(
                        bit_at_msb_first(v, k, m),
                        u16::from(unpacked[k as usize]),
                        "mismatch at v={v} m={m} k={k}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_check_batch_lengths_happy_path() {
        let n = check_batch_lengths("Test::map_bits", 4, 16, 4, 4);
        assert_eq!(n, 4);
    }

    #[test]
    #[should_panic(
        expected = "Test::map_bits: bits length 7 is not a multiple of bits_per_symbol 4"
    )]
    fn test_check_batch_lengths_bits_panics() {
        check_batch_lengths("Test::map_bits", 4, 7, 1, 1);
    }

    #[test]
    #[should_panic(expected = "Test::map_bits: out_i length 3 does not match expected 4")]
    fn test_check_batch_lengths_out_i_panics() {
        check_batch_lengths("Test::map_bits", 4, 16, 3, 4);
    }

    #[test]
    #[should_panic(expected = "Test::map_bits: out_q length 5 does not match expected 4")]
    fn test_check_batch_lengths_out_q_panics() {
        check_batch_lengths("Test::map_bits", 4, 16, 4, 5);
    }
}
