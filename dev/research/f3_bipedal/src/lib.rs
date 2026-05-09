//! Library entry point for the F_3 bipedal prototype.
//!
//! This crate is primarily a benchmark binary (see `main.rs`); the
//! library exposure lets `gf2-kernels-simd` integration tests cross-
//! check the SIMD bipedal3 path against the canonical scalar
//! [`Bipedal3`] reference per the b17bec62 acceptance criterion.
//!
//! # Crate role
//!
//! `f3_bipedal_prototype` is a standalone research prototype that lives
//! outside the gf2 workspace (it has its own `[workspace]` marker in
//! `Cargo.toml`). The library surface is intentionally minimal: it
//! re-exports the three F_3 packed encodings (`Bipedal3`, `Lut3`,
//! `Naive3`) and the shared [`F3Encoding`] trait so downstream tests
//! and benches can compare them against each other.

pub mod bipedal;
pub mod common;
pub mod lut;
pub mod naive;

pub use bipedal::Bipedal3;
pub use common::F3Encoding;
pub use lut::Lut3;
pub use naive::Naive3;
