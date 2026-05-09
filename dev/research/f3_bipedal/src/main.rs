//! F_3 bipedal validation prototype — reproduces the per-element cost of
//! the Scheinerman bipedal F_3 encoding (arxiv 2407.20205v2) for direct
//! comparison against:
//!
//! - **naive**: scalar `(a OP b) % 3` on `Vec<u8>` (in-Rust analogue of the
//!   Julia naive Ryser baseline the paper compares against).
//! - **LUT-A**: the F_5/F_7 R-decision winner (4-bit slots, 2^16 LUT)
//!   applied unmodified to F_3.
//!
//! Build:   `cargo build --release`
//! Test:    `cargo test --release`
//! Bench:   `cargo run --release`

use f3_bipedal_prototype::bipedal;
use f3_bipedal_prototype::common::{bench_op_ns_per_elem, F3Encoding, Lcg, N_BENCH, REPEATS};
use f3_bipedal_prototype::lut;
use f3_bipedal_prototype::naive;

/// Run all four ops on a single encoding and return `(add, sub, mul, div)`
/// median ns/element.
fn bench_one<E: F3Encoding + Clone>(a_vec: &[u8], b_vec: &[u8], b_nonzero: &[u8]) -> [f64; 4] {
    let n = a_vec.len();
    let a = E::pack(a_vec);
    let b = E::pack(b_vec);
    let b_nz = E::pack(b_nonzero);

    let add_ns = bench_op_ns_per_elem(
        || {
            let mut t = a.clone();
            t.add_assign(&b);
            std::hint::black_box(&t);
        },
        n,
        REPEATS,
    );
    let sub_ns = bench_op_ns_per_elem(
        || {
            let mut t = a.clone();
            t.sub_assign(&b);
            std::hint::black_box(&t);
        },
        n,
        REPEATS,
    );
    let mul_ns = bench_op_ns_per_elem(
        || {
            let mut t = a.clone();
            t.mul_assign(&b);
            std::hint::black_box(&t);
        },
        n,
        REPEATS,
    );
    let div_ns = bench_op_ns_per_elem(
        || {
            let mut t = a.clone();
            t.div_assign(&b_nz);
            std::hint::black_box(&t);
        },
        n,
        REPEATS,
    );

    [add_ns, sub_ns, mul_ns, div_ns]
}

fn print_row(name: &str, ns: [f64; 4]) {
    println!(
        "  {:<44}  add={:>7.3}  sub={:>7.3}  mul={:>7.3}  div={:>7.3}",
        name, ns[0], ns[1], ns[2], ns[3]
    );
}

fn main() {
    println!("F_3 bipedal validation prototype — JIT f10152f6 (R2 follow-up)");
    println!(
        "  N = {} elements, repeats = {}, all numbers in ns/element (median)",
        N_BENCH, REPEATS
    );
    println!();

    let mut rng = Lcg::new(0xF3_DEADBEEF);
    let a_vec = rng.f3_vec(N_BENCH);
    let b_vec = rng.f3_vec(N_BENCH);
    let b_nz = rng.f3_vec_nonzero(N_BENCH);

    // Warm: trigger LUT initialisation outside the timed regions.
    {
        let mut wa: lut::Lut3 = lut::Lut3::pack(&a_vec[..16]);
        wa.mul_assign(&lut::Lut3::pack(&b_vec[..16]));
        wa.add_assign(&lut::Lut3::pack(&b_vec[..16]));
        wa.sub_assign(&lut::Lut3::pack(&b_vec[..16]));
        wa.div_assign(&lut::Lut3::pack(&b_nz[..16]));
    }

    let ns_naive = bench_one::<naive::Naive3>(&a_vec, &b_vec, &b_nz);
    let ns_lut = bench_one::<lut::Lut3>(&a_vec, &b_vec, &b_nz);
    let ns_bipedal = bench_one::<bipedal::Bipedal3>(&a_vec, &b_vec, &b_nz);

    println!("Results (median of {} runs):", REPEATS);
    print_row(naive::Naive3::NAME, ns_naive);
    print_row(lut::Lut3::NAME, ns_lut);
    print_row(bipedal::Bipedal3::NAME, ns_bipedal);

    println!();
    println!("Speedup vs naive (>1.0 = faster than naive):");
    let label = ["add", "sub", "mul", "div"];
    let print_speedup = |name: &str, ns: [f64; 4]| {
        let s: Vec<String> = (0..4)
            .map(|i| format!("{}={:.2}x", label[i], ns_naive[i] / ns[i]))
            .collect();
        println!("  {:<44}  {}", name, s.join("  "));
    };
    print_speedup(lut::Lut3::NAME, ns_lut);
    print_speedup(bipedal::Bipedal3::NAME, ns_bipedal);

    println!();
    println!("Speedup of bipedal vs LUT-A (>1.0 = bipedal faster):");
    let s: Vec<String> = (0..4)
        .map(|i| format!("{}={:.2}x", label[i], ns_lut[i] / ns_bipedal[i]))
        .collect();
    println!("  {}", s.join("  "));

    println!();
    println!("The paper (arxiv 2407.20205v2) reports an 86.9x wall-clock speedup");
    println!("of bipedal F_3 over Julia's naive Ryser at 4.20 GHz on a single");
    println!("thread. This Rust harness uses (a OP b) % 3 on Vec<u8> as the");
    println!("in-language naive baseline — the absolute speedup factor is not");
    println!("directly comparable to the paper's Julia number, but the per-");
    println!("element op count and bipedal-vs-LUT cost ratios are.");
}
