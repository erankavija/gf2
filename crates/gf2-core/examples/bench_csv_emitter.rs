//! Hand-rolled timing harness that emits a T1-compatible CSV row for
//! every (operation, field, size, regime) cell of issue `6ed7f050`.
//!
//! ## Why a separate binary
//!
//! Criterion's default output is HTML/JSON, not CSV. The reference
//! container harness (`benchmarks/reference/fflas_bench.cpp`) writes
//! its rows directly via `clock_gettime(CLOCK_MONOTONIC)`, so for the
//! gf2 side to produce a CSV in the *same* schema we either need to
//! parse criterion's JSON output post hoc or run a parallel harness.
//! This binary takes the latter route — `std::time::Instant`,
//! warmup + iters, and one `println!` per row.
//!
//! The criterion benches in `benches/fieldmatrix_*.rs` cover the same
//! cells with criterion's statistical machinery (mean ± stdev,
//! outlier detection); this binary is the "give me the CSV in the
//! schema fflas_bench wrote" path.
//!
//! ## CSV output
//!
//! Writes to `bench_results/gf2-<timestamp>.csv` (relative to the
//! current working directory) by default. The directory is created if
//! it doesn't exist. The header row matches `benchmarks/README.md`
//! exactly:
//!
//! ```text
//! lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
//! ```
//!
//! ## Determinism
//!
//! Every matrix is drawn from the master seed in
//! `benchmarks/seeds/seed.txt` via the SplitMix64 derivation in
//! `benches/common/seed.rs`. Re-running with the same seed produces
//! byte-identical input matrices.
//!
//! ## Usage
//!
//! ```bash
//! # Default: master seed = 0x6F73AC91D31E4A7C, warmup=2, iters=3.
//! cargo run -p gf2-core --release --example bench_csv_emitter --features rand
//!
//! # Override:
//! cargo run -p gf2-core --release --example bench_csv_emitter --features rand -- \
//!     --seed 0xCAFEBABEDEADBEEF --warmup 1 --iters 2 --output bench_results/myrun.csv
//!
//! # Filter cells (substring match against `<operation>/<field>/<n>/<regime>`):
//! cargo run -p gf2-core --release --example bench_csv_emitter --features rand -- \
//!     --filter fgemm/Fp_M31
//! ```
//!
//! ## Per-cell budget
//!
//! Cells respect the same 30 s `kCellBudgetNs` cap as the reference
//! harness; if the warmup phase alone exceeds it, the cell exits early
//! and emits a row tagged with `early_exit=true` on stderr (the CSV
//! row carries the partial measurement).
//!
//! **Do not run this binary from an automated agent loop.** A full
//! sweep at default settings can take many minutes per field.

#[path = "../benches/common/seed.rs"]
mod seed;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;

use seed::{
    derive_seed, fp_matrix_from_seed, fp_rank_deficient_from_seed, fp_vec_from_seed,
    gf2m_wide_1_matrix_from_seed, gf2m_wide_1_rank_deficient_from_seed, gf2m_wide_1_vec_from_seed,
    ops_cubic, ops_gemm, splitmix64, tput, CSV_HEADER,
};

const PRIME_7: u64 = 7;
const PRIME_251: u64 = 251;
const PRIME_65521: u64 = 65521;
const MERSENNE_31: u64 = 2_147_483_647;

struct EmitterGf2m8Cfg;
impl Gf2mWideConfig<1> for EmitterGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "Gf2m8";
}

struct EmitterGf2m16Cfg;
impl Gf2mWideConfig<1> for EmitterGf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002D];
    const NAME: &'static str = "Gf2m16";
}

const CELL_BUDGET_NS: u64 = 30 * 1_000_000_000;

#[derive(Clone)]
struct Args {
    master_seed: u64,
    warmup: u32,
    iters: u32,
    output: PathBuf,
    filter: Option<String>,
}

