//! Connectors and edges joining stages in a [`Pipeline`](crate::Pipeline).
//!
//! Lifts the §1 "`Connector<T>` and `Edge`" block of the Phase 0 design doc
//! (`dev/active/ec530af9-pipeline-design.md`) into code.

use std::any::TypeId;
use std::marker::PhantomData;

use crate::stage::TypedBatch;

/// A typed connection point between two stages.
///
/// Carries the batch sizing the pipeline pre-allocates SoA buffers for. The
/// phantom `T` records the batch element type so the graph API can type-check
/// connections at compile time.
///
/// # Examples
///
/// ```
/// use gf2_sim::connector::Connector;
/// use gf2_sim::stage::BatchSize;
///
/// // `TypedBatch` is auto-implemented for any `BatchSize` batch type.
/// struct Bits(Vec<u8>);
/// impl BatchSize for Bits {
///     fn batch_size(&self) -> usize {
///         self.0.len()
///     }
/// }
///
/// let c = Connector::<Bits>::new(256, 64800);
/// assert_eq!(c.batch_size, 256);
/// assert_eq!(c.frame_len_bits, 64800);
/// ```
pub struct Connector<T: TypedBatch> {
    /// Number of frames per batch crossing this connector.
    pub batch_size: usize,
    /// Frame length in bits.
    pub frame_len_bits: usize,
    _t: PhantomData<T>,
}

impl<T: TypedBatch> Connector<T> {
    /// Creates a connector for the given batch size and frame length.
    ///
    /// # Arguments
    ///
    /// * `batch_size` — number of frames per batch.
    /// * `frame_len_bits` — frame length in bits.
    pub fn new(batch_size: usize, frame_len_bits: usize) -> Self {
        Self {
            batch_size,
            frame_len_bits,
            _t: PhantomData,
        }
    }
}

/// A directed edge in the pipeline graph, connecting a producer to a consumer.
///
/// Built during graph construction; the build pass type-checks `element_type`
/// against the producing and consuming stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// The producing stage.
    pub from: StageId,
    /// The consuming stage.
    pub to: StageId,
    /// The [`TypeId`] of the batch element flowing across this edge.
    pub element_type: TypeId,
    /// The batch size negotiated for this edge.
    pub batch_size: usize,
}

/// An opaque, stable identifier for a stage within one pipeline build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StageId(pub u32);
