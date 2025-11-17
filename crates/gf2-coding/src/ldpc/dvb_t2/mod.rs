//! DVB-T2 LDPC code construction.
//!
//! This module provides factory methods for creating DVB-T2 standard LDPC codes
//! as defined in ETSI EN 302 755.
//!
//! DVB-T2 uses quasi-cyclic LDPC codes with two frame sizes:
//! - **Short frames**: n=16200, Z=360
//! - **Normal frames**: n=64800, Z=360
//!
//! Both support 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6

use super::super::QuasiCyclicLdpc;
use crate::bch::CodeRate;

mod dvb_t2_matrices;

/// Helper to convert const base matrix slice to Vec<Vec<i32>>.
fn matrix_to_vec<const N: usize>(matrix: &[[i16; N]]) -> Vec<Vec<i32>> {
    matrix
        .iter()
        .map(|row| row.iter().map(|&x| x as i32).collect())
        .collect()
}

impl QuasiCyclicLdpc {
    /// Creates a DVB-T2 short frame LDPC code.
    ///
    /// DVB-T2 short frames have 16200 bits with expansion factor Z=360.
    ///
    /// # Arguments
    ///
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
    /// use gf2_coding::CodeRate;
    ///
    /// let qc = QuasiCyclicLdpc::dvb_t2_short(CodeRate::Rate1_2);
    /// let code = LdpcCode::from_quasi_cyclic(&qc);
    ///
    /// assert_eq!(code.n(), 16200);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the requested code rate base matrix is not yet implemented.
    ///
    /// # Note
    ///
    /// Base matrices from ETSI EN 302 755 Tables 6a-6f.
    /// Currently only rate 1/2 has a placeholder - others need actual standard matrices.
    pub fn dvb_t2_short(rate: CodeRate) -> Self {
        use dvb_t2_matrices::*;

        let base_matrix = match rate {
            CodeRate::Rate1_2 => matrix_to_vec(SHORT_RATE_1_2),
            CodeRate::Rate3_5 => matrix_to_vec(SHORT_RATE_3_5),
            CodeRate::Rate2_3 => matrix_to_vec(SHORT_RATE_2_3),
            CodeRate::Rate3_4 => matrix_to_vec(SHORT_RATE_3_4),
            CodeRate::Rate4_5 => matrix_to_vec(SHORT_RATE_4_5),
            CodeRate::Rate5_6 => matrix_to_vec(SHORT_RATE_5_6),
        };

        Self::new(base_matrix, 360)
    }

    /// Creates a DVB-T2 normal frame LDPC code.
    ///
    /// DVB-T2 normal frames have 64800 bits with expansion factor Z=360.
    ///
    /// # Arguments
    ///
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Panics
    ///
    /// Panics if the requested code rate base matrix is not yet implemented.
    ///
    /// # Note
    ///
    /// Base matrices from ETSI EN 302 755 Tables 7a-7f.
    /// TODO: All normal frame matrices need to be entered from the standard.
    pub fn dvb_t2_normal(rate: CodeRate) -> Self {
        use dvb_t2_matrices::*;

        let base_matrix = match rate {
            CodeRate::Rate1_2 => matrix_to_vec(NORMAL_RATE_1_2),
            CodeRate::Rate3_5 => matrix_to_vec(NORMAL_RATE_3_5),
            CodeRate::Rate2_3 => matrix_to_vec(NORMAL_RATE_2_3),
            CodeRate::Rate3_4 => matrix_to_vec(NORMAL_RATE_3_4),
            CodeRate::Rate4_5 => matrix_to_vec(NORMAL_RATE_4_5),
            CodeRate::Rate5_6 => matrix_to_vec(NORMAL_RATE_5_6),
        };

        Self::new(base_matrix, 360)
    }
}
