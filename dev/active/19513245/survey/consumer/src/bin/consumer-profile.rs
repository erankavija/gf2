//! Consumer profile of the current and prototype byte-field routes
//! (jit:19513245).
//!
//! The A/B receipts decide; this binary says how a route's cost is composed.
//! For every case it reports what it observes at run time: the route string
//! the consumer selects, wall time per call, the exact allocation count and
//! byte total of one call, the reuse count (how many multiplications one
//! prepared coefficient table serves), and the conversion, table-preparation
//! and setup probes. Nothing here is a prior figure: every number comes from
//! this process, and the case ladder below is a labelled plan constant.
//!
//! Wall time is measured with the counting allocator registered, which adds
//! two relaxed atomic increments to each allocation. That perturbs exactly
//! the routes whose allocation count this binary reports, so the wall times
//! are descriptive composition evidence; the campaign receipts carry the
//! timing that decides anything.
//!
//! Subcommands:
//!
//! * `ladder` prints the case ladder as JSON.
//! * `session <output.json>` measures every case once and writes the record.
//! * `counters <case-id>` repeats one case for about a second so an external
//!   counter tool can wrap the process, and prints the calls it performed.

use byte_field_gf2_side::workload::{self, ByteField, RuntimeGf256, WideGf256};
use bytefield_consumer::{
    axpy_field_vec, axpy_region, gemm_region, matrix_to_region, matvec_region, pairwise_region,
    CoefficientTable, ProductTable,
};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{batch, Gf2mField};
use serde::Serialize;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Allocation counter.
///
/// # Safety
///
/// Every method forwards to [`System`] unchanged and only adds relaxed
/// atomic counter updates, so the allocator contract is exactly `System`'s.
struct Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout)
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        System.realloc(pointer, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const KIB: usize = 1024;

/// The consumer a case measures.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Consumer {
    /// `FieldVec::axpy` in place on the vector the consumer holds.
    AxpyVector,
    /// Fixed-coefficient multiply-accumulate from and to a byte region.
    AxpyRegion,
    /// `field::matrix::gemm` on the matrices the consumer holds.
    Gemm,
    /// `field::matrix::gemm` from and to the `FieldMatrix` boundary, so the
    /// prototype pays its conversion.
    GemmWhole,
    /// `FieldMatrix::matvec`.
    Matvec,
    /// `gf2m::batch::batch_mul`, the arbitrary-multiplication control.
    Pairwise,
    /// `gf2m::batch::batch_mul` from and to a byte region.
    PairwiseRegion,
}

/// Which route a case measures.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Route {
    Current,
    Prototype,
}

/// The element representation the consumer holds.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Repr {
    Element,
    Wide,
    Batch,
}

/// One declared case of the ladder.
#[derive(Clone, Copy, Serialize)]
struct Case {
    id: &'static str,
    consumer: Consumer,
    route: Route,
    repr: Repr,
    /// Region length in bytes, or the square dimension for a matrix case.
    size: usize,
}

/// The case ladder: a labelled plan constant, not an observation.
///
/// Sizes are the region lengths and square dimensions the campaign families
/// declare, so a profile row explains a receipt row of the same shape.
fn ladder() -> Vec<Case> {
    use Consumer::*;
    use Repr::*;
    use Route::*;
    let mut cases = Vec::new();
    for (repr, tag) in [(Element, "element"), (Wide, "wide")] {
        for (consumer, label, sizes) in [
            (AxpyVector, "axpy", &[4 * KIB, 128 * KIB][..]),
            (AxpyRegion, "region", &[4 * KIB, 128 * KIB][..]),
            (Gemm, "matmul", &[64, 256][..]),
            (GemmWhole, "matmul-whole", &[256][..]),
            (Matvec, "matvec", &[256][..]),
        ] {
            for size in sizes {
                for (route, route_tag) in [(Current, "current"), (Prototype, "prototype")] {
                    cases.push(Case {
                        id: leak(format!("{label}-{}-{tag}-{route_tag}", shape(consumer, *size))),
                        consumer,
                        route,
                        repr,
                        size: *size,
                    });
                }
            }
        }
    }
    for (consumer, label, sizes) in [
        (Pairwise, "pairwise", &[4 * KIB, 128 * KIB][..]),
        (PairwiseRegion, "pairwise-region", &[128 * KIB][..]),
    ] {
        for size in sizes {
            for (route, route_tag) in [(Current, "current"), (Prototype, "prototype")] {
                cases.push(Case {
                    id: leak(format!("{label}-{}-batch-{route_tag}", shape(consumer, *size))),
                    consumer,
                    route,
                    repr: Repr::Batch,
                    size: *size,
                });
            }
        }
    }
    cases
}

