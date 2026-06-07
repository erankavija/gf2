//! Deterministic kernel-launch helpers (design doc §6 / §11).
//!
//! The pipeline's CPU-vs-GPU determinism contract (design doc §11) requires
//! that the three columns `fer` / `frames` / `errors` be byte-identical across
//! CPU-only and CPU+GPU runs at a fixed seed. The host launch path supports
//! that by mandating **fixed grid + block dimensions** (derived purely from the
//! problem size, never from runtime occupancy heuristics) and forbidding
//! kernel constructs that introduce nondeterminism:
//!
//! - **no atomic f32 reductions** — floating-point atomics commute results in
//!   hardware-scheduling order, which is not reproducible;
//! - **no unspecified-order dispatch** — every kernel must process its batch in
//!   a fixed, index-derived order.
//!
//! These are kernel-authoring obligations (the kernels land next wave via
//! `f6004add` / `a930be7f` / `d3f1616a`); this module gives those kernels a
//! single, reviewed way to compute their launch geometry so the obligation is
//! mechanically enforced at the launch boundary rather than restated per
//! kernel.
//!
//! Note: the pre-existing in-crate launch sites in `lib.rs`
//! (`launch_bcjr_batch`, `launch_gray_qam_demap`) compute their grid/block
//! inside the `.hip` kernel sources and predate this helper; rewiring them to
//! `LaunchDims` would alter their launch geometry and is therefore deferred to
//! the Phase B kernel owners, who adopt `LaunchDims` as they bring the
//! per-arch `.co` blobs online.

/// The fixed block size (threads per block) the pipeline kernels launch with.
///
/// A single compile-time constant keeps every kernel's geometry reproducible
/// across launches; 256 is a portable choice across the gfx targets in design
/// doc §6 (a multiple of the 32-/64-lane wavefront on every listed arch).
pub const MAX_BLOCK_THREADS: u32 = 256;

/// A fully specified, deterministic launch geometry.
///
/// Constructed only via [`LaunchDims::for_batch`] (or [`LaunchDims::explicit`]
/// for kernels with a bespoke 1-D mapping), so the grid is always a pure
/// function of the problem size — never of a runtime occupancy query. This is
/// what makes the launch reproducible run-to-run and host-to-host for a fixed
/// problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchDims {
    /// Number of blocks in the (1-D) grid.
    pub grid_x: u32,
    /// Threads per block.
    pub block_x: u32,
}

impl LaunchDims {
    /// Computes a 1-D launch geometry that covers `n_elements` work items with
    /// [`MAX_BLOCK_THREADS`] threads per block.
    ///
    /// The grid is `ceil(n_elements / block)` blocks, computed in integer
    /// arithmetic with no occupancy heuristics, so the result is identical on
    /// every call with the same `n_elements`. A zero `n_elements` yields a
    /// zero-block grid (a no-op launch the caller should skip).
    ///
    /// # Arguments
    ///
    /// * `n_elements` - Total number of work items (e.g. batch size, or symbols).
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn for_batch(n_elements: usize) -> Self {
        let block = MAX_BLOCK_THREADS;
        let grid = if n_elements == 0 {
            0
        } else {
            // ceil(n_elements / block), saturating so an absurd n_elements
            // can't overflow u32 silently.
            let blocks = n_elements.div_ceil(block as usize);
            u32::try_from(blocks).unwrap_or(u32::MAX)
        };
        Self {
            grid_x: grid,
            block_x: block,
        }
    }

    /// Builds a launch geometry from explicit, caller-chosen dimensions.
    ///
    /// Used by kernels whose natural mapping is one block per batch element
    /// (e.g. the existing BCJR kernel launches `grid(batch_size)`,
    /// `block(1024)`). The dimensions must still be a pure function of the
    /// problem size for the determinism contract to hold; this constructor does
    /// not enforce that, it only records the choice.
    ///
    /// # Panics
    ///
    /// Panics if `block_x == 0` (a launch with no threads is always a bug).
    pub fn explicit(grid_x: u32, block_x: u32) -> Self {
        assert!(block_x > 0, "LaunchDims::explicit: block_x must be > 0");
        Self { grid_x, block_x }
    }

    /// Returns `true` if this geometry launches no blocks (a no-op the caller
    /// should skip rather than dispatch).
    pub fn is_empty(&self) -> bool {
        self.grid_x == 0
    }

    /// Total number of threads the launch spans (`grid_x * block_x`),
    /// saturating on overflow.
    pub fn total_threads(&self) -> u64 {
        (self.grid_x as u64).saturating_mul(self.block_x as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_batch_exact_multiple() {
        let d = LaunchDims::for_batch(MAX_BLOCK_THREADS as usize * 3);
        assert_eq!(d.grid_x, 3);
        assert_eq!(d.block_x, MAX_BLOCK_THREADS);
        assert!(!d.is_empty());
    }

    #[test]
    fn test_for_batch_rounds_up() {
        let d = LaunchDims::for_batch(MAX_BLOCK_THREADS as usize + 1);
        assert_eq!(d.grid_x, 2);
    }

    #[test]
    fn test_for_batch_zero_is_empty() {
        let d = LaunchDims::for_batch(0);
        assert_eq!(d.grid_x, 0);
        assert!(d.is_empty());
    }

    #[test]
    fn test_for_batch_is_deterministic() {
        // Same input → identical geometry, every call (the determinism contract).
        for n in [1usize, 255, 256, 257, 1000, 65_536] {
            assert_eq!(LaunchDims::for_batch(n), LaunchDims::for_batch(n));
        }
    }

    #[test]
    fn test_explicit_records_dims() {
        let d = LaunchDims::explicit(42, 1024);
        assert_eq!(d.grid_x, 42);
        assert_eq!(d.block_x, 1024);
        assert_eq!(d.total_threads(), 42 * 1024);
    }

    #[test]
    #[should_panic(expected = "block_x must be > 0")]
    fn test_explicit_rejects_zero_block() {
        let _ = LaunchDims::explicit(1, 0);
    }
}