impl Args {
    fn parse() -> Self {
        let argv: Vec<String> = std::env::args().collect();
        let mut master_seed: u64 = 0x6F73_AC91_D31E_4A7C;
        let mut warmup: u32 = 2;
        let mut iters: u32 = 3;
        let mut output: Option<PathBuf> = None;
        let mut filter: Option<String> = None;
        let mut i = 1;
        while i < argv.len() {
            match argv[i].as_str() {
                "--seed" => {
                    let arg = argv.get(i + 1).expect("--seed requires an argument");
                    master_seed = parse_u64(arg);
                    i += 2;
                }
                "--warmup" => {
                    warmup = argv[i + 1].parse().expect("--warmup must be a u32");
                    i += 2;
                }
                "--iters" => {
                    iters = argv[i + 1].parse().expect("--iters must be a u32");
                    i += 2;
                }
                "--output" => {
                    output = Some(PathBuf::from(&argv[i + 1]));
                    i += 2;
                }
                "--filter" => {
                    filter = Some(argv[i + 1].clone());
                    i += 2;
                }
                "--help" | "-h" => {
                    eprintln!(
                        "Usage: bench_csv_emitter \
                         [--seed N] [--warmup K] [--iters K] \
                         [--output PATH] [--filter SUBSTRING]"
                    );
                    std::process::exit(0);
                }
                other => {
                    panic!("Unknown argument: {other}");
                }
            }
        }
        let output = output.unwrap_or_else(default_output_path);
        Args {
            master_seed,
            warmup,
            iters,
            output,
            filter,
        }
    }
}

fn parse_u64(s: &str) -> u64 {
    if let Some(stripped) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(stripped, 16).expect("expected hex u64")
    } else {
        s.parse().expect("expected decimal u64")
    }
}

fn default_output_path() -> PathBuf {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    PathBuf::from(format!("bench_results/gf2-{secs}.csv"))
}

/// Run a closure, return mean wall-clock ns over `iters` after `warmup`
/// throwaway iterations. Honours `CELL_BUDGET_NS`.
fn time_op<F: FnMut()>(mut op: F, warmup: u32, iters: u32) -> (u64, bool) {
    for _ in 0..warmup {
        op();
    }
    let mut total_ns: u64 = 0;
    let mut actual: u32 = 0;
    let mut early = false;
    for _ in 0..iters {
        let t0 = Instant::now();
        op();
        let dt = t0.elapsed().as_nanos() as u64;
        total_ns = total_ns.saturating_add(dt);
        actual += 1;
        if total_ns >= CELL_BUDGET_NS {
            early = true;
            break;
        }
    }
    let mean = total_ns / actual.max(1) as u64;
    (mean, early)
}

/// Cell key for filter matching: `"<op>/<field>/<n>[/<regime>]"`.
fn cell_key(op: &str, field: &str, n: usize, regime: &str) -> String {
    format!("{op}/{field}/{n}/{regime}")
}

fn cell_passes(filter: &Option<String>, key: &str) -> bool {
    match filter {
        Some(f) => key.contains(f),
        None => true,
    }
}

struct CsvSink {
    out: BufWriter<File>,
}

impl CsvSink {
    fn new(path: &PathBuf) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(CSV_HEADER.as_bytes())?;
        Ok(CsvSink { out })
    }

    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        operation: &str,
        field: &str,
        m: usize,
        k: usize,
        n: usize,
        rank_regime: &str,
        seed_val: u64,
        wall_ns: u64,
        throughput_ops: f64,
    ) -> std::io::Result<()> {
        writeln!(
            self.out,
            "gf2,{operation},{field},{m},{k},{n},{rank_regime},{seed_val},{wall_ns},{throughput_ops:.6e}"
        )
    }
}

// ─── Per-operation runners ─────────────────────────────────────────────────

const SQUARE_SIZES: &[usize] = &[64, 256, 1024, 4096];
/// Rectangular fgemm shapes — must match
/// `crates/gf2-core/benches/fieldmatrix_gemm.rs::RECT_SHAPES` so the
/// CSV emitter and the criterion bench cover the same `(m, k, n)`
/// cells. The `size_idx` for these shapes is
/// `SQUARE_SIZES.len() + rsi` (matching the bench's derivation).
const RECT_SHAPES: &[(usize, usize, usize)] = &[(1024, 1024, 32), (1024, 1024, 8)];
const CHARPOLY_SIZES: &[usize] = &[32, 128, 512];
const SPMV_SIZES: &[usize] = &[256, 1024, 4096];
const SPMV_DENSITIES: &[(f64, &str)] = &[(0.01, "0.01"), (0.05, "0.05")];

