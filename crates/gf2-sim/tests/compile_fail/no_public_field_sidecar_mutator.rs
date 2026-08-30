//! Field projections are emitted only by the live-approved, locked campaign
//! transaction. The public/default-build API has no path-based sidecar writer.

use gf2_sim::permanent_campaign::coordinator::emit_field_sidecar;

fn main() {
    let _ = emit_field_sidecar;
}
