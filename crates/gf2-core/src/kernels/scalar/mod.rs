//! Portable scalar kernels.

mod logical;
pub mod primitives;

pub use logical::ScalarBackend;
pub use logical::SCALAR_BACKEND;
