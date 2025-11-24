use gf2_coding::CodeRate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Test utility - may be used in future tests
pub enum FrameSize {
    Short,
    Normal,
}

#[allow(dead_code)] // Test utility - may be used in future tests
impl FrameSize {
    pub fn to_ldpc(self) -> gf2_coding::ldpc::dvb_t2::FrameSize {
        match self {
            FrameSize::Short => gf2_coding::ldpc::dvb_t2::FrameSize::Short,
            FrameSize::Normal => gf2_coding::ldpc::dvb_t2::FrameSize::Normal,
        }
    }

    pub fn to_bch(self) -> gf2_coding::bch::dvb_t2::FrameSize {
        match self {
            FrameSize::Short => gf2_coding::bch::dvb_t2::FrameSize::Short,
            FrameSize::Normal => gf2_coding::bch::dvb_t2::FrameSize::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DvbConfig {
    pub name: String,
    pub frame_size: FrameSize,
    pub code_rate: CodeRate,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid reference name: {0}")]
    InvalidReference(String),
    #[error("Unknown code rate: {0}")]
    UnknownCodeRate(String),
}

impl DvbConfig {
    /// Parse configuration from VV reference name (e.g., "VV001-CR35")
    pub fn from_reference(reference: &str) -> Result<Self, ConfigError> {
        // Extract code rate from reference (e.g., "CR35" -> Rate3_5)
        let code_rate_str = reference
            .split('-')
            .nth(1)
            .ok_or_else(|| ConfigError::InvalidReference(reference.to_string()))?;

        if !code_rate_str.starts_with("CR") {
            return Err(ConfigError::InvalidReference(reference.to_string()));
        }

        let rate_digits = &code_rate_str[2..];
        let code_rate = match rate_digits {
            "12" => CodeRate::Rate1_2,
            "35" => CodeRate::Rate3_5,
            "23" => CodeRate::Rate2_3,
            "34" => CodeRate::Rate3_4,
            "45" => CodeRate::Rate4_5,
            "56" => CodeRate::Rate5_6,
            _ => return Err(ConfigError::UnknownCodeRate(rate_digits.to_string())),
        };

        // For now, assume Normal frame size (can be extended later)
        let frame_size = FrameSize::Normal;

        Ok(DvbConfig {
            name: reference.to_string(),
            frame_size,
            code_rate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vv001_cr35() {
        let config = DvbConfig::from_reference("VV001-CR35").unwrap();
        assert_eq!(config.name, "VV001-CR35");
        assert_eq!(config.frame_size, FrameSize::Normal);
        assert_eq!(config.code_rate, CodeRate::Rate3_5);
    }

    #[test]
    fn test_parse_all_code_rates() {
        let rates = vec![
            ("VV001-CR12", CodeRate::Rate1_2),
            ("VV002-CR35", CodeRate::Rate3_5),
            ("VV003-CR23", CodeRate::Rate2_3),
            ("VV004-CR34", CodeRate::Rate3_4),
            ("VV005-CR45", CodeRate::Rate4_5),
            ("VV006-CR56", CodeRate::Rate5_6),
        ];

        for (ref_name, expected_rate) in rates {
            let config = DvbConfig::from_reference(ref_name).unwrap();
            assert_eq!(config.code_rate, expected_rate);
        }
    }

    #[test]
    fn test_invalid_reference() {
        assert!(DvbConfig::from_reference("VV001").is_err());
        assert!(DvbConfig::from_reference("INVALID").is_err());
    }

    #[test]
    fn test_unknown_code_rate() {
        assert!(DvbConfig::from_reference("VV001-CR99").is_err());
    }
}
