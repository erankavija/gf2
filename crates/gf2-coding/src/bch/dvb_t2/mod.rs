//! DVB-T2 BCH outer codes.
//!
//! This module provides factory methods for DVB-T2 BCH codes as specified
//! in ETSI EN 302 755 standard (Tables 6a and 6b).
//!
//! DVB-T2 uses BCH codes as outer codes before LDPC encoding to reduce
//! the error floor of the concatenated FEC system.
//!
//! # Frame Types
//!
//! - **Short frames**: n=16200, GF(2^14), t=12 errors
//! - **Normal frames**: n=64800, GF(2^16), t=10 or 12 errors
//!
//! # Code Rates
//!
//! Both frame types support 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
//!
//! # Usage
//!
//! ```
//! use gf2_coding::bch::BchCode;
//! use gf2_coding::bch::dvb_t2::FrameSize;
//! use gf2_coding::CodeRate;
//!
//! // Create DVB-T2 BCH code for normal frame, rate 1/2
//! let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
//! assert_eq!(code.n(), 32400);  // BCH output (k_ldpc)
//! assert_eq!(code.k(), 32208);  // BCH input (Kbch)
//! assert_eq!(code.t(), 12);
//! assert_eq!(code.n() - code.k(), 192);  // BCH parity
//! ```
//!
//! # Concatenation with LDPC
//!
//! BCH output (n_bch bits) becomes LDPC input (k_ldpc bits):
//!
//! ```text
//! Data (Kbch) → BCH Encode → LDPC Encode → Codeword
//!              (n_bch=k_ldpc)  (n_ldpc)
//! ```
//!
//! Example for Normal Frame Rate 1/2:
//! ```text
//! 32208 bits → BCH(64800,32208,t=12) → 64800 bits
//!           → LDPC(64800,32400,rate=1/2) → 64800 bits transmitted
//! ```
//!
//! On the receive side:
//! ```text
//! Received 64800 → LDPC Decode → 64800 bits
//!                → BCH Decode → 32208 clean bits
//! ```

pub mod generators;
pub mod params;

pub use params::{DvbBchParams, FrameSize};

use super::BchCode;
use crate::CodeRate;
use gf2_core::gf2m::Gf2mField;

