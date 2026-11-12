//! The BCH cutover comparison (@/issue/fd9d5416): every arm times the epic's
//! two workloads, W1 batch encoding and W2 generator materialization, on the
//! contract's rows through [`harness`]. [`current`] is the current revision's
//! gf2 arm; the pre-cutover arm and the example `main` that hosts either one
//! live in `cutover/` beside this crate, and the external arms in `external/`.

pub mod current;
pub mod harness;