/// Case identifiers are built from the ladder's own sizes, so they are
/// leaked rather than typed twice.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn shape(consumer: Consumer, size: usize) -> String {
    match consumer {
        Consumer::Gemm | Consumer::GemmWhole | Consumer::Matvec => format!("n{size}"),
        _ => {
            if size % KIB == 0 {
                format!("{}k", size / KIB)
            } else {
                format!("{size}b")
            }
        }
    }
}

/// What one case reports.
#[derive(Serialize)]
struct Record {
    #[serde(flatten)]
    case: Case,
    /// The route the consumer selected, observed at run time.
    selected_path: String,
    /// Full reduction polynomial the case works over.
    polynomial: u32,
    /// Median nanoseconds per call over the windows below.
    ns_per_call: f64,
    /// Per-window nanoseconds per call, in window order.
    windows_ns_per_call: Vec<f64>,
    /// Calls in each window.
    calls_per_window: u64,
    /// Allocations one call performs, counted by the global allocator.
    allocations_per_call: u64,
    /// Bytes one call allocates.
    allocated_bytes_per_call: u64,
    /// Multiplications one prepared coefficient table serves; zero when the
    /// route prepares no coefficient table.
    reuse_per_table: u64,
    /// Bytes of the prepared table, zero when the route prepares none.
    table_bytes: u64,
    /// Preparing the field or the library context.
    setup_ns: u64,
    /// Preparing the coefficient or full multiplication table.
    table_ns: u64,
    /// Converting the operands into the route's representation.
    pack_ns: u64,
    /// Converting the result out of it.
    unpack_ns: u64,
}

/// Windows each case times.
const WINDOWS: usize = 5;
/// Target length of one window.
const WINDOW_TARGET: Duration = Duration::from_millis(40);
/// Repetitions of a conversion probe.
const PROBE_REPETITIONS: usize = 9;

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let cases = ladder();
    match arguments.get(1).map(String::as_str) {
        Some("ladder") => {
            println!("{}", serde_json::to_string_pretty(&cases).expect("encodes"));
        }
        Some("session") => {
            let output = arguments.get(2).expect("session needs an output path");
            let records: Vec<Record> = cases.iter().map(|case| measure(*case)).collect();
            std::fs::write(
                output,
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&records).expect("encodes")
                ),
            )
            .expect("writes the session record");
            eprintln!("{} cases -> {output}", records.len());
        }
        Some("counters") => {
            let wanted = arguments.get(2).expect("counters needs a case identifier");
            let case = cases
                .iter()
                .find(|case| case.id == wanted)
                .unwrap_or_else(|| panic!("no case named {wanted}"));
            let mut body = build(*case).body;
            let started = Instant::now();
            let mut calls = 0u64;
            while started.elapsed() < Duration::from_secs(1) {
                body();
                calls += 1;
            }
            println!("calls {calls} case {wanted}");
        }
        _ => {
            eprintln!("usage: consumer-profile ladder | session <output.json> | counters <case-id>");
            std::process::exit(2);
        }
    }
}

/// A prepared case: the measured closure and everything observed around it.
struct Prepared<'a> {
    selected_path: String,
    polynomial: u32,
    reuse_per_table: u64,
    table_bytes: u64,
    setup_ns: u64,
    table_ns: u64,
    pack_ns: u64,
    unpack_ns: u64,
    body: Box<dyn FnMut() + 'a>,
}

fn measure(case: Case) -> Record {
    let prepared = build(case);
    let Prepared {
        selected_path,
        polynomial,
        reuse_per_table,
        table_bytes,
        setup_ns,
        table_ns,
        pack_ns,
        unpack_ns,
        mut body,
    } = prepared;
    // One untimed call warms the working set and settles any lazily
    // initialised dispatch table.
    body();
    // Allocation accounting of one call, before the timed windows.
    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = ALLOCATED_BYTES.load(Ordering::Relaxed);
    body();
    let allocations_per_call = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;
    let allocated_bytes_per_call = ALLOCATED_BYTES.load(Ordering::Relaxed) - bytes_before;
    // Calibrate the call count to the window target, then time the windows.
    let single = time(&mut body, 1);
    let calls_per_window = ((WINDOW_TARGET.as_nanos() as u64) / single.max(1)).max(1);
    let mut windows: Vec<f64> = Vec::with_capacity(WINDOWS);
    for _ in 0..WINDOWS {
        let elapsed = time(&mut body, calls_per_window);
        windows.push(elapsed as f64 / calls_per_window as f64);
    }
    let mut sorted = windows.clone();
    sorted.sort_by(f64::total_cmp);
    Record {
        case,
        selected_path,
        polynomial,
        ns_per_call: sorted[sorted.len() / 2],
        windows_ns_per_call: windows,
        calls_per_window,
        allocations_per_call,
        allocated_bytes_per_call,
        reuse_per_table,
        table_bytes,
        setup_ns,
        table_ns,
        pack_ns,
        unpack_ns,
    }
}