fn run_fp<const P: u64>(args: &Args, sink: &mut CsvSink, field_label: &str) -> std::io::Result<()> {
    let warmup = args.warmup;
    let iters = args.iters;
    let master = args.master_seed;
    let filter = &args.filter;

    // ── fgemm (uniform only) ──────────────────────────────────────────────
    for (si, &n) in SQUARE_SIZES.iter().enumerate() {
        let key = cell_key("fgemm", field_label, n, "uniform");
        if !cell_passes(filter, &key) {
            continue;
        }
        let seed_a = derive_seed(master, "fgemm", 0, si as u64, 0);
        let seed_b = derive_seed(master, "fgemm_b", 0, si as u64, 0);
        let a = fp_matrix_from_seed::<P>(n, n, seed_a);
        let b = fp_matrix_from_seed::<P>(n, n, seed_b);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(gemm(&a, &b));
            },
            warmup,
            iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "fgemm",
            field_label,
            n,
            n,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_gemm(n, n, n), wall_ns),
        )?;
    }

    // ── fgemm rectangular (uniform only; same shape set as bench_rect) ───
    for (rsi, &(m, k, n)) in RECT_SHAPES.iter().enumerate() {
        let key = cell_key("fgemm", field_label, n, "uniform_rect");
        if !cell_passes(filter, &key) {
            continue;
        }
        let size_idx = (SQUARE_SIZES.len() + rsi) as u64;
        let seed_a = derive_seed(master, "fgemm_rect", 0, size_idx, 0);
        let seed_b = derive_seed(master, "fgemm_rect_b", 0, size_idx, 0);
        let a = fp_matrix_from_seed::<P>(m, k, seed_a);
        let b = fp_matrix_from_seed::<P>(k, n, seed_b);
        eprintln!("[gf2-csv] {key} ({m}x{k}x{n})");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(gemm(&a, &b));
            },
            warmup,
            iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "fgemm",
            field_label,
            m,
            k,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_gemm(m, k, n), wall_ns),
        )?;
    }

    // ── factorisation ops with both regimes ──────────────────────────────
    for (op_idx, op) in [("pluq", 1u64), ("echelon", 2), ("invert", 3), ("solve", 4)] {
        let _ = op_idx;
        run_fp_factorisation::<P>(args, sink, field_label, op_idx, op)?;
    }

    // ── charpoly (uniform only) ──────────────────────────────────────────
    for (si, &n) in CHARPOLY_SIZES.iter().enumerate() {
        let key = cell_key("charpoly", field_label, n, "uniform");
        if !cell_passes(filter, &key) {
            continue;
        }
        let seed_a = derive_seed(master, "charpoly", 5, si as u64, 0);
        let a = fp_matrix_from_seed::<P>(n, n, seed_a);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(a.charpoly());
            },
            warmup,
            iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "charpoly",
            field_label,
            n,
            n,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_cubic(n), wall_ns),
        )?;
    }

    // ── SpMV (every density × size, uniform only) ────────────────────────
    for (di, &(density, density_label)) in SPMV_DENSITIES.iter().enumerate() {
        for (si, &n) in SPMV_SIZES.iter().enumerate() {
            let regime = format!("density_{density_label}");
            let key = cell_key("spmv", field_label, n, &regime);
            if !cell_passes(filter, &key) {
                continue;
            }
            let row_seed = derive_seed(master, "spmv", 11, si as u64, di as u64);
            let vec_seed = derive_seed(master, "spmv_vec", 11, si as u64, di as u64);
            let a = build_sparse_fp::<P>(n, n, density, row_seed);
            let x = fp_vec_from_seed::<P>(n, vec_seed);
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = time_op(
                || {
                    let _ = std::hint::black_box(a.matvec(&x));
                },
                warmup,
                iters,
            );
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            // Throughput: # of non-zero MAC pairs.
            let nnz_ops = a.nnz() as f64;
            sink.emit(
                "spmv",
                field_label,
                n,
                n,
                1,
                &regime,
                row_seed,
                wall_ns,
                tput(nnz_ops, wall_ns),
            )?;
        }
    }

    Ok(())
}

