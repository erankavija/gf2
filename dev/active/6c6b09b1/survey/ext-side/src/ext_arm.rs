//! External candidate arm for the byte-field survey (jit:6c6b09b1).
//!
//! Measures one pinned external library performing the operation the cell
//! names, on the same byte region the gf2 arm starts from. The backend and
//! its arithmetic variant come from the environment, so the plan's arm
//! descriptor records exactly which library and configuration produced each
//! cell:
//!
//! * `GF2_SURVEY_BACKEND` — `isa-l`, `gf-complete` or `m4rie`.
//! * `GF2_SURVEY_VARIANT` — the library's arithmetic backend, `default`
//!   unless the library offers a choice.
//!
//! An operation a backend does not provide exits with status 2 and the
//! library's own reason. No cell substitutes a neighbouring operation.

mod shim;

use byte_field_arm_common::{banks, timed, Case, Metric, Operation, SplitMix64, Workload};
use shim::{Context, Matrix};
use std::hint::black_box;

fn main() -> ! {
    // `--validate` checks the Rust side of every adapter against the shim's
    // independent scalar reference. The C conformance binary covers the same
    // ground for the libraries themselves; this covers the FFI wrapper and
    // the pack and unpack paths the arms use, before any timing happens.
    if std::env::args().nth(1).as_deref() == Some("--validate") {
        std::process::exit(validate());
    }
    byte_field_arm_common::run(|request, case| {
        if case.workers != 1 {
            return Err(format!(
                "every external region and matrix API measured here is single-threaded; \
                 cell declares {} workers",
                case.workers
            ));
        }
        let backend_name = std::env::var("GF2_SURVEY_BACKEND")
            .map_err(|_| "GF2_SURVEY_BACKEND is unset".to_owned())?;
        let variant =
            std::env::var("GF2_SURVEY_VARIANT").unwrap_or_else(|_| "default".to_owned());
        let backend = match backend_name.as_str() {
            "isa-l" => shim::BACKEND_ISAL,
            "gf-complete" => shim::BACKEND_GFCOMPLETE,
            "m4rie" => shim::BACKEND_M4RIE,
            other => return Err(format!("unknown backend {other:?}")),
        };
        let bank_count = banks(&request.cache_state);
        let (context, setup_ns) = timed(|| Context::open(backend, case.poly, &variant));
        let context = context?;
        match case.operation {
            Operation::Axpy => build_axpy(case, bank_count, context, setup_ns),
            Operation::Pairwise => build_pairwise(case, bank_count, context, setup_ns),
            Operation::Matmul => build_matmul(case, context, setup_ns),
            Operation::Encode => build_encode(case, context, setup_ns),
        }
    })
}

