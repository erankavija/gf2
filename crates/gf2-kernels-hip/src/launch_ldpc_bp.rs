//! Safe host wrappers for the device LDPC belief-propagation batch decoder
//! (`hip/ldpc_bp.hip`).
//!
//! [`GpuLdpcBp`] runs a flooding BP schedule over a caller-built
//! [`LdpcGraphLayout`]: init, then alternating check-node and variable-node
//! updates, with optional per-frame early termination. The default-stream
//! entry points use synchronous transfers; the `_on_stream` variants order
//! every launch and transfer on a caller-owned [`HipStream`], staging transfers
//! through pinned memory because a synchronous `hipMemcpy` executes on the NULL
//! stream and serializes against every other blocking stream on the device.

use std::ffi::c_void;
use std::ptr;

use crate::host::{DeviceBuffer, HipStream, PinnedHostBuffer};
use crate::{check_hip, ffi, HipError};

/// Algorithm selector matching the `LDPC_ALG_*` constants in
/// `hip/ldpc_bp.hip`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuBpAlgorithm {
    /// Standard min-sum (sign product × min magnitude).
    MinSum,
    /// Normalized min-sum: min-sum scaled by `alpha`.
    NormalizedMinSum(f32),
    /// Offset min-sum: `max(0, min - beta)` with the sign product.
    OffsetMinSum(f32),
    /// Exact sum-product (box-plus via `tanh` / `atanh`).
    SumProduct,
}

impl GpuBpAlgorithm {
    /// The integer selector passed to the kernel (`LDPC_ALG_*`).
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            GpuBpAlgorithm::MinSum => 0,
            GpuBpAlgorithm::NormalizedMinSum(_) => 1,
            GpuBpAlgorithm::OffsetMinSum(_) => 2,
            GpuBpAlgorithm::SumProduct => 3,
        }
    }

    /// The `alpha` correction (normalized min-sum only; `1.0` otherwise).
    #[must_use]
    pub fn alpha(self) -> f32 {
        match self {
            GpuBpAlgorithm::NormalizedMinSum(a) => a,
            _ => 1.0,
        }
    }

    /// The `beta` offset (offset min-sum only; `0.0` otherwise).
    #[must_use]
    pub fn beta(self) -> f32 {
        match self {
            GpuBpAlgorithm::OffsetMinSum(b) => b,
            _ => 0.0,
        }
    }
}

/// Flat Tanner-graph representation the GPU LDPC BP kernel decodes.
///
/// A double CSR: the check-major CSR (`check_row_ptr`, `check_edge_var`), the
/// variable-major CSC (`var_col_ptr`), and two cross-maps
/// (`check_edge_to_var_edge`, `var_edge_to_check_edge`) that identify the same
/// Tanner edge from the two views. The kernel gathers check-node inputs in CSR
/// row order and sums variable-node beliefs in CSC column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdpcGraphLayout {
    /// Codeword length (number of variable nodes).
    pub n: usize,
    /// Number of check nodes.
    pub m: usize,
    /// CSR row offsets, length `m + 1`. `check_row_ptr[c]..check_row_ptr[c+1]`
    /// are the check-edge indices of check `c`, in `row_iter` order.
    pub check_row_ptr: Vec<i32>,
    /// Variable index of each check-edge, length `edges`. Consumed by the
    /// syndrome kernel (per-check parity over its variables).
    pub check_edge_var: Vec<i32>,
    /// For each check-edge, the matching variable-edge index, length `edges`.
    pub check_edge_to_var_edge: Vec<i32>,
    /// CSC column offsets, length `n + 1`. `var_col_ptr[v]..var_col_ptr[v+1]`
    /// are the variable-edge indices of variable `v`, in `col_iter` order.
    pub var_col_ptr: Vec<i32>,
    /// For each variable-edge, the matching check-edge index, length `edges`.
    pub var_edge_to_check_edge: Vec<i32>,
}

impl LdpcGraphLayout {
    /// The number of Tanner-graph edges `E` (length of every per-edge array).
    #[must_use]
    pub fn edges(&self) -> usize {
        self.check_edge_var.len()
    }
}

