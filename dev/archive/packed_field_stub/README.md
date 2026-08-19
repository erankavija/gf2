This crate is the archived historical demonstration that proved the packed
trait surface before production adoption. The production home is
`crates/gf2-algebra/src/packed/mod.rs`, which declares `PackedField` and
`PackedFieldVec`. The design record is
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md`.

Issue `7f818151` archived this crate. Its stub `fold_mul` bodies return zero by
design; this crate is demonstration-only.
