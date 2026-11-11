//! LDPC systematic encoding implementations.

pub mod cache;
pub(crate) mod ira;
mod richardson_urbanke;

pub use cache::{CacheEntryFiles, CacheKey, CacheStats, EncodingCache};
pub(crate) use ira::IraEncoder;
pub(crate) use richardson_urbanke::SystematicReduction;
pub use richardson_urbanke::{PreprocessError, RuEncodingMatrices};

use gf2_core::sparse::SpBitMatrixDual;

/// The codeword positions of the message bits of the code with parity-check
/// matrix `h`, ascending: the `message_cols` of its [`SystematicReduction`].
/// The encoders of this module place message bit `i` at entry `i`, and
/// decoders read it back from there.
///
/// # Errors
///
/// [`PreprocessError::RankDeficient`] if the rank of `h` is below its row
/// count.
///
/// # Complexity
///
/// O(nnz) for a dual-diagonal parity part, else that of
/// [`SystematicReduction::of`].
pub(crate) fn message_cols(h: &SpBitMatrixDual) -> Result<Vec<usize>, PreprocessError> {
    let k = h.cols().saturating_sub(h.rows());
    // A dual-diagonal parity part is unit lower triangular, so right-to-left
    // pivoting takes exactly its columns.
    if k == 0 || ira::has_dual_diagonal_parity(h, k) {
        return Ok((0..k).collect());
    }
    Ok(SystematicReduction::of(&h.to_dense())?.message_cols)
}