/// Pinned host staging for [`GpuLdpcBp::decode_batch_on_stream`] and
/// [`GpuLdpcBp::decode_batch_with_iters_on_stream`], sized for one decoder's
/// `max_batch` / `n` and `Send`-only: one per worker thread.
pub struct LdpcStreamScratch {
    channel: PinnedHostBuffer<f32>,
    hard: PinnedHostBuffer<u8>,
    /// One buffer serves the zero-fill H2D and the post-syndrome D2H: both are
    /// ordered on the same stream, and the host touches the buffer only after
    /// the per-iteration stream synchronize.
    unsat: PinnedHostBuffer<u8>,
    done: PinnedHostBuffer<u8>,
    max_batch: usize,
    n: usize,
}

impl LdpcStreamScratch {
    /// The `max_batch` of the [`GpuLdpcBp`] this scratch was sized for.
    #[must_use]
    pub fn max_batch(&self) -> usize {
        self.max_batch
    }

    /// The codeword length `n` of the [`GpuLdpcBp`] this scratch was sized for.
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }
}

/// A reusable device-side LDPC belief-propagation batch decoder.
///
/// The graph layout is uploaded once; the message buffers (`v2c`, `c2v`),
/// channel-LLR input and hard-decision output are sized for `max_batch` frames
/// at construction.
pub struct GpuLdpcBp {
    d_check_row_ptr: DeviceBuffer<i32>,
    d_check_edge_var: DeviceBuffer<i32>,
    d_check_edge_to_var_edge: DeviceBuffer<i32>,
    d_var_col_ptr: DeviceBuffer<i32>,
    d_var_edge_to_check_edge: DeviceBuffer<i32>,
    d_channel: DeviceBuffer<f32>,
    d_v2c: DeviceBuffer<f32>,
    d_c2v: DeviceBuffer<f32>,
    d_hard: DeviceBuffer<u8>,
    d_unsatisfied: DeviceBuffer<u8>,
    /// Per-frame freeze flags for early termination (1 = converged & frozen).
    d_frame_done: DeviceBuffer<u8>,
    n: usize,
    m: usize,
    edges: usize,
    max_batch: usize,
    device_id: i32,
}

impl GpuLdpcBp {
    /// Builds a decoder for `layout` on `device_id`, sized for up to
    /// `max_batch` frames per [`decode_batch`](Self::decode_batch).
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if any device allocation or graph upload fails (an
    /// OOM is the distinguished [`HipError::OutOfMemory`]).
    ///
    /// # Panics
    ///
    /// Panics if `layout`'s CSR/CSC arrays are internally inconsistent
    /// (`check_row_ptr.len() != m + 1`, `var_col_ptr.len() != n + 1`, or the
    /// per-edge arrays disagree on `edges`).
    ///
    /// # Complexity
    ///
    /// O(`max_batch * edges`) device memory.
    pub fn new(
        layout: &LdpcGraphLayout,
        max_batch: usize,
        device_id: i32,
    ) -> Result<Self, HipError> {
        let n = layout.n;
        let m = layout.m;
        let edges = layout.edges();
        assert_eq!(
            layout.check_row_ptr.len(),
            m + 1,
            "check_row_ptr length {} != m + 1 ({})",
            layout.check_row_ptr.len(),
            m + 1
        );
        assert_eq!(
            layout.var_col_ptr.len(),
            n + 1,
            "var_col_ptr length {} != n + 1 ({})",
            layout.var_col_ptr.len(),
            n + 1
        );
        assert_eq!(
            layout.check_edge_to_var_edge.len(),
            edges,
            "check_edge_to_var_edge length must equal edges"
        );
        assert_eq!(
            layout.var_edge_to_check_edge.len(),
            edges,
            "var_edge_to_check_edge length must equal edges"
        );

        let d_check_row_ptr = DeviceBuffer::<i32>::new(m + 1, device_id)?;
        d_check_row_ptr.copy_from_host(&layout.check_row_ptr)?;
        let d_check_edge_var = DeviceBuffer::<i32>::new(edges.max(1), device_id)?;
        d_check_edge_var.copy_from_host(&layout.check_edge_var)?;
        let d_check_edge_to_var_edge = DeviceBuffer::<i32>::new(edges.max(1), device_id)?;
        d_check_edge_to_var_edge.copy_from_host(&layout.check_edge_to_var_edge)?;
        let d_var_col_ptr = DeviceBuffer::<i32>::new(n + 1, device_id)?;
        d_var_col_ptr.copy_from_host(&layout.var_col_ptr)?;
        let d_var_edge_to_check_edge = DeviceBuffer::<i32>::new(edges.max(1), device_id)?;
        d_var_edge_to_check_edge.copy_from_host(&layout.var_edge_to_check_edge)?;

        let d_channel = DeviceBuffer::<f32>::new(max_batch * n, device_id)?;
        let d_v2c = DeviceBuffer::<f32>::new(max_batch * edges.max(1), device_id)?;
        let d_c2v = DeviceBuffer::<f32>::new(max_batch * edges.max(1), device_id)?;
        let d_hard = DeviceBuffer::<u8>::new(max_batch * n, device_id)?;
        let d_unsatisfied = DeviceBuffer::<u8>::new(max_batch.max(1), device_id)?;
        let d_frame_done = DeviceBuffer::<u8>::new(max_batch.max(1), device_id)?;

        Ok(Self {
            d_check_row_ptr,
            d_check_edge_var,
            d_check_edge_to_var_edge,
            d_var_col_ptr,
            d_var_edge_to_check_edge,
            d_channel,
            d_v2c,
            d_c2v,
            d_hard,
            d_unsatisfied,
            d_frame_done,
            n,
            m,
            edges,
            max_batch,
            device_id,
        })
    }

    /// Codeword length `n`.
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }

