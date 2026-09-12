//! Rayon-based parallel dispatch for `permanent_bipedal*`.
//!
//! The work-stealing Gray-code-block schedule for `permanent_bipedal3_parallel`
//! (T15) lives in [`crate::permanent::parallel_bipedal3`], compiled only when
//! the `parallel` feature is enabled (default on). F_5 / F_7 single-word
//! analogues (`permanent_bipedal5` / `permanent_bipedal7`) landed in W4-T18/T20;
//! parallel companions for F_5 / F_7 remain a follow-up.
//!
//! # Status
//!
//! T15 complete. `permanent_bipedal3_parallel` and `CHUNK_SUBSETS` are in
//! `crate::permanent::parallel_bipedal3` and re-exported via
//! `crate::permanent::permanent_bipedal3_parallel`. This module is retained as
//! a top-level parallel scaffold for future parallel algorithms.