/// Region multiply-accumulate `dest[i] ^= a * src[i]`.
///
/// ISA-L and GF-Complete expose this as a free-standing region call, so
/// their byte region needs no conversion at all. M4RIE has no free-standing
/// region API; its equivalent is a matrix row, so it pays a pack and an
/// unpack that the other two do not.
fn build_axpy(
    case: &Case,
    bank_count: usize,
    mut context: Context,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
    let mut rng = SplitMix64::new(case.seed);
    let mut sources: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    let mut targets: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    for _ in 0..bank_count {
        let mut source = vec![0u8; case.bytes];
        let mut target = vec![0u8; case.bytes];
        rng.fill(&mut source);
        rng.fill(&mut target);
        sources.push(source);
        targets.push(target);
    }
    let coefficient = (rng.next_u64() | 1) as u8;
    let (prepared, batch_fill_ns) = timed(|| context.prepare(coefficient));
    prepared?;

    let path = format!(
        "{}/{}/poly=0x{:X}",
        context.name(),
        context.version(),
        case.poly
    );

    // M4RIE reaches the operation only through its matrix type, so it takes
    // the row-addressed path and reports the conversion the other two skip.
    // Probe the free-standing region API on scratch buffers so a backend
    // that lacks one is routed to its row form before any timing happens.
    let probe_source = vec![0u8; case.bytes.max(64)];
    let mut probe_target = vec![0u8; case.bytes.max(64)];
    if context.axpy(&probe_source, &mut probe_target).is_err() {
        let mut source_rows: Vec<Matrix> = Vec::with_capacity(bank_count);
        let mut target_rows: Vec<Matrix> = Vec::with_capacity(bank_count);
        let mut pack_ns = 0;
        for bank in 0..bank_count {
            let mut source = context.matrix(1, case.bytes)?;
            let mut target = context.matrix(1, case.bytes)?;
            let (packed, ns) = timed(|| source.pack(&sources[bank]));
            packed?;
            target.pack(&targets[bank])?;
            pack_ns += ns;
            source_rows.push(source);
            target_rows.push(target);
        }
        let mut out = vec![0u8; case.bytes];
        let (unpacked, unpack_ns) = timed(|| target_rows[0].unpack(&mut out));
        unpacked?;
        let mut conversion = byte_field_arm_common::Conversion {
            setup_ns,
            pack_ns: pack_ns / bank_count.max(1) as u64,
            unpack_ns,
            batch_fill_ns,
            dispatch_ns: 0,
        };
        let mut state = RowState {
            sources: source_rows,
            targets: target_rows,
            context,
        };
        return match case.metric {
            Metric::KernelIsolated => {
                conversion.pack_ns = 0;
                conversion.unpack_ns = 0;
                Ok(Workload {
                    selected_path: format!("{path}/mzed_add_multiple_of_row"),
                    conversion,
                    body: Box::new(move |bank| {
                        let index = bank % bank_count;
                        let (source, target) =
                            split_rows(&mut state.sources, &mut state.targets, index);
                        target
                            .row_axpy(0, &*source, 0, coefficient)
                            .expect("row axpy on a validated adapter");
                    }),
                })
            }
            Metric::WholeConsumer => Ok(Workload {
                selected_path: format!("{path}/mzed_add_multiple_of_row/whole-consumer"),
                conversion,
                body: Box::new(move |bank| {
                    let index = bank % bank_count;
                    state.context.prepare(coefficient).expect("prepare");
                    {
                        let (source, target) =
                            split_rows(&mut state.sources, &mut state.targets, index);
                        source.pack(black_box(&sources[index])).expect("pack source");
                        target.pack(black_box(&targets[index])).expect("pack target");
                        target
                            .row_axpy(0, &*source, 0, coefficient)
                            .expect("row axpy on a validated adapter");
                        target.unpack(&mut out).expect("unpack");
                    }
                    black_box(&out);
                }),
            }),
        };
    }

    // ISA-L and GF-Complete operate on the byte region directly, so packing
    // and unpacking are genuinely absent rather than merely cheap.
    let conversion = byte_field_arm_common::Conversion {
        setup_ns,
        pack_ns: 0,
        unpack_ns: 0,
        batch_fill_ns,
        dispatch_ns: 0,
    };
    match case.metric {
        Metric::KernelIsolated => Ok(Workload {
            selected_path: path,
            conversion,
            body: Box::new(move |bank| {
                let index = bank % bank_count;
                context
                    .axpy(black_box(&sources[index]), &mut targets[index])
                    .expect("region multiply-XOR on a validated adapter");
                black_box(&targets[index]);
            }),
        }),
        Metric::WholeConsumer => Ok(Workload {
            selected_path: format!("{path}/whole-consumer"),
            conversion,
            body: Box::new(move |bank| {
                let index = bank % bank_count;
                // A whole consumer applying a fresh coefficient pays the
                // table preparation on every call; the byte region needs no
                // conversion, which is the asymmetry the cell measures.
                context.prepare(coefficient).expect("prepare");
                context
                    .axpy(black_box(&sources[index]), &mut targets[index])
                    .expect("region multiply-XOR on a validated adapter");
                black_box(&targets[index]);
            }),
        }),
    }
}

