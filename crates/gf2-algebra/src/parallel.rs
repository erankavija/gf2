//! Rayon-based parallel dispatch for `permanent_bipedal*`.
//!
//! The work-stealing Gray-code-block schedule for `permanent_bipedal3_parallel`
//! (T15) lives in [`crate::permanent::parallel_bipedal3`], compiled only when
//! the `parallel` feature is enabled (default on). The W4 F_5 / F_7 analogues
//! will land in W4-T18/T20.
//!
//! # Status
//!
//! T15 complete. `permanent_bipedal3_parallel` and `CHUNK_SUBSETS` are in
//! `crate::permanent::parallel_bipedal3` and re-exported via
//! `crate::permanent::permanent_bipedal3_parallel`. This module is retained as
//! a top-level parallel scaffold for future parallel algorithms.