fn time(body: &mut Box<dyn FnMut() + '_>, calls: u64) -> u64 {
    let started = Instant::now();
    for _ in 0..calls {
        body();
    }
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Median of repeated runs of one untimed conversion.
fn probe(mut operation: impl FnMut()) -> u64 {
    let mut samples = Vec::with_capacity(PROBE_REPETITIONS);
    for _ in 0..PROBE_REPETITIONS {
        let started = Instant::now();
        operation();
        samples.push(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn build(case: Case) -> Prepared<'static> {
    match case.repr {
        Repr::Element => {
            let setup_ns = probe(|| {
                black_box(RuntimeGf256::new());
            });
            build_typed(RuntimeGf256::new(), case, setup_ns)
        }
        Repr::Wide => build_typed(WideGf256, case, 0),
        Repr::Batch => {
            let setup_ns = probe(|| {
                black_box(Gf2mField::gf256());
            });
            build_pairwise(case, setup_ns)
        }
    }
}

fn build_typed<B: ByteField + 'static>(field: B, case: Case, setup_ns: u64) -> Prepared<'static> {
    match case.consumer {
        Consumer::AxpyVector | Consumer::AxpyRegion => build_axpy(field, case, setup_ns),
        Consumer::Gemm | Consumer::GemmWhole => build_matmul(field, case, setup_ns),
        Consumer::Matvec => build_matvec(field, case, setup_ns),
        Consumer::Pairwise | Consumer::PairwiseRegion => {
            panic!("the pairwise control uses the batch representation")
        }
    }
}

fn operands(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed | 1;
    (0..length)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state >> 33) as u8
        })
        .collect()
}

fn build_axpy<B: ByteField + 'static>(
    field: B,
    case: Case,
    setup_ns: u64,
) -> Prepared<'static> {
    let length = case.size;
    let polynomial = field.polynomial();
    let source_bytes = operands(0x51D, length);
    let mut target_bytes = operands(0x7A6, length);
    let coefficient_byte = operands(0x3C1, 1)[0] | 1;
    let coefficient = field.element(coefficient_byte);
    let mut sources = workload::pack_vec(&field, &source_bytes);
    let mut targets = workload::pack_vec(&field, &target_bytes);
    let mut table = CoefficientTable::new(coefficient_byte, polynomial as u16);
    let table_ns = probe(|| table.refill(coefficient_byte, polynomial as u16));
    let pack_ns = probe(|| {
        workload::pack_into_vec(&field, &source_bytes, &mut sources);
        workload::pack_into_vec(&field, &target_bytes, &mut targets);
    });
    let mut scratch = vec![0u8; length];
    let unpack_ns = probe(|| workload::unpack_vec::<B>(&targets, &mut scratch));
    let region = case.consumer == Consumer::AxpyRegion;
    let selected_path = match (case.route, region) {
        (Route::Current, false) => format!(
            "gf2-core/FieldVec<{}>::axpy/scalar-element-loop",
            B::NAME
        ),
        (Route::Current, true) => format!(
            "gf2-core/FieldVec<{}>::axpy/scalar-element-loop/byte-region-boundary",
            B::NAME
        ),
        (Route::Prototype, false) => {
            format!("prototype/coefficient-table/in-place-FieldVec<{}>", B::NAME)
        }
        (Route::Prototype, true) => "prototype/coefficient-table/byte-region".to_owned(),
    };
    // Each route's body, built separately so no branch is inside the window.
    let body: Box<dyn FnMut()> = match (case.route, region) {
        (Route::Current, false) => {
            Box::new(move || {
                workload::axpy(&mut targets, &coefficient, black_box(&sources));
                black_box(&targets);
            })
        }
        (Route::Current, true) => Box::new(move || {
            workload::pack_into_vec(&field, black_box(&source_bytes), &mut sources);
            workload::pack_into_vec(&field, black_box(&target_bytes), &mut targets);
            workload::axpy(&mut targets, &coefficient, &sources);
            workload::unpack_vec::<B>(&targets, &mut target_bytes);
            black_box(&target_bytes);
        }),
        (Route::Prototype, false) => Box::new(move || {
            table.refill(coefficient_byte, polynomial as u16);
            axpy_field_vec(&field, &mut targets, &table, black_box(&sources));
            black_box(&targets);
        }),
        (Route::Prototype, true) => Box::new(move || {
            table.refill(coefficient_byte, polynomial as u16);
            axpy_region(&mut target_bytes, &table, black_box(&source_bytes));
            black_box(&target_bytes);
        }),
    };
    Prepared {
        selected_path,
        polynomial,
        reuse_per_table: if case.route == Route::Prototype {
            length as u64
        } else {
            0
        },
        table_bytes: if case.route == Route::Prototype { 256 } else { 0 },
        setup_ns,
        table_ns: if case.route == Route::Prototype { table_ns } else { 0 },
        pack_ns: if case.route == Route::Current && region { pack_ns } else { 0 },
        unpack_ns: if case.route == Route::Current && region { unpack_ns } else { 0 },
        body,
    }
}

