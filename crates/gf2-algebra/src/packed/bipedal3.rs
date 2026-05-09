//! Packed `F_3` element / vector / matrix encoding ("bipedal3").
//!
//! Will host `Bipedal3`, `Bipedal3Vec`, `Bipedal3Matrix`, and
//! `Fp3Accumulator` per the epic design (`dev/plans/gf2_algebra_permanent.md`
//! §7.1-§7.3). The encoding packs 64 independent `F_3` lanes into two
//! `u64` words; arithmetic is bitwise and branchless. The trait impls
//! (`PackedField<Fp<3>>`, `PackedFieldVec<Fp<3>>`) follow the surface
//! frozen in `dev/plans/d1b_packed_field_api.md`.
//!
//! # Status
//!
//! W1-T1 skeleton — empty placeholder. Body lands in the W1 (T2-T3)
//! implementation issues.
