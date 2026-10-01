//! Workload W2: generator-matrix materialization for BCH codes.
//!
//! The cells are the workload-selection contract's
//! (`dev/active/4e732b56/workload-selection.md`) W2 workload on the canonical
//! construction model: § 2's binary rows at the lengths it fixes, the two
//! DVB-T2 rows at the mother lengths § 9 fixes and at the shortened lengths
//! § 2 fixes, the evidence protocol's nonbinary corpus rows, § 4's systematic
//! `[message | parity]` output, and § 5's two cache states. Throughput is
//! reported per matrix symbol, so the elements-per-second figure Criterion
//! prints is § 1's matrix bits per second on the binary rows.
//!
//! Every `BchCode` row runs the canonical materialization beside the
//! basis-vector reference the `gf2_coding::test_support` oracle exposes, so
//! one run carries both. The parity-check group runs the same pair for
//! `parity_check_matrix_into`. The shortened rows `T2S` and `T2N` run
//! `dvb_t2_bch_code`'s generator, which holds the systematic-restriction
//! derivation: it materializes the mother's generator and copies the kept
//! rows and columns out of it.
//!
//! `T2N-mother` and `T2N` materialize the 512 MiB mother generator, so they
//! run only under `GF2_BENCH=1` on a prepared host. Each cell appends its
//! line to the dispatch record `bch_workloads` documents, naming the matrix
//! representation and materialization route it measures.

mod bch_workloads;

use std::any::type_name;

use bch_workloads::{bench_mode, build, full_id, record, BINARY_ROWS};
use criterion::measurement::WallTime;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkGroup, BenchmarkId, Criterion,
    SamplingMode, Throughput,
};
use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, FrameSize};
use gf2_coding::bch::encode::SystematicKernel;
use gf2_coding::bch::matrix::MatrixFill;
use gf2_coding::bch::spec::BchCode;
use gf2_coding::test_support::{
    bch_generator_matrix_by_encoding, bch_parity_check_matrix_by_encoding, visit_bch_corpus,
    BchCorpusRow, BchCorpusVisitor,
};
use gf2_coding::traits::block::{BlockCode, GeneratorMatrixAccess, ParityCheckMatrixAccess};
use gf2_coding::CodeRate;
use gf2_core::field::extension::FieldExtension;
use gf2_core::BitMatrix;
use serde_json::json;

const GENERATOR_GROUP: &str = "bch_genmatrix_w2";
const PARITY_CHECK_GROUP: &str = "bch_paritycheck";

/// Criterion sample count of a row: ten for the rows whose reference cell
/// costs seconds per iteration.
fn samples(large: bool) -> usize {
    if large {
        10
    } else {
        100
    }
}

/// Appends the record line of one cell and returns its Criterion ID.
fn register(
    group: &str,
    function: &str,
    row: &str,
    rows: usize,
    cols: usize,
    route: &str,
    matrix: &str,
) -> BenchmarkId {
    let (route_name, cache) = function
        .split_once('/')
        .expect("a function name of the form <route>/<cache>");
    record(&json!({
        "id": full_id(group, function, row),
        "workload": if group == GENERATOR_GROUP { "W2" } else { "W2-parity-check" },
        "row": row,
        "rows": rows,
        "cols": cols,
        "cache": cache,
        "path": route_name,
        "entry": route,
        "matrix": matrix,
    }));
    BenchmarkId::new(function, row)
}

/// Registers the generator cells of one `BchCode` row.
fn generator_cells<X, S, M>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    row: &str,
    code: &BchCode<X, S, M>,
) where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: MatrixFill<X::Base>,
{
    let (dimension, length) = (code.k(), code.n());
    let zero = BlockCode::symbol_zero(code);
    let matrix = type_name::<M>();
    group.throughput(Throughput::Elements((dimension * length) as u64));

    let reg = |function: &str, route: &str| {
        register(
            GENERATOR_GROUP,
            function,
            row,
            dimension,
            length,
            route,
            matrix,
        )
    };

    group.bench_function(
        reg("materialize/fresh-alloc", "BchCode::generator_matrix"),
        |b| {
            b.iter(|| black_box(code.generator_matrix().expect("materialization")));
        },
    );
    group.bench_function(
        reg(
            "reference/fresh-alloc",
            "test_support::bch_generator_matrix_by_encoding",
        ),
        |b| {
            b.iter(|| {
                let mut out = M::zeroed(dimension, length, &zero);
                bch_generator_matrix_by_encoding(code, &mut out).expect("reference");
                black_box(out)
            });
        },
    );

    let mut buffer = M::zeroed(dimension, length, &zero);
    group.bench_function(
        reg("materialize/warm-reuse", "BchCode::generator_matrix_into"),
        |b| {
            b.iter(|| {
                code.generator_matrix_into(black_box(&mut buffer))
                    .expect("materialization");
            });
        },
    );
    group.bench_function(
        reg(
            "reference/warm-reuse",
            "test_support::bch_generator_matrix_by_encoding",
        ),
        |b| {
            b.iter(|| {
                bch_generator_matrix_by_encoding(code, black_box(&mut buffer)).expect("reference");
            });
        },
    );
}