/// Arbitrary pairwise product `dest[i] = x[i] * y[i]`.
fn build_pairwise(
    case: &Case,
    bank_count: usize,
    mut context: Context,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
    let mut rng = SplitMix64::new(case.seed);
    let mut lefts: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    let mut rights: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    for _ in 0..bank_count {
        let mut left = vec![0u8; case.bytes];
        let mut right = vec![0u8; case.bytes];
        rng.fill(&mut left);
        rng.fill(&mut right);
        lefts.push(left);
        rights.push(right);
    }
    let mut out = vec![0u8; case.bytes];
    // Surface an unsupported operation before any timing happens.
    context.pairwise(&lefts[0], &rights[0], &mut out)?;
    let path = format!(
        "{}/{}/poly=0x{:X}/pairwise",
        context.name(),
        context.version(),
        case.poly
    );
    let conversion = byte_field_arm_common::Conversion {
        setup_ns,
        pack_ns: 0,
        unpack_ns: 0,
        batch_fill_ns: 0,
        dispatch_ns: 0,
    };
    Ok(Workload {
        selected_path: match case.metric {
            Metric::KernelIsolated => path,
            Metric::WholeConsumer => format!("{path}/whole-consumer"),
        },
        conversion,
        body: Box::new(move |bank| {
            let index = bank % bank_count;
            context
                .pairwise(black_box(&lefts[index]), black_box(&rights[index]), &mut out)
                .expect("pairwise multiply on a validated adapter");
            black_box(&out);
        }),
    })
}

/// Dense square product `C = A * B`.
fn build_matmul(
    case: &Case,
    mut context: Context,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
    let n = case.n;
    let mut rng = SplitMix64::new(case.seed);
    let mut left_bytes = vec![0u8; n * n];
    let mut right_bytes = vec![0u8; n * n];
    rng.fill(&mut left_bytes);
    rng.fill(&mut right_bytes);
    let mut left = context.matrix(n, n)?;
    let mut right = context.matrix(n, n)?;
    let mut product = context.matrix(n, n)?;
    let (packed, pack_ns) = timed(|| left.pack(&left_bytes));
    packed?;
    right.pack(&right_bytes)?;
    product.mul(&left, &right)?;
    let mut out = vec![0u8; n * n];
    let (unpacked, unpack_ns) = timed(|| product.unpack(&mut out));
    unpacked?;
    let conversion = byte_field_arm_common::Conversion {
        setup_ns,
        pack_ns,
        unpack_ns,
        batch_fill_ns: 0,
        dispatch_ns: 0,
    };
    let path = format!(
        "{}/{}/poly=0x{:X}/mzed_mul",
        context.name(),
        context.version(),
        case.poly
    );
    let mut state = MatmulState {
        product,
        left,
        right,
        context,
    };
    match case.metric {
        Metric::KernelIsolated => Ok(Workload {
            selected_path: path,
            conversion,
            body: Box::new(move |_| {
                let MatmulState {
                    product,
                    left,
                    right,
                    ..
                } = &mut state;
                product
                    .mul(black_box(&*left), black_box(&*right))
                    .expect("matmul on a validated adapter");
            }),
        }),
        Metric::WholeConsumer => Ok(Workload {
            selected_path: format!("{path}/whole-consumer"),
            conversion,
            body: Box::new(move |_| {
                let MatmulState {
                    product,
                    left,
                    right,
                    ..
                } = &mut state;
                left.pack(black_box(&left_bytes)).expect("pack left");
                right.pack(black_box(&right_bytes)).expect("pack right");
                product.mul(&*left, &*right).expect("matmul");
                product.unpack(&mut out).expect("unpack");
                black_box(&out);
            }),
        }),
    }
}

