//! Violated caller conditions of the checked kernel entry points: each
//! published pointer panics under its kernel's name instead of reaching an
//! unchecked access. Compiled under `--cfg gf2_entry_checked`, the build
//! whose detection functions publish the checked wrappers.

#![cfg(gf2_entry_checked)]
#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]

use std::panic::{catch_unwind, AssertUnwindSafe};

use gf2_kernels_simd::bipedal::{self, packed5, packed7};
use gf2_kernels_simd::{fp_medium, fp_medium_f64, fp_medium_ple, fp_small, fp_small_f32};
use gf2_kernels_simd::{fp_small_panel, fp_small_ple, modem};

/// Dimensions whose three shape products overflow `usize`.
const OVERFLOWING: (usize, usize, usize) = (1 << 63, 2, 2);

/// Requires `call` to panic with a message that opens with `kernel` and
/// names `condition`.
#[track_caller]
fn rejects(kernel: &str, condition: &str, call: impl FnOnce()) {
    let payload = catch_unwind(AssertUnwindSafe(call)).expect_err(&format!(
        "{kernel}: accepted arguments that break `{condition}`"
    ));
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_default();
    assert!(
        message.starts_with(kernel) && message.contains(condition),
        "{kernel}: panic message {message:?} does not name `{condition}`"
    );
}

/// The detected bundle, or a skip line on a host without the features.
macro_rules! detected {
    ($detect:expr) => {
        match $detect {
            Some(fns) => fns,
            None => {
                eprintln!("skipped: the host lacks the kernel's processor features");
                return;
            }
        }
    };
}

#[test]
fn small_prime_spmm_row_rejects_columns_outside_b() {
    let fns = detected!(fp_small::detect());
    let kernel = "fp_small_spmm_row";
    let b = [1u8; 32];
    let mut out = [0u8; 16];
    (fns.spmm_row_fn)(&[1], &[1], &b, 16, 16, 7, &mut out);
    assert_eq!(out, [1u8; 16]);
    (fns.spmm_row_fn)(&[], &[], &[], 16, 16, 7, &mut out);
    for col in [2, usize::MAX] {
        rejects(kernel, "column index", || {
            (fns.spmm_row_fn)(&[1], &[col], &b, 16, 16, 7, &mut [0u8; 16])
        });
    }
    rejects(kernel, "shorter than one output row", || {
        (fns.spmm_row_fn)(&[1], &[0], &b[..8], 16, 16, 7, &mut [0u8; 16])
    });
}

#[test]
fn medium_prime_spmm_row_rejects_columns_outside_b() {
    let fns = detected!(fp_medium::detect());
    let kernel = "fp_medium_spmm_row";
    let b = [1u16; 32];
    let mut out = [0u16; 16];
    (fns.spmm_row_fn)(&[1], &[1], &b, 16, 16, 257, &mut out);
    assert_eq!(out, [1u16; 16]);
    for col in [2, usize::MAX] {
        rejects(kernel, "column index", || {
            (fns.spmm_row_fn)(&[1], &[col], &b, 16, 16, 257, &mut [0u16; 16])
        });
    }
    rejects(kernel, "shorter than one output row", || {
        (fns.spmm_row_fn)(&[1], &[0], &b[..8], 16, 16, 257, &mut [0u16; 16])
    });
}

#[test]
fn small_prime_ple_panel_rejects_window_and_table() {
    let fns = detected!(fp_small_ple::detect());
    let kernel = "ple_panel_base_canonical";
    let inv_table = [0u8, 1, 4, 5, 2, 3, 6];
    let run = |window: &mut [u8], m: usize, win: usize, table: &[u8]| {
        (fns.ple_panel_base_fn)(window, m, win, 7, table, &mut [0, 1], &mut Vec::new())
    };
    assert_eq!(run(&mut [1, 0, 0, 1], 2, 2, &inv_table), 2);
    rejects(kernel, "m * win cells", || {
        run(&mut [1, 0, 0], 2, 2, &inv_table);
    });
    rejects(kernel, "overflows", || {
        run(&mut [], usize::MAX, 2, &inv_table);
    });
    rejects(kernel, "not below p", || {
        run(&mut [1, 0, 7, 1], 2, 2, &inv_table);
    });
    rejects(kernel, "shorter than p", || {
        run(&mut [1, 0, 0, 1], 2, 2, &inv_table[..6]);
    });
}

#[test]
fn medium_prime_ple_panel_rejects_window_and_table() {
    let fns = detected!(fp_medium_ple::detect());
    let kernel = "ple_panel_base_canonical_u16";
    let mut inv_table = vec![0u16; 257];
    inv_table[1] = 1;
    let run = |window: &mut [u16], m: usize, win: usize, table: &[u16]| {
        (fns.ple_panel_base_fn)(window, m, win, 257, table, &mut [0, 1], &mut Vec::new())
    };
    assert_eq!(run(&mut [1, 0, 0, 1], 2, 2, &inv_table), 2);
    rejects(kernel, "m * win cells", || {
        run(&mut [1, 0, 0], 2, 2, &inv_table);
    });
    rejects(kernel, "overflows", || {
        run(&mut [], usize::MAX, 2, &inv_table);
    });
    rejects(kernel, "not below p", || {
        run(&mut [1, 0, 257, 1], 2, 2, &inv_table);
    });
    rejects(kernel, "shorter than p", || {
        run(&mut [1, 0, 0, 1], 2, 2, &inv_table[..256]);
    });
}

