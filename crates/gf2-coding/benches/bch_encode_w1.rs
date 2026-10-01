//! Workload W1: large-batch systematic BCH encoding.
//!
//! The cells are the workload-selection contract's
//! (`dev/active/4e732b56/workload-selection.md`) W1 workload on the canonical
//! construction model, over its § 3 batch sizes, § 5 cache states, and § 6
//! worker counts, with the cell set its d1b4f85e amendment names:
//!
//! - the binary rows of `bch_workloads::BINARY_ROWS` in the packed
//!   representation, the two DVB-T2 rows at their mother lengths;
//! - the DVB-T2 rows `T2S` and `T2N` at the shortened lengths § 2 fixes,
//!   through `dvb_t2_bch_code`'s `Shortened` systematic restriction;
//! - the nonbinary rows of the evidence protocol's corpus (`N1`–`N4`) in the
//!   field-generic representation.
//!
//! Throughput is reported per message symbol, so the elements-per-second
//! figure Criterion prints is § 1's information bits per second on the binary
//! rows and information symbols per second on the nonbinary ones.
//!
//! # Benchmark IDs
//!
//! `bch_encode_w1/<path>/W<workers>/<cache>/<row>/B=<batch>`, where `<path>`
//! names the dispatch path the measured call takes:
//!
//! - `family=<name>[<kernel>]` — `encode_batch_family_into` with one
//!   workspace, under the named family. Every family the row's
//!   representation makes available is measured. The bracket names the
//!   kernel bundle for the families that run one; each such family is
//!   measured on the detected bundle and, on a host whose detected bundle is
//!   accelerated, again on the portable `scalar` bundle.
//! - `selected=<name>[<kernel>]` — the batch entry points that select the
//!   family from the active tuning profile: `encode_batch` (`W1`,
//!   `fresh-alloc`), `encode_batch_parallel_into` (`W6`, `warm-reuse`), and
//!   `encode_batch_parallel` (`W6`, `fresh-alloc`). The name is the family
//!   `selected_encode_family` reports for the cell's batch length.
//! - `route=shortened-restriction` — the DVB-T2 rows' `BlockEncoder`, one
//!   message at a time: `encode` per message (`fresh-alloc`) and
//!   `encode_into` into reused codeword buffers (`warm-reuse`).
//!
//! Every cell of one row and batch writes the same codewords; the bench
//! asserts it from the output digest before measuring. `bch_workloads`
//! documents the dispatch record each cell appends.
//!
//! The `W6` cells need the `parallel` feature and a pool of six threads
//! (`RAYON_NUM_THREADS=6`); the record carries the pool width observed at
//! run time.

mod bch_workloads;

use std::any::type_name;
use std::collections::HashMap;
use std::num::NonZeroUsize;

use bch_workloads::{
    binary_messages, build, full_id, record, OutputDigest, BATCHES, BINARY_ROWS, PARALLEL_WORKERS,
};
use criterion::measurement::WallTime;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkGroup, BenchmarkId, Criterion,
    SamplingMode, Throughput,
};
use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, FrameSize};
use gf2_coding::bch::encode::{
    max_parallel_batch_workers, EncodeFamily, SystematicKernel, SystematicLayout,
};
use gf2_coding::bch::matrix::MatrixFill;
use gf2_coding::bch::spec::BchCode;
use gf2_coding::test_support::{
    bch_base_order, bch_corpus_dense_twin, bch_corpus_element, force_scalar_encode_kernels,
    selected_encode_kernel, visit_bch_corpus, BchCorpusRow, BchCorpusVisitor, BCH_CORPUS_SEED,
};
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use gf2_coding::transform::ShortenedDerivation;
use gf2_coding::CodeRate;
use gf2_core::field::extension::FieldExtension;
use gf2_core::field::FieldVec;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::json;

const GROUP: &str = "bch_encode_w1";

/// The kernel bundle name of the portable arm.
const SCALAR_KERNEL: &str = "scalar";

/// Whether `family` runs the `gf2_kernels_simd::bch_encode` kernel bundle,
/// as the `gf2_coding::bch::encode` module documentation states for the
/// bit-sliced and fold families.
fn runs_kernel_bundle(family: EncodeFamily) -> bool {
    matches!(
        family,
        EncodeFamily::BitsliceInterleaved | EncodeFamily::ClmulFold
    )
}

