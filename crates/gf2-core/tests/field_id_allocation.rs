//! Allocation witness for prime-field identities: building one and asking a
//! prime-field carrier for its identity hint request no heap memory.

use gf2_core::field::extension::{FieldId, FieldIdentity};
use gf2_core::gfp::Fp;
use thread_allocations::{count, CountingAllocator};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn prime_identity_construction_allocates_nothing() {
    for characteristic in [2u64, 3, 7, (1u64 << 63) - 25, 1u64 << 63] {
        let (id, counts) = count(|| FieldId::prime(characteristic).unwrap());
        assert!(counts.is_zero(), "p = {characteristic}: {counts:?}");
        let (clone, counts) = count(|| id.clone());
        assert!(counts.is_zero(), "clone, p = {characteristic}: {counts:?}");
        assert_eq!(clone, id);
    }
}

#[test]
fn prime_field_identity_hint_allocates_nothing() {
    let (hint2, counts2) = count(<Fp<2> as FieldIdentity>::field_id_hint);
    let (hint3, counts3) = count(<Fp<3> as FieldIdentity>::field_id_hint);
    let (hint_big, counts_big) = count(<Fp<9223372036854775783> as FieldIdentity>::field_id_hint);
    for counts in [counts2, counts3, counts_big] {
        assert!(counts.is_zero(), "{counts:?}");
    }
    assert_eq!(hint2, FieldId::prime(2).ok());
    assert_eq!(hint3, FieldId::prime(3).ok());
    assert_eq!(hint_big, FieldId::prime(9223372036854775783).ok());
}
