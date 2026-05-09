//! Gray-code subset enumeration used by Ryser's permanent formula and the
//! `permanent_bipedal*` kernels.
//!
//! Hosts the `gray_code_iter` enumerator that walks the `2^n` subsets of
//! a length-`n` vector by toggling one bit per step, plus its supporting
//! types. See `dev/plans/d1a_gf2_algebra_boundary.md` §4.2 for why this
//! lives in `gf2-algebra` rather than alongside the unrelated M4RM Gray
//! table in `gf2-core::alg::m4rm`.
//!
//! # Status
//!
//! W1-T1 skeleton — the enumerator is added by the W1 implementation
//! issues (T2-T6) and exercised by the bipedal permanent kernels.