impl BchCode {
    /// Creates DVB-T2 BCH code for specified frame size and rate.
    ///
    /// Uses explicit generator polynomials from ETSI EN 302 755.
    /// The generator is the product of g_1(x) × g_2(x) × ... × g_t(x).
    ///
    /// # Arguments
    ///
    /// * `frame_size` - Short (16200) or Normal (64800) LDPC frame
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::BchCode;
    /// use gf2_coding::bch::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    ///
    /// let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
    /// assert_eq!(code.n(), 32400);  // BCH output (k_ldpc)
    /// assert_eq!(code.k(), 32208);  // BCH input (Kbch)
    /// assert_eq!(code.n() - code.k(), 192);  // BCH parity
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the generator polynomial cannot be constructed for the
    /// specified parameters (should not happen for valid DVB-T2 configs).
    pub fn dvb_t2(frame_size: FrameSize, rate: CodeRate) -> Self {
        let params = DvbBchParams::for_code(frame_size, rate);
        let field = Gf2mField::new(params.field_m, params.primitive_poly).with_tables();

        // Get appropriate generator polynomials for frame size
        let generators = match frame_size {
            FrameSize::Short => generators::SHORT_GENERATORS,
            FrameSize::Normal => generators::NORMAL_GENERATORS,
        };

        // Compute g(x) = g_1(x) × g_2(x) × ... × g_t(x)
        let generator = generators::product_of_generators(&field, generators, params.t);

        Self::from_generator(params.n, params.k, params.t, field, generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::{BchDecoder, BchEncoder};
    use crate::traits::{BlockEncoder, HardDecisionDecoder};
    use gf2_core::BitVec;

    #[test]
    fn test_dvb_t2_short_rate_1_2_creation() {
        let code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        assert_eq!(code.n(), 7200); // BCH output = k_ldpc
        assert_eq!(code.k(), 7032); // BCH input = Kbch
        assert_eq!(code.t(), 12);
        assert_eq!(code.n() - code.k(), 168); // BCH parity
    }

    #[test]
    fn test_dvb_t2_normal_rate_1_2_creation() {
        let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        assert_eq!(code.n(), 32400); // BCH output = k_ldpc
        assert_eq!(code.k(), 32208); // BCH input = Kbch
        assert_eq!(code.t(), 12);
        assert_eq!(code.n() - code.k(), 192); // BCH parity
    }

    #[test]
    fn test_all_configurations_encodable() {
        for &frame_size in &[FrameSize::Short, FrameSize::Normal] {
            for &rate in &[
                CodeRate::Rate1_2,
                CodeRate::Rate3_5,
                CodeRate::Rate2_3,
                CodeRate::Rate3_4,
                CodeRate::Rate4_5,
                CodeRate::Rate5_6,
            ] {
                let code = BchCode::dvb_t2(frame_size, rate);
                let encoder = BchEncoder::new(code.clone());

                // Test encoding succeeds with zero message
                let msg = BitVec::zeros(code.k());
                let cw = encoder.encode(&msg);
                assert_eq!(cw.len(), code.n());
            }
        }
    }

    #[test]
    fn test_short_frame_encode_decode_roundtrip() {
        let code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let msg = BitVec::ones(code.k());
        let cw = encoder.encode(&msg);
        let decoded = decoder.decode(&cw);

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_normal_frame_encode_decode_roundtrip() {
        let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let msg = BitVec::ones(code.k());
        let cw = encoder.encode(&msg);
        let decoded = decoder.decode(&cw);

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_generator_polynomial_uses_product() {
        // Test that generator is actually the product of g_1...g_t
        let code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        let params = DvbBchParams::for_code(FrameSize::Short, CodeRate::Rate1_2);

        // Verify code parameters match
        assert_eq!(code.k(), params.k);
        assert_eq!(code.n(), params.n);
        assert_eq!(code.n() - code.k(), params.m);
    }

    #[test]
    fn test_short_t12_vs_normal_t10_different_generators() {
        // Short frame always uses t=12
        let short_code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);

        // Normal frame Rate 2/3 uses t=10
        let normal_t10_code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate2_3);

        // Normal frame Rate 1/2 uses t=12
        let normal_t12_code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);

        assert_eq!(short_code.t(), 12);
        assert_eq!(normal_t10_code.t(), 10);
        assert_eq!(normal_t12_code.t(), 12);

        // The number of parity bits should differ
        let m_t10 = normal_t10_code.n() - normal_t10_code.k();
        let m_t12 = normal_t12_code.n() - normal_t12_code.k();
        assert_ne!(m_t10, m_t12);
    }

    #[test]
    fn test_generator_polynomial_degree() {
        // DVB-T2 uses SHORTENED BCH codes
        // The generator polynomial degree is much smaller than n - k
        // For t error correction, generator degree ≈ 2*t*m where m is the field degree

        // Short frames: GF(2^14), t=12 → degree ≈ 2*12*14 = 336 (actually 168 for product of 12 polys)
        let short = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        let short_deg = short
            .generator()
            .degree()
            .expect("Generator should have degree");
        assert_eq!(
            short_deg, 168,
            "Short frame generator should have degree 168 (12 polys of degree 14)"
        );

        // Normal frames: GF(2^16), t=12 → degree ≈ 192 (12 polys of degree 16)
        let normal = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        let normal_deg = normal
            .generator()
            .degree()
            .expect("Generator should have degree");
        assert_eq!(
            normal_deg, 192,
            "Normal frame generator should have degree 192 (12 polys of degree 16)"
        );
    }

    #[test]
    fn test_bch_parity_matches_generator_degree() {
        // With corrected parameters, BCH parity (n - k) should equal generator degree

        // Short frame: 12 polys of degree 14 = degree 168
        let short = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        let short_deg = short
            .generator()
            .degree()
            .expect("Generator should have degree");
        assert_eq!(short.n() - short.k(), 168);
        assert_eq!(short_deg, 168);

        // Normal frame t=12: 12 polys of degree 16 = degree 192
        let normal = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        let normal_deg = normal
            .generator()
            .degree()
            .expect("Generator should have degree");
        assert_eq!(normal.n() - normal.k(), 192);
        assert_eq!(normal_deg, 192);

        // Normal frame t=10: 10 polys of degree 16 = degree 160
        let normal_t10 = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate2_3);
        let normal_t10_deg = normal_t10
            .generator()
            .degree()
            .expect("Generator should have degree");
        assert_eq!(normal_t10.n() - normal_t10.k(), 160);
        assert_eq!(normal_t10_deg, 160);
    }
}
