//! F_17 packed-encoding prototype — empirical test of the Fermat-like
//! mul-via-log prediction from the cross-prime generalization analysis.
//!
//! Three encodings: naive, LUT-A, (z, log) split. The headline question is
//! whether F_17-B's bit-parallel mod-16 log addition delivers the same
//! ~10× mul speedup over LUT-A that F_5-B and F_7-B did over their
//! respective LUT baselines.
//!
//! Build:   `cargo build --release`
//! Test:    `cargo test --release`
//! Bench:   `cargo run --release`

mod common;
mod lut;
mod naive;
mod z_log;

use common::{bench_op_ns_per_elem, F17Encoding, Lcg, N_BENCH, REPEATS};

fn bench_one<E: F17Encoding + Clone>(a_vec: &[u8], b_vec: &[u8], b_nonzero: &[u8]) -> [f64; 4] {
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
    println!("F_17 packed-encoding prototype — JIT f10152f6 (R2 follow-up)");
    println!(
        "  N = {} elements, repeats = {}, all numbers in ns/element (median)",
        N_BENCH, REPEATS
    );
    println!();

    let mut rng = Lcg::new(0xF17_DEADBEEF);
    let a_vec = rng.f17_vec(N_BENCH);
    let b_vec = rng.f17_vec(N_BENCH);
    let b_nz = rng.f17_vec_nonzero(N_BENCH);

    {
        let mut wa: lut::Lut17 = lut::Lut17::pack(&a_vec[..16]);
        wa.mul_assign(&lut::Lut17::pack(&b_vec[..16]));
        wa.add_assign(&lut::Lut17::pack(&b_vec[..16]));
        wa.sub_assign(&lut::Lut17::pack(&b_vec[..16]));
        wa.div_assign(&lut::Lut17::pack(&b_nz[..16]));
    }

    let ns_naive = bench_one::<naive::Naive17>(&a_vec, &b_vec, &b_nz);
    let ns_lut = bench_one::<lut::Lut17>(&a_vec, &b_vec, &b_nz);
    let ns_b = bench_one::<z_log::ZLog17>(&a_vec, &b_vec, &b_nz);

    println!("Results (median of {} runs):", REPEATS);
    print_row(naive::Naive17::NAME, ns_naive);
    print_row(lut::Lut17::NAME, ns_lut);
    print_row(z_log::ZLog17::NAME, ns_b);

    println!();
    println!("Speedup vs LUT-A (>1.0 = faster than LUT-A):");
    let label = ["add", "sub", "mul", "div"];
    let print_speedup = |name: &str, ns: [f64; 4]| {
        let s: Vec<String> = (0..4)
            .map(|i| format!("{}={:.2}x", label[i], ns_lut[i] / ns[i]))
            .collect();
        println!("  {:<44}  {}", name, s.join("  "));
    };
    print_speedup(z_log::ZLog17::NAME, ns_b);

    println!();
    println!("Cross-prime prediction check: B should give cheap mul (Fermat-like");
    println!("p−1 = 16 = 2^4) and slow add (per-element fallback). Compare vs");
    println!("F_5-B (mul=0.015 ns/elem, add=10 ns/elem) and F_7-B (mul=0.02,");
    println!("add=8.4) to see the pattern.");
}
