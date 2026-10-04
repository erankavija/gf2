//! Systematic encoding of DVB-T2 LDPC codes: codeword = [message | parity].

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::path::PathBuf;

fn try_load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

fn create_encoder(code: LdpcCode, cache: Option<&EncodingCache>) -> LdpcEncoder {
    match cache {
        Some(c) => LdpcEncoder::with_cache(code, c),
        None => LdpcEncoder::new(code),
    }
}

#[cfg(test)]
mod systematic_encoding_basic_tests {
    use super::*;

    #[test]
    fn test_ldpc_encoder_construction() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code, try_load_cache().as_ref());

        assert_eq!(encoder.k(), 7200);
        assert_eq!(encoder.n(), 16200);
    }

    #[test]
    fn test_systematic_form() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code.clone(), try_load_cache().as_ref());

        let mut message = BitVec::zeros(encoder.k());
        message.set(0, true);
        message.set(10, true);
        message.set(100, true);

        let codeword = encoder.encode(&message);

        for i in 0..encoder.k() {
            assert_eq!(
                codeword.get(i),
                message.get(i),
                "Message bit {} not preserved in systematic position",
                i
            );
        }

        assert_eq!(codeword.len(), encoder.n());
    }

    #[test]
    fn test_encoded_codeword_is_valid() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code.clone(), try_load_cache().as_ref());

        let message = BitVec::zeros(encoder.k());
        let codeword = encoder.encode(&message);

        assert!(
            code.is_valid_codeword(&codeword),
            "Encoded codeword must be valid (zero syndrome)"
        );
    }

    #[test]
    fn test_encode_multiple_messages() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code.clone(), try_load_cache().as_ref());

        let test_messages = vec![
            BitVec::zeros(encoder.k()),
            {
                let mut bv = BitVec::zeros(encoder.k());
                bv.set(0, true);
                bv
            },
            {
                let mut bv = BitVec::zeros(encoder.k());
                for i in 0..encoder.k() {
                    bv.set(i, i % 2 == 0);
                }
                bv
            },
        ];

        for message in test_messages {
            let codeword = encoder.encode(&message);

            for i in 0..encoder.k() {
                assert_eq!(codeword.get(i), message.get(i));
            }

            assert!(code.is_valid_codeword(&codeword));
        }
    }
}

#[cfg(test)]
mod systematic_encoding_linearity_tests {
    use super::*;

    #[test]
    fn test_encoding_linearity() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code, try_load_cache().as_ref());

        let mut m1 = BitVec::zeros(encoder.k());
        m1.set(5, true);
        m1.set(10, true);

        let mut m2 = BitVec::zeros(encoder.k());
        m2.set(10, true);
        m2.set(20, true);

        let c1 = encoder.encode(&m1);
        let c2 = encoder.encode(&m2);

        let mut m_xor = m1.clone();
        for i in 0..encoder.k() {
            m_xor.set(i, m1.get(i) ^ m2.get(i));
        }

        let c_xor = encoder.encode(&m_xor);

        let mut c1_xor_c2 = c1.clone();
        for i in 0..encoder.n() {
            c1_xor_c2.set(i, c1.get(i) ^ c2.get(i));
        }

        assert_eq!(
            c_xor.to_bytes_le(),
            c1_xor_c2.to_bytes_le(),
            "Encoding must be linear"
        );
    }

    #[test]
    fn test_zero_message_encodes_to_zero() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code, try_load_cache().as_ref());

        let zero_message = BitVec::zeros(encoder.k());
        let codeword = encoder.encode(&zero_message);

        assert_eq!(
            codeword.count_ones(),
            0,
            "Zero message should encode to all-zero codeword"
        );
    }
}

#[cfg(test)]
mod dvb_t2_encoding_tests {
    use super::*;

    #[test]
    fn test_dvb_t2_normal_all_rates() {
        let rates = vec![
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ];

        for rate in rates {
            let code = LdpcCode::dvb_t2_normal(rate);
            // DVB-T2 codes take the IRA path, which never consults the cache.
            let encoder = LdpcEncoder::new(code.clone());

            let message = BitVec::zeros(encoder.k());
            let codeword = encoder.encode(&message);

            assert_eq!(codeword.len(), 64800);
            assert!(code.is_valid_codeword(&codeword));
        }
    }

    #[test]
    fn test_dvb_t2_short_all_rates() {
        let rates = vec![
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ];

        for rate in rates {
            let code = LdpcCode::dvb_t2_short(rate);
            let encoder = LdpcEncoder::new(code.clone());

            let message = BitVec::zeros(encoder.k());
            let codeword = encoder.encode(&message);

            assert_eq!(codeword.len(), 16200);
            assert!(code.is_valid_codeword(&codeword));
        }
    }
}

#[cfg(test)]
mod parity_computation_tests {
    use super::*;

    #[test]
    fn test_parity_bits_computed() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code, try_load_cache().as_ref());

        let mut message = BitVec::zeros(encoder.k());
        message.set(0, true);
        message.set(100, true);

        let codeword = encoder.encode(&message);

        let mut parity_ones = 0;
        for i in encoder.k()..encoder.n() {
            if codeword.get(i) {
                parity_ones += 1;
            }
        }

        assert!(
            parity_ones > 0,
            "Parity bits should be computed for non-zero message"
        );
    }

    #[test]
    fn test_dvb_t2_parity_structure() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code.clone(), try_load_cache().as_ref());

        let mut message = BitVec::zeros(encoder.k());
        message.set(50, true);

        let codeword = encoder.encode(&message);

        assert!(
            code.is_valid_codeword(&codeword),
            "Codeword must satisfy dual-diagonal parity constraints"
        );
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_encode_exact_length() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code, try_load_cache().as_ref());

        let message = BitVec::zeros(encoder.k());
        let codeword = encoder.encode(&message);

        assert_eq!(codeword.len(), encoder.n());
    }

    #[test]
    fn test_all_message_bits_preserved() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = create_encoder(code, try_load_cache().as_ref());

        let mut message = BitVec::zeros(encoder.k());
        for i in (0..encoder.k()).step_by(100) {
            message.set(i, true);
        }

        let codeword = encoder.encode(&message);

        for i in 0..encoder.k() {
            assert_eq!(
                codeword.get(i),
                message.get(i),
                "Message bit {} not preserved",
                i
            );
        }
    }
}
