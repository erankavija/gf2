//! Runs gf2's shared field-law suite over both GF(2^8) compile-time element
//! types this assessment validates against (jit:19513245).
//!
//! `test_field_axioms` is the canonical harness every gf2 field type runs.
//! gf2 ships no GF(2^8) `Gf2mWideConfig`, so its own suite instantiates
//! neither `Gf2mWide<1, Gf256x11d>` nor `Gf2mWide<1, Gf256x11b>`; this binary
//! does, with the shared strategy and case budget. The runtime
//! `Gf2mField::gf256()` element is covered by gf2-core's own
//! `test_gf2_8_field_axioms`, which the launcher runs beside this binary.
//! A violated axiom panics, so the exit status is nonzero on any failure.
//!
//! Both polynomials matter here because the prototype builds its
//! multiplication table from the field's own reduction polynomial: the
//! validation has to establish that both fields are fields before it
//! compares any route over them.

use byte_field_gf2_side::workload::{Gf256x11d, WideElement};
use bytefield_consumer::{Gf256x11b, WideElement11b};
use gf2_core::field::axiom_tests::{gf2m_wide_strategy, test_field_axioms};

fn main() {
    test_field_axioms::<WideElement>(gf2m_wide_strategy::<1, Gf256x11d>(), 2);
    println!(
        "ok        test_field_axioms over Gf2mWide<1,Gf256x11d>, GF(2^8) modulo 0x11D, \
         characteristic 2"
    );
    test_field_axioms::<WideElement11b>(gf2m_wide_strategy::<1, Gf256x11b>(), 2);
    println!(
        "ok        test_field_axioms over Gf2mWide<1,Gf256x11b>, GF(2^8) modulo 0x11B, \
         characteristic 2"
    );
}
