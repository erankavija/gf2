//! Packed `F_5` element / vector / matrix encoding ("bipedal5").
//!
//! Will host the `Bipedal5*` types per the epic design
//! (`dev/plans/gf2_algebra_permanent.md` §8). The exact body shape
//! (lanes-per-word, redundant-pair vs balanced encoding) is pinned by
//! research deliverable R1
//! (`dev/plans/r1_f5_encoding_decision.md`). Trait impls follow the
//! surface in `dev/plans/d1b_packed_field_api.md`.
//!
//! # Feature gating
//!
//! Compiled only when the `f5` Cargo feature is enabled. Off by default
//! at W1; flips to default-on as part of the W4 closing edit (D1c §8.1).
//!
//! # Status
//!
//! W1-T1 skeleton — empty placeholder. Body lands in W4.