    /// Number of check nodes `m`.
    #[must_use]
    pub fn m(&self) -> usize {
        self.m
    }

    /// Maximum frames per decode call.
    #[must_use]
    pub fn max_batch(&self) -> usize {
        self.max_batch
    }

    /// The device this decoder's buffers are bound to.
    #[must_use]
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Allocates the pinned host staging the stream-ordered decode variants
    /// require.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if a pinned allocation fails (an OOM is
    /// [`HipError::OutOfMemory`]).
    ///
    /// # Complexity
    ///
    /// O(`max_batch * n`) pinned host memory.
    pub fn new_stream_scratch(&self) -> Result<LdpcStreamScratch, HipError> {
        Ok(LdpcStreamScratch {
            channel: PinnedHostBuffer::new(self.max_batch * self.n, self.device_id)?,
            hard: PinnedHostBuffer::new(self.max_batch * self.n, self.device_id)?,
            unsat: PinnedHostBuffer::new(self.max_batch.max(1), self.device_id)?,
            done: PinnedHostBuffer::new(self.max_batch.max(1), self.device_id)?,
            max_batch: self.max_batch,
            n: self.n,
        })
    }

    /// Decodes a batch of channel-LLR frames, one length-`n` vector each, to
    /// their `n`-bit hard-decision codewords (`true` = bit 1).
    ///
    /// Runs the flooding BP schedule for up to `max_iterations`. With
    /// `early_termination` each frame is frozen at the first iteration its
    /// syndrome passes, and the loop stops once every frame is frozen.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or synchronization
    /// failure.
    ///
    /// # Panics
    ///
    /// Panics if `llr_blocks.len() > max_batch`, any block length != `n`, or
    /// `max_iterations == 0`.
    ///
    /// # Complexity
    ///
    /// O(`max_iterations * batch * edges`) device work, plus one `batch`-byte
    /// read-back per iteration when `early_termination` is set.
    pub fn decode_batch(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
    ) -> Result<Vec<Vec<bool>>, HipError> {
        let (hard, _iters) =
            self.decode_batch_with_iters(llr_blocks, algorithm, max_iterations, early_termination)?;
        Ok(hard)
    }

    /// Like [`decode_batch`](Self::decode_batch), and also returns the
    /// per-frame BP iteration count in `1..=max_iterations`.
    ///
    /// A frame whose syndrome first passes at 0-indexed loop pass `i` reports
    /// `i + 1`; a frame that never converges, or any frame when
    /// `early_termination == false`, reports `max_iterations`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or synchronization
    /// failure.
    ///
    /// # Panics
    ///
    /// Panics if `llr_blocks.len() > max_batch`, any block length != `n`, or
    /// `max_iterations == 0`.
    pub fn decode_batch_with_iters(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
    ) -> Result<(Vec<Vec<bool>>, Vec<u32>), HipError> {
        self.decode_inner(
            llr_blocks,
            algorithm,
            max_iterations,
            early_termination,
            None,
        )
    }

