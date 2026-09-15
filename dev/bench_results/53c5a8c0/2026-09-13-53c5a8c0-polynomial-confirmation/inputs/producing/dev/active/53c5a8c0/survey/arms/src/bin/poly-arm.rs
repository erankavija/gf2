//! gf2 polynomial arm executable (jit:53c5a8c0).
//!
//! Speaks the canonical child-v2 contract and times the gf2 long product
//! through the public `clmul_wide_slice` and the whole-consumer wide-field
//! product through `Gf2mWide::mul_ref`. It links no external library, so its
//! digest identifies the gf2 build alone.

use clmul_crossover_arms::gf2_backend::Gf2Backend;

fn main() {
    clmul_crossover_arms::run_arm::<Gf2Backend>("poly-arm");
}
