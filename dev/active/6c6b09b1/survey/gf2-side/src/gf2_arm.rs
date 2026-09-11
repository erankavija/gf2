//! gf2 baseline arm for the byte-field survey (jit:6c6b09b1).
//!
//! Measures the gf2 consumer API named by the cell's operation. It links no
//! external library, so its executable identity covers the gf2 side only.
//! `GF2_SURVEY_GF2_REPR` selects the element representation: `element` for
//! `Gf2mElement` over `Gf2mField::gf256()`, `wide` for
//! `Gf2mWide<1, Gf256x11d>`.
//!
//! A kernel-isolated cell times the gf2 call on operands already in gf2's
//! representation. A whole-consumer cell starts and ends at the shared byte
//! region, so the window also converts the operands into the `FieldVec` or
//! `FieldMatrix` the consumer keeps and converts the result back.

use byte_field_arm_common::{
    banks, probe_ns, Case, Conversion, Metric, Operation, SplitMix64, Workload,
};
use byte_field_gf2_side::workload::{
    self, batch_kernel_available, ByteField, RuntimeGf256, WideGf256,
};
use gf2_core::field::FieldVec;
use std::hint::black_box;

fn main() -> ! {
    byte_field_arm_common::run(|request, case| {
        if case.workers != 1 {
            return Err(format!(
                "the gf2 region, pairwise and matrix APIs are single-threaded; cell declares {} workers",
                case.workers
            ));
        }
        let representation = std::env::var("GF2_SURVEY_GF2_REPR")
            .map_err(|_| "GF2_SURVEY_GF2_REPR is unset".to_owned())?;
        let bank_count = banks(&request.cache_state);
        match representation.as_str() {
            "element" => {
                let setup_ns = probe_ns(|| {
                    black_box(RuntimeGf256::new());
                });
                let field = RuntimeGf256::new();
                check_polynomial(&field, case)?;
                match case.operation {
                    Operation::Pairwise => build_pairwise(field, case, bank_count, setup_ns),
                    _ => build(field, case, bank_count, setup_ns),
                }
            }
            "wide" => {
                check_polynomial(&WideGf256, case)?;
                match case.operation {
                    Operation::Pairwise => Err(
                        "the pairwise control measures gf2m::batch::batch_mul, which takes the \
                         runtime field; the wide representation has no batch entry point"
                            .to_owned(),
                    ),
                    // A compile-time field has no runtime context to build.
                    _ => build(WideGf256, case, bank_count, 0),
                }
            }
            other => Err(format!("unknown gf2 representation {other:?}")),
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
    bank_count: usize,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
    match case.operation {
        Operation::Axpy => Ok(build_axpy(field, case, bank_count, setup_ns)),
        Operation::Matmul => Ok(build_product(field, case, case.n, case.n, case.n, setup_ns)),
        Operation::Encode => Ok(build_product(
            field, case, case.rows, case.k, case.bytes, setup_ns,
        )),
        Operation::Pairwise => Err("pairwise is built by build_pairwise".to_owned()),
    }
}

fn suffix(metric: Metric) -> &'static str {
    match metric {
        Metric::KernelIsolated => "",
        Metric::WholeConsumer => "/whole-consumer",
    }
}

/// Region multiply-accumulate `y[i] += a * x[i]` through `FieldVec::axpy`.
fn build_axpy<B: ByteField + 'static>(
    field: B,
    case: &Case,
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
    let coefficient = field.element(rng.coefficient());
    let mut sources: Vec<FieldVec<B::Elem>> = source_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut targets: Vec<FieldVec<B::Elem>> = target_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut scratch = vec![0u8; case.bytes];
    let conversion = Conversion {
        setup_ns,
        // Both operands enter the representation; the result leaves it.
        pack_ns: probe_ns(|| {
            workload::pack_into_vec(&field, &source_bytes[0], &mut sources[0]);
            workload::pack_into_vec(&field, &target_bytes[0], &mut targets[0]);
        }),
        unpack_ns: probe_ns(|| workload::unpack_vec::<B>(&targets[0], &mut scratch)),
        // `FieldVec::axpy` takes the scalar directly; there is no coefficient
        // table to prepare and no selection step outside the call.
        batch_fill_ns: 0,
        dispatch_ns: 0,
    };
    let selected_path = format!(
        "gf2-core/FieldVec<{}>::axpy/poly=0x{:X}{}",
        B::NAME,
        case.poly,
        suffix(case.metric)
    );
    let body: Box<dyn FnMut(usize)> = match case.metric {
        Metric::KernelIsolated => Box::new(move |bank| {
            let index = bank % bank_count;
            workload::axpy(
                &mut targets[index],
                &coefficient,
                black_box(&sources[index]),
            );
            black_box(&targets[index]);
        }),
        Metric::WholeConsumer => Box::new(move |bank| {
            let index = bank % bank_count;
            let (source, target) = (&mut sources[index], &mut targets[index]);
            workload::pack_into_vec(&field, black_box(&source_bytes[index]), source);
            workload::pack_into_vec(&field, black_box(&target_bytes[index]), target);
            workload::axpy(target, &coefficient, source);
            workload::unpack_vec::<B>(target, &mut target_bytes[index]);
            black_box(&target_bytes[index]);
        }),
    };
    Workload {
        selected_path,
        conversion,
        body,
    }
}

/// Arbitrary pairwise product through `gf2m::batch::batch_mul`.
///
/// The kernel reads `u64` lanes, so a byte-region consumer pays a widening
/// and a narrowing pass around it.
fn build_pairwise(
    field: RuntimeGf256,
    case: &Case,
    bank_count: usize,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
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
    let mut lefts: Vec<Vec<u64>> = left_bytes.iter().map(|b| workload::widen(b)).collect();
    let mut rights: Vec<Vec<u64>> = right_bytes.iter().map(|b| workload::widen(b)).collect();
    let mut lanes = vec![0u64; case.bytes];
    let mut out = vec![0u8; case.bytes];
    workload::pairwise(&field.field, &lefts[0], &rights[0], &mut lanes);
    let conversion = Conversion {
        setup_ns,
        pack_ns: probe_ns(|| {
            workload::widen_into(&left_bytes[0], &mut lefts[0]);
            workload::widen_into(&right_bytes[0], &mut rights[0]);
        }),
        unpack_ns: probe_ns(|| workload::narrow(&lanes, &mut out)),
        batch_fill_ns: 0,
        dispatch_ns: 0,
    };
    let kernel = if batch_kernel_available() {
        "vpclmulqdq-batch"
    } else {
        "scalar-batch"
    };
    let selected_path = format!(
        "gf2-core/gf2m::batch::batch_mul/u64-lanes/{kernel}/poly=0x{:X}{}",
        case.poly,
        suffix(case.metric)
    );
    let gf = field.field;
    let body: Box<dyn FnMut(usize)> = match case.metric {
        Metric::KernelIsolated => Box::new(move |bank| {
            let index = bank % bank_count;
            workload::pairwise(
                &gf,
                black_box(&lefts[index]),
                black_box(&rights[index]),
                &mut lanes,
            );
            black_box(&lanes);
        }),
        Metric::WholeConsumer => Box::new(move |bank| {
            let index = bank % bank_count;
            let (left, right) = (&mut lefts[index], &mut rights[index]);
            workload::widen_into(black_box(&left_bytes[index]), left);
            workload::widen_into(black_box(&right_bytes[index]), right);
            workload::pairwise(&gf, left, right, &mut lanes);
            workload::narrow(&lanes, &mut out);
            black_box(&out);
        }),
    };
    Ok(Workload {
        selected_path,
        conversion,
        body,
    })
}

/// Dense product `C = A * B` through `field::matrix::gemm`, with `A` of
/// shape `rows x inner` and `B` of shape `inner x cols`.
///
/// A square cell sets all three to `n`. A generator-encode cell sets `A` to
/// the `rows x k` generator and `B` to the `k x bytes` data regions, the
/// shape ISA-L's `ec_encode_data` has; on the gf2 side this is the same
/// `gemm` entry point, not a separate kernel. For an encode cell the
/// generator conversion is the gf2 analogue of a table preparation and is
/// reported as `batch_fill_ns`; for a square cell both operands are data.
fn build_product<B: ByteField + 'static>(
    field: B,
    case: &Case,
    rows: usize,
    inner: usize,
    cols: usize,
    setup_ns: u64,
) -> Workload<'static> {
    let encode = case.operation == Operation::Encode;
    let mut rng = SplitMix64::new(case.seed);
    let mut left_bytes = vec![0u8; rows * inner];
    let mut right_bytes = vec![0u8; inner * cols];
    rng.fill(&mut left_bytes);
    rng.fill(&mut right_bytes);
    let mut left = workload::pack_matrix(&field, &left_bytes, rows, inner);
    let mut right = workload::pack_matrix(&field, &right_bytes, inner, cols);
    let product = workload::matmul(&left, &right);
    let mut out = vec![0u8; rows * cols];
    let left_pack_ns = probe_ns(|| workload::pack_into_matrix(&field, &left_bytes, &mut left));
    let right_pack_ns = probe_ns(|| workload::pack_into_matrix(&field, &right_bytes, &mut right));
    let unpack_ns = probe_ns(|| workload::unpack_matrix::<B>(&product, &mut out));
    drop(product);
    let conversion = if encode {
        Conversion {
            setup_ns,
            pack_ns: right_pack_ns,
            unpack_ns,
            batch_fill_ns: left_pack_ns,
            dispatch_ns: 0,
        }
    } else {
        Conversion {
            setup_ns,
            pack_ns: left_pack_ns.saturating_add(right_pack_ns),
            unpack_ns,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }
    };
    let shape = if encode {
        format!("generator-{rows}x{inner}-by-{inner}x{cols}")
    } else {
        format!("square-{rows}")
    };
    let selected_path = format!(
        "gf2-core/field::matrix::gemm/FieldMatrix<{}>/{}/{shape}/poly=0x{:X}{}",
        B::NAME,
        B::gemm_route(),
        case.poly,
        suffix(case.metric)
    );
    // A product has one working set; the streaming banks would multiply the
    // resident footprint without changing the operation, so product cells
    // declare `warm` and the bank index is unused.
    let body: Box<dyn FnMut(usize)> = match case.metric {
        Metric::KernelIsolated => Box::new(move |_| {
            let product = workload::matmul(black_box(&left), black_box(&right));
            black_box(&product);
        }),
        Metric::WholeConsumer => Box::new(move |_| {
            workload::pack_into_matrix(&field, black_box(&left_bytes), &mut left);
            workload::pack_into_matrix(&field, black_box(&right_bytes), &mut right);
            let product = workload::matmul(&left, &right);
            workload::unpack_matrix::<B>(&product, &mut out);
            black_box(&out);
        }),
    };
    Workload {
        selected_path,
        conversion,
        body,
    }
}
