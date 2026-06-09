//! Safe host wrappers for the device LDPC belief-propagation batch decoder
//! (`hip/ldpc_bp.hip`, design doc §6 / §10 / §11).
//!
//! [`GpuLdpcBp`] owns the device-resident Tanner-graph layout (a double CSR:
//! check-major + variable-major, with the two cross-maps that link a check-edge
//! to its matching variable-edge) plus the reusable per-batch message and
//! hard-decision buffers. It runs the same flooding BP schedule as the CPU
//! `gf2_coding::ldpc::core::LdpcDecoder` — init, then alternating check-node and
//! variable-node updates with optional per-iteration syndrome early-termination
//! — and returns the hard-decision codeword bits (all `n` positions), which are
//! byte-identical to the CPU decoder's hard decision (design doc §11: the
//! hard-decision verdict is robust to the 1-3 ULP RDNA2 transcendental drift).
//!
//! # Early-termination = per-frame freeze (matches the CPU loop)
//!
//! `LdpcDecoder::decode_to_codeword(.., early_termination=true)` freezes a
//! frame's hard decision at the FIRST iteration its syndrome passes. To
//! reproduce that bit-for-bit across a batch (where frames converge at different
//! iterations), the host maintains a per-frame `frame_done` flag: the moment a
//! frame's syndrome passes it is marked done, and every subsequent kernel skips
//! it, freezing its `hard_bits` / `v2c` / `c2v` at the first-convergence state
//! (also a perf win). The loop stops when all frames are done or `max_iters` is
//! reached. With early termination off no frame is frozen and all `max_iters`
//! run (matching the CPU `early_termination == false` path).
//!
//! # Why the graph layout is built by the caller
//!
//! `gf2-kernels-hip` owns all device FFI and the SAFETY-annotated launch path,
//! so the graph upload + iteration loop is a single reviewed unit here. The
//! `gf2-sim` `GpuLdpcBp` stage (the §8 fallback-bearing consumer) flattens its
//! `LdpcCode` into the [`LdpcGraphLayout`] CSR arrays and hands them to
//! [`GpuLdpcBp::new`] without touching FFI, preserving `gf2-sim`'s
//! `#![deny(unsafe_code)]`.

use std::ptr;

use crate::host::DeviceBuffer;
use crate::{check_hip, ffi, HipError};

/// Algorithm selector matching `gf2_coding::ldpc::DecoderAlgorithm` and the
/// `LDPC_ALG_*` constants in `hip/ldpc_bp.hip`.
///
/// The min-sum family carries its correction parameter inline; SumProduct has
/// none. The host wrapper unpacks this into the `(algorithm, alpha, beta)`
/// triple the kernel takes.
///
/// # Examples
///
/// ```
/// use gf2_kernels_hip::launch_ldpc_bp::GpuBpAlgorithm;
///
/// let a = GpuBpAlgorithm::NormalizedMinSum(0.75);
/// assert_eq!(a.code(), 1);
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_kernels_hip::launch_ldpc_bp::GpuBpAlgorithm;
    ///
    /// assert_eq!(GpuBpAlgorithm::MinSum.code(), 0);
    /// assert_eq!(GpuBpAlgorithm::SumProduct.code(), 3);
    /// ```
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

