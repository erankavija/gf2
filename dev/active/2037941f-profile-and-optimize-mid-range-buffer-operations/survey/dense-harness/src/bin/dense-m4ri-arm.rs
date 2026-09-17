//! Protocol-v4 external comparator arm: M4RI `mzd_mul(y, A, x, 0)`.
//!
//! The arm receives the same canonical gf2 words its gf2 peer measures, writes
//! them to public M4RI coordinates, executes the matched product, reads the
//! result back into a gf2 `BitVec`, and releases every `mzd_t` it owns. The
//! public coordinate accessors are static inline in `m4ri/mzd.h`, so those
//! charged components live in the C shim `survey/m4ri_matvec_arm.c` that
//! `build.rs` compiles against the qualified install.
//!
//! `--backend` reports the linked entry point and the qualified shapes.
//! `--oracle` runs the comparator's semantic cases against the gf2 peer. With
//! no argument the executable is a child-v2 campaign arm.

use dense_parity_harness::cells::{Cache, Workload, M4RI_SHAPES};
use dense_parity_harness::fixture::MatvecBanks;
use dense_parity_harness::oracle::OracleCase;
use dense_parity_harness::routes::{observe_output, run_windows, WindowPlan};
use dense_parity_harness::wire::{
    read_request, require_window_unless_child, ArmResult, ConversionCosts,
};
use gf2_core::{BitMatrix, BitVec};
use std::cell::{Cell as MutCell, RefCell};
use std::ffi::c_void;
use std::hint::black_box;
use std::time::Instant;

unsafe extern "C" {
    /// One fresh whole-consumer call, including disposal of every `mzd_t`.
    fn gf2_m4ri_fresh_call(
        rows_words: *const u64,
        x_words: *const u64,
        nrows: i32,
        ncols: i32,
        y_words: *mut u64,
    ) -> i32;
    /// Converts and retains the inputs of a retained-state cell.
    fn gf2_m4ri_retain(
        rows_words: *const u64,
        x_words: *const u64,
        nrows: i32,
        ncols: i32,
    ) -> *mut c_void;
    /// One retained-state call: only the output `mzd_t` is created and freed.
    fn gf2_m4ri_retained_call(state: *const c_void, y_words: *mut u64) -> i32;
    /// Releases retained state.
    fn gf2_m4ri_release(state: *mut c_void);
}

/// Canonical row-major words of a public matrix, as the shim consumes them.
fn row_major_words(matrix: &BitMatrix) -> Vec<u64> {
    let mut words = Vec::with_capacity(matrix.rows() * matrix.stride_words());
    for row in 0..matrix.rows() {
        words.extend_from_slice(matrix.row_words(row));
    }
    words
}

/// One fresh whole-consumer M4RI call, output allocation and release included.
#[inline]
fn fresh_operation(rows_words: &[u64], x_words: &[u64], rows: usize, cols: usize) -> Option<u64> {
    let mut y_words = vec![0_u64; rows.div_ceil(64)];
    // SAFETY: both input slices outlive the call and hold the row-major and
    // vector words of a `rows`-by-`cols` shape, and `y_words` holds the
    // `rows.div_ceil(64)` writable words the shim fills. The shim frees every
    // `mzd_t` it creates and writes nothing else.
    let status = unsafe {
        gf2_m4ri_fresh_call(
            rows_words.as_ptr(),
            x_words.as_ptr(),
            rows as i32,
            cols as i32,
            y_words.as_mut_ptr(),
        )
    };
    (status == 0).then(|| observe_output(&BitVec::from_words(y_words, rows)))
}

/// Retained converted M4RI inputs, released when the guard drops.
struct RetainedState {
    handle: *mut c_void,
    rows: usize,
}

impl RetainedState {
    fn new(rows_words: &[u64], x_words: &[u64], rows: usize, cols: usize) -> Option<Self> {
        // SAFETY: as `fresh_operation`; the shim copies the words it needs into
        // its own `mzd_t` storage and returns null on failure.
        let handle = unsafe {
            gf2_m4ri_retain(rows_words.as_ptr(), x_words.as_ptr(), rows as i32, cols as i32)
        };
        (!handle.is_null()).then_some(Self { handle, rows })
    }

    #[inline]
    fn call(&self) -> Option<u64> {
        let mut y_words = vec![0_u64; self.rows.div_ceil(64)];
        // SAFETY: `self.handle` came from `gf2_m4ri_retain` and has not been
        // released, and `y_words` holds the writable words the shim fills.
        let status = unsafe { gf2_m4ri_retained_call(self.handle, y_words.as_mut_ptr()) };
        (status == 0).then(|| observe_output(&BitVec::from_words(y_words, self.rows)))
    }
}

impl Drop for RetainedState {
    fn drop(&mut self) {
        // SAFETY: the handle came from `gf2_m4ri_retain` and is released once.
        unsafe { gf2_m4ri_release(self.handle) }
    }
}

fn backend_record() -> String {
    let shapes = M4RI_SHAPES
        .iter()
        .map(|shape| format!("{}x{}", shape.rows, shape.cols))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"library\":\"m4ri\",\"entrypoint\":\"mzd_mul\",\"matched_operation\":\
         \"mzd_mul(y, A, x, 0)\",\"coordinates\":\"mzd_write_bit/mzd_read_bit\",\
         \"qualified_shapes\":\"{shapes}\",\"comparator\":\"{}\"}}",
        dense_parity_harness::cells::COMPARATOR_PATH
    )
}

