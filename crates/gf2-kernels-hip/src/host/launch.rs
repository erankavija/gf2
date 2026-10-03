//! Fixed kernel-launch geometry.
//!
//! [`LaunchDims::for_batch`] derives the grid from the work-item count alone,
//! so its launch geometry is reproducible for a fixed problem.

/// The fixed block size (threads per block) of [`LaunchDims::for_batch`]: a
/// multiple of the 32- and 64-lane wavefront sizes.
pub const MAX_BLOCK_THREADS: u32 = 256;

/// A 1-D kernel-launch geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchDims {
    /// Number of blocks in the (1-D) grid.
    pub grid_x: u32,
    /// Threads per block.
    pub block_x: u32,
}

impl LaunchDims {
    /// Computes a 1-D launch geometry that covers `n_elements` work items with
    /// [`MAX_BLOCK_THREADS`] threads per block: `ceil(n_elements / block)`
    /// blocks, saturating at `u32::MAX`. Zero `n_elements` yields a zero-block
    /// grid, which the caller skips.
    pub fn for_batch(n_elements: usize) -> Self {
        let block = MAX_BLOCK_THREADS;
        let grid = if n_elements == 0 {
            0
        } else {
            let blocks = n_elements.div_ceil(block as usize);
            u32::try_from(blocks).unwrap_or(u32::MAX)
        };
        Self {
            grid_x: grid,
            block_x: block,
        }
    }

    /// Builds a launch geometry from caller-chosen dimensions.
    ///
    /// # Panics
    ///
    /// Panics if `block_x == 0`.
    pub fn explicit(grid_x: u32, block_x: u32) -> Self {
        assert!(block_x > 0, "LaunchDims::explicit: block_x must be > 0");
        Self { grid_x, block_x }
    }

    /// Returns `true` if this geometry launches no blocks.
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
