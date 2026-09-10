//! gf2 baseline arm for the byte-field survey (jit:6c6b09b1).
//!
//! Measures the gf2 consumer API named by the cell's operation. It links no
//! external library, so its executable identity covers the gf2 side only.
//!
//! A kernel-isolated cell times the gf2 call on operands already in gf2's
//! representation. A whole-consumer cell starts and ends at the shared byte
//! region, so the window includes converting into `FieldVec` or
//! `FieldMatrix` and converting the result back. That difference is the
//! whole point of the survey: the gf2 types this survey measures store one
//! reference-counted field element per coefficient, not one byte.

mod workload;

use byte_field_arm_common::{banks, timed, Case, Metric, Operation, SplitMix64, Workload};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::Gf2mElement;
use std::hint::black_box;

fn main() -> ! {
    byte_field_arm_common::run(|request, case| {
        if case.workers != 1 {
            return Err(format!(
                "the gf2 region and matrix APIs are single-threaded; cell declares {} workers",
                case.workers
            ));
        }
        let bank_count = banks(&request.cache_state);
        match case.operation {
            Operation::Axpy => build_axpy(case, bank_count),
            Operation::Pairwise => build_pairwise(case, bank_count),
            Operation::Matmul => build_matmul(case, bank_count),
            Operation::Encode => build_encode(case, bank_count),
        }
    })
}

/// Region multiply-accumulate `y[i] += a * x[i]` through `FieldVec::axpy`.
fn build_axpy(case: &Case, bank_count: usize) -> Result<Workload<'static>, String> {
    let (field, mut conversion) = workload::setup(case.poly);
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
    let coefficient = field.element(u64::from((rng.next_u64() | 1) as u8));

    // Charge the byte-to-FieldVec conversion once so the whole-consumer
    // window's composition is visible even for the isolated cell.
    let (sources, pack_ns) = timed(|| {
        source_bytes
            .iter()
            .map(|bytes| workload::pack_vec(&field, bytes))
            .collect::<Vec<_>>()
    });
    let targets: Vec<FieldVec<Gf2mElement>> = target_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut scratch = vec![0u8; case.bytes];
    let (_, unpack_ns) = timed(|| workload::unpack_vec(&targets[0], &mut scratch));
    conversion.pack_ns = pack_ns / bank_count.max(1) as u64;
    conversion.unpack_ns = unpack_ns;
    // `FieldVec::axpy` takes the scalar directly; there is no coefficient
    // table to prepare and no per-call dispatch decision outside the call.
    conversion.batch_fill_ns = 0;
    conversion.dispatch_ns = 0;

    let selected_path = format!(
        "gf2-core/FieldVec::axpy/Gf2mElement/poly=0x{:X}",
        case.poly
    );
    match case.metric {
        Metric::KernelIsolated => {
            let mut targets = targets;
            Ok(Workload {
                selected_path,
                conversion,
                body: Box::new(move |bank| {
                    let index = bank % bank_count;
                    let (source, target) = (&sources[index], &mut targets[index]);
                    workload::axpy(target, &coefficient, black_box(source));
                    black_box(target);
                }),
            })
        }
        Metric::WholeConsumer => {
            drop(sources);
            drop(targets);
            let mut out = vec![0u8; case.bytes];
            Ok(Workload {
                selected_path: format!("{selected_path}/whole-consumer"),
                conversion,
                body: Box::new(move |bank| {
                    let index = bank % bank_count;
                    let source = workload::pack_vec(&field, black_box(&source_bytes[index]));
                    let mut target = workload::pack_vec(&field, black_box(&target_bytes[index]));
                    workload::axpy(&mut target, &coefficient, &source);
                    workload::unpack_vec(&target, &mut out);
                    black_box(&out);
                }),
            })
        }
    }
}

/// Arbitrary pairwise product through `gf2m::batch::batch_mul`.
///
/// The kernel reads `u64` lanes, so a byte-region consumer pays a widening
/// and a narrowing pass around it.
fn build_pairwise(case: &Case, bank_count: usize) -> Result<Workload<'static>, String> {
    let (field, mut conversion) = workload::setup(case.poly);
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
    let (lefts, pack_ns) = timed(|| {
        left_bytes
            .iter()
            .map(|bytes| workload::widen(bytes))
            .collect::<Vec<_>>()
    });
    let rights: Vec<Vec<u64>> = right_bytes.iter().map(|bytes| workload::widen(bytes)).collect();
    let mut lanes = vec![0u64; case.bytes];
    let mut out = vec![0u8; case.bytes];
    let (_, unpack_ns) = timed(|| workload::narrow(&lanes, &mut out));
    conversion.pack_ns = pack_ns / bank_count.max(1) as u64;
    conversion.unpack_ns = unpack_ns;
    conversion.batch_fill_ns = 0;
    conversion.dispatch_ns = 0;

    let selected_path = format!("gf2-core/gf2m::batch::batch_mul/u64-lanes/poly=0x{:X}", case.poly);
    match case.metric {
        Metric::KernelIsolated => Ok(Workload {
            selected_path,
            conversion,
            body: Box::new(move |bank| {
                let index = bank % bank_count;
                workload::pairwise(
                    &field,
                    black_box(&lefts[index]),
                    black_box(&rights[index]),
                    &mut lanes,
                );
                black_box(&lanes);
            }),
        }),
        Metric::WholeConsumer => {
            drop(lefts);
            drop(rights);
            Ok(Workload {
                selected_path: format!("{selected_path}/whole-consumer"),
                conversion,
                body: Box::new(move |bank| {
                    let index = bank % bank_count;
                    let left = workload::widen(black_box(&left_bytes[index]));
                    let right = workload::widen(black_box(&right_bytes[index]));
                    workload::pairwise(&field, &left, &right, &mut lanes);
                    workload::narrow(&lanes, &mut out);
                    black_box(&out);
                }),
            })
        }
    }
}

