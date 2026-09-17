//! Portable residual funnel: the fallback every capability-gated form falls
//! back to, and the shape `BitVec::shift_left` / `shift_right` run today.

/// Writes `data[ws + 1 ..]` as `(data[i - ws] << b) | (data[i - ws - 1] >> (64 - b))`.
/// `b` lies in `1..64`, so the complement shift never reaches 64.
pub fn shift_left_funnel(data: &mut [u64], ws: usize, b: u32) {
    let inv = 64 - b;
    for i in (ws + 1..data.len()).rev() {
        data[i] = (data[i - ws] << b) | (data[i - ws - 1] >> inv);
    }
}

/// Writes `data[.. len - ws - 1]` as `(data[i + ws] >> b) | (data[i + ws + 1] << (64 - b))`.
pub fn shift_right_funnel(data: &mut [u64], ws: usize, b: u32) {
    let inv = 64 - b;
    for i in 0..data.len() - ws - 1 {
        data[i] = (data[i + ws] >> b) | (data[i + ws + 1] << inv);
    }
}
