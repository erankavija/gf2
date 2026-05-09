//! Rayon-based parallel dispatch for `permanent_bipedal*`.
//!
//! Will host the work-stealing Gray-code-block schedule that drives
//! `permanent_bipedal3_par` (and the W4 F_5 / F_7 analogues) per the
//! epic design §10 / §11. Compiled only when the `parallel` Cargo
//! feature is enabled (default on).
//!
//! # Status
//!
//! W1-T1 skeleton — empty placeholder. Body lands in W3.