fn build_matmul<B: ByteField + 'static>(
    field: B,
    case: Case,
    setup_ns: u64,
) -> Prepared<'static> {
    let n = case.size;
    let polynomial = field.polynomial();
    let mut left_bytes = operands(0x11EE, n * n);
    let mut right_bytes = operands(0x22FF, n * n);
    let left = workload::pack_matrix(&field, &left_bytes, n, n);
    let right = workload::pack_matrix(&field, &right_bytes, n, n);
    let mut product = workload::matmul(&left, &right);
    let mut out_bytes = vec![0u8; n * n];
    let table_ns = probe(|| {
        black_box(ProductTable::new(polynomial as u16));
    });
    let table = ProductTable::new(polynomial as u16);
    let pack_ns = probe(|| {
        matrix_to_region::<B>(&left, &mut left_bytes);
        matrix_to_region::<B>(&right, &mut right_bytes);
    });
    let unpack_ns = probe(|| workload::pack_into_matrix(&field, &out_bytes, &mut product));
    let whole = case.consumer == Consumer::GemmWhole;
    let selected_path = match case.route {
        Route::Current => format!(
            "gf2-core/field::matrix::gemm/FieldMatrix<{}>/{}",
            B::NAME,
            B::gemm_route()
        ),
        Route::Prototype => format!(
            "prototype/product-table-{}/byte-matrix{}",
            table.bytes(),
            if whole { "/FieldMatrix-boundary" } else { "" }
        ),
    };
    let table_bytes = table.bytes() as u64;
    let body: Box<dyn FnMut()> = match (case.route, whole) {
        (Route::Current, _) => Box::new(move || {
            let product = workload::matmul(black_box(&left), black_box(&right));
            black_box(&product);
        }),
        (Route::Prototype, false) => Box::new(move || {
            gemm_region(
                black_box(&left_bytes),
                black_box(&right_bytes),
                &mut out_bytes,
                n,
                n,
                n,
                &table,
            );
            black_box(&out_bytes);
        }),
        (Route::Prototype, true) => Box::new(move || {
            matrix_to_region::<B>(black_box(&left), &mut left_bytes);
            matrix_to_region::<B>(black_box(&right), &mut right_bytes);
            gemm_region(&left_bytes, &right_bytes, &mut out_bytes, n, n, n, &table);
            workload::pack_into_matrix(&field, &out_bytes, &mut product);
            black_box(&product);
        }),
    };
    Prepared {
        selected_path,
        polynomial,
        // One left-operand element is the coefficient of a whole output row.
        reuse_per_table: if case.route == Route::Prototype { n as u64 } else { 0 },
        table_bytes: if case.route == Route::Prototype { table_bytes } else { 0 },
        setup_ns,
        table_ns: if case.route == Route::Prototype { table_ns } else { 0 },
        pack_ns: if case.route == Route::Prototype && whole { pack_ns } else { 0 },
        unpack_ns: if case.route == Route::Prototype && whole { unpack_ns } else { 0 },
        body,
    }
}

