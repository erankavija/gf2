//! Code selection shared by the `export_alist` and `ldpc_bler_sweep` binaries.

use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::{CodeRate, LdpcCode};

/// The LDPC codes of the comparison against AFF3CT
/// (`@/citation/Cassagne2019`).
///
/// The `export_alist` bin exports the `H` of [`build`](Self::build) and the
/// `ldpc_bler_sweep` bin decodes the same code, so both sides share one
/// parity-check matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonCode {
    /// DVB-T2 r1/2 Normal LDPC (`@/citation/Etsi2015`): N = 64800, K = 32400.
    DvbT2R12,
    /// 5G NR BG1 mother code (Z = 384): N = 68·384 = 26112, K = 22·384 =
    /// 8448, decoded without puncturing or shortening.
    NrBg1R12,
}

impl ComparisonCode {
    /// Parses the harness CLI name (`dvb-t2-r12` / `nr-bg1-r12`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "dvb-t2-r12" => Ok(Self::DvbT2R12),
            "nr-bg1-r12" => Ok(Self::NrBg1R12),
            other => Err(format!(
                "unknown --code '{other}' (expected 'dvb-t2-r12' or 'nr-bg1-r12')"
            )),
        }
    }

    /// Builds the `LdpcCode` for this configuration.
    ///
    /// # Complexity
    ///
    /// That of the selected code-family constructor. The NR path calls
    /// [`QuasiCyclicLdpc::nr_5g_rate_matched`], including its encoder-data
    /// preparation, before retaining the mother code.
    #[must_use]
    pub fn build(self) -> LdpcCode {
        match self {
            Self::DvbT2R12 => LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
            Self::NrBg1R12 => {
                // nr_5g_rate_matched(1, 16896, 8448) selects Z = 384 for BG1;
                // the comparison uses its full (un-rate-matched) mother code.
                let rm = QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448);
                rm.mother_code().clone()
            }
        }
    }
}
