//! Consumer-side byte-field arm (jit:19513245).
//!
//! One executable carries both sides of every pair. `GF2_BYTEFIELD_ROUTE`
//! selects the route (`current` for the gf2-core entry point, `prototype` for
//! the byte-oriented table route) and `GF2_BYTEFIELD_REPR` the element
//! representation the consumer holds (`element` for `Gf2mElement` over
//! `Gf2mField::gf256()`, `wide` for `Gf2mWide<1, Gf256x11d>`, `batch` for the
//! `u64` lanes `gf2m::batch::batch_mul` reads). Both sides of a pair share
//! one build identity, so a cell compares routes and nothing else.
//!
//! What each metric kind times:
//!
//! * `kernel-isolated`: the call on operands already in the route's own
//!   representation. For the vector cells both routes hold the same
//!   `FieldVec`, so the cell is a direct route comparison; for the matrix
//!   cells the current route holds a `FieldMatrix` and the prototype a byte
//!   matrix, and the whole-consumer twin is what fixes a shared boundary.
//! * `whole-consumer`: the window starts and ends at one shared boundary and
//!   includes every conversion each route needs to reach it. The vector
//!   family's boundary is the byte region, the region-shaped workload; the
//!   matrix family's boundary is the `FieldMatrix` the library consumer
//!   actually holds, so it is the prototype that pays conversion there.
//!
//! The coefficient table of a vector cell is prepared inside the timed call,
//! because the consumer passes a new coefficient with every call. The full
//! 256-by-256 table of a matrix or pairwise cell is prepared during
//! preparation and reported as `setup_ns`, because it depends on the field
//! alone and a production design would build it once per field.

use byte_field_arm_common::{
    banks, probe_ns, Case, Conversion, Metric, OperandStream, Operation, SplitMix64, Workload,
};
use byte_field_gf2_side::workload::{
    self, batch_kernel_available, ByteField, RuntimeGf256, WideGf256,
};
use bytefield_consumer::{
    axpy_field_vec, axpy_region, gemm_region, matrix_to_region, pairwise_region, CoefficientTable,
    ProductTable,
};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{batch, Gf2mField};
use std::hint::black_box;

/// Which of the two routes an arm measures.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    /// The gf2-core entry point as it stands.
    Current,
    /// The byte-oriented table prototype.
    Prototype,
}

fn main() -> ! {
    byte_field_arm_common::run(|request, case| {
        if case.workers != 1 {
            return Err(format!(
                "every measured consumer entry point is single-threaded; the cell declares {} workers",
                case.workers
            ));
        }
        let route = match std::env::var("GF2_BYTEFIELD_ROUTE").as_deref() {
            Ok("current") => Route::Current,
            Ok("prototype") => Route::Prototype,
            Ok(other) => return Err(format!("unknown route {other:?}")),
            Err(_) => return Err("GF2_BYTEFIELD_ROUTE is unset".to_owned()),
        };
        let representation = std::env::var("GF2_BYTEFIELD_REPR")
            .map_err(|_| "GF2_BYTEFIELD_REPR is unset".to_owned())?;
        let bank_count = banks(request.cache_state);
        match representation.as_str() {
            "element" => {
                let setup_ns = probe_ns(|| {
                    black_box(RuntimeGf256::new());
                });
                let field = RuntimeGf256::new();
                check_polynomial(&field, case)?;
                build(field, case, route, bank_count, setup_ns)
            }
            "wide" => {
                check_polynomial(&WideGf256, case)?;
                // A compile-time field has no runtime context to build.
                build(WideGf256, case, route, bank_count, 0)
            }
            "batch" => {
                let setup_ns = probe_ns(|| {
                    black_box(Gf2mField::gf256());
                });
                let field = RuntimeGf256::new();
                check_polynomial(&field, case)?;
                if case.operation != Operation::Pairwise {
                    return Err(format!(
                        "the batch representation measures arbitrary pairwise products; the cell \
                         declares {:?}",
                        case.operation
                    ));
                }
                Ok(build_pairwise(field, case, route, bank_count, setup_ns))
            }
            other => Err(format!("unknown representation {other:?}")),
        }
    })
}

fn check_polynomial<B: ByteField>(field: &B, case: &Case) -> Result<(), String> {
    if field.polynomial() == case.poly {
        Ok(())
    } else {
        Err(format!(
            "{} works modulo 0x{:X}; the cell declares 0x{:X}",
            B::NAME,
            field.polynomial(),
            case.poly
        ))
    }
}

