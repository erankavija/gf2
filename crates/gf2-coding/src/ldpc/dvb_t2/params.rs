//! DVB-T2 LDPC code parameters.
//!
//! This module defines the parameters for DVB-T2 LDPC codes as specified
//! in ETSI EN 302 755.

use crate::bch::CodeRate;

/// DVB-T2 frame size (codeword length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSize {
    /// Short frame: n = 16200 bits
    Short,
    /// Normal frame: n = 64800 bits
    Normal,
}

/// DVB-T2 LDPC code parameters.
///
/// Parameters are derived from the frame size and code rate according to
/// ETSI EN 302 755 standard tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DvbParams {
    /// Codeword length
    pub n: usize,
    /// Information bits
    pub k: usize,
    /// Number of parity bits (n - k)
    pub m: usize,
    /// Step size for block structure (q in standard)
    pub step_size: usize,
    /// Expansion factor (360 for DVB-T2)
    pub expansion_factor: usize,
    /// Number of information bit blocks
    pub num_info_blocks: usize,
}

impl DvbParams {
    /// Get parameters for a DVB-T2 configuration.
    ///
    /// # Arguments
    ///
    /// * `frame_size` - Short (16200) or Normal (64800) frame
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// # use gf2_coding::ldpc::dvb_t2::{DvbParams, FrameSize};
    /// # use gf2_coding::CodeRate;
    /// #
    /// let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
    /// assert_eq!(params.n, 64800);
    /// assert_eq!(params.k, 32400);
    /// assert_eq!(params.m, 32400);
    /// ```
    pub fn for_code(frame_size: FrameSize, rate: CodeRate) -> Self {
        let expansion_factor = 360;
        
        let (n, k, step_size) = match (frame_size, rate) {
            (FrameSize::Normal, CodeRate::Rate1_2) => (64800, 32400, 90),
            (FrameSize::Normal, CodeRate::Rate3_5) => (64800, 38880, 96),
            (FrameSize::Normal, CodeRate::Rate2_3) => (64800, 43200, 60),
            (FrameSize::Normal, CodeRate::Rate3_4) => (64800, 48600, 45),
            (FrameSize::Normal, CodeRate::Rate4_5) => (64800, 51840, 36),
            (FrameSize::Normal, CodeRate::Rate5_6) => (64800, 54000, 30),
            (FrameSize::Short, CodeRate::Rate1_2) => (16200, 7200, 25),
            (FrameSize::Short, CodeRate::Rate3_5) => (16200, 9720, 27),
            (FrameSize::Short, CodeRate::Rate2_3) => (16200, 10800, 15),
            (FrameSize::Short, CodeRate::Rate3_4) => (16200, 11880, 12),
            (FrameSize::Short, CodeRate::Rate4_5) => (16200, 12600, 9),
            (FrameSize::Short, CodeRate::Rate5_6) => (16200, 13320, 8),
        };
        
        let m = n - k;
        let num_info_blocks = k / expansion_factor;
        
        Self {
            n,
            k,
            m,
            step_size,
            expansion_factor,
            num_info_blocks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_frame_rate_1_2() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 32400);
        assert_eq!(params.m, 32400);
        assert_eq!(params.step_size, 90);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 90);
        
        // Verify invariants
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_normal_frame_rate_3_5() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate3_5);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 38880);
        assert_eq!(params.m, 25920);
        assert_eq!(params.step_size, 96);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 108);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_normal_frame_rate_2_3() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate2_3);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 43200);
        assert_eq!(params.m, 21600);
        assert_eq!(params.step_size, 60);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 120);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_normal_frame_rate_3_4() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate3_4);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 48600);
        assert_eq!(params.m, 16200);
        assert_eq!(params.step_size, 45);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 135);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_normal_frame_rate_4_5() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate4_5);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 51840);
        assert_eq!(params.m, 12960);
        assert_eq!(params.step_size, 36);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 144);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_normal_frame_rate_5_6() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate5_6);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 54000);
        assert_eq!(params.m, 10800);
        assert_eq!(params.step_size, 30);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 150);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_short_frame_rate_1_2() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 7200);
        assert_eq!(params.m, 9000);
        assert_eq!(params.step_size, 25);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 20);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_short_frame_rate_3_5() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate3_5);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 9720);
        assert_eq!(params.m, 6480);
        assert_eq!(params.step_size, 27);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 27);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_short_frame_rate_2_3() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate2_3);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 10800);
        assert_eq!(params.m, 5400);
        assert_eq!(params.step_size, 15);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 30);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_short_frame_rate_3_4() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate3_4);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 11880);
        assert_eq!(params.m, 4320);
        assert_eq!(params.step_size, 12);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 33);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_short_frame_rate_4_5() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate4_5);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 12600);
        assert_eq!(params.m, 3600);
        assert_eq!(params.step_size, 9);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 35);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }

    #[test]
    fn test_short_frame_rate_5_6() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate5_6);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 13320);
        assert_eq!(params.m, 2880);
        assert_eq!(params.step_size, 8);
        assert_eq!(params.expansion_factor, 360);
        assert_eq!(params.num_info_blocks, 37);
        
        assert_eq!(params.n, params.k + params.m);
        assert_eq!(params.k, params.num_info_blocks * params.expansion_factor);
    }
}
