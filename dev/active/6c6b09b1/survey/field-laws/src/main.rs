//! Runs gf2's shared field-law suite over the compile-time GF(2^8) element
//! the survey measures (jit:6c6b09b1).
//!
//! `test_field_axioms` is the canonical harness every gf2 field type runs.
//! gf2 ships no GF(2^8) `Gf2mWideConfig`, so its own suite never
//! instantiates `Gf2mWide<1, Gf256x11d>`; this binary does, with the shared
//! strategy and case budget. The runtime `Gf2mField::gf256()` element is
//! covered by gf2-core's own `test_gf2_8_field_axioms`, which the launcher
//! runs beside this binary. A violated axiom panics, so the exit status is
//! nonzero on any failure.

use byte_field_gf2_side::workload::{Gf256x11d, WideElement};
use gf2_core::field::axiom_tests::{gf2m_wide_strategy, test_field_axioms};

fn main() {
    test_field_axioms::<WideElement>(gf2m_wide_strategy::<1, Gf256x11d>(), 2);
    println!(
        "ok        test_field_axioms over Gf2mWide<1,Gf256x11d>, GF(2^8) modulo 0x11D, \
         characteristic 2"
    );
}
