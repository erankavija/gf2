//! Population-count and fused-reduction optimization arms (jit:5cbb6545).
//!
//! Every arm of the three families is a plain function pointer or a small
//! consumer routine resolved once, before timing: the pre-change gf2 routes
//! spelled out word for word, the public entry points as this issue leaves
//! them, each kernel of the detected bundle, and the vendored libpopcnt and
//! Mula kernels linked in from `build.rs`. The arm binary times whichever
//! route the runner names through one shared timed loop, so the per-call
//! harness code is identical machine code for every arm of a family.

pub mod arms;
pub mod external;
pub mod fixture;

pub use arms::{AndArm, AndPopcountFn, MatvecArm, PopcountArm, PopcountFn};
pub use fixture::{Fixture, Pattern, VECTOR_BYTES};
