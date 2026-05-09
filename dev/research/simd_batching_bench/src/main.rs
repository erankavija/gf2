//! Bench driver entry point. Run via `cargo run --release`.

use simd_batching_bench::bench::{format_table, run_all};

fn main() {
    println!("simd_batching_bench (R4 / JIT issue c7542983)");
    println!("===============================================");
    println!();
    let (cells, avx2_ok) = run_all();
    if !avx2_ok {
        println!("AVX2 not detected at runtime — bench skipped.");
        std::process::exit(2);
    }
    println!("AVX2 detected: yes");
    println!();
    println!("{}", format_table(&cells));
    println!();
    println!("Notes:");
    println!(
        "- 'cycles/op' is the minimum across {} reps of {} inner invocations each.",
        21, 1024
    );
    println!("- 'ratio' = generic / per-prime; >1 means generic is slower.");
    println!("- All measurements use std::arch::x86_64::_rdtsc + std::time::Instant.");
}