/// Generator-matrix region encode `C[r][i] = sum_j G[r][j] * D[j][i]`.
fn build_encode(
    case: &Case,
    mut context: Context,
    setup_ns: u64,
) -> Result<Workload<'static>, String> {
    let (k, rows, len) = (case.k, case.rows, case.bytes);
    let mut rng = SplitMix64::new(case.seed);
    let mut generator = vec![0u8; rows * k];
    let mut data = vec![0u8; k * len];
    let mut coding = vec![0u8; rows * len];
    rng.fill(&mut generator);
    rng.fill(&mut data);
    let (prepared, batch_fill_ns) = timed(|| context.encode_prepare(&generator, k, rows));
    prepared?;
    let path = format!(
        "{}/{}/poly=0x{:X}/ec_encode_data",
        context.name(),
        context.version(),
        case.poly
    );
    let conversion = byte_field_arm_common::Conversion {
        setup_ns,
        pack_ns: 0,
        unpack_ns: 0,
        batch_fill_ns,
        dispatch_ns: 0,
    };
    let whole = matches!(case.metric, Metric::WholeConsumer);
    Ok(Workload {
        selected_path: if whole {
            format!("{path}/whole-consumer")
        } else {
            path
        },
        conversion,
        body: Box::new(move |_| {
            if whole {
                context
                    .encode_prepare(&generator, k, rows)
                    .expect("generator table preparation");
            }
            // SAFETY: `data` holds `k * len` bytes and `coding` holds
            // `rows * len`, so every offset below stays inside its
            // allocation, and the two buffers are distinct.
            let mut sources: Vec<*mut u8> = (0..k)
                .map(|index| unsafe { data.as_mut_ptr().add(index * len) })
                .collect();
            let mut outputs: Vec<*mut u8> = (0..rows)
                .map(|index| unsafe { coding.as_mut_ptr().add(index * len) })
                .collect();
            context
                .encode(len, &mut sources, &mut outputs)
                .expect("generator encode on a validated adapter");
            black_box(&coding);
        }),
    })
}

/// Matrices and the context they came from, in an order that guarantees the
/// matrices are released first.
///
/// A `Matrix` holds a pointer into a field the `Context` owns, so dropping
/// the context first would leave every matrix pointing at freed memory.
/// Rust drops struct fields in declaration order, so keeping the context
/// last makes the ordering explicit rather than incidental.
struct RowState {
    sources: Vec<Matrix>,
    targets: Vec<Matrix>,
    context: Context,
}

/// Square-matrix operands and the context they came from, ordered so the
/// matrices are released before the field they point into.
struct MatmulState {
    product: Matrix,
    left: Matrix,
    right: Matrix,
    /// Held only so the field outlives the matrices that point into it.
    #[allow(dead_code)]
    context: Context,
}

/// Borrows one source row and one target row from two disjoint vectors.
fn split_rows<'a>(
    sources: &'a mut [Matrix],
    targets: &'a mut [Matrix],
    index: usize,
) -> (&'a mut Matrix, &'a mut Matrix) {
    (&mut sources[index], &mut targets[index])
}

/// Checks every backend's region, matrix and encode path through the Rust
/// wrapper against the shim's independent scalar reference.
fn validate() -> i32 {
    const POLY: u32 = 0x11D;
    let mut failures = 0;
    let backends = [
        ("isa-l", shim::BACKEND_ISAL, "default"),
        ("gf-complete", shim::BACKEND_GFCOMPLETE, "default"),
        ("gf-complete", shim::BACKEND_GFCOMPLETE, "split-table-simd"),
        ("m4rie", shim::BACKEND_M4RIE, "default"),
    ];
    for (label, backend, variant) in backends {
        let mut context = match Context::open(backend, POLY, variant) {
            Ok(context) => context,
            Err(reason) => {
                println!("unsupported {label}/{variant}: {reason}");
                continue;
            }
        };
        println!("backend   {label}/{variant} -> {} ({})", context.name(), context.version());
        failures += validate_region(&mut context, label, variant);
        failures += validate_matrix(&mut context, label, variant);
        failures += validate_encode(&mut context, label, variant);
    }
    if failures == 0 {
        println!("\nevery external adapter agrees with the independent reference");
    }
    i32::from(failures > 0)
}

