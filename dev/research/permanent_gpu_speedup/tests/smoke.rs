//! Smoke test for the S1g GPU speedup harness (jit:9480f8a6).
//!
//! These tests are gated behind `#[cfg(feature = "hip")]` and require a
//! gfx1030 device at runtime; they are marked `#[ignore = "external: ..."]`
//! so they are skipped in CI and normal `cargo nextest run`.
//!
//! Run manually on the gfx1030 host:
//!   cargo test --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//!       --release --features hip -- --ignored

#![cfg(feature = "hip")]

use gf2_algebra::gpu::permanent_batch_bipedal3;
use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::testutil::random_matrix_with_rng;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

/// Correctness smoke test at n=36, M=1: GPU output must match CPU SIMD
/// on the same seeded matrix.  This satisfies the [hard] determinism criterion.
///
/// Note: n=36 GPU takes ~7200 s (~2 h), so this test carries #[ignore = "external:..."].
/// It is the canonical determinism check run manually on the gfx1030 host.
#[test]
#[ignore = "external: gfx1030 device required; n=36 takes ~7200 s (~2 h)"]
fn test_gpu_matches_simd_at_n36() {
    let n = 36;
    let seed = 0x9480_F8A6_0000_0024_u64; // S1g seed XOR n

    let mut rng = Lcg::new(seed);
    let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
    let mat = Bipedal3Matrix::from_row_major(&elems, n, n);

    // CPU SIMD result.
    let cpu_result = permanent_bipedal3(&mat);

    // GPU batch result (batch of 1).
    let gpu_results = permanent_batch_bipedal3(&[mat]);
    assert_eq!(gpu_results.len(), 1);
    let gpu_result = gpu_results[0];

    assert_eq!(
        cpu_result, gpu_result,
        "GPU and CPU SIMD disagree at n={n}: cpu={cpu_result:?}, gpu={gpu_result:?}"
    );
    println!("determinism check n={n}: cpu={cpu_result:?} == gpu={gpu_result:?}");
}

/// Quick correctness smoke test at small n=8, M=4: both paths agree and
/// neither panics.  Runs in seconds.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_gpu_matches_simd_n8() {
    let n = 8;
    let m = 4;
    let seed = 0x9480_F8A6_0000_0008_u64;

    let mut rng = Lcg::new(seed);
    let matrices: Vec<Bipedal3Matrix> = (0..m)
        .map(|_| {
            let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
            Bipedal3Matrix::from_row_major(&elems, n, n)
        })
        .collect();

    let cpu_results: Vec<Fp<3>> = matrices.iter().map(permanent_bipedal3).collect();
    let gpu_results = permanent_batch_bipedal3(&matrices);

    assert_eq!(cpu_results.len(), gpu_results.len());
    for (i, (cpu_r, gpu_r)) in cpu_results.iter().zip(gpu_results.iter()).enumerate() {
        assert_eq!(
            cpu_r, gpu_r,
            "mismatch at matrix {i}: cpu={cpu_r:?}, gpu={gpu_r:?}"
        );
    }
    println!("smoke n={n} M={m}: all {m} permanents agree");
}

/// Verify the GPU path does not panic for n=36 with a realistic batch size.
/// Does NOT check correctness (too slow) — only checks liveness.
#[test]
#[ignore = "external: gfx1030 device required; n=36 M=4 takes ~450 s"]
fn test_gpu_liveness_n36_small_batch() {
    let n = 36;
    let m = 4; // use small M for a feasible liveness check in ~500 s
    let seed = 0x9480_F8A6_DEAD_BEEF_u64;

    let mut rng = Lcg::new(seed);
    let matrices: Vec<Bipedal3Matrix> = (0..m)
        .map(|_| {
            let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
            Bipedal3Matrix::from_row_major(&elems, n, n)
        })
        .collect();

    let results = permanent_batch_bipedal3(&matrices);
    assert_eq!(results.len(), m, "result count mismatch");
    println!("liveness n={n} M={m}: GPU returned {m} results without panic");
}