#[test]
fn bipedal_binary_kernels_reject_slice_shapes() {
    let fns = detected!(bipedal::detect_avx2());
    let (four, eight, three) = ([0u64; 4], [0u64; 8], [0u64; 3]);
    for (name, kernel) in [
        ("add", fns.add_fn),
        ("sub", fns.sub_fn),
        ("mul", fns.mul_fn),
    ] {
        let label = format!("bipedal {name} kernel");
        kernel(&four, &four, &four, &four, &mut [0; 4], &mut [0; 4]);
        rejects(&label, "lengths differ", || {
            kernel(&eight, &four, &eight, &eight, &mut [0; 8], &mut [0; 8])
        });
        rejects(&label, "lengths differ", || {
            kernel(&four, &four, &four, &four, &mut [0; 4], &mut [0; 8])
        });
        rejects(&label, "multiple of four", || {
            kernel(&three, &three, &three, &three, &mut [0; 3], &mut [0; 3])
        });
    }
}

#[test]
fn bipedal_unary_kernel_rejects_slice_shapes() {
    let fns = detected!(bipedal::detect_avx2());
    let label = "bipedal neg kernel";
    (fns.neg_fn)(&[0; 4], &[0; 4], &mut [0; 4], &mut [0; 4]);
    rejects(label, "lengths differ", || {
        (fns.neg_fn)(&[0; 8], &[0; 4], &mut [0; 8], &mut [0; 8])
    });
    rejects(label, "multiple of four", || {
        (fns.neg_fn)(&[0; 3], &[0; 3], &mut [0; 3], &mut [0; 3])
    });
}

#[test]
fn f5_binary_kernels_reject_slice_shapes() {
    let fns = detected!(packed5::detect_avx2_f5());
    let (four, eight, three) = ([0u64; 4], [0u64; 8], [0u64; 3]);
    for (name, kernel) in [
        ("add", fns.add_fn),
        ("sub", fns.sub_fn),
        ("mul", fns.mul_fn),
    ] {
        let label = format!("F_5 {name} kernel");
        let (mut o0, mut o1, mut o2) = ([0u64; 4], [0u64; 4], [0u64; 4]);
        kernel(
            &four, &four, &four, &four, &four, &four, &mut o0, &mut o1, &mut o2,
        );
        rejects(&label, "lengths differ", || {
            let (mut o0, mut o1, mut o2) = ([0u64; 8], [0u64; 8], [0u64; 8]);
            kernel(
                &eight, &eight, &eight, &eight, &four, &eight, &mut o0, &mut o1, &mut o2,
            )
        });
        rejects(&label, "multiple of four", || {
            let (mut o0, mut o1, mut o2) = ([0u64; 3], [0u64; 3], [0u64; 3]);
            kernel(
                &three, &three, &three, &three, &three, &three, &mut o0, &mut o1, &mut o2,
            )
        });
    }
}

#[test]
fn f5_unary_kernel_rejects_slice_shapes() {
    let fns = detected!(packed5::detect_avx2_f5());
    let label = "F_5 neg kernel";
    (fns.neg_fn)(
        &[0; 4],
        &[0; 4],
        &[0; 4],
        &mut [0; 4],
        &mut [0; 4],
        &mut [0; 4],
    );
    rejects(label, "lengths differ", || {
        (fns.neg_fn)(
            &[0; 8],
            &[0; 8],
            &[0; 4],
            &mut [0; 8],
            &mut [0; 8],
            &mut [0; 8],
        )
    });
    rejects(label, "multiple of four", || {
        (fns.neg_fn)(
            &[0; 3],
            &[0; 3],
            &[0; 3],
            &mut [0; 3],
            &mut [0; 3],
            &mut [0; 3],
        )
    });
}

#[test]
fn f7_binary_kernels_reject_slice_shapes() {
    let fns = detected!(packed7::detect_avx2_f7());
    for (name, kernel) in [
        ("add", fns.add_fn),
        ("sub", fns.sub_fn),
        ("mul", fns.mul_fn),
    ] {
        let label = format!("F_7 {name} kernel");
        kernel(&[0; 4], &[0; 4], &mut [0; 4]);
        rejects(&label, "lengths differ", || {
            kernel(&[0; 8], &[0; 4], &mut [0; 8])
        });
        rejects(&label, "lengths differ", || {
            kernel(&[0; 4], &[0; 4], &mut [0; 8])
        });
        rejects(&label, "multiple of four", || {
            kernel(&[0; 3], &[0; 3], &mut [0; 3])
        });
    }
}

