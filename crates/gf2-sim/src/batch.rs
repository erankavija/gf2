//! Concrete cross-stage batch types.
//!
//! This module supplies the four concrete batch newtypes that the DVB-T2 BICM
//! [`stages`](crate::stages) move between, per §1 ("batch types") and §2 ("SoA
//! layout") of the Phase 0 design doc (`dev/active/ec530af9-pipeline-design.md`).
//!
//! Each type implements the scaffolding [`BatchSize`] trait;
//! the blanket [`TypedBatch`](crate::TypedBatch) impl in
//! [`stage`](crate::stage) then makes every one of them usable as a
//! type-erased batch crossing a [`Stage`](crate::Stage) boundary.
//!
//! # Layout
//!
//! A "batch" here is a collection of independent FEC *frames*. The batch types
//! store one frame per outer-`Vec` element (structure-of-frames); within a
//! frame the per-attribute storage follows the design-doc SoA convention
//! (parallel `I`/`Q` lanes for [`SymbolBatch`]). [`BatchSize::batch_size`]
//! returns the number of frames.
//!
//! # Precision
//!
//! Symbols are stored as `f32` and LLRs as [`gf2_coding::Llr`] (f32 by default,
//! f64 under the `llr-f64` feature), matching design-doc §10.

use gf2_coding::Llr;
use gf2_core::BitVec;

use crate::BatchSize;

/// A batch of bit-packed frames (BBFRAME info bits or FECFRAME coded bits).
///
/// Each element is one frame stored as a [`BitVec`]. Used as the input to
/// [`DvbT2Encode`](crate::stages::DvbT2Encode) (BBFRAME info bits), the output
/// of encoding / decoding (FECFRAME coded bits / recovered BBFRAME bits), and
/// the bit-interleaver I/O.
///
/// # Examples
///
/// ```
/// use gf2_sim::batch::BitPackedBatch;
/// use gf2_sim::BatchSize;
/// use gf2_core::BitVec;
///
/// let batch = BitPackedBatch::new(vec![BitVec::zeros(8), BitVec::zeros(8)]);
/// assert_eq!(batch.batch_size(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BitPackedBatch {
    /// One bit-packed frame per batch element.
    pub frames: Vec<BitVec>,
}

impl BitPackedBatch {
    /// Wraps a vector of bit-packed frames into a batch.
    ///
    /// # Arguments
    ///
    /// * `frames` — one [`BitVec`] per frame.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::batch::BitPackedBatch;
    /// use gf2_core::BitVec;
    ///
    /// let batch = BitPackedBatch::new(vec![BitVec::zeros(4)]);
    /// assert_eq!(batch.frames.len(), 1);
    /// ```
    pub fn new(frames: Vec<BitVec>) -> Self {
        Self { frames }
    }
}

impl BatchSize for BitPackedBatch {
    fn batch_size(&self) -> usize {
        self.frames.len()
    }
}

/// A batch of Gray-QAM IQ symbol frames in structure-of-arrays form.
///
/// Each batch element is one frame; within a frame the in-phase (`i`) and
/// quadrature (`q`) components are stored as parallel `f32` lanes
/// (`i[k]` and `q[k]` are the components of symbol `k`), per the SoA layout in
/// design-doc §2. `i[f].len() == q[f].len()` for every frame `f`.
///
/// # Examples
///
/// ```
/// use gf2_sim::batch::SymbolBatch;
/// use gf2_sim::BatchSize;
///
/// let batch = SymbolBatch::new(vec![vec![0.5_f32]], vec![vec![-0.5_f32]]);
/// assert_eq!(batch.batch_size(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolBatch {
    /// In-phase lane per frame.
    pub i: Vec<Vec<f32>>,
    /// Quadrature lane per frame.
    pub q: Vec<Vec<f32>>,
}

impl SymbolBatch {
    /// Wraps parallel per-frame I/Q lanes into a batch.
    ///
    /// # Arguments
    ///
    /// * `i` — in-phase lane per frame.
    /// * `q` — quadrature lane per frame.
    ///
    /// # Panics
    ///
    /// Panics if `i.len() != q.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::batch::SymbolBatch;
    ///
    /// let batch = SymbolBatch::new(vec![vec![1.0_f32, -1.0]], vec![vec![1.0_f32, -1.0]]);
    /// assert_eq!(batch.i.len(), 1);
    /// ```
    pub fn new(i: Vec<Vec<f32>>, q: Vec<Vec<f32>>) -> Self {
        assert_eq!(
            i.len(),
            q.len(),
            "SymbolBatch: I lane frame count ({}) must equal Q lane frame count ({})",
            i.len(),
            q.len()
        );
        Self { i, q }
    }
}

impl BatchSize for SymbolBatch {
    fn batch_size(&self) -> usize {
        self.i.len()
    }
}

