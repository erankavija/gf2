//! Attribution probe for the wide carry-less dispatch (jit:53c5a8c0).
//!
//! Times the public owned product, the public accumulating product, the kernel
//! function pointer the dispatch resolves, and the portable schoolbook, on the
//! same operands in one process. The four figures separate what a caller pays
//! for the entry point from what the kernel itself costs. Each is amortised
//! over the shared probe repetition count and reported as one JSON record.
//!
//! Usage: dispatch-probe --words <4|9> --seed <u64>

use clmul_crossover_arms::{amortised_probe_total_ns, banks, Case, AMORTISED_PROBE_REPEATS};
use gf2_core::gf2m::wide::{clmul_wide, clmul_wide_slice, clmul_wide_slice_portable};
use std::hint::black_box;

fn argument(name: &str) -> String {
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == name {
            return args.next().unwrap_or_else(|| panic!("{name} needs a value"));
        }
    }
    panic!("missing required argument {name}");
}

macro_rules! probe {
    ($n:literal, $m:literal, $bank:expr, $case:expr) => {{
        let a: &[u64; $n] = (&$bank.a[..]).try_into().expect("operand width");
        let b: &[u64; $n] = (&$bank.b[..]).try_into().expect("operand width");
        let kernel = gf2_kernels_simd::gf2m_wide::detect_wide();
        let mut out = [0u64; $m];
        let owned = amortised_probe_total_ns(|| {
            black_box(clmul_wide::<$n, $m>(black_box(a), black_box(b)));
        });
        let accumulate = amortised_probe_total_ns(|| {
            clmul_wide_slice::<$n>(black_box(a), black_box(b), black_box(&mut out[..]));
        });
        let portable = amortised_probe_total_ns(|| {
            clmul_wide_slice_portable::<$n>(black_box(a), black_box(b), black_box(&mut out[..]));
        });
        let raw = match ($n, kernel.as_ref()) {
            (4, Some(fns)) => {
                let mut product = [0u64; 8];
                let clmul = fns.wide256.clmul;
                amortised_probe_total_ns(|| {
                    let a: &[u64; 4] = (&$bank.a[..]).try_into().expect("4 words");
                    let b: &[u64; 4] = (&$bank.b[..]).try_into().expect("4 words");
                    clmul(black_box(a), black_box(b), black_box(&mut product));
                })
            }
            (9, Some(fns)) => {
                let mut product = [0u64; 18];
                let clmul = fns.wide571.clmul;
                amortised_probe_total_ns(|| {
                    let a: &[u64; 9] = (&$bank.a[..]).try_into().expect("9 words");
                    let b: &[u64; 9] = (&$bank.b[..]).try_into().expect("9 words");
                    clmul(black_box(a), black_box(b), black_box(&mut product));
                })
            }
            _ => 0,
        };
        let through_helper = {
            let a = $bank.a.clone();
            let b = $bank.b.clone();
            amortised_probe_total_ns(|| {
                clmul_crossover_arms::gf2_backend::long_product(
                    $n,
                    black_box(&a),
                    black_box(&b),
                );
            })
        };
        let through_arm = {
            let mut backend =
                <clmul_crossover_arms::gf2_backend::Gf2Backend as clmul_crossover_arms::Backend>::create(&$case);
            let mut slot = $bank;
            amortised_probe_total_ns(|| {
                <clmul_crossover_arms::gf2_backend::Gf2Backend as clmul_crossover_arms::Backend>::run(
                    &mut backend, &$case, &mut slot,
                );
            })
        };
        (owned, accumulate, portable, raw, through_arm, through_helper)
    }};
}

fn main() {
    let words: usize = argument("--words").parse().expect("--words is a width");
    let seed: u64 = argument("--seed").parse().expect("--seed is a u64");
    let case = Case::PolyMul {
        words,
        inner: 1,
        seed,
    };
    let bank = banks(&case, 0, 1).pop().expect("one bank");
    let (owned, accumulate, portable, raw, through_arm, through_helper) = match words {
        4 => probe!(4, 8, bank, case),
        9 => probe!(9, 18, bank, case),
        other => panic!("no probe for {other}-word operands"),
    };
    let record = serde_json::json!({
        "schema": "clmul-crossover-dispatch-probe-v1",
        "words": words,
        "seed": seed,
        "repeats": AMORTISED_PROBE_REPEATS,
        "lane": gf2_kernels_simd::gf2m_wide::detect_wide().map(|fns| fns.name),
        "total_ns": {
            "public_owned": owned,
            "public_accumulate": accumulate,
            "portable_schoolbook": portable,
            "resolved_kernel": raw,
            "through_arm_backend": through_arm,
            "through_width_helper": through_helper,
        },
    });
    println!("{record}");
}
