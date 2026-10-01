//! Workload W2, generator- and parity-check-matrix materialization: the
//! canonical path beside the basis-vector reference on every `BchCode` row,
//! and the shipped DVB-T2 generator on the shortened rows. The cells, their
//! IDs, and the dispatch record are the d1b4f85e amendment of
//! `dev/active/4e732b56/workload-selection.md`; `T2N-mother` and `T2N`
//! materialize the 512 MiB mother generator and run only under `GF2_BENCH=1`.
//! Every path of a row is checked to write the same matrix before any cell of
//! that row is timed.

mod bch_workloads;

use std::any::type_name;

use bch_workloads::{bench_mode, build, full_id, record, MatrixDigest, BINARY_ROWS};
use criterion::measurement::WallTime;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkGroup, BenchmarkId, Criterion,
    SamplingMode, Throughput,
};
use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, FrameSize};
use gf2_coding::bch::encode::{max_parallel_batch_workers, SystematicKernel};
use gf2_coding::bch::matrix::MatrixFill;
use gf2_coding::bch::spec::BchCode;
use gf2_coding::test_support::{
    bch_corpus_dense_twin, bch_generator_matrix_by_encoding, bch_parity_check_matrix_by_encoding,
    visit_bch_corpus, BchCorpusRow, BchCorpusVisitor,
};
use gf2_coding::traits::block::conformance::generator_rows_encode_basis;
use gf2_coding::traits::block::{BlockCode, GeneratorMatrixAccess, ParityCheckMatrixAccess};
use gf2_coding::CodeRate;
use gf2_core::field::extension::FieldExtension;
use gf2_core::field::FiniteField;
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

/// The facts one row's cells share in their record lines.
struct RowFacts<'a> {
    group: &'a str,
    row: &'a str,
    rows: usize,
    cols: usize,
    matrix: &'a str,
    /// Digest of the matrix every path of the row writes.
    digest: u64,
}

/// Returns the digest every path of a row agrees on.
///
/// # Panics
///
/// Panics, aborting the run before the row is timed, if two paths wrote
/// different matrices.
fn agreed_digest(row: &str, paths: &[(&str, u64)]) -> u64 {
    let (first, digest) = paths[0];
    for &(path, other) in &paths[1..] {
        assert_eq!(
            other, digest,
            "{row}: {path} writes a different matrix from {first}"
        );
    }
    digest
}

/// Appends the record line of one cell and returns its Criterion ID.
fn register(facts: &RowFacts<'_>, function: &str, route: &str) -> BenchmarkId {
    let RowFacts {
        group,
        row,
        rows,
        cols,
        matrix,
        digest,
    } = *facts;
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
        "workers": 1,
        "rayon_pool_width": max_parallel_batch_workers().get(),
        "output_fnv1a": format!("{digest:016x}"),
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
    M: MatrixFill<X::Base> + MatrixDigest,
{
    let (dimension, length) = (code.k(), code.n());
    let zero = BlockCode::symbol_zero(code);
    group.throughput(Throughput::Elements((dimension * length) as u64));

    let mut buffer = M::zeroed(dimension, length, &zero);
    code.generator_matrix_into(&mut buffer)
        .expect("materialization");
    let into = buffer.matrix_digest();
    bch_generator_matrix_by_encoding(code, &mut buffer).expect("reference");
    let reference = buffer.matrix_digest();
    let fresh = code
        .generator_matrix()
        .expect("materialization")
        .matrix_digest();
    let facts = RowFacts {
        group: GENERATOR_GROUP,
        row,
        rows: dimension,
        cols: length,
        matrix: type_name::<M>(),
        digest: agreed_digest(
            row,
            &[
                ("generator_matrix", fresh),
                ("generator_matrix_into", into),
                ("bch_generator_matrix_by_encoding", reference),
            ],
        ),
    };
    let reg = |function: &str, route: &str| register(&facts, function, route);

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
    M: MatrixFill<X::Base> + MatrixDigest,
{
    let (redundancy, length) = (code.redundancy(), code.n());
    group.throughput(Throughput::Elements((redundancy * length) as u64));

    let mut buffer = M::zeroed(redundancy, length, &BlockCode::symbol_zero(code));
    code.parity_check_matrix_into(&mut buffer)
        .expect("materialization");
    let canonical = buffer.matrix_digest();
    bch_parity_check_matrix_by_encoding(code, &mut buffer).expect("reference");
    let reference = buffer.matrix_digest();
    let facts = RowFacts {
        group: PARITY_CHECK_GROUP,
        row,
        rows: redundancy,
        cols: length,
        matrix: type_name::<M>(),
        digest: agreed_digest(
            row,
            &[
                ("parity_check_matrix_into", canonical),
                ("bch_parity_check_matrix_by_encoding", reference),
            ],
        ),
    };
    let id = register(
        &facts,
        "materialize/warm-reuse",
        "BchCode::parity_check_matrix_into",
    );
    group.bench_function(id, |b| {
        b.iter(|| {
            code.parity_check_matrix_into(black_box(&mut buffer))
                .expect("materialization");
        });
    });
    let id = register(
        &facts,
        "reference/warm-reuse",
        "test_support::bch_parity_check_matrix_by_encoding",
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
        // The twin names the corpus's field-generic representation in the
        // type system, which is what reaches its matrix digest.
        let code = bch_corpus_dense_twin(row, code);
        if self.parity_check {
            parity_check_cells(self.group, row.id, &code);
        } else {
            generator_cells(self.group, row.id, &code);
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

        // The reference is the matrix contract itself: row i encodes basis
        // vector i through the shipped encoder.
        generator_rows_encode_basis(&code, &code.symbol_zero().one_like());
        let fresh = code
            .generator_matrix()
            .expect("materialization")
            .matrix_digest();
        let mut buffer = BitMatrix::zeros(dimension, length);
        code.generator_matrix_into(&mut buffer)
            .expect("materialization");
        let into = buffer.matrix_digest();
        let facts = RowFacts {
            group: GENERATOR_GROUP,
            row,
            rows: dimension,
            cols: length,
            matrix: type_name::<BitMatrix>(),
            digest: agreed_digest(
                row,
                &[("generator_matrix", fresh), ("generator_matrix_into", into)],
            ),
        };

        let id = register(&facts, "materialize/fresh-alloc", &route);
        group.bench_function(id, |b| {
            b.iter(|| black_box(code.generator_matrix().expect("materialization")));
        });
        let id = register(
            &facts,
            "materialize/warm-reuse",
            &route.replace("generator_matrix", "generator_matrix_into"),
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