/// Registers the parity-check cells of one `BchCode` row.
fn parity_check_cells<X, S, M>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    row: &str,
    code: &BchCode<X, S, M>,
) where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: MatrixFill<X::Base>,
{
    let (redundancy, length) = (code.redundancy(), code.n());
    let matrix = type_name::<M>();
    group.throughput(Throughput::Elements((redundancy * length) as u64));

    let mut buffer = M::zeroed(redundancy, length, &BlockCode::symbol_zero(code));
    let id = register(
        PARITY_CHECK_GROUP,
        "materialize/warm-reuse",
        row,
        redundancy,
        length,
        "BchCode::parity_check_matrix_into",
        matrix,
    );
    group.bench_function(id, |b| {
        b.iter(|| {
            code.parity_check_matrix_into(black_box(&mut buffer))
                .expect("materialization");
        });
    });
    let id = register(
        PARITY_CHECK_GROUP,
        "reference/warm-reuse",
        row,
        redundancy,
        length,
        "test_support::bch_parity_check_matrix_by_encoding",
        matrix,
    );
    group.bench_function(id, |b| {
        b.iter(|| {
            bch_parity_check_matrix_by_encoding(code, black_box(&mut buffer)).expect("reference");
        });
    });
}

/// Hands each nonbinary corpus row to a cell registrar.
struct NonbinaryRows<'g, 'c> {
    group: &'g mut BenchmarkGroup<'c, WallTime>,
    parity_check: bool,
}

impl BchCorpusVisitor for NonbinaryRows<'_, '_> {
    fn visit<X, S, M>(&mut self, row: &BchCorpusRow, code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        X::Base: Send + Sync + 'static,
        S: SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
    {
        if !row.id.starts_with('N') {
            return;
        }
        if self.parity_check {
            parity_check_cells(self.group, row.id, code);
        } else {
            generator_cells(self.group, row.id, code);
        }
    }
}

fn generator_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group(GENERATOR_GROUP);
    for row in BINARY_ROWS {
        if row.materialization_bench_mode_only && !bench_mode() {
            continue;
        }
        group.sample_size(samples(row.large));
        generator_cells(&mut group, row.name, &build(row));
    }

    group.sample_size(samples(false));
    visit_bch_corpus(&mut NonbinaryRows {
        group: &mut group,
        parity_check: false,
    });

    // The DVB-T2 rows at the shortened lengths § 2 fixes.
    group.sample_size(samples(true));
    group.sampling_mode(SamplingMode::Flat);
    for (row, frame, bench_mode_only) in [
        ("T2S", FrameSize::Short, false),
        ("T2N", FrameSize::Normal, true),
    ] {
        if bench_mode_only && !bench_mode() {
            continue;
        }
        let code = dvb_t2_bch_code(frame, CodeRate::Rate1_2).expect("the DVB-T2 rate-1/2 code");
        let (dimension, length) = (code.k(), code.n());
        let route = format!("Shortened::generator_matrix ({:?})", code.derivation());
        group.throughput(Throughput::Elements((dimension * length) as u64));
        let matrix = type_name::<BitMatrix>();

        let id = register(
            GENERATOR_GROUP,
            "materialize/fresh-alloc",
            row,
            dimension,
            length,
            &route,
            matrix,
        );
        group.bench_function(id, |b| {
            b.iter(|| black_box(code.generator_matrix().expect("materialization")));
        });
        let mut buffer = BitMatrix::zeros(dimension, length);
        let id = register(
            GENERATOR_GROUP,
            "materialize/warm-reuse",
            row,
            dimension,
            length,
            &route.replace("generator_matrix", "generator_matrix_into"),
            matrix,
        );
        group.bench_function(id, |b| {
            b.iter(|| {
                code.generator_matrix_into(black_box(&mut buffer))
                    .expect("materialization");
            });
        });
    }
    group.finish();
}

fn parity_check_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group(PARITY_CHECK_GROUP);
    for row in BINARY_ROWS {
        if row.materialization_bench_mode_only && !bench_mode() {
            continue;
        }
        group.sample_size(samples(row.large));
        parity_check_cells(&mut group, row.name, &build(row));
    }
    group.sample_size(samples(false));
    visit_bch_corpus(&mut NonbinaryRows {
        group: &mut group,
        parity_check: true,
    });
    group.finish();
}

criterion_group!(
    benches,
    generator_materialization,
    parity_check_materialization
);
criterion_main!(benches);
