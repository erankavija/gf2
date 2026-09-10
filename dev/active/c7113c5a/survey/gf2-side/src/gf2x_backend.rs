//! The gf2x side of every cell (jit:c7113c5a).
//!
//! Every operation is `gf2x_mul_r`, gf2x's reentrant top-level polynomial
//! multiplication, on operands laid out exactly as gf2 lays them out: `n`
//! `unsigned long` words per operand, `2n` words of product, bit `i` of word
//! `j` the coefficient of `x^(64j + i)`. A pool is initialised once per worker
//! because that is gf2x's own contract for repeated calls; the pool
//! initialisation is reported as setup rather than hidden inside a window.
//!
//! gf2x supplies no field reduction, so the whole-consumer dot-product arm
//! composes gf2x products with gf2's `BarrettReducer` for the same field.
//! `FieldVec::simd_dot_product` passes that reducer the PCLMULQDQ carry-less
//! multiply of gf2's default GF(2^m) bundle, while this arm passes the scalar
//! `barrett::clmul`, so the cell compares the product-and-accumulate stages up
//! to the difference between those two reductions, which each arm reports
//! separately. The 4-word and 9-word long-product cells likewise report the
//! gf2 `BarrettReducerWide` reduction of this arm's own product, the stage a
//! GF(2^256) or GF(2^571) consumer composing gf2x would add.

use gf2_core::gf2m::barrett::{clmul, BarrettReducer};
use std::os::raw::c_int;

use poly_baseline_arms::wide_field::reduction_probe_ns;
use poly_baseline_arms::{
    dot_reducer, dot_reduction_probe_ns, probe_ns, Backend, Bank, Case, Conversion,
    DOT_FIELD_DEGREE,
};

/// gf2x's reentrant scratch pool, `struct gf2x_mul_pool_s` in `gf2x.h`.
#[repr(C)]
#[derive(Clone, Copy)]
struct Gf2xMulPool {
    stk: *mut u64,
    stk_size: usize,
}

// SAFETY: a pool is a plain scratch allocation that gf2x mutates only through
// the `gf2x_mul_r` call it is passed to. Each worker owns its own pool and
// never shares it, so moving one to the thread that created it is sound.
unsafe impl Send for Gf2xMulPool {}

#[link(name = "gf2x")]
extern "C" {
    fn gf2x_mul_pool_init(pool: *mut Gf2xMulPool);
    fn gf2x_mul_pool_clear(pool: *mut Gf2xMulPool);
    fn gf2x_mul_r(
        c: *mut u64,
        a: *const u64,
        an: u64,
        b: *const u64,
        bn: u64,
        pool: *mut Gf2xMulPool,
    ) -> c_int;
}

/// Multiplies two GF(2) polynomials with gf2x.
///
/// `a` and `b` are the operand words and `out` receives `a.len() + b.len()`
/// words of unreduced product.
///
/// # Panics
///
/// Panics if `out` is shorter than the product or if gf2x reports an error.
pub fn mul(pool: &mut Gf2xPool, a: &[u64], b: &[u64], out: &mut [u64]) {
    assert!(
        out.len() >= a.len() + b.len(),
        "gf2x product needs {} words, got {}",
        a.len() + b.len(),
        out.len()
    );
    // SAFETY: `out` holds at least `an + bn` words as gf2x requires, the
    // operand pointers are valid for `an` and `bn` words, the destination does
    // not overlap either operand, and `pool` is an initialised pool owned by
    // this thread.
    let status = unsafe {
        gf2x_mul_r(
            out.as_mut_ptr(),
            a.as_ptr(),
            a.len() as u64,
            b.as_ptr(),
            b.len() as u64,
            &mut pool.0,
        )
    };
    assert_eq!(status, 0, "gf2x_mul_r returned {status}");
}

/// The gf2x shared object this process actually loaded.
///
/// Read from `/proc/self/maps` rather than from the link line, so a system copy
/// reached through `LD_LIBRARY_PATH` or a stale rpath is visible instead of
/// silently replacing the pinned build.
pub fn loaded_library() -> String {
    let maps = std::fs::read_to_string("/proc/self/maps").unwrap_or_default();
    let mut paths: Vec<&str> = maps
        .lines()
        .filter_map(|line| line.split_whitespace().nth(5))
        .filter(|path| path.contains("libgf2x"))
        .collect();
    paths.sort_unstable();
    paths.dedup();
    match paths.as_slice() {
        [] => "(no libgf2x mapping)".to_owned(),
        paths => paths.join(","),
    }
}

/// SHA-256 of the gf2x shared object this process mapped.
///
/// The runner digests only the arm executable, so the library bytes the
/// dynamic loader resolved enter the receipt through the selected path this
/// arm reports.
pub fn loaded_library_sha256() -> String {
    use sha2::{Digest, Sha256};
    let path = loaded_library();
    match std::fs::read(&path) {
        Ok(bytes) => format!("{:x}", Sha256::digest(bytes)),
        Err(error) => format!("(unreadable: {error})"),
    }
}

