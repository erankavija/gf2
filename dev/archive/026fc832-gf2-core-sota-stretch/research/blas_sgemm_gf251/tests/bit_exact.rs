//! Bit-exact equality at GF(251)/n ∈ {64, 256, 1024} against
//! `gf2_core::field::matrix::gemm` (Candidate C dispatch).
//!
//! Criterion source: JIT issue 91429c1c, "Bit-exact equality vs the
//! existing Candidate C output at GF(251)/n in {64, 256, 1024} on
//! canonical seeds."
//!
//! `n=1024` is roughly 80 ms wall-time (route-B sgemm + a slower
//! reference gemm); both are well under criterion-test bounds for
//! `cargo test --release`.

use blas_sgemm_gf251::{blas_gf251_gemm, matrix_to_canonical_bytes, P_GF251};
use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm as core_gemm;

fn check_bit_exact_square(n: usize, seed_salt: u64) {
    let seed_a = 0x9142_9c1c_dead_beef_u64 ^ ((n as u64) << 32) ^ seed_salt;
    let seed_b = 0x9142_9c1c_face_feed_u64 ^ ((n as u64) << 32) ^ seed_salt;
    let a = fp_matrix_from_seed::<P_GF251>(n, n, seed_a);
    let b = fp_matrix_from_seed::<P_GF251>(n, n, seed_b);
    let blas_c = blas_gf251_gemm(&a, &b);
    let core_c = core_gemm(&a, &b);
    assert_eq!(
        matrix_to_canonical_bytes(&blas_c),
        matrix_to_canonical_bytes(&core_c),
        "n={n} mismatch"
    );
}

#[test]
fn bit_exact_n64() {
    check_bit_exact_square(64, 1);
}

#[test]
fn bit_exact_n256() {
    check_bit_exact_square(256, 1);
}

#[test]
fn bit_exact_n1024() {
    check_bit_exact_square(1024, 1);
}
