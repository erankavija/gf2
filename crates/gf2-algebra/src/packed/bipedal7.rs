//! Packed `F_7` element / vector / matrix encoding ("bipedal7").
//!
//! Will host the `Bipedal7*` types per the epic design
//! (`dev/plans/gf2_algebra_permanent.md` §8). The exact body shape is
//! pinned by research deliverable R2
//! (`dev/plans/r2_f7_encoding_decision.md`,
//! `dev/plans/r2_packed_encoding_generalizations.md`). Trait impls
//! follow the surface in `dev/plans/d1b_packed_field_api.md`.
//!
//! # Feature gating
//!
//! Compiled only when the `f7` Cargo feature is enabled. Off by default
//! at W1; flips to default-on as part of the W4 closing edit (D1c §8.1).
//!
//! # Status
//!
//! W1-T1 skeleton — empty placeholder. Body lands in W4.