/// The `<path>` segment of a benchmark ID.
fn path_label(selection: &str, family: EncodeFamily, kernel: Option<&str>) -> String {
    match kernel {
        Some(kernel) => format!("{selection}={family}[{kernel}]"),
        None => format!("{selection}={family}"),
    }
}

fn configure(group: &mut BenchmarkGroup<'_, WallTime>, large: bool) {
    if large {
        group.sample_size(10);
        group.sampling_mode(SamplingMode::Flat);
    } else {
        group.sample_size(100);
        group.sampling_mode(SamplingMode::Auto);
    }
}

/// The facts one cell records beside its Criterion ID.
struct Cell<'a> {
    row: &'a str,
    batch: usize,
    workers: usize,
    cache: &'a str,
    entry: &'a str,
    selection: &'a str,
    family: EncodeFamily,
    kernel: Option<&'a str>,
}

/// Output digests per row and batch; every path must agree.
#[derive(Default)]
struct Agreement(HashMap<(String, usize), u64>);

impl Agreement {
    fn check(&mut self, row: &str, batch: usize, id: &str, digest: u64) {
        let expected = *self.0.entry((row.to_owned(), batch)).or_insert(digest);
        assert_eq!(
            digest, expected,
            "{id} writes different codewords from the other paths of its row and batch"
        );
    }
}

/// Appends the record line of `cell` and returns its Criterion ID parts.
fn register(
    agreement: &mut Agreement,
    cell: &Cell<'_>,
    symbols: &str,
    n: usize,
    k: usize,
    digest: u64,
) -> (String, String) {
    let function = format!(
        "{}/W{}/{}",
        path_label(cell.selection, cell.family, cell.kernel),
        cell.workers,
        cell.cache
    );
    let parameter = format!("{}/B={}", cell.row, cell.batch);
    let id = full_id(GROUP, &function, &parameter);
    agreement.check(cell.row, cell.batch, &id, digest);
    record(&json!({
        "id": id,
        "workload": "W1",
        "row": cell.row,
        "n": n,
        "k": k,
        "batch": cell.batch,
        "workers": cell.workers,
        "cache": cell.cache,
        "entry": cell.entry,
        "selection": cell.selection,
        "family": cell.family.name(),
        "kernel": cell.kernel,
        "symbols": symbols,
        "rayon_pool_width": max_parallel_batch_workers().get(),
        "output_fnv1a": format!("{digest:016x}"),
    }));
    (function, parameter)
}

