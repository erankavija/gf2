//! Isolated check-node-update arms of the min-sum change (jit:07ca8585).
//!
//! REQ-10 asks for the matched comparison at three granularities. Whole
//! decoding is the `3be770d5` steady-state operation, which this issue's
//! before/after and comparator families measure. This crate supplies the
//! isolated check-node granularity: one flooding check-node pass over a
//! prepared variable-to-check message array, timed on each side through the
//! path that side's production decoder uses, with no variable update, no
//! termination rule and no conversion in the timed call.
//!
//! [`prepare`] derives the prepared array both arms read. It is one function,
//! called by both arms, so the two arms cannot prepare different messages: the
//! array is a declared deterministic function of the frozen recorded LLRs and
//! the graph, and each arm reports the checksum of the array it prepared and of
//! the outputs it wrote, in the canonical check-major edge order, so a mismatch
//! fails the arm rather than being published.
//!
//! [`cell`] runs the timed cell. It is the `3be770d5` driver's shape without
//! the decision check, because an isolated kernel decodes nothing: the
//! per-frame quality evidence a whole decode is checked against does not exist
//! for it. The declared cell is `kernel-isolated` with no decoder block, so the
//! runner asks the arms for no quality.

pub mod cell;
pub mod prepare;

#[cfg(feature = "aff3ct")]
pub mod aff3ct_update;

/// The 64-bit FNV-1a offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// The 64-bit FNV-1a prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over the little-endian bytes of `values`.
///
/// The checksum identifies a message array by its exact bit pattern; it is a
/// declared 64-bit hash rather than a cryptographic digest, because it only has
/// to catch a mismatch between two arms of one cell.
#[must_use]
pub fn checksum(values: &[f32]) -> u64 {
    let mut hash = FNV_OFFSET;
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// The checksum of `values` as sixteen lower-case hex digits.
#[must_use]
pub fn checksum_hex(values: &[f32]) -> String {
    format!("{:016x}", checksum(values))
}