/// Dense square product through `field::matrix::gemm`.
fn build_matmul(case: &Case, bank_count: usize) -> Result<Workload<'static>, String> {
    let (field, mut conversion) = workload::setup(case.poly);
    let n = case.n;
    let mut rng = SplitMix64::new(case.seed);
    let mut left_bytes = vec![0u8; n * n];
    let mut right_bytes = vec![0u8; n * n];
    rng.fill(&mut left_bytes);
    rng.fill(&mut right_bytes);
    let (left, pack_ns) = timed(|| workload::pack_matrix(&field, &left_bytes, n, n));
    let right = workload::pack_matrix(&field, &right_bytes, n, n);
    let product = workload::matmul(&left, &right);
    let mut out = vec![0u8; n * n];
    let (_, unpack_ns) = timed(|| workload::unpack_matrix(&product, &mut out));
    conversion.pack_ns = pack_ns;
    conversion.unpack_ns = unpack_ns;
    conversion.batch_fill_ns = 0;
    conversion.dispatch_ns = 0;
    drop(product);

    let selected_path = format!("gf2-core/field::matrix::gemm/Gf2mElement/poly=0x{:X}", case.poly);
    // A square GEMM has one working set; the streaming banks would multiply
    // the resident footprint without changing the operation, so matmul cells
    // declare `warm` and this arm ignores the bank index.
    let _ = bank_count;
    match case.metric {
        Metric::KernelIsolated => Ok(Workload {
            selected_path,
            conversion,
            body: Box::new(move |_| {
                let product = workload::matmul(black_box(&left), black_box(&right));
                black_box(&product);
            }),
        }),
        Metric::WholeConsumer => {
            drop(left);
            drop(right);
            Ok(Workload {
                selected_path: format!("{selected_path}/whole-consumer"),
                conversion,
                body: Box::new(move |_| {
                    let left = workload::pack_matrix(&field, black_box(&left_bytes), n, n);
                    let right = workload::pack_matrix(&field, black_box(&right_bytes), n, n);
                    let product = workload::matmul(&left, &right);
                    workload::unpack_matrix(&product, &mut out);
                    black_box(&out);
                }),
            })
        }
    }
}

/// Generator-matrix region encode `C = G * D` with `G` of shape
/// `rows x k` and `D` of shape `k x bytes`, the shape ISA-L's
/// `ec_encode_data` has. On the gf2 side this is the same `gemm` entry
/// point at a rectangular shape, not a separate kernel.
fn build_encode(case: &Case, bank_count: usize) -> Result<Workload<'static>, String> {
    let (field, mut conversion) = workload::setup(case.poly);
    let (k, rows, len) = (case.k, case.rows, case.bytes);
    let mut rng = SplitMix64::new(case.seed);
    let mut generator_bytes = vec![0u8; rows * k];
    let mut data_bytes = vec![0u8; k * len];
    rng.fill(&mut generator_bytes);
    rng.fill(&mut data_bytes);
    let (generator, generator_pack_ns) =
        timed(|| workload::pack_matrix(&field, &generator_bytes, rows, k));
    let (data, data_pack_ns) = timed(|| workload::pack_matrix(&field, &data_bytes, k, len));
    let coding = workload::matmul(&generator, &data);
    let mut out = vec![0u8; rows * len];
    let (_, unpack_ns) = timed(|| workload::unpack_matrix(&coding, &mut out));
    conversion.pack_ns = data_pack_ns;
    conversion.unpack_ns = unpack_ns;
    // The generator conversion is the gf2 analogue of a table preparation:
    // it happens once per coefficient set, not once per data region.
    conversion.batch_fill_ns = generator_pack_ns;
    conversion.dispatch_ns = 0;
    drop(coding);
    let _ = bank_count;

    let selected_path = format!(
        "gf2-core/field::matrix::gemm/generator-{rows}x{k}/poly=0x{:X}",
        case.poly
    );
    match case.metric {
        Metric::KernelIsolated => Ok(Workload {
            selected_path,
            conversion,
            body: Box::new(move |_| {
                let coding = workload::matmul(black_box(&generator), black_box(&data));
                black_box(&coding);
            }),
        }),
        Metric::WholeConsumer => {
            drop(generator);
            drop(data);
            Ok(Workload {
                selected_path: format!("{selected_path}/whole-consumer"),
                conversion,
                body: Box::new(move |_| {
                    let generator =
                        workload::pack_matrix(&field, black_box(&generator_bytes), rows, k);
                    let data = workload::pack_matrix(&field, black_box(&data_bytes), k, len);
                    let coding = workload::matmul(&generator, &data);
                    workload::unpack_matrix(&coding, &mut out);
                    black_box(&out);
                }),
            })
        }
    }
}