fn validate_region(context: &mut Context, label: &str, variant: &str) -> u32 {
    const POLY: u32 = 0x11D;
    let mut rng = SplitMix64::new(0x6C6B_09B1);
    let length = 4096usize;
    let coefficient = 0x53u8;
    let mut source = vec![0u8; length];
    let mut target = vec![0u8; length];
    rng.fill(&mut source);
    rng.fill(&mut target);
    let want: Vec<u8> = target
        .iter()
        .zip(source.iter())
        .map(|(y, x)| y ^ shim::reference_mul(coefficient, *x, POLY))
        .collect();
    if let Err(reason) = context.prepare(coefficient) {
        println!("unsupported {label}/{variant} region prepare: {reason}");
        return 0;
    }
    let mut scratch = target.clone();
    match context.axpy(&source, &mut scratch) {
        Ok(()) => {
            if scratch == want {
                println!("ok        {label}/{variant} region multiply-XOR through the wrapper");
                0
            } else {
                eprintln!("FAIL {label}/{variant} region multiply-XOR disagrees");
                1
            }
        }
        Err(_) => match context.matrix(1, length) {
            Ok(mut source_row) => {
                let mut target_row = context.matrix(1, length).expect("target row");
                source_row.pack(&source).expect("pack source");
                target_row.pack(&target).expect("pack target");
                target_row
                    .row_axpy(0, &source_row, 0, coefficient)
                    .expect("row axpy");
                let mut got = vec![0u8; length];
                target_row.unpack(&mut got).expect("unpack");
                if got == want {
                    println!("ok        {label}/{variant} row region multiply-XOR through the wrapper");
                    0
                } else {
                    eprintln!("FAIL {label}/{variant} row region multiply-XOR disagrees");
                    1
                }
            }
            Err(reason) => {
                println!("unsupported {label}/{variant} region multiply-XOR: {reason}");
                0
            }
        },
    }
}

fn validate_matrix(context: &mut Context, label: &str, variant: &str) -> u32 {
    const POLY: u32 = 0x11D;
    let n = 16usize;
    let mut rng = SplitMix64::new(0x4D_4154);
    let mut left = vec![0u8; n * n];
    let mut right = vec![0u8; n * n];
    rng.fill(&mut left);
    rng.fill(&mut right);
    let mut a = match context.matrix(n, n) {
        Ok(matrix) => matrix,
        Err(reason) => {
            println!("unsupported {label}/{variant} dense matmul: {reason}");
            return 0;
        }
    };
    let mut b = context.matrix(n, n).expect("right matrix");
    let mut c = context.matrix(n, n).expect("product matrix");
    a.pack(&left).expect("pack left");
    b.pack(&right).expect("pack right");
    c.mul(&a, &b).expect("matmul");
    let mut got = vec![0u8; n * n];
    c.unpack(&mut got).expect("unpack");
    let mut want = vec![0u8; n * n];
    for row in 0..n {
        for col in 0..n {
            let mut accumulator = 0u8;
            for k in 0..n {
                accumulator ^= shim::reference_mul(left[row * n + k], right[k * n + col], POLY);
            }
            want[row * n + col] = accumulator;
        }
    }
    if got == want {
        println!("ok        {label}/{variant} dense matmul through the wrapper");
        0
    } else {
        eprintln!("FAIL {label}/{variant} dense matmul disagrees");
        1
    }
}

fn validate_encode(context: &mut Context, label: &str, variant: &str) -> u32 {
    const POLY: u32 = 0x11D;
    let (k, rows, len) = (6usize, 3usize, 512usize);
    let mut rng = SplitMix64::new(0x454E_43);
    let mut generator = vec![0u8; rows * k];
    let mut data = vec![0u8; k * len];
    let mut coding = vec![0u8; rows * len];
    rng.fill(&mut generator);
    rng.fill(&mut data);
    if let Err(reason) = context.encode_prepare(&generator, k, rows) {
        println!("unsupported {label}/{variant} generator encode: {reason}");
        return 0;
    }
    let mut want = vec![0u8; rows * len];
    for row in 0..rows {
        for index in 0..len {
            let mut accumulator = 0u8;
            for column in 0..k {
                accumulator ^= shim::reference_mul(
                    generator[row * k + column],
                    data[column * len + index],
                    POLY,
                );
            }
            want[row * len + index] = accumulator;
        }
    }
    // SAFETY: `data` holds `k * len` bytes and `coding` holds `rows * len`,
    // so every offset stays inside its allocation, and the two are distinct.
    let mut sources: Vec<*mut u8> = (0..k)
        .map(|index| unsafe { data.as_mut_ptr().add(index * len) })
        .collect();
    let mut outputs: Vec<*mut u8> = (0..rows)
        .map(|index| unsafe { coding.as_mut_ptr().add(index * len) })
        .collect();
    context.encode(len, &mut sources, &mut outputs).expect("encode");
    if coding == want {
        println!("ok        {label}/{variant} generator encode through the wrapper");
        0
    } else {
        eprintln!("FAIL {label}/{variant} generator encode disagrees");
        1
    }
}