    /// Like [`decode_batch`](Self::decode_batch), with every kernel launch and
    /// transfer enqueued on the caller-owned `stream` and completion awaited
    /// with [`HipStream::synchronize`]. `scratch` comes from
    /// [`new_stream_scratch`](Self::new_stream_scratch).
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or stream
    /// synchronization failure.
    ///
    /// # Panics
    ///
    /// Panics if `llr_blocks.len() > max_batch`, any block length != `n`,
    /// `max_iterations == 0`, or `scratch` was sized for a different decoder
    /// (`max_batch` / `n` mismatch).
    pub fn decode_batch_on_stream(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
        stream: &HipStream,
        scratch: &mut LdpcStreamScratch,
    ) -> Result<Vec<Vec<bool>>, HipError> {
        let (hard, _iters) = self.decode_batch_with_iters_on_stream(
            llr_blocks,
            algorithm,
            max_iterations,
            early_termination,
            stream,
            scratch,
        )?;
        Ok(hard)
    }

    /// The stream-ordered form of
    /// [`decode_batch_with_iters`](Self::decode_batch_with_iters); see
    /// [`decode_batch_on_stream`](Self::decode_batch_on_stream) for the stream
    /// arguments, errors and panics.
    /// `test_decode_on_stream_matches_default_stream` checks on a two-frame
    /// batch that it returns the hard decisions and counts of the
    /// default-stream variant.
    pub fn decode_batch_with_iters_on_stream(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
        stream: &HipStream,
        scratch: &mut LdpcStreamScratch,
    ) -> Result<(Vec<Vec<bool>>, Vec<u32>), HipError> {
        assert_eq!(
            scratch.max_batch, self.max_batch,
            "LdpcStreamScratch max_batch {} does not match decoder max_batch {}",
            scratch.max_batch, self.max_batch
        );
        assert_eq!(
            scratch.n, self.n,
            "LdpcStreamScratch n {} does not match decoder n {}",
            scratch.n, self.n
        );
        self.decode_inner(
            llr_blocks,
            algorithm,
            max_iterations,
            early_termination,
            Some((stream, scratch)),
        )
    }

