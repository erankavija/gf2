//! Calls one kernel entry point through its published safe pointer with
//! arguments that break its condition (jit:bfc2ceab). The
//! argument names the inventory entry; `../../run-witnesses.py` runs each in
//! a process of its own and records how it ends.

use gf2_kernels_simd::{bipedal, fp_medium, fp_medium_f64, fp_medium_ple, fp_small, fp_small_f32};
use gf2_kernels_simd::{fp_small_ple, modem};

/// Dimensions whose three shape products wrap to 0, 4 and 0.
const WRAPPING: (usize, usize, usize) = (1 << 63, 2, 2);
/// A length far beyond the short operands of a packed-kernel call.
const LONG_WORDS: usize = 1 << 20;

fn main() {
    let entry = std::env::args().nth(1).expect("an inventory entry id");
    let (m, k, n) = WRAPPING;
    let long = vec![0u64; LONG_WORDS];
    let short = vec![0u64; 4];
    let (mut out0, mut out1, mut out2) = (vec![0u64; 4], vec![0u64; 4], vec![0u64; 4]);
    match entry.as_str() {
        "small-spmm" => {
            let fns = fp_small::detect().expect("AVX2");
            let (b, mut out) = (vec![1u8; 16], vec![0u8; 16]);
            (fns.spmm_fn)(&[0, 1], &[1 << 40], &[1], &b, 16, 7, &mut out);
        }
        "medium-spmm" => {
            let fns = fp_medium::detect().expect("AVX2");
            let (b, mut out) = (vec![1u16; 16], vec![0u16; 16]);
            (fns.spmm_fn)(&[0, 1], &[1 << 40], &[1], &b, 16, 65521, &mut out);
        }
        "small-ple" => {
            let fns = fp_small_ple::detect().expect("AVX2");
            let inv_table = [0u8, 1, 4, 5, 2, 3, 6];
            let mut pivots = Vec::new();
            (fns.ple_panel_base_fn)(&mut [], 1 << 30, 1, 7, &inv_table, &mut [0], &mut pivots);
        }
        "medium-ple" => {
            let fns = fp_medium_ple::detect().expect("AVX2");
            let inv_table = [0u16; 257];
            let mut pivots = Vec::new();
            (fns.ple_panel_base_fn)(&mut [], 1 << 30, 1, 257, &inv_table, &mut [0], &mut pivots);
        }
        "bipedal-add" => {
            let fns = bipedal::detect_avx2().expect("AVX2");
            (fns.add_fn)(&long, &short, &short, &short, &mut out0, &mut out1);
        }
        "bipedal-sub" => {
            let fns = bipedal::detect_avx2().expect("AVX2");
            (fns.sub_fn)(&long, &short, &short, &short, &mut out0, &mut out1);
        }
        "bipedal-mul" => {
            let fns = bipedal::detect_avx2().expect("AVX2");
            (fns.mul_fn)(&long, &short, &short, &short, &mut out0, &mut out1);
        }
        "bipedal-neg" => {
            let fns = bipedal::detect_avx2().expect("AVX2");
            (fns.neg_fn)(&long, &short, &mut out0, &mut out1);
        }
        "f5-add" => {
            let fns = bipedal::packed5::detect_avx2_f5().expect("AVX2");
            (fns.add_fn)(
                &long, &short, &short, &short, &short, &short, &mut out0, &mut out1, &mut out2,
            );
        }
        "f5-sub" => {
            let fns = bipedal::packed5::detect_avx2_f5().expect("AVX2");
            (fns.sub_fn)(
                &long, &short, &short, &short, &short, &short, &mut out0, &mut out1, &mut out2,
            );
        }
        "f5-mul" => {
            let fns = bipedal::packed5::detect_avx2_f5().expect("AVX2");
            (fns.mul_fn)(
                &long, &short, &short, &short, &short, &short, &mut out0, &mut out1, &mut out2,
            );
        }
        "f5-neg" => {
            let fns = bipedal::packed5::detect_avx2_f5().expect("AVX2");
            (fns.neg_fn)(&long, &short, &short, &mut out0, &mut out1, &mut out2);
        }
        "f7-add" => {
            let fns = bipedal::packed7::detect_avx2_f7().expect("AVX2");
            (fns.add_fn)(&long, &short, &mut out0);
        }
        "f7-sub" => {
            let fns = bipedal::packed7::detect_avx2_f7().expect("AVX2");
            (fns.sub_fn)(&long, &short, &mut out0);
        }
        "f7-mul" => {
            let fns = bipedal::packed7::detect_avx2_f7().expect("AVX2");
            (fns.mul_fn)(&long, &short, &mut out0);
        }
        "f7-neg" => {
            let fns = bipedal::packed7::detect_avx2_f7().expect("AVX2");
            (fns.neg_fn)(&long, &mut out0);
        }
        "medium-gemm-panel" => {
            let fns = fp_medium::detect().expect("AVX2");
            (fns.gemm_panel_fn)(&[], &[1; 4], m, k, n, 65521, &mut []);
        }
        "medium-f64-gemm" => {
            let fns = fp_medium_f64::detect().expect("AVX2 and FMA");
            (fns.batch_gemm_fn)(
                &[],
                &[1.0; 4],
                m,
                k,
                n,
                65521,
                fp_medium_f64::K_CHUNK_CAP,
                &mut [],
            );
        }
        "small-f32-gemm" => {
            let fns = fp_small_f32::detect().expect("AVX2 and FMA");
            (fns.batch_gemm_fn)(
                &[],
                &[1.0; 4],
                m,
                k,
                n,
                7,
                fp_small_f32::K_CHUNK_CAP,
                &mut [],
            );
        }
        "small-f32-gemm-route-a" => {
            let fns = fp_small_f32::detect().expect("AVX2 and FMA");
            (fns.batch_gemm_route_a_fn)(
                &[],
                &[1.0; 4],
                m,
                k,
                n,
                7,
                fp_small_f32::K_CHUNK_CAP,
                &mut [],
            );
        }
        "control" => {
            // A call that meets every condition, so the record shows a clean exit.
            let fns = modem::detect_f32();
            let mut out = [0.0f32; 4];
            (fns.pam_sq_distances_fn)(&[0.5], &[1.0], &[2.0], &[-3.0, -1.0, 1.0, 3.0], &mut out);
        }
        other => panic!("no witness for {other}"),
    }
    println!("{entry}: returned");
}
