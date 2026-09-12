//! gf2 arm executable for the polynomial-multiplication baseline survey
//! (jit:c7113c5a).
//!
//! Speaks the canonical child-v2 contract and measures the gf2 production
//! entry points named in `gf2_backend`. It links no external library, so its
//! executable digest identifies the gf2 build alone.

use poly_baseline_arms::gf2_backend::Gf2Backend;

fn main() {
    poly_baseline_arms::run_arm::<Gf2Backend>("gf2-poly-arm");
}