fn run_fp_factorisation<const P: u64>(
    args: &Args,
    sink: &mut CsvSink,
    field_label: &str,
    op: &str,
    op_idx: u64,
) -> std::io::Result<()> {
    for (si, &n) in SQUARE_SIZES.iter().enumerate() {
        for (regime, regime_idx) in [("uniform", 0u64), ("deficient", 1)] {
            let key = cell_key(op, field_label, n, regime);
            if !cell_passes(&args.filter, &key) {
                continue;
            }
            let row_seed = derive_seed(args.master_seed, op, op_idx, si as u64, regime_idx);
            let a = if regime_idx == 0 {
                fp_matrix_from_seed::<P>(n, n, row_seed)
            } else {
                fp_rank_deficient_from_seed::<P>(n, n, n / 2, row_seed)
            };
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = match op {
                "pluq" => time_op(
                    || {
                        let _ = std::hint::black_box(a.ple());
                    },
                    args.warmup,
                    args.iters,
                ),
                "echelon" => time_op(
                    || {
                        let _ = std::hint::black_box(a.row_echelon());
                    },
                    args.warmup,
                    args.iters,
                ),
                "invert" => time_op(
                    || {
                        let _ = std::hint::black_box(a.inv());
                    },
                    args.warmup,
                    args.iters,
                ),
                "solve" => {
                    let bvec_seed =
                        derive_seed(args.master_seed, "solve_rhs", op_idx, si as u64, regime_idx);
                    let b = fp_vec_from_seed::<P>(n, bvec_seed);
                    time_op(
                        || {
                            let _ = std::hint::black_box(a.solve(&b));
                        },
                        args.warmup,
                        args.iters,
                    )
                }
                other => panic!("unknown op {other}"),
            };
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            sink.emit(
                op,
                field_label,
                n,
                n,
                n,
                regime,
                row_seed,
                wall_ns,
                tput(ops_cubic(n), wall_ns),
            )?;
        }
    }
    Ok(())
}