/// Registers every batch-encoding cell of one row of a BCH code.
fn encode_cells<X, S, M>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    agreement: &mut Agreement,
    row: &str,
    code: &BchCode<X, S, M>,
    messages: &[S],
) where
    X: FieldExtension,
    X::Base: Send + Sync,
    S: SystematicKernel<X::Base> + Send + Sync + OutputDigest,
    S::Word: Send,
    M: gf2_coding::traits::block::SymbolMatrix<X::Base>,
{
    let layout = SystematicLayout::default();
    let (n, k) = (code.n(), code.k());
    let zero = BlockCode::symbol_zero(code);
    let symbols = type_name::<S>();
    let workers = NonZeroUsize::new(PARALLEL_WORKERS).expect("a nonzero worker count");

    for &batch in &BATCHES {
        let batch_messages = &messages[..batch];
        group.throughput(Throughput::Elements((batch * k) as u64));

        // W = 1, warm-reuse: every available family by name.
        for &family in EncodeFamily::REGISTERED {
            if !code.encode_family_available(family, layout) {
                continue;
            }
            let arms: Vec<Option<&'static str>> = if runs_kernel_bundle(family) {
                force_scalar_encode_kernels(false);
                let detected = selected_encode_kernel();
                if detected == SCALAR_KERNEL {
                    vec![Some(detected)]
                } else {
                    vec![Some(detected), Some(SCALAR_KERNEL)]
                }
            } else {
                vec![None]
            };
            for arm in arms {
                force_scalar_encode_kernels(arm == Some(SCALAR_KERNEL));
                if let Some(arm) = arm {
                    assert_eq!(selected_encode_kernel(), arm, "the forced kernel arm");
                }
                let mut workspace = code.encode_workspace();
                let mut codewords = vec![S::zeroed(n, &zero); batch];
                code.encode_batch_family_into(
                    family,
                    batch_messages,
                    layout,
                    &mut workspace,
                    &mut codewords,
                )
                .expect("an available family encodes the batch");
                let cell = Cell {
                    row,
                    batch,
                    workers: 1,
                    cache: "warm-reuse",
                    entry: "BchCode::encode_batch_family_into",
                    selection: "family",
                    family,
                    kernel: arm,
                };
                let (function, parameter) =
                    register(agreement, &cell, symbols, n, k, S::digest(&codewords));
                group.bench_function(BenchmarkId::new(function, parameter), |b| {
                    b.iter(|| {
                        code.encode_batch_family_into(
                            family,
                            black_box(batch_messages),
                            layout,
                            &mut workspace,
                            &mut codewords,
                        )
                        .expect("encode");
                    });
                });
            }
            force_scalar_encode_kernels(false);
        }

        // The profile-selected entry points.
        let selected = code.selected_encode_family(layout, batch);
        let kernel = runs_kernel_bundle(selected).then(selected_encode_kernel);
        let selected_cell = |workers: usize, cache: &'static str, entry: &'static str| Cell {
            row,
            batch,
            workers,
            cache,
            entry,
            selection: "selected",
            family: selected,
            kernel,
        };

        // W = 1, fresh-alloc: the allocating batch entry point.
        let codewords = code
            .encode_batch(batch_messages, layout)
            .expect("encode_batch");
        let (function, parameter) = register(
            agreement,
            &selected_cell(1, "fresh-alloc", "BchCode::encode_batch"),
            symbols,
            n,
            k,
            S::digest(&codewords),
        );
        group.bench_function(BenchmarkId::new(function, parameter), |b| {
            b.iter(|| {
                black_box(
                    code.encode_batch(black_box(batch_messages), layout)
                        .expect("encode"),
                )
            });
        });

        // W = 6, warm-reuse: one workspace per worker, reused output.
        let mut workspaces = code.encode_workspaces(workers);
        let mut codewords = vec![S::zeroed(n, &zero); batch];
        code.encode_batch_parallel_into(batch_messages, layout, &mut workspaces, &mut codewords)
            .expect("encode_batch_parallel_into");
        let (function, parameter) = register(
            agreement,
            &selected_cell(
                PARALLEL_WORKERS,
                "warm-reuse",
                "BchCode::encode_batch_parallel_into",
            ),
            symbols,
            n,
            k,
            S::digest(&codewords),
        );
        group.bench_function(BenchmarkId::new(function, parameter), |b| {
            b.iter(|| {
                code.encode_batch_parallel_into(
                    black_box(batch_messages),
                    layout,
                    &mut workspaces,
                    &mut codewords,
                )
                .expect("encode");
            });
        });

        // W = 6, fresh-alloc: workspaces and output allocated per call.
        let codewords = code
            .encode_batch_parallel(batch_messages, layout, workers)
            .expect("encode_batch_parallel");
        let (function, parameter) = register(
            agreement,
            &selected_cell(
                PARALLEL_WORKERS,
                "fresh-alloc",
                "BchCode::encode_batch_parallel",
            ),
            symbols,
            n,
            k,
            S::digest(&codewords),
        );
        group.bench_function(BenchmarkId::new(function, parameter), |b| {
            b.iter(|| {
                black_box(
                    code.encode_batch_parallel(black_box(batch_messages), layout, workers)
                        .expect("encode"),
                )
            });
        });
    }
}

fn binary_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group(GROUP);
    let mut agreement = Agreement::default();
    let largest = *BATCHES.last().expect("a batch ladder");
    for row in BINARY_ROWS {
        let code = build(row);
        let messages = binary_messages(code.k(), largest);
        configure(&mut group, row.large);
        encode_cells(&mut group, &mut agreement, row.name, &code, &messages);
    }
    group.finish();
}

