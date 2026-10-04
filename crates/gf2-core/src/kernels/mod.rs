//! Word-level bit kernels behind the [`Backend`] trait: a scalar implementation
//! that is always available and, under the `simd` feature, an AVX2
//! implementation selected by runtime CPU detection.

pub mod backend;
pub mod ops;
pub mod scalar;

#[cfg(feature = "simd")]
pub mod simd;

#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod x86;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

pub use backend::{select_backend_for_size, Backend, SelectedBackend};
pub use scalar::ScalarBackend;