fn build_matvec<B: ByteField + 'static>(
    field: B,
    case: Case,
    setup_ns: u64,
) -> Prepared<'static> {
    let n = case.size;
    let polynomial = field.polynomial();
    let matrix_bytes = operands(0x33AA, n * n);
    let vector_bytes = operands(0x44BB, n);
    let matrix = workload::pack_matrix(&field, &matrix_bytes, n, n);
    let vector = FieldVec::from(
        vector_bytes
            .iter()
            .map(|byte| field.element(*byte))
            .collect::<Vec<_>>(),
    );
    let mut out_bytes = vec![0u8; n];
    let table_ns = probe(|| {
        black_box(ProductTable::new(polynomial as u16));
    });
    let table = ProductTable::new(polynomial as u16);
    let table_bytes = table.bytes() as u64;
    let selected_path = match case.route {
        Route::Current => format!(
            "gf2-core/FieldMatrix<{}>::matvec/dot-product-slices-scalar-chain",
            B::NAME
        ),
        Route::Prototype => format!("prototype/product-table-{}/byte-matrix", table.bytes()),
    };
    let body: Box<dyn FnMut()> = match case.route {
        Route::Current => Box::new(move || {
            let out = matrix.matvec(black_box(&vector));
            black_box(&out);
        }),
        Route::Prototype => Box::new(move || {
            matvec_region(
                black_box(&matrix_bytes),
                black_box(&vector_bytes),
                &mut out_bytes,
                n,
                n,
                &table,
            );
            black_box(&out_bytes);
        }),
    };
    Prepared {
        selected_path,
        polynomial,
        // Row-major traversal reuses no coefficient, so the full table
        // serves every product and no coefficient table is prepared.
        reuse_per_table: 0,
        table_bytes: if case.route == Route::Prototype { table_bytes } else { 0 },
        setup_ns,
        table_ns: if case.route == Route::Prototype { table_ns } else { 0 },
        pack_ns: 0,
        unpack_ns: 0,
        body,
    }
}

fn build_pairwise(case: Case, setup_ns: u64) -> Prepared<'static> {
    let length = case.size;
    let field = Gf2mField::gf256();
    let polynomial = field.primitive_polynomial() as u32;
    let left_bytes = operands(0x55CC, length);
    let right_bytes = operands(0x66DD, length);
    let mut left_lanes = workload::widen(&left_bytes);
    let mut right_lanes = workload::widen(&right_bytes);
    let mut lanes = vec![0u64; length];
    let mut out_bytes = vec![0u8; length];
    let table_ns = probe(|| {
        black_box(ProductTable::new(polynomial as u16));
    });
    let table = ProductTable::new(polynomial as u16);
    let table_bytes = table.bytes() as u64;
    batch::batch_mul(&field, &left_lanes, &right_lanes, &mut lanes);
    let pack_ns = probe(|| {
        workload::widen_into(&left_bytes, &mut left_lanes);
        workload::widen_into(&right_bytes, &mut right_lanes);
    });
    let unpack_ns = probe(|| workload::narrow(&lanes, &mut out_bytes));
    let region = case.consumer == Consumer::PairwiseRegion;
    let kernel = if workload::batch_kernel_available() {
        "vpclmulqdq-batch"
    } else {
        "scalar-batch"
    };
    let selected_path = match case.route {
        Route::Current => format!(
            "gf2-core/gf2m::batch::batch_mul/u64-lanes/{kernel}{}",
            if region { "/byte-region-boundary" } else { "" }
        ),
        Route::Prototype => format!("prototype/product-table-{}/byte-region", table.bytes()),
    };
    let body: Box<dyn FnMut()> = match (case.route, region) {
        (Route::Current, false) => Box::new(move || {
            batch::batch_mul(
                &field,
                black_box(&left_lanes),
                black_box(&right_lanes),
                &mut lanes,
            );
            black_box(&lanes);
        }),
        (Route::Current, true) => Box::new(move || {
            workload::widen_into(black_box(&left_bytes), &mut left_lanes);
            workload::widen_into(black_box(&right_bytes), &mut right_lanes);
            batch::batch_mul(&field, &left_lanes, &right_lanes, &mut lanes);
            workload::narrow(&lanes, &mut out_bytes);
            black_box(&out_bytes);
        }),
        (Route::Prototype, _) => Box::new(move || {
            pairwise_region(
                black_box(&left_bytes),
                black_box(&right_bytes),
                &mut out_bytes,
                &table,
            );
            black_box(&out_bytes);
        }),
    };
    Prepared {
        selected_path,
        polynomial,
        reuse_per_table: 0,
        table_bytes: if case.route == Route::Prototype { table_bytes } else { 0 },
        setup_ns,
        table_ns: if case.route == Route::Prototype { table_ns } else { 0 },
        pack_ns: if case.route == Route::Current && region { pack_ns } else { 0 },
        unpack_ns: if case.route == Route::Current && region { unpack_ns } else { 0 },
        body,
    }
}
