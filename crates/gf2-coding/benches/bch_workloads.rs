//! Rows, fixtures, output digests, and the dispatch record shared by the BCH
//! workload benches, included through `mod bch_workloads;`. The cells, their
//! IDs, and the record format are the d1b4f85e amendment of
//! `dev/active/4e732b56/workload-selection.md`.

#![allow(dead_code)]

use std::fs::OpenOptions;
use std::io::Write;

use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::test_support::bch_corpus_index;
use gf2_core::field::extension::{BinaryPrimeExt, FieldIdentity};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::{FieldVec, FiniteField};
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};

/// Batch sizes $B$ of the contract's § 3.
pub const BATCHES: [usize; 4] = [1, 16, 256, 4096];

/// The parallel worker count $W > 1$ of the contract's § 6.
pub const PARALLEL_WORKERS: usize = 6;

/// Environment variable naming the dispatch-record file.
pub const DISPATCH_RECORD_ENV: &str = "GF2_BCH_DISPATCH_RECORD";

/// One binary benchmark code: the contract row it realizes and its cost tier.
pub struct BinaryRow {
    /// Contract row name, used as the Criterion parameter prefix.
    pub name: &'static str,
    /// Extension degree of the mother field.
    pub degree: usize,
    /// Primitive polynomial, matching the contract's `prim` column.
    pub modulus: u64,
    /// Designed distance $\delta$ of the contract row.
    pub designed_distance: u64,
    /// Whether a cell on this row costs enough per iteration to run at ten
    /// flat samples rather than Criterion's default hundred.
    pub large: bool,
    /// Whether materializing this row's generator needs `GF2_BENCH=1`: the
    /// `T2N-mother` generator is 512 MiB and its basis-vector reference
    /// costs $O(k^2 r)$.
    pub materialization_bench_mode_only: bool,
}

/// The binary rows, in contract order.
pub const BINARY_ROWS: &[BinaryRow] = &[
    BinaryRow {
        name: "B1",
        degree: 4,
        modulus: 0b1_0011,
        designed_distance: 7,
        large: false,
        materialization_bench_mode_only: false,
    },
    BinaryRow {
        name: "B2",
        degree: 7,
        modulus: 0b1000_0011,
        designed_distance: 21,
        large: false,
        materialization_bench_mode_only: false,
    },
    BinaryRow {
        name: "B3",
        degree: 8,
        modulus: 0b1_0001_1101,
        designed_distance: 9,
        large: false,
        materialization_bench_mode_only: false,
    },
    // The DVB-T2 rows at the mother lengths § 9 fixes.
    BinaryRow {
        name: "T2S-mother",
        degree: 14,
        modulus: 0b100_0000_0010_1011,
        designed_distance: 25,
        large: true,
        materialization_bench_mode_only: false,
    },
    BinaryRow {
        name: "T2N-mother",
        degree: 16,
        modulus: 0b1_0000_0000_0010_1101,
        designed_distance: 25,
        large: true,
        materialization_bench_mode_only: true,
    },
];

/// Whether the repository's benchmark mode is on.
pub fn bench_mode() -> bool {
    matches!(std::env::var("GF2_BENCH"), Ok(ref value) if value != "0")
}

/// Builds the primitive narrow-sense code of `row`.
pub fn build(row: &BinaryRow) -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(row.degree, row.modulus))
        .expect("the contract's prim column is a primitive polynomial");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(row.designed_distance)
            .expect("a positive designed distance"),
    })
    .expect("a narrow-sense construction over the contract's mother field")
}

/// Seed of binary message `index`: the evidence protocol's seed offset by
/// the message index, through [`BitVec::random_seeded`].
pub fn binary_message_seed(index: usize) -> u64 {
    gf2_coding::test_support::BCH_CORPUS_SEED.wrapping_add(index as u64)
}

/// `count` seeded binary messages of `k` bits.
pub fn binary_messages(k: usize, count: usize) -> Vec<BitVec> {
    (0..count)
        .map(|index| BitVec::random_seeded(k, binary_message_seed(index)))
        .collect()
}

/// FNV-1a over a sequence of 64-bit values.
fn fnv1a(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in values {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// A digest of a cell's output, identical for identical outputs.
pub trait OutputDigest {
    /// Digests `items` in order, each with its length.
    fn digest(items: &[Self]) -> u64
    where
        Self: Sized;
}

impl OutputDigest for BitVec {
    fn digest(items: &[Self]) -> u64 {
        fnv1a(items.iter().flat_map(|item| {
            std::iter::once(item.len() as u64).chain(item.words().iter().copied())
        }))
    }
}

impl<F: FiniteField + FieldIdentity> OutputDigest for FieldVec<F> {
    fn digest(items: &[Self]) -> u64 {
        fnv1a(items.iter().flat_map(|item| {
            std::iter::once(item.len() as u64)
                .chain(item.iter().map(|symbol| bch_corpus_index(symbol) as u64))
        }))
    }
}

/// A digest of a materialized matrix, identical for identical matrices.
pub trait MatrixDigest {
    /// Digests the shape and every entry in row-major order.
    fn matrix_digest(&self) -> u64;
}

impl MatrixDigest for BitMatrix {
    fn matrix_digest(&self) -> u64 {
        let shape = [self.rows() as u64, self.cols() as u64];
        fnv1a(
            shape
                .into_iter()
                .chain((0..self.rows()).flat_map(|row| self.row_words(row).iter().copied())),
        )
    }
}

impl<F: FiniteField + FieldIdentity> MatrixDigest for FieldMatrix<F> {
    fn matrix_digest(&self) -> u64 {
        let shape = [self.rows() as u64, self.cols() as u64];
        fnv1a(shape.into_iter().chain((0..self.rows()).flat_map(|row| {
            self.row(row)
                .iter()
                .map(|symbol| bch_corpus_index(symbol) as u64)
        })))
    }
}

/// Appends one line to the dispatch record, when one is requested.
///
/// # Panics
///
/// Panics if the requested file cannot be opened or written: a run that asked
/// for a record and lost a line would attribute an estimate to nothing.
pub fn record(line: &serde_json::Value) {
    let Ok(path) = std::env::var(DISPATCH_RECORD_ENV) else {
        return;
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|error| panic!("open dispatch record {path}: {error}"));
    writeln!(file, "{line}")
        .unwrap_or_else(|error| panic!("write dispatch record {path}: {error}"));
}

/// The Criterion full ID of a benchmark registered as
/// `BenchmarkId::new(function, parameter)` in `group`.
pub fn full_id(group: &str, function: &str, parameter: &str) -> String {
    format!("{group}/{function}/{parameter}")
}