/// Host-side flat, standard-agnostic Tanner-graph representation the GPU LDPC BP
/// kernel decodes.
///
/// This is the double-CSR encoding the kernel consumes (see `hip/ldpc_bp.hip`):
/// the check-major CSR (`check_row_ptr`, `check_edge_var`), the variable-major
/// CSC (`var_col_ptr`), and the two cross-maps (`check_edge_to_var_edge`,
/// `var_edge_to_check_edge`) that identify the SAME Tanner edge from the two
/// views.
///
/// The caller (the `gf2-sim` stage, which owns the `LdpcCode`) builds this from
/// the parity-check matrix so that the kernel's check-node gather order is
/// **exactly** the CPU decoder's `check_neighbors` (CSR `row_iter`) order and
/// the variable-node belief sum order is exactly the `var_neighbors` (CSC
/// `col_iter`) order — the basis of the CPU↔GPU byte-identity of the hard
/// decision.
///
/// # Standard-agnostic by construction (design doc §6 shared binary)
///
/// This layout is the standard seam: the kernel decodes whatever flat Tanner
/// graph is encoded here, with no notion of DVB-T2 vs 5G NR and no in-kernel
/// shift parameter. DVB-T2 builds this from a fully-expanded parity-check matrix
/// today. 5G NR support is a Phase E (`23d3525f`) **constructor** that expands a
/// base graph + per-`i_LS` lifting-set shift table into this same flat layout
/// host-side; the kernel binary is reused unchanged.
///
/// # Examples
///
/// ```
/// use gf2_kernels_hip::launch_ldpc_bp::LdpcGraphLayout;
///
/// // A single check connecting variables {0, 1, 2}: one CSR row of 3 edges.
/// let layout = LdpcGraphLayout {
///     n: 3,
///     m: 1,
///     check_row_ptr: vec![0, 3],
///     check_edge_var: vec![0, 1, 2],
///     check_edge_to_var_edge: vec![0, 1, 2],
///     var_col_ptr: vec![0, 1, 2, 3],
///     var_edge_to_check_edge: vec![0, 1, 2],
/// };
/// assert_eq!(layout.edges(), 3);
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_kernels_hip::launch_ldpc_bp::LdpcGraphLayout;
    ///
    /// let layout = LdpcGraphLayout {
    ///     n: 2, m: 1,
    ///     check_row_ptr: vec![0, 2],
    ///     check_edge_var: vec![0, 1],
    ///     check_edge_to_var_edge: vec![0, 1],
    ///     var_col_ptr: vec![0, 1, 2],
    ///     var_edge_to_check_edge: vec![0, 1],
    /// };
    /// assert_eq!(layout.edges(), 2);
    /// ```
    #[must_use]
    pub fn edges(&self) -> usize {
        self.check_edge_var.len()
    }
}

/// A reusable device-side LDPC belief-propagation batch decoder.
///
/// Holds the persistent device-resident graph layout (uploaded once) plus the
/// reusable per-batch message buffers (`v2c`, `c2v`), channel-LLR input, and
/// hard-decision output, sized for up to `max_batch` frames at construction.
/// Repeated [`decode_batch`](Self::decode_batch) calls reuse the same
/// allocations.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_hip::launch_ldpc_bp::{GpuBpAlgorithm, GpuLdpcBp, LdpcGraphLayout};
///
/// // Requires a real HIP device, so this is `no_run`.
/// let layout = LdpcGraphLayout {
///     n: 3, m: 1,
///     check_row_ptr: vec![0, 3],
///     check_edge_var: vec![0, 1, 2],
///     check_edge_to_var_edge: vec![0, 1, 2],
///     var_col_ptr: vec![0, 1, 2, 3],
///     var_edge_to_check_edge: vec![0, 1, 2],
/// };
/// let dec = GpuLdpcBp::new(&layout, 8, 0).expect("build decoder");
/// let llrs = vec![vec![2.0f32, 2.0, 2.0]];
/// let bits = dec
///     .decode_batch(&llrs, GpuBpAlgorithm::SumProduct, 50, true)
///     .expect("decode");
/// assert_eq!(bits[0].len(), 3);
/// ```
pub struct GpuLdpcBp {
    // Persistent graph layout (uploaded once).
    d_check_row_ptr: DeviceBuffer<i32>,
    d_check_edge_var: DeviceBuffer<i32>,
    d_check_edge_to_var_edge: DeviceBuffer<i32>,
    d_var_col_ptr: DeviceBuffer<i32>,
    d_var_edge_to_check_edge: DeviceBuffer<i32>,
    // Per-batch reusable buffers.
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
    /// The graph layout is uploaded once; the per-batch message buffers
    /// (`max_batch * edges` f32s each), channel-LLR input (`max_batch * n`),
    /// and hard-decision output (`max_batch * n` bytes) are allocated up front.
    ///
    /// # Arguments
    ///
    /// * `layout` — the flattened Tanner-graph layout.
    /// * `max_batch` — maximum frames per decode call (sizes device buffers).
    /// * `device_id` — the HIP device to allocate on.
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

    /// Decodes a batch of channel-LLR frames to their hard-decision codewords.
    ///
    /// Runs the flooding BP schedule (init → alternating check / variable
    /// updates) for up to `max_iterations`. When `early_termination` is set each
    /// frame is **frozen** at the first iteration its syndrome passes (its
    /// `hard_bits` / messages are not touched again), exactly matching the CPU
    /// `decode_to_codeword` first-convergence break; the host loop stops once
    /// every frame is frozen. The returned bits are the full `n`-bit
    /// hard-decision codeword per frame (`true` = bit 1).
    ///
    /// # Arguments
    ///
    /// * `llr_blocks` — one channel-LLR vector of length `n` per frame.
    /// * `algorithm` — the box-plus rule (with its correction parameter).
    /// * `max_iterations` — BP iteration cap.
    /// * `early_termination` — freeze each frame at first convergence and stop
    ///   the loop once all frames are frozen.
    ///
    /// # Returns
    ///
    /// One `Vec<bool>` of length `n` per frame (the hard-decision codeword).
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
    /// O(`max_iterations * batch * edges`) device work (less under early
    /// termination as frozen frames are skipped); host-side cost is the per-call
    /// H2D of `batch * n` f32s and the D2H of `batch * n` bytes, plus one
    /// `batch`-byte read-back per iteration when `early_termination` is set.
    pub fn decode_batch(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
    ) -> Result<Vec<Vec<bool>>, HipError> {
        // Delegate to the iteration-counting variant and drop the counts; the
        // hard-decision output is byte-for-byte identical to the standalone loop.
        let (hard, _iters) =
            self.decode_batch_with_iters(llr_blocks, algorithm, max_iterations, early_termination)?;
        Ok(hard)
    }

