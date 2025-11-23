//! Scalar backend module with sub-modules for different operation types.

mod logical;
pub mod primitives;

pub use logical::ScalarBackend;
pub use logical::SCALAR_BACKEND;
