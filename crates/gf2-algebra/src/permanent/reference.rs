//! `permanent_mod3_reference` cross-check driver.
//!
//! Will host a clean-room scalar `F_3` permanent used as the
//! ground-truth oracle that the `permanent_bipedal3` kernels are
//! checked against (epic design §16). Distinct from
//! [`super::ryser`] in that the reference avoids any of the bit-twiddle
//! tricks the fast paths exercise.
//!
//! # Status
//!
//! W1-T1 skeleton — empty placeholder. Body lands in W1-T5 / W2.
