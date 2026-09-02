//! Workload W2: GF(2) generator-matrix materialization for BCH codes.
//!
//! The cells are the workload-selection contract's
//! (`dev/active/4e732b56/workload-selection.md`) W2 workload on the canonical
//! construction model: § 2's binary rows at the lengths it fixes, the two
//! DVB-T2 rows at their mother lengths per the § 9 amendment of 2026-09-01,
//! § 4's systematic `[message | parity]` output, and § 5's two cache states.
//! Throughput is reported per matrix bit, so the elements-per-second figure
//! Criterion prints is § 1's matrix bits per second.
//!
//! Every row runs the canonical materialization beside the basis-vector
//! reference `gf2_coding::test_support` exposes, so one run carries the pair
//! a before-and-after comparison needs. The parity-check group repeats the
//! pair for `parity_check_matrix_into`.
//!
//! `T2N-mother` materializes a 512 MiB generator and its reference costs
//! $O(k^2 r)$, so it runs only under `GF2_BENCH=1` on a prepared host.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::test_support::{
    bch_generator_matrix_by_encoding, bch_parity_check_matrix_by_encoding,
};
use gf2_coding::traits::block::{BlockCode, GeneratorMatrixAccess, ParityCheckMatrixAccess};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::BitMatrix;

/// One measured code: the contract row it realizes and how heavy it is.
struct Row {
    /// Contract row name, used as the Criterion parameter.
    name: &'static str,
    /// Extension degree of the mother field.
    degree: usize,
    /// Primitive polynomial, matching the contract's `prim` column.
    modulus: u64,
    /// Designed distance $\delta$ of the contract row.
    designed_distance: u64,
    /// Criterion sample count, lowered for the rows whose reference cell
    /// costs seconds per iteration.
    samples: usize,
    /// Whether the row needs `GF2_BENCH=1` to run.
    bench_mode_only: bool,
}

const ROWS: &[Row] = &[
    Row {
        name: "B1",
        degree: 4,
        modulus: 0b1_0011,
        designed_distance: 7,
        samples: 100,
        bench_mode_only: false,
    },
    Row {
        name: "B2",
        degree: 7,
        modulus: 0b1000_0011,
        designed_distance: 21,
        samples: 100,
        bench_mode_only: false,
    },
    Row {
        name: "B3",
        degree: 8,
        modulus: 0b1_0001_1101,
        designed_distance: 9,
        samples: 100,
        bench_mode_only: false,
    },
    Row {
        name: "T2S-mother",
        degree: 14,
        modulus: 0b100_0000_0010_1011,
        designed_distance: 25,
        samples: 10,
        bench_mode_only: false,
    },
    Row {
        name: "T2N-mother",
        degree: 16,
        modulus: 0b1_0000_0000_0010_1101,
        designed_distance: 25,
        samples: 10,
        bench_mode_only: true,
    },
];

/// Whether the repository's benchmark mode is on.
fn bench_mode() -> bool {
    matches!(std::env::var("GF2_BENCH"), Ok(ref value) if value != "0")
}

fn build(row: &Row) -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(row.degree, row.modulus))
        .expect("the contract's prim column is a primitive polynomial");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(row.designed_distance)
            .expect("a positive designed distance"),
    })
    .expect("a narrow-sense construction over the contract's mother field")
}

fn generator_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_genmatrix_w2");
    for row in ROWS {
        if row.bench_mode_only && !bench_mode() {
            continue;
        }
        let code = build(row);
        let (dimension, length) = (code.k(), code.n());
        group.sample_size(row.samples);
        group.throughput(Throughput::Elements((dimension * length) as u64));

        group.bench_function(BenchmarkId::new("materialize/fresh-alloc", row.name), |b| {
            b.iter(|| black_box(code.generator_matrix().expect("materialization")));
        });
        group.bench_function(BenchmarkId::new("reference/fresh-alloc", row.name), |b| {
            b.iter(|| {
                let mut out = BitMatrix::zeros(dimension, length);
                bch_generator_matrix_by_encoding(&code, &mut out).expect("reference");
                black_box(out)
            });
        });

        let mut buffer = BitMatrix::zeros(dimension, length);
        group.bench_function(BenchmarkId::new("materialize/warm-reuse", row.name), |b| {
            b.iter(|| {
                code.generator_matrix_into(black_box(&mut buffer))
                    .expect("materialization");
            });
        });
        group.bench_function(BenchmarkId::new("reference/warm-reuse", row.name), |b| {
            b.iter(|| {
                bch_generator_matrix_by_encoding(&code, black_box(&mut buffer)).expect("reference");
            });
        });
    }
    group.finish();
}

fn parity_check_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_paritycheck");
    for row in ROWS {
        if row.bench_mode_only && !bench_mode() {
            continue;
        }
        let code = build(row);
        let (redundancy, length) = (code.redundancy(), code.n());
        group.sample_size(row.samples);
        group.throughput(Throughput::Elements((redundancy * length) as u64));

        let mut buffer = BitMatrix::zeros(redundancy, length);
        group.bench_function(BenchmarkId::new("materialize/warm-reuse", row.name), |b| {
            b.iter(|| {
                code.parity_check_matrix_into(black_box(&mut buffer))
                    .expect("materialization");
            });
        });
        group.bench_function(BenchmarkId::new("reference/warm-reuse", row.name), |b| {
            b.iter(|| {
                bch_parity_check_matrix_by_encoding(&code, black_box(&mut buffer))
                    .expect("reference");
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    generator_materialization,
    parity_check_materialization
);
criterion_main!(benches);