/// The comparator's semantic cases against the gf2 peer.
fn oracle() -> Result<Vec<OracleCase>, String> {
    let mut report = Vec::new();
    for shape in M4RI_SHAPES {
        let name = format!("m4ri-{}x{}", shape.rows, shape.cols);
        let seed = dense_parity_harness::cells::CAMPAIGN_SEED ^ ((shape.cols as u64) << 24);
        let banks = MatvecBanks::build(shape.rows, shape.cols, Cache::Warm, seed);
        let item = banks.item(0, 0);
        let rows_words = row_major_words(&item.matrix);
        let x_words = item.vector.words().to_vec();
        let mut checks = 0;

        // The shim sets bits into a zeroed buffer, so every set word it
        // leaves behind came from an `mzd_read_bit` of the product.
        let mut y_words = vec![0_u64; shape.rows.div_ceil(64)];
        // SAFETY: as `fresh_operation`.
        let status = unsafe {
            gf2_m4ri_fresh_call(
                rows_words.as_ptr(),
                x_words.as_ptr(),
                shape.rows as i32,
                shape.cols as i32,
                y_words.as_mut_ptr(),
            )
        };
        if status != 0 {
            return Err(format!("FAIL {name}: gf2_m4ri_fresh_call returned {status}"));
        }
        checks += 1;

        let peer = item.matrix.matvec(&item.vector);
        let external = BitVec::from_words(y_words.clone(), shape.rows);
        for row in 0..shape.rows {
            if external.get(row) != peer.get(row) {
                return Err(format!("FAIL {name}: row {row} differs from the gf2 peer"));
            }
            checks += 1;
        }
        let tail = shape.rows % 64;
        if tail != 0 && y_words[y_words.len() - 1] >> tail != 0 {
            return Err(format!("FAIL {name}: the unpacked output carries tail padding"));
        }
        checks += 1;

        // The retained arm converts once and still produces the same product.
        let retained = RetainedState::new(&rows_words, &x_words, shape.rows, shape.cols)
            .ok_or_else(|| format!("FAIL {name}: retained conversion failed"))?;
        let first = retained
            .call()
            .ok_or_else(|| format!("FAIL {name}: the retained call failed"))?;
        let second = retained
            .call()
            .ok_or_else(|| format!("FAIL {name}: the retained call failed twice"))?;
        if first != second || first != observe_output(&peer) {
            return Err(format!("FAIL {name}: the retained arm differs from the gf2 peer"));
        }
        checks += 2;

        let fresh = MatvecBanks::build(shape.rows, shape.cols, Cache::Warm, seed);
        if row_major_words(&fresh.item(0, 0).matrix) != rows_words
            || fresh.item(0, 0).vector.words() != x_words
        {
            return Err(format!("FAIL {name}: an input changed"));
        }
        checks += 2;
        report.push(OracleCase { name, checks });
    }
    Ok(report)
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match arguments.as_slice() {
        [] => run(),
        [flag] if flag == "--backend" => {
            println!("{}", backend_record());
            Ok(())
        }
        [flag] if flag == "--oracle" => oracle().map(|report| {
            for case in report {
                println!("{case}");
            }
        }),
        _ => Err("usage: dense-m4ri-arm [--backend | --oracle]".to_owned()),
    };
    if let Err(error) = outcome {
        eprintln!("dense-m4ri-arm: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    require_window_unless_child()?;
    let (request, case, cache) = read_request()?;
    let Workload::M4riGap { shape, retained } = case.workload()? else {
        return Err("the M4RI comparator arm serves only matvec-vs-m4ri cells".into());
    };
    let setup_start = Instant::now();
    let banks = MatvecBanks::build(shape.rows, shape.cols, cache, case.seed());
    let item = banks.item(0, 0);
    let rows_words = row_major_words(&item.matrix);
    let x_words = item.vector.words().to_vec();
    let state = match retained {
        true => Some(
            RetainedState::new(&rows_words, &x_words, shape.rows, shape.cols)
                .ok_or("the retained conversion failed")?,
        ),
        false => None,
    };
    let selected_path = format!(
        "m4ri/mzd_mul(y,A,x,0)/{}/rows={}/cols={}/words={}/coordinates=mzd_write_bit+mzd_read_bit",
        if retained { "retained-state" } else { "fresh-whole-consumer" },
        shape.rows,
        shape.cols,
        shape.stride_words()
    );

    let failure = RefCell::new(None);
    let observed = MutCell::new(0_u64);
    let mut body = |_: usize, _: usize| {
        let value = match &state {
            Some(state) => state.call(),
            None => fresh_operation(&rows_words, &x_words, shape.rows, shape.cols),
        };
        match value {
            Some(value) => observed.set(observed.get() ^ value),
            None => *failure.borrow_mut() = Some("an M4RI call failed".to_owned()),
        }
    };
    let setup_ns = u64::try_from(setup_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let samples = run_windows(
        WindowPlan {
            cache,
            cold_calls: request.cold_calls,
            windows: request.windows,
            window_target_ms: request.window_target_ms,
            banks: cache.banks(),
            items: banks.items(),
        },
        &mut body,
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    if let Some(error) = failure.borrow().clone() {
        return Err(error);
    }
    black_box(observed.get());
    ArmResult::new(
        &samples,
        cache,
        selected_path,
        ConversionCosts {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
    )
    .emit()
    .map_err(|error| error.to_string())
}
