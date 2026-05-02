//! F_5 packed-encoding research prototype (JIT issue 6b3f6054, R1).
//!
//! Prototypes 4 candidate encodings for $\mathbb{F}_5$ vectors and measures
//! per-element wall-clock time for `add`, `sub`, `mul`, `div`. Mirrors the
//! shape of `dev/research/rns_prototype/`. Not part of the gf2 workspace.
//!
//! Build:   `cargo build --release`
//! Test:    `cargo test --release`     (correctness vs `(a OP b) % 5`)
//! Bench:   `cargo run --release`      (prints comparison table)

mod cand_a;
mod cand_b;
mod cand_c;
mod cand_d;
mod common;

use common::{bench_op_ns_per_elem, F5Encoding, Lcg, N_BENCH, REPEATS};

/// Run all four ops on a single encoding and return `(add, sub, mul, div)`
/// median ns/element.
fn bench_one<E: F5Encoding + Clone>(a_vec: &[u8], b_vec: &[u8], b_nonzero: &[u8]) -> [f64; 4] {
    let n = a_vec.len();
    let a = E::pack(a_vec);
    let b = E::pack(b_vec);
    let b_nz = E::pack(b_nonzero);

    // Each op is benched on a fresh clone so we measure pure op work, not
    // setup. The bench loop runs the op once per repeat — the op itself
    // touches all `n` elements, so per-element ns is `elapsed / n`.
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
        "  {:<40}  add={:>7.2}  sub={:>7.2}  mul={:>7.2}  div={:>7.2}",
        name, ns[0], ns[1], ns[2], ns[3]
    );
}

fn main() {
    println!("F_5 packed-encoding prototype — JIT 6b3f6054 (R1)");
    println!(
        "  N = {} elements, repeats = {}, all numbers in ns/element (median)",
        N_BENCH, REPEATS
    );
    println!();

    let mut rng = Lcg::new(0xF5_DEADBEEF);
    let a_vec = rng.f5_vec(N_BENCH);
    let b_vec = rng.f5_vec(N_BENCH);
    let b_nz = rng.f5_vec_nonzero(N_BENCH);

    // Warm: trigger LUT initialisation outside the timed regions.
    {
        let mut wa: cand_a::VecA = cand_a::VecA::pack(&a_vec[..16]);
        wa.mul_assign(&cand_a::VecA::pack(&b_vec[..16]));
        wa.add_assign(&cand_a::VecA::pack(&b_vec[..16]));
        wa.sub_assign(&cand_a::VecA::pack(&b_vec[..16]));
        wa.div_assign(&cand_a::VecA::pack(&b_nz[..16]));
        let mut wc: cand_c::VecC = cand_c::VecC::pack(&a_vec[..16]);
        wc.mul_assign(&cand_c::VecC::pack(&b_vec[..16]));
        wc.add_assign(&cand_c::VecC::pack(&b_vec[..16]));
        wc.sub_assign(&cand_c::VecC::pack(&b_vec[..16]));
        wc.div_assign(&cand_c::VecC::pack(&b_nz[..16]));
    }

    let ns_a = bench_one::<cand_a::VecA>(&a_vec, &b_vec, &b_nz);
    let ns_b = bench_one::<cand_b::VecB>(&a_vec, &b_vec, &b_nz);
    let ns_c = bench_one::<cand_c::VecC>(&a_vec, &b_vec, &b_nz);
    let ns_d = bench_one::<cand_d::VecD>(&a_vec, &b_vec, &b_nz);

    println!("Results (median of {} runs):", REPEATS);
    print_row(cand_a::VecA::NAME, ns_a);
    print_row(cand_b::VecB::NAME, ns_b);
    print_row(cand_c::VecC::NAME, ns_c);
    print_row(cand_d::VecD::NAME, ns_d);

    println!();
    println!("Speedup vs Candidate A (baseline; >1.0 = faster):");
    let label = ["add", "sub", "mul", "div"];
    let print_speedup = |name: &str, ns: [f64; 4]| {
        let s: Vec<String> = (0..4)
            .map(|i| format!("{}={:.2}x", label[i], ns_a[i] / ns[i]))
            .collect();
        println!("  {:<40}  {}", name, s.join("  "));
    };
    print_speedup(cand_b::VecB::NAME, ns_b);
    print_speedup(cand_c::VecC::NAME, ns_c);
    print_speedup(cand_d::VecD::NAME, ns_d);
    println!();
    println!("Hard-fallback bar: any encoding is preferred over A only if it");
    println!("achieves ≥1.5× speedup on a weighted mix dominated by mul (the");
    println!("hot op in Ryser's permanent formula). See");
    println!("dev/plans/r1_f5_encoding_decision.md for the chosen winner.");
}
