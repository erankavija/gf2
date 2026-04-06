//! 5G NR LDPC lifting size table from 3GPP TS 38.212 Table 5.3.2-1.
//!
//! The lifting size Z determines the expansion factor for the quasi-cyclic
//! LDPC code. Each Z belongs to one of 8 sets (i_LS = 0..7), which determines
//! which column of the base matrix shift table to use.

/// All valid lifting sizes from 3GPP TS 38.212 Table 5.3.2-1.
///
/// Organized by set index i_LS (0..7). Each set contains Z values of the form
/// `a * 2^j` where `a` is the set's base factor.
///
/// | i_LS | Base factor a | Z values |
/// |------|--------------|----------|
/// | 0 | 2 | 2, 4, 8, 16, 32, 64, 128, 256 |
/// | 1 | 3 | 3, 6, 12, 24, 48, 96, 192, 384 |
/// | 2 | 5 | 5, 10, 20, 40, 80, 160, 320 |
/// | 3 | 7 | 7, 14, 28, 56, 112, 224 |
/// | 4 | 9 | 9, 18, 36, 72, 144, 288 |
/// | 5 | 11 | 11, 22, 44, 88, 176, 352 |
/// | 6 | 13 | 13, 26, 52, 104, 208 |
/// | 7 | 15 | 15, 30, 60, 120, 240 |
pub const LIFTING_SIZE_SETS: &[&[u16]] = &[
    // i_LS = 0: a = 2
    &[2, 4, 8, 16, 32, 64, 128, 256],
    // i_LS = 1: a = 3
    &[3, 6, 12, 24, 48, 96, 192, 384],
    // i_LS = 2: a = 5
    &[5, 10, 20, 40, 80, 160, 320],
    // i_LS = 3: a = 7
    &[7, 14, 28, 56, 112, 224],
    // i_LS = 4: a = 9
    &[9, 18, 36, 72, 144, 288],
    // i_LS = 5: a = 11
    &[11, 22, 44, 88, 176, 352],
    // i_LS = 6: a = 13
    &[13, 26, 52, 104, 208],
    // i_LS = 7: a = 15
    &[15, 30, 60, 120, 240],
];

/// Returns the set index i_LS for a given lifting size Z.
///
/// # Arguments
///
/// * `z` - Lifting size
///
/// # Returns
///
/// The set index (0..7) if Z is a valid lifting size, or `None` otherwise.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::lifting_set_index;
///
/// assert_eq!(lifting_set_index(2), Some(0));
/// assert_eq!(lifting_set_index(384), Some(1));
/// assert_eq!(lifting_set_index(320), Some(2));
/// assert_eq!(lifting_set_index(7), Some(3));
/// assert_eq!(lifting_set_index(100), None); // Not a valid lifting size
/// ```
pub fn lifting_set_index(z: u16) -> Option<usize> {
    for (i_ls, set) in LIFTING_SIZE_SETS.iter().enumerate() {
        if set.contains(&z) {
            return Some(i_ls);
        }
    }
    None
}

/// Returns all valid lifting sizes in ascending order.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::all_lifting_sizes;
///
/// let sizes = all_lifting_sizes();
/// assert_eq!(sizes[0], 2);
/// assert_eq!(*sizes.last().unwrap(), 384);
/// assert_eq!(sizes.len(), 51);
/// ```
///
/// # Complexity
///
/// O(1) — returns a static slice.
pub fn all_lifting_sizes() -> Vec<u16> {
    let mut sizes: Vec<u16> = LIFTING_SIZE_SETS
        .iter()
        .flat_map(|s| s.iter().copied())
        .collect();
    sizes.sort_unstable();
    sizes
}

/// Checks whether a given Z is a valid 5G NR lifting size.
///
/// # Arguments
///
/// * `z` - Candidate lifting size
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::is_valid_lifting_size;
///
/// assert!(is_valid_lifting_size(384));
/// assert!(is_valid_lifting_size(2));
/// assert!(!is_valid_lifting_size(100));
/// assert!(!is_valid_lifting_size(0));
/// ```
pub fn is_valid_lifting_size(z: u16) -> bool {
    lifting_set_index(z).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifting_set_index_set0() {
        // i_LS = 0: powers of 2
        for &z in &[2, 4, 8, 16, 32, 64, 128, 256] {
            assert_eq!(lifting_set_index(z), Some(0), "Z={z} should be in set 0");
        }
    }

    #[test]
    fn test_lifting_set_index_set1() {
        // i_LS = 1: multiples of 3 * powers of 2
        for &z in &[3, 6, 12, 24, 48, 96, 192, 384] {
            assert_eq!(lifting_set_index(z), Some(1), "Z={z} should be in set 1");
        }
    }

    #[test]
    fn test_lifting_set_index_all_sets() {
        // Spot check each set
        assert_eq!(lifting_set_index(5), Some(2));
        assert_eq!(lifting_set_index(320), Some(2));
        assert_eq!(lifting_set_index(7), Some(3));
        assert_eq!(lifting_set_index(224), Some(3));
        assert_eq!(lifting_set_index(9), Some(4));
        assert_eq!(lifting_set_index(288), Some(4));
        assert_eq!(lifting_set_index(11), Some(5));
        assert_eq!(lifting_set_index(352), Some(5));
        assert_eq!(lifting_set_index(13), Some(6));
        assert_eq!(lifting_set_index(208), Some(6));
        assert_eq!(lifting_set_index(15), Some(7));
        assert_eq!(lifting_set_index(240), Some(7));
    }

    #[test]
    fn test_lifting_set_index_invalid() {
        assert_eq!(lifting_set_index(0), None);
        assert_eq!(lifting_set_index(1), None);
        assert_eq!(lifting_set_index(100), None);
        assert_eq!(lifting_set_index(385), None);
        assert_eq!(lifting_set_index(17), None);
    }

    #[test]
    fn test_all_lifting_sizes_count() {
        let sizes = all_lifting_sizes();
        // 8 + 8 + 7 + 6 + 6 + 6 + 5 + 5 = 51
        assert_eq!(sizes.len(), 51);
    }

    #[test]
    fn test_all_lifting_sizes_sorted() {
        let sizes = all_lifting_sizes();
        for w in sizes.windows(2) {
            assert!(
                w[0] < w[1],
                "Sizes must be strictly ascending: {} >= {}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn test_all_lifting_sizes_range() {
        let sizes = all_lifting_sizes();
        assert_eq!(sizes[0], 2);
        assert_eq!(*sizes.last().unwrap(), 384);
    }

    #[test]
    fn test_is_valid_lifting_size() {
        assert!(is_valid_lifting_size(2));
        assert!(is_valid_lifting_size(384));
        assert!(is_valid_lifting_size(52));
        assert!(!is_valid_lifting_size(0));
        assert!(!is_valid_lifting_size(1));
        assert!(!is_valid_lifting_size(100));
    }

    #[test]
    fn test_no_duplicate_lifting_sizes() {
        let sizes = all_lifting_sizes();
        let mut unique = sizes.clone();
        unique.dedup();
        assert_eq!(
            sizes.len(),
            unique.len(),
            "No duplicates allowed across sets"
        );
    }
}