    /// `io == None` is the default-stream path; `io == Some((stream,
    /// scratch))` orders every launch and pinned-staged transfer on `stream`.
    fn decode_inner(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
        io: Option<(&HipStream, &mut LdpcStreamScratch)>,
    ) -> Result<(Vec<Vec<bool>>, Vec<u32>), HipError> {
        // Split the optional stream context once: `stream` is a copied shared
        // borrow (used for the launch handle and synchronize), `staging` keeps
        // the unique borrow over the pinned buffers.
        let (stream, mut staging): (Option<&HipStream>, Option<&mut LdpcStreamScratch>) = match io {
            Some((s, sc)) => (Some(s), Some(sc)),
            None => (None, None),
        };
        let stream_raw: *mut c_void = stream.map_or(ptr::null_mut(), HipStream::as_raw);

        let batch = llr_blocks.len();
        assert!(
            batch <= self.max_batch,
            "decode_batch: batch {batch} > max_batch {}",
            self.max_batch
        );
        assert!(max_iterations >= 1, "max_iterations must be >= 1");
        if batch == 0 {
            return Ok((Vec::new(), Vec::new()));
        }
        for (i, blk) in llr_blocks.iter().enumerate() {
            assert_eq!(
                blk.len(),
                self.n,
                "llr block {i} has length {}, expected n = {}",
                blk.len(),
                self.n
            );
        }

        let mut flat: Vec<f32> = Vec::with_capacity(batch * self.n);
        for blk in llr_blocks {
            flat.extend_from_slice(blk);
        }
        match (stream, staging.as_deref_mut()) {
            (Some(stream), Some(scratch)) => {
                scratch.channel.as_mut_slice()[..flat.len()].copy_from_slice(&flat);
                self.d_channel
                    .copy_from_pinned_async(&scratch.channel, stream)?;
            }
            _ => self.d_channel.copy_from_host(&flat)?,
        }

        let n = self.n as i32;
        let m = self.m as i32;
        let edges = self.edges as i32;
        let b = batch as i32;

        // `frame_done_ptr` is null when early termination is off, so no frame is
        // skipped.
        let mut frame_done_host = vec![0u8; batch];
        let frame_done_ptr: *const u8 = if early_termination {
            match (stream, staging.as_deref_mut()) {
                (Some(stream), Some(scratch)) => {
                    scratch.done.as_mut_slice()[..batch].copy_from_slice(&frame_done_host);
                    self.d_frame_done
                        .copy_from_pinned_async(&scratch.done, stream)?;
                }
                _ => self.d_frame_done.copy_from_host(&frame_done_host)?,
            }
            self.d_frame_done.as_ptr() as *const u8
        } else {
            ptr::null()
        };

        // SAFETY: all device pointers were allocated in `new` sized for
        // `max_batch` frames; `batch <= max_batch` and every block has length
        // `n` (asserted). The kernel writes only the leading `batch * edges`
        // v2c lanes. `stream_raw` is either null (default stream) or the
        // caller's live stream handle.
        check_hip(
            unsafe {
                ffi::launch_ldpc_init(
                    self.d_channel.as_ptr() as *const f32,
                    self.d_v2c.as_mut_ptr() as *mut f32,
                    self.d_var_col_ptr.as_ptr() as *const i32,
                    n,
                    edges,
                    b,
                    stream_raw,
                )
            },
            "launch_ldpc_init",
        )?;

        let alg = algorithm.code();
        let alpha = algorithm.alpha();
        let beta = algorithm.beta();

        let mut iters = vec![max_iterations as u32; batch];

        for _iter in 0..max_iterations {
            // SAFETY: device pointers from `new`; kernel reads `v2c`, writes
            // `c2v`, both sized `>= batch * edges`. `frame_done_ptr` is either
            // null (early-term off) or the live `[batch]` flag buffer.
            // `stream_raw` is null (default stream) or the caller's stream.
            check_hip(
                unsafe {
                    ffi::launch_ldpc_check_update(
                        self.d_v2c.as_ptr() as *const f32,
                        self.d_c2v.as_mut_ptr() as *mut f32,
                        self.d_check_row_ptr.as_ptr() as *const i32,
                        self.d_check_edge_to_var_edge.as_ptr() as *const i32,
                        frame_done_ptr,
                        m,
                        edges,
                        b,
                        alg,
                        alpha,
                        beta,
                        stream_raw,
                    )
                },
                "launch_ldpc_check_update",
            )?;

            // SAFETY: device pointers from `new`; kernel reads `channel`/`c2v`,
            // writes `v2c` and `hard_bits` (sized `>= batch * n`). `stream_raw`
            // is null (default stream) or the caller's stream.
            check_hip(
                unsafe {
                    ffi::launch_ldpc_var_update(
                        self.d_channel.as_ptr() as *const f32,
                        self.d_v2c.as_mut_ptr() as *mut f32,
                        self.d_c2v.as_ptr() as *const f32,
                        self.d_var_col_ptr.as_ptr() as *const i32,
                        self.d_var_edge_to_check_edge.as_ptr() as *const i32,
                        self.d_hard.as_mut_ptr() as *mut u8,
                        frame_done_ptr,
                        n,
                        edges,
                        b,
                        stream_raw,
                    )
                },
                "launch_ldpc_var_update",
            )?;

            if early_termination {
                // A frame whose syndrome passes this iteration is frozen from
                // the next one, so its hard decision is the first-convergence
                // codeword. The previous pass's stream synchronize drained any
                // in-flight D2H into `unsat`, so the host-side refill is
                // race-free.
                match (stream, staging.as_deref_mut()) {
                    (Some(stream), Some(scratch)) => {
                        scratch.unsat.as_mut_slice()[..batch].fill(0);
                        self.d_unsatisfied
                            .copy_from_pinned_async(&scratch.unsat, stream)?;
                    }
                    _ => self.clear_unsatisfied(batch)?,
                }
                // SAFETY: device pointers from `new`; kernel reads `hard_bits`,
                // writes the leading `batch` `frame_unsatisfied` bytes; skips
                // frames flagged in `frame_done_ptr`. `stream_raw` is null
                // (default stream) or the caller's stream.
                check_hip(
                    unsafe {
                        ffi::launch_ldpc_syndrome(
                            self.d_hard.as_ptr() as *const u8,
                            self.d_check_row_ptr.as_ptr() as *const i32,
                            self.d_check_edge_var.as_ptr() as *const i32,
                            self.d_unsatisfied.as_mut_ptr() as *mut u8,
                            frame_done_ptr,
                            m,
                            n,
                            b,
                            stream_raw,
                        )
                    },
                    "launch_ldpc_syndrome",
                )?;
                let mut flags = vec![0u8; batch];
                match (stream, staging.as_deref_mut()) {
                    (Some(stream), Some(scratch)) => {
                        self.d_unsatisfied
                            .copy_to_pinned_async(&mut scratch.unsat, stream)?;
                        stream.synchronize()?;
                        flags.copy_from_slice(&scratch.unsat.as_slice()[..batch]);
                    }
                    _ => {
                        // SAFETY: hipDeviceSynchronize blocks until the launches
                        // above complete; no preconditions.
                        check_hip(
                            unsafe { ffi::hip_device_synchronize() },
                            "hipDeviceSynchronize",
                        )?;
                        self.d_unsatisfied.copy_to_host(&mut flags)?;
                    }
                }

                // `frame_done` only transitions 0 -> 1.
                let mut all_done = true;
                for f in 0..batch {
                    if frame_done_host[f] == 0 && flags[f] == 0 {
                        frame_done_host[f] = 1;
                        iters[f] = _iter as u32 + 1;
                    }
                    if frame_done_host[f] == 0 {
                        all_done = false;
                    }
                }
                if all_done {
                    break;
                }
                // The stream was synchronized above, so the refill is race-free.
                match (stream, staging.as_deref_mut()) {
                    (Some(stream), Some(scratch)) => {
                        scratch.done.as_mut_slice()[..batch].copy_from_slice(&frame_done_host);
                        self.d_frame_done
                            .copy_from_pinned_async(&scratch.done, stream)?;
                    }
                    _ => self.d_frame_done.copy_from_host(&frame_done_host)?,
                }
            }
        }

        // A run that never early-terminates has not synchronized yet.
        let mut hard = vec![0u8; batch * self.n];
        match (stream, staging) {
            (Some(stream), Some(scratch)) => {
                self.d_hard
                    .copy_to_pinned_async(&mut scratch.hard, stream)?;
                stream.synchronize()?;
                hard.copy_from_slice(&scratch.hard.as_slice()[..batch * self.n]);
            }
            _ => {
                // SAFETY: blocks until all preceding default-stream work completes.
                check_hip(
                    unsafe { ffi::hip_device_synchronize() },
                    "hipDeviceSynchronize",
                )?;
                self.d_hard.copy_to_host(&mut hard)?;
            }
        }

        let mut out = Vec::with_capacity(batch);
        for f in 0..batch {
            let row = &hard[f * self.n..(f + 1) * self.n];
            out.push(row.iter().map(|&x| x != 0).collect());
        }
        Ok((out, iters))
    }