/// A batch of soft-LLR frames.
///
/// Each batch element is one frame of channel [`Llr`] values (one LLR per coded
/// bit). Produced by [`GrayQamDemap`](crate::stages::GrayQamDemap) and consumed
/// by [`DvbT2Decode`](crate::stages::DvbT2Decode). LLR sign convention follows
/// [`gf2_coding::Llr`]: positive favours bit 0, negative favours bit 1.
///
/// # Examples
///
/// ```
/// use gf2_sim::batch::LlrBatch;
/// use gf2_sim::BatchSize;
/// use gf2_coding::Llr;
///
/// let batch = LlrBatch::new(vec![vec![Llr::new(1.0), Llr::new(-1.0)]]);
/// assert_eq!(batch.batch_size(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LlrBatch {
    /// One LLR frame per batch element.
    pub frames: Vec<Vec<Llr>>,
}

impl LlrBatch {
    /// Wraps a vector of LLR frames into a batch.
    ///
    /// # Arguments
    ///
    /// * `frames` — one `Vec<Llr>` per frame.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::batch::LlrBatch;
    /// use gf2_coding::Llr;
    ///
    /// let batch = LlrBatch::new(vec![vec![Llr::new(2.0)]]);
    /// assert_eq!(batch.frames.len(), 1);
    /// ```
    pub fn new(frames: Vec<Vec<Llr>>) -> Self {
        Self { frames }
    }
}

impl BatchSize for LlrBatch {
    fn batch_size(&self) -> usize {
        self.frames.len()
    }
}

/// A batch of decoded hard-decision bit frames.
///
/// Each element is one recovered frame as a [`BitVec`]. This is the terminal
/// output type of the decode path; it is kept distinct from
/// [`BitPackedBatch`] (a separate [`std::any::TypeId`]) so a decode stage's
/// output never silently feeds back into an encode stage at the type-erased
/// connector boundary.
///
/// # Examples
///
/// ```
/// use gf2_sim::batch::HardDecisionBatch;
/// use gf2_sim::BatchSize;
/// use gf2_core::BitVec;
///
/// let batch = HardDecisionBatch::new(vec![BitVec::zeros(8)]);
/// assert_eq!(batch.batch_size(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HardDecisionBatch {
    /// One decoded hard-decision frame per batch element.
    pub frames: Vec<BitVec>,
}

impl HardDecisionBatch {
    /// Wraps a vector of decoded frames into a batch.
    ///
    /// # Arguments
    ///
    /// * `frames` — one [`BitVec`] per frame.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_sim::batch::HardDecisionBatch;
    /// use gf2_core::BitVec;
    ///
    /// let batch = HardDecisionBatch::new(vec![BitVec::zeros(4)]);
    /// assert_eq!(batch.frames.len(), 1);
    /// ```
    pub fn new(frames: Vec<BitVec>) -> Self {
        Self { frames }
    }
}

impl BatchSize for HardDecisionBatch {
    fn batch_size(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TypedBatch;
    use std::any::TypeId;

    #[test]
    fn test_bit_packed_batch_size_counts_frames() {
        let batch = BitPackedBatch::new(vec![BitVec::zeros(8), BitVec::zeros(8), BitVec::zeros(8)]);
        assert_eq!(BatchSize::batch_size(&batch), 3);
        // Blanket TypedBatch also reports the same size.
        assert_eq!(TypedBatch::batch_size(&batch), 3);
    }

    #[test]
    fn test_symbol_batch_size_counts_frames() {
        let batch = SymbolBatch::new(
            vec![vec![0.0_f32; 4], vec![0.0_f32; 4]],
            vec![vec![0.0_f32; 4], vec![0.0_f32; 4]],
        );
        assert_eq!(BatchSize::batch_size(&batch), 2);
    }

    #[test]
    #[should_panic(expected = "frame count")]
    fn test_symbol_batch_mismatched_lanes_panic() {
        let _ = SymbolBatch::new(vec![vec![0.0_f32]], vec![]);
    }

    #[test]
    fn test_llr_batch_size_counts_frames() {
        let batch = LlrBatch::new(vec![vec![Llr::new(1.0)], vec![Llr::new(-1.0)]]);
        assert_eq!(BatchSize::batch_size(&batch), 2);
    }

    #[test]
    fn test_hard_decision_batch_size_counts_frames() {
        let batch = HardDecisionBatch::new(vec![BitVec::zeros(4)]);
        assert_eq!(BatchSize::batch_size(&batch), 1);
    }

    #[test]
    fn test_batch_types_have_distinct_type_ids() {
        // The four batch types must be distinct erased types so the connector
        // boundary can tell them apart.
        let ids = [
            TypeId::of::<BitPackedBatch>(),
            TypeId::of::<SymbolBatch>(),
            TypeId::of::<LlrBatch>(),
            TypeId::of::<HardDecisionBatch>(),
        ];
        for (a, x) in ids.iter().enumerate() {
            for y in ids.iter().skip(a + 1) {
                assert_ne!(x, y, "batch type ids must be pairwise distinct");
            }
        }
    }

    #[test]
    fn test_batches_are_typed_batch_objects() {
        // Confirm each concrete batch can be boxed behind dyn TypedBatch and
        // downcast back via the as_any hook.
        let b: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![BitVec::zeros(8)]));
        assert!(b.as_any().downcast_ref::<BitPackedBatch>().is_some());
        assert_eq!(b.batch_size(), 1);
    }
}
