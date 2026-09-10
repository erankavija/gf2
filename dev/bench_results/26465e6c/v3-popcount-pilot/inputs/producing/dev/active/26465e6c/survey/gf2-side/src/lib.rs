//! Population-count and fused-reduction survey arms (jit:26465e6c).
//!
//! Every arm of both families is a plain function pointer resolved once, before
//! timing: gf2's production dispatcher, gf2's internal controls, and the
//! vendored libpopcnt and Mula kernels linked in from `build.rs`. The arm
//! binary times whichever pointer the runner names through one shared timed
//! loop, so the per-call harness code is identical machine code for every arm.

pub mod arms;
pub mod external;
pub mod fixture;

pub use arms::{AndArm, AndPopcountFn, PopcountArm, PopcountFn};
pub use fixture::{Fixture, Pattern, VECTOR_BYTES};
