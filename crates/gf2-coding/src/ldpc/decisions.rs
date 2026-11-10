//! Hard decisions packed into canonical bit words.

/// Bit 0 of each little-endian byte of `bytes`, byte `i` to bit `i`.
#[inline]
pub(super) fn pack_eight(bytes: u64) -> u64 {
    // Byte i's bit lands at product bit 56 + i.
    (bytes & 0x0101_0101_0101_0101).wrapping_mul(0x0102_0408_1020_4080) >> 56
}

/// Bit 0 of each byte of `bits`, packed in canonical order: byte `i` becomes
/// bit `i & 63` of word `i >> 6`. Replaces the contents of `words`; bits past
/// `bits.len()` are zero.
pub(super) fn pack_bits(bits: &[u8], words: &mut Vec<u64>) {
    words.clear();
    words.extend(bits.chunks(64).map(|chunk| {
        chunk.chunks(8).enumerate().fold(0, |word, (byte, eight)| {
            let mut lanes = [0u8; 8];
            lanes[..eight.len()].copy_from_slice(eight);
            word | pack_eight(u64::from_le_bytes(lanes)) << (8 * byte)
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_pack_to_canonical_positions_with_a_zero_tail() {
        for len in [0usize, 1, 7, 8, 9, 63, 64, 65, 130] {
            // Every bit of every byte varies, so only bit 0 decides.
            let bits: Vec<u8> = (0..len).map(|i| (i * 37 + i / 5) as u8).collect();
            let mut words = vec![u64::MAX; 9];
            pack_bits(&bits, &mut words);
            assert_eq!(words.len(), len.div_ceil(64));
            for i in 0..words.len() * 64 {
                let want = i < len && bits[i] & 1 == 1;
                assert_eq!(
                    words[i >> 6] >> (i & 63) & 1 == 1,
                    want,
                    "len {len} bit {i}"
                );
            }
        }
    }
}
