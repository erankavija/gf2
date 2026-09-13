//! Crossover arm executable (jit:53c5a8c0).
//!
//! Speaks the canonical child-v2 contract and times the raw carry-less batch,
//! the whole-consumer GF(2^m) dot product and the whole-consumer GF(2^m)
//! element-wise product. `GF2_CROSSOVER_PATH` selects the per-element or the
//! batched entry point, so one executable serves both arms of every crossover
//! cell. It links no external library, so its digest identifies the gf2 build
//! alone.

use clmul_crossover_arms::gf2_backend::Gf2Backend;

fn main() {
    clmul_crossover_arms::run_arm::<Gf2Backend>("crossover-arm");
}