fn build_sparse_fp<const P: u64>(
    rows: usize,
    cols: usize,
    density: f64,
    seed_val: u64,
) -> SparseFieldMatrix<Fp<P>> {
    let mut st = seed_val;
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    let threshold = (density * (u64::MAX as f64 + 1.0)) as u64;
    for r in 0..rows {
        for c in 0..cols {
            let draw = splitmix64(&mut st);
            if draw < threshold {
                let v_raw = splitmix64(&mut st);
                let v = (v_raw % (P - 1)) + 1;
                m.set(r, c, Fp::<P>::new(v));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn run_gf2m<C: Gf2mWideConfig<1>>(
    args: &Args,
    sink: &mut CsvSink,
    field_label: &str,
) -> std::io::Result<()> {
    // ── fgemm (uniform only) ──────────────────────────────────────────────
    for (si, &n) in SQUARE_SIZES.iter().enumerate() {
        let key = cell_key("fgemm", field_label, n, "uniform");
        if !cell_passes(&args.filter, &key) {
            continue;
        }
        let seed_a = derive_seed(args.master_seed, "fgemm", 0, si as u64, 0);
        let seed_b = derive_seed(args.master_seed, "fgemm_b", 0, si as u64, 0);
        let a: FieldMatrix<Gf2mWide<1, C>> = gf2m_wide_1_matrix_from_seed::<C>(n, n, seed_a);
        let b: FieldMatrix<Gf2mWide<1, C>> = gf2m_wide_1_matrix_from_seed::<C>(n, n, seed_b);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(gemm(&a, &b));
            },
            args.warmup,
            args.iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "fgemm",
            field_label,
            n,
            n,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_gemm(n, n, n), wall_ns),
        )?;
    }

    // ── fgemm rectangular (uniform only; same shape set as bench_rect) ───
    for (rsi, &(m, k, n)) in RECT_SHAPES.iter().enumerate() {
        let key = cell_key("fgemm", field_label, n, "uniform_rect");
        if !cell_passes(&args.filter, &key) {
            continue;
        }
        let size_idx = (SQUARE_SIZES.len() + rsi) as u64;
        let seed_a = derive_seed(args.master_seed, "fgemm_rect", 0, size_idx, 0);
        let seed_b = derive_seed(args.master_seed, "fgemm_rect_b", 0, size_idx, 0);
        let a: FieldMatrix<Gf2mWide<1, C>> = gf2m_wide_1_matrix_from_seed::<C>(m, k, seed_a);
        let b: FieldMatrix<Gf2mWide<1, C>> = gf2m_wide_1_matrix_from_seed::<C>(k, n, seed_b);
        eprintln!("[gf2-csv] {key} ({m}x{k}x{n})");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(gemm(&a, &b));
            },
            args.warmup,
            args.iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "fgemm",
            field_label,
            m,
            k,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_gemm(m, k, n), wall_ns),
        )?;
    }

    // ── factorisation ops with both regimes ──────────────────────────────
    for (op_idx, op) in [("pluq", 1u64), ("echelon", 2), ("invert", 3), ("solve", 4)] {
        let _ = op_idx;
        run_gf2m_factorisation::<C>(args, sink, field_label, op_idx, op)?;
    }

    // ── charpoly (uniform only) ──────────────────────────────────────────
    for (si, &n) in CHARPOLY_SIZES.iter().enumerate() {
        let key = cell_key("charpoly", field_label, n, "uniform");
        if !cell_passes(&args.filter, &key) {
            continue;
        }
        let seed_a = derive_seed(args.master_seed, "charpoly", 5, si as u64, 0);
        let a: FieldMatrix<Gf2mWide<1, C>> = gf2m_wide_1_matrix_from_seed::<C>(n, n, seed_a);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(a.charpoly());
            },
            args.warmup,
            args.iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "charpoly",
            field_label,
            n,
            n,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_cubic(n), wall_ns),
        )?;
    }

    // ── SpMV ─────────────────────────────────────────────────────────────
    for (di, &(density, density_label)) in SPMV_DENSITIES.iter().enumerate() {
        for (si, &n) in SPMV_SIZES.iter().enumerate() {
            let regime = format!("density_{density_label}");
            let key = cell_key("spmv", field_label, n, &regime);
            if !cell_passes(&args.filter, &key) {
                continue;
            }
            let row_seed = derive_seed(args.master_seed, "spmv", 11, si as u64, di as u64);
            let vec_seed = derive_seed(args.master_seed, "spmv_vec", 11, si as u64, di as u64);
            let a = build_sparse_gf2m::<C>(n, n, density, row_seed);
            let x: FieldVec<Gf2mWide<1, C>> = gf2m_wide_1_vec_from_seed::<C>(n, vec_seed);
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = time_op(
                || {
                    let _ = std::hint::black_box(a.matvec(&x));
                },
                args.warmup,
                args.iters,
            );
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            let nnz_ops = a.nnz() as f64;
            sink.emit(
                "spmv",
                field_label,
                n,
                n,
                1,
                &regime,
                row_seed,
                wall_ns,
                tput(nnz_ops, wall_ns),
            )?;
        }
    }

    Ok(())
}

fn run_gf2m_factorisation<C: Gf2mWideConfig<1>>(
    args: &Args,
    sink: &mut CsvSink,
    field_label: &str,
    op: &str,
    op_idx: u64,
) -> std::io::Result<()> {
    for (si, &n) in SQUARE_SIZES.iter().enumerate() {
        for (regime, regime_idx) in [("uniform", 0u64), ("deficient", 1)] {
            let key = cell_key(op, field_label, n, regime);
            if !cell_passes(&args.filter, &key) {
                continue;
            }
            let row_seed = derive_seed(args.master_seed, op, op_idx, si as u64, regime_idx);
            let a: FieldMatrix<Gf2mWide<1, C>> = if regime_idx == 0 {
                gf2m_wide_1_matrix_from_seed::<C>(n, n, row_seed)
            } else {
                gf2m_wide_1_rank_deficient_from_seed::<C>(n, n, n / 2, row_seed)
            };
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = match op {
                "pluq" => time_op(
                    || {
                        let _ = std::hint::black_box(a.ple());
                    },
                    args.warmup,
                    args.iters,
                ),
                "echelon" => time_op(
                    || {
                        let _ = std::hint::black_box(a.row_echelon());
                    },
                    args.warmup,
                    args.iters,
                ),
                "invert" => time_op(
                    || {
                        let _ = std::hint::black_box(a.inv());
                    },
                    args.warmup,
                    args.iters,
                ),
                "solve" => {
                    let bvec_seed =
                        derive_seed(args.master_seed, "solve_rhs", op_idx, si as u64, regime_idx);
                    let b: FieldVec<Gf2mWide<1, C>> = gf2m_wide_1_vec_from_seed::<C>(n, bvec_seed);
                    time_op(
                        || {
                            let _ = std::hint::black_box(a.solve(&b));
                        },
                        args.warmup,
                        args.iters,
                    )
                }
                other => panic!("unknown op {other}"),
            };
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            sink.emit(
                op,
                field_label,
                n,
                n,
                n,
                regime,
                row_seed,
                wall_ns,
                tput(ops_cubic(n), wall_ns),
            )?;
        }
    }
    Ok(())
}