    /// Zeroes the leading `batch` per-frame unsatisfied flags.
    fn clear_unsatisfied(&self, batch: usize) -> Result<(), HipError> {
        let zeros = vec![0u8; batch];
        self.d_unsatisfied.copy_from_host(&zeros)
    }
}

// `GpuLdpcBp` is `Send` by auto-derive; this assertion keeps it so.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<GpuLdpcBp>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::ldpc::{min_sum_check_row, MinSumRule};
    use gf2_coding::llr::Llr;

    fn assert_min_sum_contract_matches_cpu(values: &[f32]) {
        let inputs: Vec<Llr> = values.iter().copied().map(Llr::new).collect();
        for (gpu_algorithm, cpu_rule) in [
            (GpuBpAlgorithm::MinSum, MinSumRule::Plain),
            (
                GpuBpAlgorithm::NormalizedMinSum(0.75),
                MinSumRule::Normalized(0.75),
            ),
            (GpuBpAlgorithm::OffsetMinSum(0.5), MinSumRule::Offset(0.5)),
        ] {
            let mut cpu_outputs = vec![Llr::zero(); values.len()];
            min_sum_check_row(cpu_rule, &inputs, &mut cpu_outputs);
            for (excluded, cpu_output) in cpu_outputs.iter().enumerate() {
                // SAFETY: `values` remains alive for the call, its pointer and
                // length describe the whole slice, and `excluded` is produced
                // by iterating that slice's output positions.
                let gpu_contract = unsafe {
                    ffi::gf2_ldpc_min_sum_contract_reduce(
                        values.as_ptr(),
                        values.len() as i32,
                        excluded as i32,
                        gpu_algorithm.code(),
                        gpu_algorithm.alpha(),
                        gpu_algorithm.beta(),
                    )
                };
                assert_eq!(
                    gpu_contract.to_bits(),
                    cpu_output.value().to_bits(),
                    "{gpu_algorithm:?}, output {excluded} of {values:?}"
                );
            }
        }
    }

    #[test]
    fn min_sum_host_device_contract_matches_cpu_on_signed_zero_and_nan() {
        let negative_nan = f32::from_bits(f32::NAN.to_bits() | (1_u32 << 31));
        assert_min_sum_contract_matches_cpu(&[-0.0, 1.0, 2.0]);
        assert_min_sum_contract_matches_cpu(&[0.0, -0.0, -3.0]);
        assert_min_sum_contract_matches_cpu(&[f32::NAN, 2.0, -1.0]);
        assert_min_sum_contract_matches_cpu(&[negative_nan, 2.0, -1.0]);
        assert_min_sum_contract_matches_cpu(&[f32::NAN, f32::NAN]);
        assert_min_sum_contract_matches_cpu(&[f32::NAN, -0.0, -2.0]);
    }

    #[cfg(feature = "hip")]
    #[test]
    fn device_min_sum_matches_cpu_on_signed_zero_and_nan() {
        match crate::host::device_mem_info() {
            Ok((free, total)) => {
                eprintln!("device-memory free={free} total={total}");
            }
            Err(error) if std::env::var_os("GF2_REQUIRE_GPU").is_some() => {
                panic!("GF2_REQUIRE_GPU is set but device discovery failed: {error}");
            }
            Err(error) => {
                eprintln!("skipping device min-sum conformance: {error}");
                return;
            }
        }

        let positive_nan = f32::from_bits(0x7fc0_0000);
        let negative_nan = f32::from_bits(0xffc0_0000);
        let cases = [
            ("negative-zero", [-0.0, 1.0, 2.0]),
            ("both-zeros", [0.0, -0.0, -3.0]),
            ("positive-nan", [positive_nan, 2.0, -1.0]),
            ("negative-nan", [negative_nan, 2.0, -1.0]),
            ("all-nan", [positive_nan, negative_nan, positive_nan]),
            ("nan-and-negative-zero", [positive_nan, -0.0, -2.0]),
        ];
        let layout = LdpcGraphLayout {
            n: 3,
            m: 1,
            check_row_ptr: vec![0, 3],
            check_edge_var: vec![0, 1, 2],
            check_edge_to_var_edge: vec![0, 1, 2],
            var_col_ptr: vec![0, 1, 2, 3],
            var_edge_to_check_edge: vec![0, 1, 2],
        };
        let decoder = GpuLdpcBp::new(&layout, cases.len(), 0)
            .expect("allocate the device check-update fixture");
        let device_inputs: Vec<f32> = cases
            .iter()
            .flat_map(|(_, values)| values.iter().copied())
            .collect();
        decoder
            .d_v2c
            .copy_from_host(&device_inputs)
            .expect("upload device check-update inputs");

        for (gpu_algorithm, cpu_rule) in [
            (GpuBpAlgorithm::MinSum, MinSumRule::Plain),
            (
                GpuBpAlgorithm::NormalizedMinSum(0.75),
                MinSumRule::Normalized(0.75),
            ),
            (GpuBpAlgorithm::OffsetMinSum(0.5), MinSumRule::Offset(0.5)),
        ] {
            // SAFETY: every device buffer belongs to `decoder`; `d_v2c` and
            // `d_c2v` contain `cases.len() * 3` lanes, the uploaded layout has
            // one three-edge check, and the null stream/frame flags select the
            // synchronized default-stream path without dereferencing null.
            check_hip(
                unsafe {
                    ffi::launch_ldpc_check_update(
                        decoder.d_v2c.as_ptr() as *const f32,
                        decoder.d_c2v.as_mut_ptr() as *mut f32,
                        decoder.d_check_row_ptr.as_ptr() as *const i32,
                        decoder.d_check_edge_to_var_edge.as_ptr() as *const i32,
                        ptr::null(),
                        1,
                        3,
                        cases.len() as i32,
                        gpu_algorithm.code(),
                        gpu_algorithm.alpha(),
                        gpu_algorithm.beta(),
                        ptr::null_mut(),
                    )
                },
                "launch_ldpc_check_update(device conformance)",
            )
            .expect("launch device check update");
            // SAFETY: the default-stream launch above is valid and this call
            // only waits for outstanding device work to finish.
            check_hip(
                unsafe { ffi::hip_device_synchronize() },
                "hipDeviceSynchronize(device conformance)",
            )
            .expect("synchronize device check update");

            let mut device_outputs = vec![0.0_f32; device_inputs.len()];
            decoder
                .d_c2v
                .copy_to_host(&mut device_outputs)
                .expect("download device check-update outputs");
            for (case_index, (label, values)) in cases.iter().enumerate() {
                let inputs: Vec<Llr> = values.iter().copied().map(Llr::new).collect();
                let mut cpu_outputs = vec![Llr::zero(); values.len()];
                min_sum_check_row(cpu_rule, &inputs, &mut cpu_outputs);
                let device_row = &device_outputs[case_index * 3..(case_index + 1) * 3];
                let device_bits: Vec<u32> =
                    device_row.iter().map(|value| value.to_bits()).collect();
                let cpu_bits: Vec<u32> = cpu_outputs
                    .iter()
                    .map(|value| value.value().to_bits())
                    .collect();
                eprintln!(
                    "device-min-sum algorithm={gpu_algorithm:?} case={label} \
                     output-bits={device_bits:08x?}"
                );
                assert_eq!(
                    device_bits, cpu_bits,
                    "device {gpu_algorithm:?}, case {label}, inputs {values:?}"
                );
            }
        }
    }

    #[test]
    fn test_algorithm_code_and_params() {
        assert_eq!(GpuBpAlgorithm::MinSum.code(), 0);
        assert_eq!(GpuBpAlgorithm::NormalizedMinSum(0.75).code(), 1);
        assert_eq!(GpuBpAlgorithm::OffsetMinSum(0.5).code(), 2);
        assert_eq!(GpuBpAlgorithm::SumProduct.code(), 3);

        assert_eq!(GpuBpAlgorithm::NormalizedMinSum(0.75).alpha(), 0.75);
        assert_eq!(GpuBpAlgorithm::MinSum.alpha(), 1.0);
        assert_eq!(GpuBpAlgorithm::OffsetMinSum(0.5).beta(), 0.5);
        assert_eq!(GpuBpAlgorithm::MinSum.beta(), 0.0);
    }

    #[test]
    fn test_layout_edges_count() {
        let layout = LdpcGraphLayout {
            n: 3,
            m: 1,
            check_row_ptr: vec![0, 3],
            check_edge_var: vec![0, 1, 2],
            check_edge_to_var_edge: vec![0, 1, 2],
            var_col_ptr: vec![0, 1, 2, 3],
            var_edge_to_check_edge: vec![0, 1, 2],
        };
        assert_eq!(layout.edges(), 3);
    }

    #[cfg(feature = "hip")]
    #[test]
    fn test_decode_on_stream_matches_default_stream() {
        let layout = LdpcGraphLayout {
            n: 3,
            m: 1,
            check_row_ptr: vec![0, 3],
            check_edge_var: vec![0, 1, 2],
            check_edge_to_var_edge: vec![0, 1, 2],
            var_col_ptr: vec![0, 1, 2, 3],
            var_edge_to_check_edge: vec![0, 1, 2],
        };
        let dec = GpuLdpcBp::new(&layout, 4, 0).expect("build decoder");
        // Frame 0 satisfies the single parity check immediately (all zeros);
        // frame 1 violates it (odd parity), so BP iterates — both the
        // early-freeze and the iterate paths are exercised.
        let llrs = vec![vec![2.0f32, 2.0, 2.0], vec![2.0, -2.0, 2.0]];

        let (hard_default, iters_default) = dec
            .decode_batch_with_iters(&llrs, GpuBpAlgorithm::SumProduct, 10, true)
            .expect("default-stream decode");

        let stream = HipStream::new().expect("create stream");
        let mut scratch = dec.new_stream_scratch().expect("pinned staging");
        let (hard_stream, iters_stream) = dec
            .decode_batch_with_iters_on_stream(
                &llrs,
                GpuBpAlgorithm::SumProduct,
                10,
                true,
                &stream,
                &mut scratch,
            )
            .expect("stream-ordered decode");

        assert_eq!(hard_default, hard_stream, "hard decisions must match");
        assert_eq!(iters_default, iters_stream, "iteration counts must match");
    }
}
