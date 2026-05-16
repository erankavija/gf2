// S1g (jit:9480f8a6): CPU-only SIMD permanent check at n=36.
//
// Computes permanent_bipedal3 (AVX2 SIMD path) for the same seeded matrix
// as det_check uses.  Runs without GPU/HIP so it can be used to get the
// expected SIMD value independently of the long GPU run.
//
// Build and run (no ROCm required):
//   cargo build --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//       --release --bin simd_check
//   cargo run --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
//       --release --bin simd_check
//
// Expected output: "SIMD result: Fp<3>(v)" where v is the computed permanent.
// Expected wall-clock: ~848 s (~14 min) at n=36 with AVX2.

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::testutil::random_matrix_with_rng;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;
use std::time::Instant;

fn main() {
    // Use the same seed as det_check at n=36.
    // det_check: seed = 0x9480_F8A6_0000_0000 ^ (n as u64)
    //            n=36 → seed = 0x9480_F8A6_0000_0024
    let n = 36_usize;
    let seed = 0x9480_F8A6_0000_0000_u64 ^ (n as u64);

    println!("S1g simd_check: n={n}  seed={seed:#018x}");
    println!("  Building 1 random {n}x{n} F_3 matrix...");

    let mut rng = Lcg::new(seed);
    let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(&mut rng, n);
    let mat = Bipedal3Matrix::from_row_major(&elems, n, n);

    println!("  Running permanent_bipedal3 (SIMD path, expected ~848 s)...");
    let t0 = Instant::now();
    let result = permanent_bipedal3(&mat);
    let elapsed = t0.elapsed().as_secs_f64();

    println!("  Done in {elapsed:.1} s");
    println!("SIMD result: {result:?}  (n={n}, seed={seed:#018x})");
}