fn build_sparse_gf2m<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    density: f64,
    seed_val: u64,
) -> SparseFieldMatrix<Gf2mWide<1, C>> {
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    let mut st = seed_val;
    let mut m = FieldMatrix::<Gf2mWide<1, C>>::zeros(rows, cols);
    let threshold = (density * (u64::MAX as f64 + 1.0)) as u64;
    for r in 0..rows {
        for c in 0..cols {
            let draw = splitmix64(&mut st);
            if draw < threshold {
                let v_raw = splitmix64(&mut st) & mask;
                let v = if v_raw == 0 { 1 } else { v_raw };
                m.set(r, c, Gf2mWide::<1, C>::new([v]));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    eprintln!(
        "[gf2-csv] master_seed=0x{:016x} warmup={} iters={} output={}",
        args.master_seed,
        args.warmup,
        args.iters,
        args.output.display()
    );

    let mut sink = CsvSink::new(&args.output)?;

    run_fp::<PRIME_7>(&args, &mut sink, "GF(7)")?;
    run_fp::<PRIME_251>(&args, &mut sink, "GF(251)")?;
    run_fp::<PRIME_65521>(&args, &mut sink, "GF(65521)")?;
    run_fp::<MERSENNE_31>(&args, &mut sink, "GF(2^31-1)")?;
    run_gf2m::<EmitterGf2m8Cfg>(&args, &mut sink, "GF(2^8)")?;
    run_gf2m::<EmitterGf2m16Cfg>(&args, &mut sink, "GF(2^16)")?;

    eprintln!("[gf2-csv] wrote {}", args.output.display());
    Ok(())
}

// ─── Determinism doctest ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// First-row hash sanity check at master_seed = 0. Computed once
    /// from the Rust SplitMix64 implementation and pinned. Any change
    /// to the seed-derivation logic that breaks bit-identicality with
    /// the reference harness will trip this check.
    ///
    /// A separate cross-language test (in `tests/seed_compat.rs`)
    /// bridges this hash to the C reference's
    /// `seed_helpers.h` via independently-computable hardcoded values.
    #[test]
    fn first_row_hash_pinned_at_seed_0() {
        // Master seed = 0, tag="fgemm", op_idx=0, size_idx=0, regime_idx=0.
        let row_seed = derive_seed(0, "fgemm", 0, 0, 0);
        // First 4 SplitMix64 outputs from `row_seed`.
        let mut st = row_seed;
        let outs: [u64; 4] = [
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
        ];
        // Pinned values (computed from this Rust implementation; updates
        // require a corresponding C-reference cross-check). If you see
        // this fail, rerun the cross-language test in
        // `tests/seed_compat.rs` before changing the constants.
        assert_eq!(row_seed, 0xa1f5_dbf0_5125_7436);
        assert_eq!(outs[0], 0xc17b_957b_cba3_b185);
        assert_eq!(outs[1], 0x09c2_e9a9_f50d_f92d);
        assert_eq!(outs[2], 0xc8a2_51f5_c85e_fb2e);
        assert_eq!(outs[3], 0xab69_df17_63cf_f87a);
    }
}
