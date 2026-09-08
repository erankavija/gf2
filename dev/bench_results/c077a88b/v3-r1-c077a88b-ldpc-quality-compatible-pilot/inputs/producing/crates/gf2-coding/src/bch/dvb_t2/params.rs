//! DVB-T2 BCH code parameters from ETSI EN 302 755.

use crate::CodeRate;

/// DVB-T2 frame size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSize {
    /// Short frame: n_ldpc = 16200, BCH uses GF(2^14)
    Short,
    /// Normal frame: n_ldpc = 64800, BCH uses GF(2^16)
    Normal,
}

/// DVB-T2 BCH code parameters.
///
/// BCH codes in DVB-T2 are outer codes that protect LDPC information bits.
/// The BCH codeword length (n) equals the LDPC information length (k_ldpc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DvbBchParams {
    /// BCH codeword length (matches LDPC k)
    pub n: usize,
    /// BCH information bits (Kbch in standard)
    pub k: usize,
    /// BCH parity bits (n - k)
    pub m: usize,
    /// Error correction capability
    pub t: usize,
    /// Extension field degree
    pub field_m: usize,
    /// Primitive polynomial (binary representation)
    pub primitive_poly: u64,
}

impl DvbBchParams {
    /// Get DVB-T2 BCH parameters for a given configuration.
    ///
    /// # Arguments
    ///
    /// * `frame_size` - Short (16200) or Normal (64800) LDPC frame
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::dvb_t2::{DvbBchParams, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
    /// assert_eq!(params.n, 32400);  // BCH output = k_ldpc
    /// assert_eq!(params.k, 32208);  // BCH input = Kbch
    /// assert_eq!(params.m, 192);    // BCH parity
    /// assert_eq!(params.t, 12);
    /// ```
    pub fn for_code(frame_size: FrameSize, rate: CodeRate) -> Self {
        let (n, k, t, field_m, primitive_poly) = match (frame_size, rate) {
            // Short frames: GF(2^14), x^14 + x^5 + x^3 + x + 1, t=12 for all rates
            // n = k_ldpc (BCH output = LDPC input), k = Kbch, m = BCH parity = 168
            // From ETSI EN 302 755 Table 6a
            (FrameSize::Short, CodeRate::Rate1_2) => (7200, 7032, 12, 14, 0b100000000101011),
            (FrameSize::Short, CodeRate::Rate3_5) => (9720, 9552, 12, 14, 0b100000000101011),
            (FrameSize::Short, CodeRate::Rate2_3) => (10800, 10632, 12, 14, 0b100000000101011),
            (FrameSize::Short, CodeRate::Rate3_4) => (11880, 11712, 12, 14, 0b100000000101011),
            (FrameSize::Short, CodeRate::Rate4_5) => (12600, 12432, 12, 14, 0b100000000101011),
            (FrameSize::Short, CodeRate::Rate5_6) => (13320, 13152, 12, 14, 0b100000000101011),

            // Normal frames: GF(2^16), x^16 + x^5 + x^3 + x^2 + 1, t=12 or t=10
            // n = k_ldpc (BCH output = LDPC input), k = Kbch, m = BCH parity = 192 or 160
            // From ETSI EN 302 755 Table 6b
            (FrameSize::Normal, CodeRate::Rate1_2) => (32400, 32208, 12, 16, 0b10000000000101101),
            (FrameSize::Normal, CodeRate::Rate3_5) => (38880, 38688, 12, 16, 0b10000000000101101),
            (FrameSize::Normal, CodeRate::Rate2_3) => (43200, 43040, 10, 16, 0b10000000000101101),
            (FrameSize::Normal, CodeRate::Rate3_4) => (48600, 48408, 12, 16, 0b10000000000101101),
            (FrameSize::Normal, CodeRate::Rate4_5) => (51840, 51648, 12, 16, 0b10000000000101101),
            (FrameSize::Normal, CodeRate::Rate5_6) => (54000, 53840, 10, 16, 0b10000000000101101),
        };

        let m = n - k;

        Self {
            n,
            k,
            m,
            t,
            field_m,
            primitive_poly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_frame_rate_1_2() {
        let params = DvbBchParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
        assert_eq!(params.n, 7200); // k_ldpc
        assert_eq!(params.k, 7032); // Kbch
        assert_eq!(params.m, 168); // BCH parity
        assert_eq!(params.t, 12);
        assert_eq!(params.field_m, 14);
        assert_eq!(params.n, params.k + params.m);
    }

    #[test]
    fn test_normal_frame_rate_1_2() {
        let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
        assert_eq!(params.n, 32400); // k_ldpc
        assert_eq!(params.k, 32208); // Kbch
        assert_eq!(params.m, 192); // BCH parity
        assert_eq!(params.t, 12);
        assert_eq!(params.field_m, 16);
        assert_eq!(params.n, params.k + params.m);
    }

    #[test]
    fn test_normal_frame_rate_2_3_uses_t10() {
        let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate2_3);
        assert_eq!(params.n, 43200); // k_ldpc
        assert_eq!(params.k, 43040); // Kbch
        assert_eq!(params.m, 160); // BCH parity for t=10
        assert_eq!(params.t, 10); // t=10 for this rate
        assert_eq!(params.field_m, 16);
    }

    #[test]
    fn test_normal_frame_rate_5_6_uses_t10() {
        let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate5_6);
        assert_eq!(params.n, 54000); // k_ldpc
        assert_eq!(params.k, 53840); // Kbch
        assert_eq!(params.m, 160); // BCH parity for t=10
        assert_eq!(params.t, 10); // t=10 for this rate
        assert_eq!(params.field_m, 16);
    }

    #[test]
    fn test_all_configurations_valid() {
        for &frame_size in &[FrameSize::Short, FrameSize::Normal] {
            for &rate in &[
                CodeRate::Rate1_2,
                CodeRate::Rate3_5,
                CodeRate::Rate2_3,
                CodeRate::Rate3_4,
                CodeRate::Rate4_5,
                CodeRate::Rate5_6,
            ] {
                let params = DvbBchParams::for_code(frame_size, rate);
                assert_eq!(params.n, params.k + params.m, "n = k + m invariant");
                assert!(params.t > 0, "t must be positive");
                assert!(params.t <= 12, "t must be at most 12");
                assert!(params.k > 0, "k must be positive");
                assert!(params.m > 0, "m must be positive");
            }
        }
    }

    #[test]
    fn test_short_frames_use_gf2_14() {
        for &rate in &[
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ] {
            let params = DvbBchParams::for_code(FrameSize::Short, rate);
            assert_eq!(params.field_m, 14);
            assert_eq!(params.t, 12);
        }
    }

    #[test]
    fn test_normal_frames_use_gf2_16() {
        for &rate in &[
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ] {
            let params = DvbBchParams::for_code(FrameSize::Normal, rate);
            assert_eq!(params.field_m, 16);
        }
    }
}
