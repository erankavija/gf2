//! The gf2x side of the polynomial cells (jit:53c5a8c0).
//!
//! Every product is `gf2x_mul_r`, gf2x's reentrant top-level polynomial
//! multiplication, on operands laid out exactly as gf2 lays them out: `n`
//! `unsigned long` words per operand, `2n` words of product, bit `i` of word
//! `j` the coefficient of `x^(64j + i)`. A pool is initialised once per worker
//! because that is gf2x's own contract for repeated calls; the pool
//! initialisation is reported as setup rather than hidden inside a window.
//!
//! gf2x supplies no field reduction and no batch entry point, so:
//!
//! * the whole-consumer wide-field cell composes one `gf2x_mul_r` with gf2's
//!   own `BarrettReducerWide` for the same field, which is the operation a
//!   consumer replacing only the product stage would perform; and
//! * the raw-batch cell issues one single-word `gf2x_mul_r` per product. That
//!   is a composed arm, not an equivalent operation: gf2x exposes no
//!   independent batch, and its per-call overhead is inside every product. The
//!   cell records what a consumer reaching gf2x through its public API pays,
//!   and states no gf2x basecase rate.

use std::os::raw::c_int;

use clmul_crossover_arms::wide_field::WideReducer;
use clmul_crossover_arms::{probe_ns, Backend, Bank, Case, Conversion};

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

/// gf2x arm state: one scratch pool, the reducer the composed whole-consumer
/// arm applies, and the scratch a composed call needs.
pub struct Gf2xBackend {
    pool: Gf2xPool,
    reducer: Option<WideReducer>,
    scratch: Vec<u64>,
    packed_a: Vec<u64>,
    packed_b: Vec<u64>,
}

impl Backend for Gf2xBackend {
    fn selected_path(&self) -> String {
        selected_path()
    }

    fn create(case: &Case) -> Self {
        assert_pinned_library();
        let (reducer, scratch, packed) = match case {
            Case::WideFieldMul { words, .. } => (
                WideReducer::for_words(*words),
                vec![0u64; 2 * words],
                *words,
            ),
            Case::RawBatch { .. } => (None, vec![0u64; 2], 0),
            Case::PolyMul { words, .. } => (None, vec![0u64; 2 * words], 0),
            other => panic!("the gf2x arm performs no {other:?}"),
        };
        Self {
            pool: Gf2xPool::default(),
            reducer,
            scratch,
            packed_a: vec![0u64; packed],
            packed_b: vec![0u64; packed],
        }
    }

    fn run(&mut self, case: &Case, bank: &mut Bank) {
        match case {
            Case::PolyMul { .. } => mul(&mut self.pool, &bank.a, &bank.b, &mut bank.out),
            Case::RawBatch { count, .. } => {
                for index in 0..*count {
                    mul(
                        &mut self.pool,
                        &bank.a[index..index + 1],
                        &bank.b[index..index + 1],
                        &mut bank.out[2 * index..2 * index + 2],
                    );
                }
            }
            Case::WideFieldMul { words, .. } => {
                let reducer = self
                    .reducer
                    .as_ref()
                    .expect("a wide-field case resolved its reducer");
                reducer.element_into(&bank.a, &mut self.packed_a);
                reducer.element_into(&bank.b, &mut self.packed_b);
                mul(
                    &mut self.pool,
                    &self.packed_a,
                    &self.packed_b,
                    &mut self.scratch,
                );
                reducer.reduce_into(&self.scratch, &mut bank.out[..*words]);
            }
            other => panic!("the gf2x arm performs no {other:?}"),
        }
    }

    fn conversion(&mut self, case: &Case, bank: &mut Bank) -> Conversion {
        let mut pool = None;
        let setup_ns = probe_ns(|| pool = Some(Gf2xPool::default()));
        drop(pool);
        match case {
            Case::PolyMul { words, .. } => Conversion {
                setup_ns,
                unpack_ns: clmul_crossover_arms::wide_field::reduction_probe_ns(
                    *words,
                    bank,
                    |field| self.run(case, field),
                ),
                ..Conversion::default()
            },
            Case::RawBatch { .. } => Conversion {
                setup_ns,
                ..Conversion::default()
            },
            Case::WideFieldMul { words, .. } => {
                let words = *words;
                let reducer = self
                    .reducer
                    .as_ref()
                    .expect("a wide-field case resolved its reducer");
                let pack_ns = probe_ns(|| {
                    std::hint::black_box(reducer.element(&bank.a));
                });
                let masked_a = reducer.element(&bank.a);
                let masked_b = reducer.element(&bank.b);
                let mut product = vec![0u64; 2 * words];
                mul(&mut self.pool, &masked_a, &masked_b, &mut product);
                let unpack_ns = reducer.probe_ns(&product);
                let batch_fill_ns = probe_ns(|| self.run(case, bank));
                Conversion {
                    setup_ns,
                    pack_ns,
                    unpack_ns,
                    batch_fill_ns,
                    dispatch_ns: 0,
                }
            }
            other => panic!("the gf2x arm performs no {other:?}"),
        }
    }
}
