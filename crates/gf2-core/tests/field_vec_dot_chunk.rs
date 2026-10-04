//! Scratch-length observation for the SIMD dot-product walk in the default
//! build: `field_vec.dot_chunk_len` reaches the walk as its step, and the walk
//! records the widest chunk it fills. The observation is a process-wide
//! maximum, so exactly one test per binary resets and reads it.
#![cfg(all(feature = "simd", feature = "test-support", not(gf2_tuning_baked)))]

use gf2_core::field::vec::{
    max_effective_dot_chunk_len, reset_max_effective_dot_chunk_len, DotChunkCandidate,
};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::tuning::CoreTuning;

#[test]
fn the_walk_steps_by_the_conservative_chunk_length() {
    let chunk = CoreTuning::CONSERVATIVE.field_vec().dot_chunk_len();
    let field = Gf2mField::gf256();
    let n = 2 * chunk as u64 + 1;
    let a: FieldVec<Gf2mElement> = (0..n).map(|i| field.element((i * 37 + 13) % 256)).collect();
    let b: FieldVec<Gf2mElement> = (0..n).map(|i| field.element((i * 53 + 7) % 256)).collect();

    reset_max_effective_dot_chunk_len();
    let simd = a.simd_dot_product(&b);

    assert_eq!(simd, a.dot_product(&b));
    assert_eq!(
        max_effective_dot_chunk_len(),
        chunk,
        "the SIMD dot-product walk did not step by field_vec.dot_chunk_len; \
         a zero observation means the host offers no batched carry-less \
        multiply and the walk never ran"
    );

    assert_eq!(
        DotChunkCandidate::ALL.map(DotChunkCandidate::chunk_len),
        [128, 256, 512]
    );
    for candidate in DotChunkCandidate::ALL {
        let len = candidate.chunk_len() + 65;
        let lhs: FieldVec<Gf2mElement> = (0..len as u64)
            .map(|i| field.element((i * 11 + 5) % 256))
            .collect();
        let rhs: FieldVec<Gf2mElement> = (0..len as u64)
            .map(|i| field.element((i * 19 + 3) % 256))
            .collect();
        let function = candidate.function();
        reset_max_effective_dot_chunk_len();
        assert_eq!(function(&lhs, &rhs), lhs.dot_product(&rhs));
        assert_eq!(max_effective_dot_chunk_len(), candidate.chunk_len());
    }
}
