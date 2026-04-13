//! Shared MSB-first bit-label packing helpers.
//!
//! Used by every `BatchMapper` implementation that assembles a `u16`
//! label from a contiguous run of `bool` bits, and by every test that
//! explodes a `u16` label into such a run. Keeping this in one place
//! enforces a single source of truth for the canonical MSB-first,
//! symbol-major ordering documented at the modem trait layer.
//!
//! This module is crate-private (`pub(crate)`).

/// Length-checks a mapper batch and returns the number of symbols.
///
/// All `BatchMapper` implementations for `ModemScalar` impose the same
/// three length constraints: `bits.len()` must be a multiple of
/// `bits_per_symbol`, and both output slices must have that many
/// entries. Centralizing the assertion keeps the panic messages
/// identical in shape across backends.
///
/// # Panics
///
/// Panics with a descriptive message if any constraint is violated.
/// `mapper_name` is embedded in the panic message (for example
/// `"GrayQamMapper::map_bits"`) so callers can still tell which backend
/// triggered the check.
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
        bits_len % m == 0,
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

/// Assembles an MSB-first `u16` label from a slice of bits.
///
/// The slice must have length `bits_per_symbol`; bit at index 0 is the
/// most significant bit of the returned label. This matches the
/// canonical modem bit ordering (MSB-first within each symbol,
/// symbol-major across symbols).
///
/// The returned `u16` has the packed label in its low `bits_per_symbol`
/// bits; higher bits are zero.
///
/// # Complexity
///
/// O(bits_per_symbol).
#[inline]
pub(crate) fn pack_label_msb_first(symbol_bits: &[bool]) -> u16 {
    let mut label: u16 = 0;
    for &b in symbol_bits {
        label = (label << 1) | u16::from(b);
    }
    label
}

/// Explodes a `u16` label into an MSB-first `Vec<bool>` of length `m`.
///
/// Used by tests and examples to construct synthetic bit inputs for
/// `BatchMapper` implementations. Inverse of [`pack_label_msb_first`].
///
/// This is a testing/utility helper — not a core part of the public
/// modem surface — and is re-exported from `gf2_coding::modem` as a
/// doc-hidden item so integration tests and internal property tests
/// share a single implementation.
///
/// # Complexity
///
/// O(`m`).
#[inline]
pub fn unpack_label_msb_first(label: u16, m: u8) -> Vec<bool> {
    (0..m).map(|k| ((label >> (m - 1 - k)) & 1) == 1).collect()
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
        // bit index 0 (first entry) should be the MSB.
        let bits = vec![true, false, false, false];
        assert_eq!(pack_label_msb_first(&bits), 0b1000);
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
