//! Hard decisions packed into canonical bit words.

/// Bit `shift` of each little-endian byte of `bytes`, byte `i` to bit `i`.
#[inline]
pub(super) fn pack_eight(bytes: u64, shift: u32) -> u64 {
    // Byte i's bit lands at product bit 56 + i.
    ((bytes >> shift) & 0x0101_0101_0101_0101).wrapping_mul(0x0102_0408_1020_4080) >> 56
}

/// Bit `shift` of `to_byte(values[i])` for each `i`, packed in canonical order:
/// value `i` becomes bit `i & 63` of word `i >> 6`. Replaces the contents of
/// `words`; bits past `values.len()` are zero.
pub(super) fn pack_bytes<T: Copy>(
    values: &[T],
    to_byte: impl Fn(T) -> u8,
    shift: u32,
    words: &mut Vec<u64>,
) {
    words.clear();
    words.extend(values.chunks(64).map(|chunk| {
        chunk.chunks(8).enumerate().fold(0, |word, (byte, eight)| {
            let mut lanes = [0u8; 8];
            for (lane, &value) in lanes.iter_mut().zip(eight) {
                *lane = to_byte(value);
            }
            word | pack_eight(u64::from_le_bytes(lanes), shift) << (8 * byte)
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_pack_to_canonical_bit_positions_with_a_zero_tail() {
        for len in [0usize, 1, 7, 8, 9, 63, 64, 65, 130] {
            for shift in [0, 7] {
                // Every bit of every byte varies, so only bit `shift` decides.
                let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + i / 5) as u8).collect();
                let mut words = vec![u64::MAX; 9];
                pack_bytes(&bytes, |b| b, shift, &mut words);
                assert_eq!(words.len(), len.div_ceil(64));
                for i in 0..words.len() * 64 {
                    let want = i < len && bytes[i] >> shift & 1 == 1;
                    assert_eq!(
                        words[i >> 6] >> (i & 63) & 1 == 1,
                        want,
                        "len {len} shift {shift} bit {i}"
                    );
                }
            }
        }
    }
}
