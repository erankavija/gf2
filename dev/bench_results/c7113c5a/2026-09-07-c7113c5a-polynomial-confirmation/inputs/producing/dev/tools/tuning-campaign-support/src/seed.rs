//! Deterministic fixture seed derivation.

/// Root for the a835 extent fixture streams.
pub const EXTENT_SEED_ROOT: u64 = 0x5ecc_9bf8_0000_0000;
/// Stable name of the retained mixer.
pub const SEED_DERIVATION: &str = "gf2-calibration-seed-v1";

/// Adds a fixture bank to a role in the protocol's reserved high bits.
pub const fn bank_role(role: u64, bank: usize) -> u64 {
    role.wrapping_add((bank as u64) << 16)
}

/// Mixes a root, field tag, shape key, and role exactly as preregistered.
pub const fn fixture_seed(seed_root: u64, field_tag: u64, shape_key: u64, role: u64) -> u64 {
    let mut value = seed_root ^ role.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let words = [field_tag, shape_key];
    let mut index = 0;
    while index < words.len() {
        value ^= words[index];
        value = value
            .wrapping_mul(0xbf58_476d_1ce4_e5b9)
            .rotate_left(27)
            .wrapping_add(0x94d0_49bb_1331_11eb);
        index += 1;
    }
    value ^ (value >> 31)
}
