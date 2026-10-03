//! Batch types exchanged between the DVB-T2 BICM [`stages`](crate::stages).
//!
//! A batch holds independent FEC frames, one per outer-`Vec` element;
//! [`BatchSize::batch_size`] returns the frame count.

use gf2_coding::Llr;
use gf2_core::BitVec;

use crate::BatchSize;

/// A batch of bit-packed frames (BBFRAME info bits or FECFRAME coded bits).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BitPackedBatch {
    /// One bit-packed frame per batch element.
    pub frames: Vec<BitVec>,
}

impl BitPackedBatch {
    /// Wraps a vector of bit-packed frames into a batch.
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
/// (`i[k]` and `q[k]` are the components of symbol `k`).
/// `i[f].len() == q[f].len()` for every frame `f`.
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
    /// # Panics
    ///
    /// Panics if `i.len() != q.len()` or if any frame `f` has
    /// `i[f].len() != q[f].len()`.
    pub fn new(i: Vec<Vec<f32>>, q: Vec<Vec<f32>>) -> Self {
        assert_eq!(
            i.len(),
            q.len(),
            "SymbolBatch: I lane frame count ({}) must equal Q lane frame count ({})",
            i.len(),
            q.len()
        );
        for (f, (i_frame, q_frame)) in i.iter().zip(q.iter()).enumerate() {
            assert_eq!(
                i_frame.len(),
                q_frame.len(),
                "SymbolBatch: frame {} I lane length ({}) must equal Q lane length ({})",
                f,
                i_frame.len(),
                q_frame.len()
            );
        }
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
/// Each batch element is one frame of channel [`Llr`] values, one per coded
/// bit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LlrBatch {
    /// One LLR frame per batch element.
    pub frames: Vec<Vec<Llr>>,
}

impl LlrBatch {
    /// Wraps a vector of LLR frames into a batch.
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
/// Distinct from [`BitPackedBatch`] so that the type-erased connector check
/// rejects a decode output wired into an encode input.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HardDecisionBatch {
    /// One decoded hard-decision frame per batch element.
    pub frames: Vec<BitVec>,
}

impl HardDecisionBatch {
    /// Wraps a vector of decoded frames into a batch.
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
    #[should_panic(expected = "frame 0 I lane length")]
    fn test_symbol_batch_mismatched_per_frame_lane_lengths_panic() {
        let _ = SymbolBatch::new(vec![vec![0.0_f32; 4]], vec![vec![0.0_f32; 3]]);
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
        let b: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![BitVec::zeros(8)]));
        assert!(b.as_any().downcast_ref::<BitPackedBatch>().is_some());
        assert_eq!(b.batch_size(), 1);
    }
}