fn build<B: ByteField + 'static>(
    field: B,
    case: &Case,
    route: Route,
    bank_count: usize,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
    match case.operation {
        Operation::Axpy => Ok(build_axpy(field, case, route, bank_count, setup_ns)),
        Operation::Matmul => Ok(build_matmul(field, case, route, setup_ns)),
        Operation::Pairwise => Err(
            "the arbitrary pairwise control measures gf2m::batch::batch_mul, which takes the \
             runtime field; declare the batch representation for it"
                .to_owned(),
        ),
        Operation::Encode => Err(
            "this assessment measures the consumers gf2 exposes (FieldVec::axpy, \
             field::matrix::gemm, gf2m::batch::batch_mul); the generator-encode shape belongs to \
             the comparison survey 6c6b09b1"
                .to_owned(),
        ),
    }
}

fn suffix(metric: Metric) -> &'static str {
    match metric {
        Metric::KernelIsolated => "",
        Metric::WholeConsumer => "/whole-consumer",
    }
}

/// Fixed-coefficient multiply-accumulate, vector-shaped in place on the
/// `FieldVec` the consumer holds (`kernel-isolated`) or region-shaped from
/// and to a byte region (`whole-consumer`).
fn build_axpy<B: ByteField + 'static>(
    field: B,
    case: &Case,
    route: Route,
    bank_count: usize,
    setup_ns: u64,
) -> Workload<'static> {
    let mut rng = SplitMix64::new(case.seed);
    let mut source_bytes: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    let mut target_bytes: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    for _ in 0..bank_count {
        let mut source = vec![0u8; case.bytes];
        let mut target = vec![0u8; case.bytes];
        rng.fill(&mut source);
        rng.fill(&mut target);
        source_bytes.push(source);
        target_bytes.push(target);
    }
    let coefficient_byte = rng.coefficient();
    let coefficient = field.element(coefficient_byte);
    let polynomial = field.polynomial() as u16;
    let mut sources: Vec<FieldVec<B::Elem>> = source_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut targets: Vec<FieldVec<B::Elem>> = target_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut scratch = vec![0u8; case.bytes];
    let mut table = CoefficientTable::new(coefficient_byte, polynomial);
    let table_ns = probe_ns(|| table.refill(coefficient_byte, polynomial));
    let pack_ns = probe_ns(|| {
        workload::pack_into_vec(&field, &source_bytes[0], &mut sources[0]);
        workload::pack_into_vec(&field, &target_bytes[0], &mut targets[0]);
    });
    let unpack_ns = probe_ns(|| workload::unpack_vec::<B>(&targets[0], &mut scratch));
    let conversion = match (route, case.metric) {
        // The current route takes the coefficient as a field element and
        // prepares nothing; the prototype prepares one 256-byte table per
        // call. Neither route converts a representation in a vector cell,
        // and in a region cell the current route converts both operands in
        // and the result out while the prototype works on the region itself.
        (Route::Current, Metric::KernelIsolated) => Conversion {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
        (Route::Current, Metric::WholeConsumer) => Conversion {
            setup_ns,
            pack_ns,
            unpack_ns,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
        (Route::Prototype, _) => Conversion {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: table_ns,
            dispatch_ns: 0,
        },
    };
    let selected_path = match (route, case.metric) {
        (Route::Current, Metric::KernelIsolated) => format!(
            "gf2-core/FieldVec<{}>::axpy/scalar-element-loop/poly=0x{:X}",
            B::NAME,
            case.poly
        ),
        (Route::Current, Metric::WholeConsumer) => format!(
            "gf2-core/FieldVec<{}>::axpy/scalar-element-loop/poly=0x{:X}{}",
            B::NAME,
            case.poly,
            suffix(case.metric)
        ),
        (Route::Prototype, Metric::KernelIsolated) => format!(
            "prototype/coefficient-table-{}/in-place-FieldVec<{}>/poly=0x{:X}",
            table.entries().len(),
            B::NAME,
            case.poly
        ),
        (Route::Prototype, Metric::WholeConsumer) => format!(
            "prototype/coefficient-table-{}/byte-region/poly=0x{:X}{}",
            table.entries().len(),
            case.poly,
            suffix(case.metric)
        ),
    };
    let body: Box<dyn FnMut(usize)> = match (route, case.metric) {
        (Route::Current, Metric::KernelIsolated) => Box::new(move |bank| {
            let index = bank % bank_count;
            workload::axpy(
                &mut targets[index],
                &coefficient,
                black_box(&sources[index]),
            );
            black_box(&targets[index]);
        }),
        (Route::Current, Metric::WholeConsumer) => Box::new(move |bank| {
            let index = bank % bank_count;
            let (source, target) = (&mut sources[index], &mut targets[index]);
            workload::pack_into_vec(&field, black_box(&source_bytes[index]), source);
            workload::pack_into_vec(&field, black_box(&target_bytes[index]), target);
            workload::axpy(target, &coefficient, source);
            workload::unpack_vec::<B>(target, &mut target_bytes[index]);
            black_box(&target_bytes[index]);
        }),
        (Route::Prototype, Metric::KernelIsolated) => Box::new(move |bank| {
            let index = bank % bank_count;
            table.refill(coefficient_byte, polynomial);
            axpy_field_vec(
                &field,
                &mut targets[index],
                &table,
                black_box(&sources[index]),
            );
            black_box(&targets[index]);
        }),
        (Route::Prototype, Metric::WholeConsumer) => Box::new(move |bank| {
            let index = bank % bank_count;
            table.refill(coefficient_byte, polynomial);
            axpy_region(
                &mut target_bytes[index],
                &table,
                black_box(&source_bytes[index]),
            );
            black_box(&target_bytes[index]);
        }),
    };
    Workload {
        selected_path,
        conversion,
        body,
    }
}

/// Dense square product, on the representation each route holds
/// (`kernel-isolated`) or from and to the `FieldMatrix` a library consumer
/// holds (`whole-consumer`).
fn build_matmul<B: ByteField + 'static>(
    field: B,
    case: &Case,
    route: Route,
    setup_ns: u64,
) -> Workload<'static> {
    let n = case.n;
    let mut rng = SplitMix64::new(case.seed);
    let mut left_bytes = vec![0u8; n * n];
    let mut right_bytes = vec![0u8; n * n];
    rng.fill(&mut left_bytes);
    rng.fill(&mut right_bytes);
    let mut left = workload::pack_matrix(&field, &left_bytes, n, n);
    let mut right = workload::pack_matrix(&field, &right_bytes, n, n);
    let mut product = workload::matmul(&left, &right);
    let mut out_bytes = vec![0u8; n * n];
    let polynomial = field.polynomial() as u16;
    let table_ns = probe_ns(|| {
        black_box(ProductTable::new(polynomial));
    });
    let table = ProductTable::new(polynomial);
    let to_region_ns = probe_ns(|| {
        matrix_to_region::<B>(&left, &mut left_bytes);
        matrix_to_region::<B>(&right, &mut right_bytes);
    });
    let from_region_ns = probe_ns(|| workload::pack_into_matrix(&field, &out_bytes, &mut product));
    let pack_ns = probe_ns(|| {
        workload::pack_into_matrix(&field, &left_bytes, &mut left);
        workload::pack_into_matrix(&field, &right_bytes, &mut right);
    });
    let unpack_ns = probe_ns(|| workload::unpack_matrix::<B>(&product, &mut out_bytes));
    let conversion = match (route, case.metric) {
        // The current route is already at the FieldMatrix boundary, so the
        // whole-consumer twin adds nothing to it; the prototype converts
        // both operands out of the representation and the product back in.
        // The full table depends on the field alone and is prepared once, so
        // it is a setup cost rather than a per-call one.
        (Route::Current, _) => Conversion {
            setup_ns,
            pack_ns,
            unpack_ns,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
        (Route::Prototype, Metric::KernelIsolated) => Conversion {
            setup_ns: setup_ns.saturating_add(table_ns),
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
        (Route::Prototype, Metric::WholeConsumer) => Conversion {
            setup_ns: setup_ns.saturating_add(table_ns),
            pack_ns: to_region_ns,
            unpack_ns: from_region_ns,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
    };
    let selected_path = match route {
        Route::Current => format!(
            "gf2-core/field::matrix::gemm/FieldMatrix<{}>/{}/square-{n}/poly=0x{:X}{}",
            B::NAME,
            B::gemm_route(),
            case.poly,
            suffix(case.metric)
        ),
        Route::Prototype => format!(
            "prototype/product-table-{}/byte-matrix/square-{n}/poly=0x{:X}{}",
            table.bytes(),
            case.poly,
            suffix(case.metric)
        ),
    };
    let body: Box<dyn FnMut(usize)> = match (route, case.metric) {
        (Route::Current, _) => Box::new(move |_| {
            let product = workload::matmul(black_box(&left), black_box(&right));
            black_box(&product);
        }),
        (Route::Prototype, Metric::KernelIsolated) => Box::new(move |_| {
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
        (Route::Prototype, Metric::WholeConsumer) => Box::new(move |_| {
            matrix_to_region::<B>(black_box(&left), &mut left_bytes);
            matrix_to_region::<B>(black_box(&right), &mut right_bytes);
            gemm_region(&left_bytes, &right_bytes, &mut out_bytes, n, n, n, &table);
            workload::pack_into_matrix(&field, &out_bytes, &mut product);
            black_box(&product);
        }),
    };
    Workload {
        selected_path,
        conversion,
        body,
    }
}

/// Arbitrary pairwise product, the control: no coefficient repeats, so the
/// byte-oriented route can amortise no coefficient table and reaches the
/// full 64 KiB table instead.
fn build_pairwise(
    field: RuntimeGf256,
    case: &Case,
    route: Route,
    bank_count: usize,
    setup_ns: u64,
) -> Workload<'static> {
    let mut rng = SplitMix64::new(case.seed);
    let mut left_bytes: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    let mut right_bytes: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    for _ in 0..bank_count {
        let mut left = vec![0u8; case.bytes];
        let mut right = vec![0u8; case.bytes];
        rng.fill(&mut left);
        rng.fill(&mut right);
        left_bytes.push(left);
        right_bytes.push(right);
    }
    let polynomial = field.polynomial() as u16;
    let mut lefts: Vec<Vec<u64>> = left_bytes.iter().map(|b| workload::widen(b)).collect();
    let mut rights: Vec<Vec<u64>> = right_bytes.iter().map(|b| workload::widen(b)).collect();
    let mut lanes = vec![0u64; case.bytes];
    let mut out_bytes = vec![0u8; case.bytes];
    let table_ns = probe_ns(|| {
        black_box(ProductTable::new(polynomial));
    });
    let table = ProductTable::new(polynomial);
    workload::pairwise(&field.field, &lefts[0], &rights[0], &mut lanes);
    let widen_ns = probe_ns(|| {
        workload::widen_into(&left_bytes[0], &mut lefts[0]);
        workload::widen_into(&right_bytes[0], &mut rights[0]);
    });
    let narrow_ns = probe_ns(|| workload::narrow(&lanes, &mut out_bytes));
    let conversion = match route {
        Route::Current => Conversion {
            setup_ns,
            pack_ns: widen_ns,
            unpack_ns: narrow_ns,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
        Route::Prototype => Conversion {
            setup_ns: setup_ns.saturating_add(table_ns),
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
    };
    let kernel = if batch_kernel_available() {
        "vpclmulqdq-batch"
    } else {
        "scalar-batch"
    };
    let selected_path = match route {
        Route::Current => format!(
            "gf2-core/gf2m::batch::batch_mul/u64-lanes/{kernel}/poly=0x{:X}{}",
            case.poly,
            suffix(case.metric)
        ),
        Route::Prototype => format!(
            "prototype/product-table-{}/byte-region/poly=0x{:X}{}",
            table.bytes(),
            case.poly,
            suffix(case.metric)
        ),
    };
    let gf = field.field;
    let body: Box<dyn FnMut(usize)> = match (route, case.metric) {
        (Route::Current, Metric::KernelIsolated) => Box::new(move |bank| {
            let index = bank % bank_count;
            batch::batch_mul(
                &gf,
                black_box(&lefts[index]),
                black_box(&rights[index]),
                &mut lanes,
            );
            black_box(&lanes);
        }),
        (Route::Current, Metric::WholeConsumer) => Box::new(move |bank| {
            let index = bank % bank_count;
            let (left, right) = (&mut lefts[index], &mut rights[index]);
            workload::widen_into(black_box(&left_bytes[index]), left);
            workload::widen_into(black_box(&right_bytes[index]), right);
            batch::batch_mul(&gf, left, right, &mut lanes);
            workload::narrow(&lanes, &mut out_bytes);
            black_box(&out_bytes);
        }),
        (Route::Prototype, _) => Box::new(move |bank| {
            let index = bank % bank_count;
            pairwise_region(
                black_box(&left_bytes[index]),
                black_box(&right_bytes[index]),
                &mut out_bytes,
                &table,
            );
            black_box(&out_bytes);
        }),
    };
    Workload {
        selected_path,
        conversion,
        body,
    }
}