    /// Like [`decode_batch`](Self::decode_batch), but also returns the per-frame
    /// BP iteration count.
    ///
    /// The hard-decision codewords are **byte-for-byte identical** to
    /// [`decode_batch`](Self::decode_batch) (that method delegates here and
    /// discards the counts); this is a purely additive observability API.
    ///
    /// # Iteration-count convention (aligned to the CPU `decode_to_codeword`)
    ///
    /// Each host-loop pass runs one check-node + variable-node update, then
    /// (when `early_termination` is set) tests the syndrome — exactly the CPU
    /// `decode_to_codeword` shape, whose reported count is `iter + 1` at the
    /// pass that first passes the syndrome. So:
    ///
    /// * A frame that freezes (syndrome passes) at 0-indexed loop pass `i`
    ///   reports `i + 1` — identical to the CPU count for the same convergence
    ///   pass.
    /// * A frame that never converges (or `early_termination == false`) reports
    ///   `max_iterations`, matching the CPU loop that runs the full cap.
    ///
    /// The counts are diagnostic only: per design-doc §11 `mean_iters` is
    /// EXCLUDED from CPU-vs-GPU byte-identity (RDNA2 transcendental ULP drift can
    /// shift the convergence pass by ±1), so a caller may LOG but must not ASSERT
    /// the CPU-vs-GPU iteration diff.
    ///
    /// # Arguments
    ///
    /// Same as [`decode_batch`](Self::decode_batch).
    ///
    /// # Returns
    ///
    /// `(hard, iters)` where `hard` is one `Vec<bool>` of length `n` per frame
    /// (the hard-decision codeword) and `iters[f]` is frame `f`'s BP iteration
    /// count (`1..=max_iterations`).
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
    /// Identical to [`decode_batch`](Self::decode_batch); the per-frame count is
    /// derived from the freeze bookkeeping the early-termination path already
    /// maintains (no extra device work).
    pub fn decode_batch_with_iters(
        &self,
        llr_blocks: &[Vec<f32>],
        algorithm: GpuBpAlgorithm,
        max_iterations: usize,
        early_termination: bool,
    ) -> Result<(Vec<Vec<bool>>, Vec<u32>), HipError> {
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

        // Flatten + upload channel LLRs (batch-major).
        let mut flat: Vec<f32> = Vec::with_capacity(batch * self.n);
        for blk in llr_blocks {
            flat.extend_from_slice(blk);
        }
        self.d_channel.copy_from_host(&flat)?;

        let n = self.n as i32;
        let m = self.m as i32;
        let edges = self.edges as i32;
        let b = batch as i32;

        // The per-frame freeze flags pointer is null when early termination is
        // off (no frame is ever frozen — all `max_iters` run, matching CPU).
        // Otherwise the flags start cleared (all frames active) and are flipped
        // to 1 as frames converge.
        let mut frame_done_host = vec![0u8; batch];
        let frame_done_ptr: *const u8 = if early_termination {
            self.d_frame_done.copy_from_host(&frame_done_host)?;
            self.d_frame_done.as_ptr() as *const u8
        } else {
            ptr::null()
        };

        // Init v2c = channel LLRs.
        // SAFETY: all device pointers were allocated in `new` sized for
        // `max_batch` frames; `batch <= max_batch` and every block has length
        // `n` (asserted). The kernel writes only the leading `batch * edges`
        // v2c lanes.
        check_hip(
            unsafe {
                ffi::launch_ldpc_init(
                    self.d_channel.as_ptr() as *const f32,
                    self.d_v2c.as_mut_ptr() as *mut f32,
                    self.d_var_col_ptr.as_ptr() as *const i32,
                    n,
                    edges,
                    b,
                    ptr::null_mut(),
                )
            },
            "launch_ldpc_init",
        )?;

        let alg = algorithm.code();
        let alpha = algorithm.alpha();
        let beta = algorithm.beta();

        // Per-frame BP iteration count, CPU-aligned. A frame that never freezes
        // (or with early termination off) reports the full `max_iterations`; a
        // frame that freezes at 0-indexed pass `i` is overwritten to `i + 1`
        // below (matching the CPU `iterations = iter + 1`).
        let mut iters = vec![max_iterations as u32; batch];

        for _iter in 0..max_iterations {
            // Check-node update. Frozen frames (when early-term on) are skipped
            // device-side via `frame_done_ptr`.
            // SAFETY: device pointers from `new`; kernel reads `v2c`, writes
            // `c2v`, both sized `>= batch * edges`. `frame_done_ptr` is either
            // null (early-term off) or the live `[batch]` flag buffer.
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
                        ptr::null_mut(),
                    )
                },
                "launch_ldpc_check_update",
            )?;

            // Variable-node update (also writes the hard decision). Frozen
            // frames keep their first-convergence `hard_bits` / `v2c`.
            // SAFETY: device pointers from `new`; kernel reads `channel`/`c2v`,
            // writes `v2c` and `hard_bits` (sized `>= batch * n`).
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
                        ptr::null_mut(),
                    )
                },
                "launch_ldpc_var_update",
            )?;

            if early_termination {
                // Clear the per-frame unsatisfied flags, run the syndrome check
                // (frozen frames skipped — they stay satisfied), and read it
                // back. A frame whose syndrome passes THIS iteration is frozen
                // from the next one, so its hard decision is the first-convergence
                // codeword — matching the CPU `is_valid_codeword` break.
                self.clear_unsatisfied(batch)?;
                // SAFETY: device pointers from `new`; kernel reads `hard_bits`,
                // writes the leading `batch` `frame_unsatisfied` bytes; skips
                // frames flagged in `frame_done_ptr`.
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
                            ptr::null_mut(),
                        )
                    },
                    "launch_ldpc_syndrome",
                )?;
                // SAFETY: hipDeviceSynchronize blocks until the launches above
                // complete; no preconditions.
                check_hip(
                    unsafe { ffi::hip_device_synchronize() },
                    "hipDeviceSynchronize",
                )?;
                let mut flags = vec![0u8; batch];
                self.d_unsatisfied.copy_to_host(&mut flags)?;

                // Freeze every frame whose syndrome passed this iteration (an
                // active frame with no unsatisfied check). `frame_done` only ever
                // transitions 0 -> 1, so a frozen frame stays frozen.
                let mut all_done = true;
                for f in 0..batch {
                    if frame_done_host[f] == 0 && flags[f] == 0 {
                        frame_done_host[f] = 1; // converged this iteration: freeze
                                                // CPU convention: `iterations = iter + 1` at the pass
                                                // that first passes the syndrome.
                        iters[f] = _iter as u32 + 1;
                    }
                    if frame_done_host[f] == 0 {
                        all_done = false;
                    }
                }
                if all_done {
                    break;
                }
                // Upload the updated freeze flags for the next iteration's kernels.
                self.d_frame_done.copy_from_host(&frame_done_host)?;
            }
        }

        // Final sync (the early-term path already synced inside the loop, but a
        // run that never early-terminates needs this) and hard-decision D2H.
        // SAFETY: blocks until all preceding default-stream work completes.
        check_hip(
            unsafe { ffi::hip_device_synchronize() },
            "hipDeviceSynchronize",
        )?;
        let mut hard = vec![0u8; batch * self.n];
        self.d_hard.copy_to_host(&mut hard)?;

        let mut out = Vec::with_capacity(batch);
        for f in 0..batch {
            let row = &hard[f * self.n..(f + 1) * self.n];
            out.push(row.iter().map(|&x| x != 0).collect());
        }
        Ok((out, iters))
    }

    /// Zeroes the leading `batch` per-frame unsatisfied flags before a syndrome
    /// launch (a small H2D of zeros — `batch` is tiny relative to a frame).
    fn clear_unsatisfied(&self, batch: usize) -> Result<(), HipError> {
        let zeros = vec![0u8; batch];
        self.d_unsatisfied.copy_from_host(&zeros)
    }
}

// `GpuLdpcBp` is `Send` by auto-derive (every field is a `DeviceBuffer<_>`,
// which is `Send`, or a `Copy` scalar). It is deliberately NOT `Sync`: its
// `decode_batch` mutates device memory through `&self`, so it follows the
// per-worker-owned-buffer doctrine documented on `DeviceBuffer`.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<GpuLdpcBp>();
};

#[cfg(test)]
mod tests {
    use super::*;

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
}
