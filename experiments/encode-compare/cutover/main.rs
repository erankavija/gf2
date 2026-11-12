//! The gf2 arm binary of the cutover comparison, built by `cutover.sh` as the
//! `bch_cutover` example of `gf2-coding` inside an archive of one revision:
//! `harness.rs` is `src/cutover/harness.rs` and `api.rs` is that revision's
//! adapter, `cutover/pre.rs` or `src/cutover/current.rs`.

// The current adapter also exports helpers the external arms use.
#[allow(dead_code)]
mod api;
mod harness;

fn main() {
    let revision = std::env::var("CUTOVER_REVISION").expect("CUTOVER_REVISION names the revision");
    harness::main(&api::Gf2 { revision });
}
