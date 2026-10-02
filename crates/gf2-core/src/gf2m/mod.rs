//! GF(2^m) - Binary Extension Field Arithmetic
//!
//! Items are defined in the private `field` submodule and re-exported here.

pub mod barrett;
pub mod batch;
mod byte_table;
mod field;
pub mod generation;
/// Monomorphized u64 GF(2^m) multiplication for formal verification via Charon/Aeneas.
pub mod mul_raw;
pub mod poly_helpers;
mod thread_safety_tests;
pub mod uint_ext;
pub mod wide;
pub mod wide_config;

#[cfg(any(test, feature = "test-support"))]
pub use byte_table::{
    force_scalar_gf256_table, gf256_table_builds, last_gf256_table_lane, GF256_SCALAR_LANE,
    GF256_TABLE_LANE,
};
pub use field::*;
pub use uint_ext::UintExt;
pub use wide::Gf2mWide;
pub use wide_config::Gf2mWideConfig;
