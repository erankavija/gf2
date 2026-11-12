//! Times gf2's long carry-less product and gf2x's `gf2x_mul_r`
//! (`@/citation/GfTwoX2026`) on identical operands at each size of
//! [`clmul_compare::SIZES`], after checking that the two agree word for word.
//! Prints one tab-separated line per arm and size: arm, words, products per
//! second. With `--digests <path>...` it prints the SHA-256 of itself and of
//! each path instead.

use clmul_compare::{gf2_product, operands, products_per_second, SIZES};
use std::os::raw::c_int;

/// gf2x's reentrant scratch pool, `struct gf2x_mul_pool_s` in `gf2x.h`.
#[repr(C)]
struct Pool {
    stk: *mut u64,
    stk_size: usize,
}

#[link(name = "gf2x")]
extern "C" {
    fn gf2x_mul_pool_init(pool: *mut Pool);
    fn gf2x_mul_pool_clear(pool: *mut Pool);
    fn gf2x_mul_r(
        c: *mut u64,
        a: *const u64,
        an: u64,
        b: *const u64,
        bn: u64,
        pool: *mut Pool,
    ) -> c_int;
}

fn gf2x_product(pool: &mut Pool, a: &[u64], b: &[u64], out: &mut [u64]) {
    assert!(out.len() >= a.len() + b.len());
    // SAFETY: `out` holds an + bn words, the operands hold an and bn words,
    // nothing overlaps, and `pool` is an initialised pool of this thread.
    let status = unsafe {
        gf2x_mul_r(
            out.as_mut_ptr(),
            a.as_ptr(),
            a.len() as u64,
            b.as_ptr(),
            b.len() as u64,
            pool,
        )
    };
    assert_eq!(status, 0, "gf2x_mul_r failed");
}

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--digests") {
        experiments_common::write_executable_digests(&mut std::io::stdout(), args)
            .expect("the executable and the listed files are readable");
        return;
    }
    let mut pool = Pool {
        stk: std::ptr::null_mut(),
        stk_size: 0,
    };
    // SAFETY: gf2x initialises the zeroed pool in place.
    unsafe { gf2x_mul_pool_init(&mut pool) };
    for words in SIZES {
        let (a, b) = operands(words, 0x6766_3278 ^ words as u64);
        let (mut ours, mut theirs) = (vec![0u64; 2 * words], vec![0u64; 2 * words]);
        gf2_product(&a, &b, &mut ours);
        gf2x_product(&mut pool, &a, &b, &mut theirs);
        assert_eq!(ours, theirs, "gf2 and gf2x disagree at {words} words");
        let gf2 = products_per_second(|| gf2_product(&a, &b, &mut ours));
        let gf2x = products_per_second(|| gf2x_product(&mut pool, &a, &b, &mut theirs));
        println!("gf2\t{words}\t{gf2:.1}");
        println!("gf2x\t{words}\t{gf2x:.1}");
    }
    // SAFETY: the pool was initialised above and is not used again.
    unsafe { gf2x_mul_pool_clear(&mut pool) };
}