#[test]
fn f7_unary_kernel_rejects_slice_shapes() {
    let fns = detected!(packed7::detect_avx2_f7());
    let label = "F_7 neg kernel";
    (fns.neg_fn)(&[0; 4], &mut [0; 4]);
    rejects(label, "lengths differ", || {
        (fns.neg_fn)(&[0; 8], &mut [0; 4])
    });
    rejects(label, "multiple of four", || {
        (fns.neg_fn)(&[0; 3], &mut [0; 3])
    });
}

#[test]
fn small_prime_gemm_row_panel_rejects_wrapping_shape() {
    let fns = detected!(fp_small::detect());
    let kernel = "fp_small_gemm_row_panel";
    let mut out = [0u8; 2];
    (fns.gemm_row_panel_fn)(&[1, 2], &[1, 1, 2, 2], 2, 2, 7, &mut out);
    assert_eq!(out, [3, 6]);
    rejects(kernel, "overflows", || {
        (fns.gemm_row_panel_fn)(&[1, 2], &[], 2, usize::MAX, 7, &mut [])
    });
    rejects(kernel, "bt shape", || {
        (fns.gemm_row_panel_fn)(&[1, 2], &[1, 1, 2], 2, 2, 7, &mut [0; 2])
    });
}

/// Runs the whole-product cases over one pointer of the shape
/// `(a, bt, m, k, n, p, c)`.
macro_rules! whole_gemm_cases {
    ($kernel:expr, $name:literal, $one:expr, $p:expr, $zero:expr) => {{
        let kernel = $kernel;
        let (m, k, n) = OVERFLOWING;
        let mut c = [$zero; 1];
        kernel(&[$one; 1], &[$one; 1], 1, 1, 1, $p, &mut c);
        rejects($name, "overflows", || {
            kernel(&[], &[$one; 4], m, k, n, $p, &mut [])
        });
        rejects($name, "a shape", || {
            kernel(&[$one; 3], &[$one; 4], 2, 2, 2, $p, &mut [$zero; 4])
        });
        rejects($name, "c shape", || {
            kernel(&[$one; 4], &[$one; 4], 2, 2, 2, $p, &mut [$zero; 3])
        });
    }};
}

#[test]
fn small_prime_panel_gemm_rejects_wrapping_shape() {
    let fns = detected!(fp_small_panel::detect());
    whole_gemm_cases!(fns.batch_gemm_fn, "fp_small_panel_gemm", 1u8, 7u8, 0u8);
}

#[test]
fn small_prime_f32_gemm_rejects_wrapping_shape() {
    let fns = detected!(fp_small_f32::detect());
    whole_gemm_cases!(fns.batch_gemm_fn, "fp_small_f32_gemm", 1f32, 7u8, 0u8);
    whole_gemm_cases!(
        fns.batch_gemm_route_a_fn,
        "fp_small_f32_gemm_route_a",
        1f32,
        7u8,
        0u8
    );
}

#[test]
fn medium_prime_gemm_panel_rejects_wrapping_shape() {
    let fns = detected!(fp_medium::detect());
    whole_gemm_cases!(
        fns.gemm_panel_fn,
        "fp_medium_gemm_panel",
        1u16,
        257u16,
        0u16
    );
}

#[test]
fn medium_prime_f64_gemm_rejects_wrapping_shape() {
    let fns = detected!(fp_medium_f64::detect());
    whole_gemm_cases!(fns.batch_gemm_fn, "fp_medium_f64_gemm", 1f64, 257u16, 0u16);
}

/// The wrapped product of two slice lengths needs operands no test holds, so
/// these cases cover the length assertion the checked product feeds; the unit
/// test of `entry_check::checked_cells` covers the overflow branch.
#[test]
fn pam_distance_kernels_reject_output_shape() {
    let refuses = |call: &dyn Fn()| catch_unwind(AssertUnwindSafe(call)).is_err();
    let levels32 = [-3.0f32, -1.0, 1.0, 3.0];
    let f32_fns = modem::detect_f32();
    (f32_fns.pam_sq_distances_fn)(&[0.5], &[1.0], &[2.0], &levels32, &mut [0.0; 4]);
    assert!(refuses(&|| {
        (f32_fns.pam_sq_distances_fn)(&[0.5], &[1.0], &[2.0], &levels32, &mut [0.0; 3])
    }));
    let levels64 = [-3.0f64, -1.0, 1.0, 3.0];
    let f64_fns = modem::detect_f64();
    (f64_fns.pam_sq_distances_fn)(&[0.5], &[1.0], &[2.0], &levels64, &mut [0.0; 4]);
    assert!(refuses(&|| {
        (f64_fns.pam_sq_distances_fn)(&[0.5], &[1.0], &[2.0], &levels64, &mut [0.0; 3])
    }));
}
