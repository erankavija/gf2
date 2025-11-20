//! DVB-T2 LDPC code construction from standard tables.
//!
//! This module implements DVB-T2 LDPC codes by directly building sparse
//! parity-check matrices from ETSI EN 302 755 standard tables.
//!
//! DVB-T2 supports two frame sizes:
//! - **Short frames**: n=16200, Z=360
//! - **Normal frames**: n=64800, Z=360
//!
//! Both support 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
//!
//! # Usage
//!
//! Use the factory methods on `LdpcCode`:
//! ```
//! use gf2_coding::ldpc::LdpcCode;
//! use gf2_coding::CodeRate;
//!
//! let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
//! assert_eq!(code.n(), 64800);
//! ```

pub(crate) mod params;
pub(crate) mod builder;
pub(crate) mod dvb_t2_matrices;

pub use params::{DvbParams, FrameSize};
