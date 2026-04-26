//! Bench-side re-export shim for the lib's [`gf2_core::bench_seed`]
//! module. Keeping this file here lets every bench file in this
//! directory keep its existing `#[path = "common/seed.rs"] mod seed;`
//! inclusion idiom; the actual implementation now lives in the lib
//! (gated behind the `test-support` feature) so that:
//!
//! - the doctests in [`gf2_core::bench_seed`] auto-run via
//!   `cargo test --doc -F test-support`, and
//! - the bench-side and example-side seeded fixture generators
//!   share a single source of truth.
//!
//! Per `6ed7f050` R2: lifted from a `#[path]`-only module to a public
//! lib-side module so the seed-derivation doctest is exercised by the
//! standard test runner.

#![allow(dead_code)]

pub use gf2_core::bench_seed::*;