/// The DVB-T2 rows at their shortened lengths, through the shipped encoder.
fn shortened_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group(GROUP);
    let mut agreement = Agreement::default();
    let largest = *BATCHES.last().expect("a batch ladder");
    configure(&mut group, true);
    for (row, frame) in [("T2S", FrameSize::Short), ("T2N", FrameSize::Normal)] {
        let code = dvb_t2_bch_code(frame, CodeRate::Rate1_2).expect("the DVB-T2 rate-1/2 code");
        let derivation = code.derivation();
        assert_eq!(derivation, ShortenedDerivation::SystematicRestriction);
        let (n, k) = (code.n(), code.k());
        let messages = binary_messages(k, largest);
        // The single-message path runs the mother's reference recurrence;
        // the family seam belongs to the batch entry points.
        let family = EncodeFamily::REFERENCE;

        for &batch in &BATCHES {
            let batch_messages = &messages[..batch];
            group.throughput(Throughput::Elements((batch * k) as u64));

            for (cache, entry) in [
                ("fresh-alloc", "Shortened::encode"),
                ("warm-reuse", "Shortened::encode_into"),
            ] {
                let mut codewords = vec![BitVec::zeros(n); batch];
                for (message, codeword) in batch_messages.iter().zip(&mut codewords) {
                    code.encode_into(message, codeword).expect("encode_into");
                }
                let digest = BitVec::digest(&codewords);
                let function = format!("route=shortened-restriction/W1/{cache}");
                let parameter = format!("{row}/B={batch}");
                let id = full_id(GROUP, &function, &parameter);
                agreement.check(row, batch, &id, digest);
                record(&json!({
                    "id": id,
                    "workload": "W1",
                    "row": row,
                    "n": n,
                    "k": k,
                    "batch": batch,
                    "workers": 1,
                    "cache": cache,
                    "entry": entry,
                    "selection": "route",
                    "derivation": format!("{derivation:?}"),
                    "family": family.name(),
                    "kernel": null,
                    "symbols": type_name::<BitVec>(),
                    "rayon_pool_width": max_parallel_batch_workers().get(),
                    "output_fnv1a": format!("{digest:016x}"),
                }));
                group.bench_function(BenchmarkId::new(function, parameter), |b| {
                    if cache == "fresh-alloc" {
                        b.iter(|| {
                            black_box(
                                black_box(batch_messages)
                                    .iter()
                                    .map(|message| code.encode(message).expect("encode"))
                                    .collect::<Vec<_>>(),
                            )
                        });
                    } else {
                        b.iter(|| {
                            for (message, codeword) in
                                black_box(batch_messages).iter().zip(&mut codewords)
                            {
                                code.encode_into(message, codeword).expect("encode_into");
                            }
                        });
                    }
                });
            }
        }
    }
    group.finish();
}

/// Registers the field-generic cells of the corpus's nonbinary rows.
struct NonbinaryRows<'g, 'c> {
    group: &'g mut BenchmarkGroup<'c, WallTime>,
    agreement: Agreement,
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
        // The corpus declares every nonbinary row field-generic; the twin
        // names that representation in the type system.
        let code = bch_corpus_dense_twin(row, code);
        let (k, zero) = (code.k(), code.extension().base_zero());
        let order = u128::from(bch_base_order(&code));
        let mut rng = StdRng::seed_from_u64(BCH_CORPUS_SEED);
        let largest = *BATCHES.last().expect("a batch ladder");
        let messages: Vec<FieldVec<X::Base>> = (0..largest)
            .map(|_| {
                let mut message = FieldVec::zeros_from(k, &zero);
                for index in 0..k {
                    message.set(index, bch_corpus_element(&zero, rng.gen_range(0..order)));
                }
                message
            })
            .collect();
        encode_cells(self.group, &mut self.agreement, row.id, &code, &messages);
    }
}

fn nonbinary_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group(GROUP);
    configure(&mut group, false);
    let mut rows = NonbinaryRows {
        group: &mut group,
        agreement: Agreement::default(),
    };
    visit_bch_corpus(&mut rows);
    group.finish();
}

criterion_group!(benches, binary_rows, shortened_rows, nonbinary_rows);
criterion_main!(benches);