/// Aborts unless the loaded gf2x is the build `GF2X_PREFIX` named at compile
/// time.
///
/// # Panics
///
/// Panics when the process mapped a different gf2x object, because a
/// measurement or a validation taken against an unpinned library carries no
/// provenance.
pub fn assert_pinned_library() {
    let loaded = loaded_library();
    let prefix = env!("GF2X_PREFIX_USED");
    assert!(
        loaded.starts_with(prefix),
        "loaded gf2x {loaded} is not the pinned build under {prefix}"
    );
}

/// Runtime-observed identity of the gf2x build this process executes: entry
/// point, mapped library path and digest, and the CFLAGS it was built with.
pub fn selected_path() -> String {
    format!(
        "gf2x_mul_r library={} sha256={} cflags={}",
        loaded_library(),
        loaded_library_sha256(),
        env!("GF2X_CFLAGS_USED")
    )
}

/// An initialised gf2x scratch pool owned by one worker.
pub struct Gf2xPool(Gf2xMulPool);

impl Default for Gf2xPool {
    fn default() -> Self {
        let mut pool = Gf2xMulPool {
            stk: std::ptr::null_mut(),
            stk_size: 0,
        };
        // SAFETY: `pool` is a live, correctly shaped `gf2x_mul_pool_t`.
        unsafe { gf2x_mul_pool_init(&mut pool) };
        Self(pool)
    }
}

impl Drop for Gf2xPool {
    fn drop(&mut self) {
        // SAFETY: the pool was initialised by `gf2x_mul_pool_init` and is
        // cleared exactly once, at the end of its owner's life.
        unsafe { gf2x_mul_pool_clear(&mut self.0) };
    }
}

// SAFETY: the wrapped pool is never shared; see the `Gf2xMulPool` note.
unsafe impl Send for Gf2xPool {}

/// gf2x arm state: one scratch pool, plus the gf2 reducer the composed
/// whole-consumer arm applies after accumulation.
pub struct Gf2xBackend {
    pool: Gf2xPool,
    reducer: Option<BarrettReducer>,
    scratch: Vec<u64>,
}

impl Gf2xBackend {
    /// The raw XOR of the dot-product cell's `count` unreduced gf2x products
    /// of the operands masked to field elements: the value this arm reduces.
    pub fn dot_accumulator(&mut self, count: usize, bank: &Bank) -> u128 {
        let mask = (1u64 << DOT_FIELD_DEGREE) - 1;
        let mut accumulator: u128 = 0;
        for index in 0..count {
            let left = [bank.a[index] & mask];
            let right = [bank.b[index] & mask];
            mul(&mut self.pool, &left, &right, &mut self.scratch);
            accumulator ^= u128::from(self.scratch[0]) | (u128::from(self.scratch[1]) << 64);
        }
        accumulator
    }
}

impl Backend for Gf2xBackend {
    fn selected_path(&self) -> String {
        selected_path()
    }

    fn create(case: &Case) -> Self {
        assert_pinned_library();
        let reducer = match case {
            Case::Gf2mDot { .. } => Some(dot_reducer()),
            _ => None,
        };
        Self {
            pool: Gf2xPool::default(),
            reducer,
            scratch: vec![0u64; 2],
        }
    }

    fn run(&mut self, case: &Case, bank: &mut Bank) {
        match case {
            Case::PolyMul { .. } => mul(&mut self.pool, &bank.a, &bank.b, &mut bank.out),
            Case::ClmulBatch { count, .. } => {
                for index in 0..*count {
                    mul(
                        &mut self.pool,
                        &bank.a[index..index + 1],
                        &bank.b[index..index + 1],
                        &mut bank.out[2 * index..2 * index + 2],
                    );
                }
            }
            Case::Gf2mDot { count, .. } => {
                let accumulator = self.dot_accumulator(*count, bank);
                let reducer = self.reducer.as_ref().expect("the dot case built a reducer");
                bank.out[0] = reducer.reduce_with_clmul(accumulator, clmul);
            }
        }
    }

    fn conversion(&mut self, case: &Case, bank: &mut Bank) -> Conversion {
        let mut pool = None;
        let setup_ns = probe_ns(|| pool = Some(Gf2xPool::default()));
        drop(pool);
        match case {
            Case::PolyMul { words, .. } => Conversion {
                setup_ns,
                unpack_ns: reduction_probe_ns(*words, bank, |field| self.run(case, field)),
                ..Conversion::default()
            },
            Case::ClmulBatch { .. } => Conversion {
                setup_ns,
                ..Conversion::default()
            },
            Case::Gf2mDot { count, .. } => {
                let mask = (1u64 << DOT_FIELD_DEGREE) - 1;
                let count = *count;
                let mut packed = Vec::new();
                let pack_ns = probe_ns(|| {
                    packed = (0..count).map(|index| bank.a[index] & mask).collect();
                });
                std::hint::black_box(&packed);
                let batch_fill_ns = probe_ns(|| self.run(case, bank));
                // The reduction `run` applies: the same reducer and scalar
                // carry-less multiply, on the raw accumulator rather than on
                // the reduced element `run` writes.
                let accumulator = self.dot_accumulator(count, bank);
                let unpack_ns = dot_reduction_probe_ns(accumulator, clmul);
                Conversion {
                    setup_ns,
                    pack_ns,
                    unpack_ns,
                    batch_fill_ns,
                    dispatch_ns: 0,
                }
            }
        }
    }
}
