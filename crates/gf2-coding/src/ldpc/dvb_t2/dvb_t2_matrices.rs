//! DVB-T2 LDPC base matrices from ETSI EN 302 755.
//!
//! This module contains the base matrices for DVB-T2 LDPC codes as defined in
//! ETSI EN 302 755 V1.4.1 (2015-07) Tables 6a-6f (short frames) and 7a-7f (normal frames).
//!
//! # Matrix Format
//!
//! Each base matrix is represented as a 2D array where:
//! - `-1` indicates an empty position (no circulant submatrix)
//! - `0..Z-1` indicates a circulant submatrix with that shift amount
//! - `Z` is the expansion factor (360 for DVB-T2)
//!
//! # Structure
//!
//! DVB-T2 uses irregular quasi-cyclic LDPC codes:
//! - **Short frames**: n=16200, Z=360, with 45 columns in base matrix
//! - **Normal frames**: n=64800, Z=360, with 180 columns in base matrix
//!
//! # Code Rates
//!
//! Both frame types support 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
//!
//! # References
//!
//! ETSI EN 302 755 V1.4.1: Digital Video Broadcasting (DVB); Frame structure
//! channel coding and modulation for a second generation digital terrestrial
//! television broadcasting system (DVB-T2)

/// DVB-T2 short frame (n=16200, Z=360) rate 1/2 base matrix.
///
/// From ETSI EN 302 755 Table 6a.
/// Dimensions: ~23 rows × 45 columns
///
/// Note: This is a placeholder. The actual DVB-T2 rate 1/2 short frame
/// has specific structure with systematic part and parity part.
/// TODO: Replace with actual Table 6a from ETSI EN 302 755.
pub const SHORT_RATE_1_2: &[[i16; 45]] = &[
    // TODO: Enter actual base matrix from ETSI EN 302 755 Table 6a
    // Placeholder: simplified structure for demonstration
    [
        0, 1, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    ],
];

/// DVB-T2 short frame (n=16200, Z=360) rate 3/5 base matrix.
///
/// From ETSI EN 302 755 Table 6b.
/// TODO: Add actual base matrix from standard.
pub const SHORT_RATE_3_5: &[[i16; 45]] = &[
    // TODO: Enter actual Table 6b
    [0; 45], // Placeholder
];

/// DVB-T2 short frame (n=16200, Z=360) rate 2/3 base matrix.
///
/// From ETSI EN 302 755 Table 6c.
/// TODO: Add actual base matrix from standard.
pub const SHORT_RATE_2_3: &[[i16; 45]] = &[
    // TODO: Enter actual Table 6c
    [0; 45], // Placeholder
];

/// DVB-T2 short frame (n=16200, Z=360) rate 3/4 base matrix.
///
/// From ETSI EN 302 755 Table 6d.
/// TODO: Add actual base matrix from standard.
pub const SHORT_RATE_3_4: &[[i16; 45]] = &[
    // TODO: Enter actual Table 6d
    [0; 45], // Placeholder
];

/// DVB-T2 short frame (n=16200, Z=360) rate 4/5 base matrix.
///
/// From ETSI EN 302 755 Table 6e.
/// TODO: Add actual base matrix from standard.
pub const SHORT_RATE_4_5: &[[i16; 45]] = &[
    // TODO: Enter actual Table 6e
    [0; 45], // Placeholder
];

/// DVB-T2 short frame (n=16200, Z=360) rate 5/6 base matrix.
///
/// From ETSI EN 302 755 Table 6f.
/// TODO: Add actual base matrix from standard.
pub const SHORT_RATE_5_6: &[[i16; 45]] = &[
    // TODO: Enter actual Table 6f
    [0; 45], // Placeholder
];

// Normal frames - n=64800, Z=360

/// DVB-T2 normal frame (n=64800, Z=360) rate 1/2 base matrix.
///
/// From ETSI EN 302 755 Table 7a.
/// Dimensions: ~90 rows × 180 columns
/// TODO: Add actual base matrix from standard.
pub const NORMAL_RATE_1_2: &[[i16; 180]] = &[
    // TODO: Enter actual Table 7a
    [0; 180], // Placeholder
];

/// DVB-T2 normal frame (n=64800, Z=360) rate 3/5 base matrix.
///
/// From ETSI EN 302 755 Table 7b.
/// TODO: Add actual base matrix from standard.
pub const NORMAL_RATE_3_5: &[[i16; 180]] = &[
    // TODO: Enter actual Table 7b
    [0; 180], // Placeholder
];

/// DVB-T2 normal frame (n=64800, Z=360) rate 2/3 base matrix.
///
/// From ETSI EN 302 755 Table 7c.
/// TODO: Add actual base matrix from standard.
pub const NORMAL_RATE_2_3: &[[i16; 180]] = &[
    // TODO: Enter actual Table 7c
    [0; 180], // Placeholder
];

/// DVB-T2 normal frame (n=64800, Z=360) rate 3/4 base matrix.
///
/// From ETSI EN 302 755 Table 7d.
/// TODO: Add actual base matrix from standard.
pub const NORMAL_RATE_3_4: &[[i16; 180]] = &[
    // TODO: Enter actual Table 7d
    [0; 180], // Placeholder
];

/// DVB-T2 normal frame (n=64800, Z=360) rate 4/5 base matrix.
///
/// From ETSI EN 302 755 Table 7e.
/// TODO: Add actual base matrix from standard.
pub const NORMAL_RATE_4_5: &[[i16; 180]] = &[
    // TODO: Enter actual Table 7e
    [0; 180], // Placeholder
];

/// DVB-T2 normal frame (n=64800, Z=360) rate 5/6 base matrix.
///
/// From ETSI EN 302 755 Table 7f.
/// TODO: Add actual base matrix from standard.
pub const NORMAL_RATE_5_6: &[[i16; 180]] = &[
    // TODO: Enter actual Table 7f
    [0; 180], // Placeholder
];
