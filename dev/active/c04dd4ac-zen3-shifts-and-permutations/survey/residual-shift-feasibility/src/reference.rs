//! Bit-addressed zero-fill reference, independent of every word funnel under
//! test: it reads and writes one bit at a time and shares no code with the
//! forms it judges.

fn get_bit(data: &[u64], i: usize) -> bool {
    data[i / 64] >> (i % 64) & 1 == 1
}

fn set_bit(data: &mut [u64], i: usize) {
    data[i / 64] |= 1u64 << (i % 64);
}

/// Bit `i` of the result is bit `i - k` of the input; lower bits are zero.
pub fn shift_left(data: &mut [u64], len_bits: usize, k: usize) {
    let mut out = vec![0u64; data.len()];
    for i in k..len_bits {
        if get_bit(data, i - k) {
            set_bit(&mut out, i);
        }
    }
    data.copy_from_slice(&out);
}

/// Bit `i` of the result is bit `i + k` of the input; higher bits are zero.
pub fn shift_right(data: &mut [u64], len_bits: usize, k: usize) {
    let mut out = vec![0u64; data.len()];
    for i in 0..len_bits.saturating_sub(k) {
        if get_bit(data, i + k) {
            set_bit(&mut out, i);
        }
    }
    data.copy_from_slice(&out);
}
