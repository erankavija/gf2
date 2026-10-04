//! Times every (operation, field, size, regime) cell with `std::time::Instant`
//! and writes one row per cell under `CSV_HEADER`, by default to
//! `bench_results/gf2-<timestamp>.csv`. Inputs derive from the master seed
//! through `benches/common/seed.rs`. A cell stops iterating once its measured
//! time reaches `CELL_BUDGET_NS` and reports `early_exit` on stderr. Flags:
//! `--seed`, `--warmup`, `--iters`, `--output` and `--filter` (a substring of
//! `<operation>/<field>/<n>/<regime>`).

#[path = "../benches/common/seed.rs"]
mod seed;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::vec::FieldVec;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

use seed::{
    bitmatrix_from_seed, bitmatrix_rank_deficient_from_seed, bitmatrix_sparse_from_seed,
    bitvec_from_seed, derive_seed, fp_matrix_from_seed, fp_rank_deficient_from_seed,
    fp_sparse_from_seed, fp_vec_from_seed, gf2m_wide_1_matrix_from_seed,
    gf2m_wide_1_rank_deficient_from_seed, gf2m_wide_1_sparse_from_seed, gf2m_wide_1_vec_from_seed,
    ops_cubic, ops_gemm, ops_quartic, tput, CSV_HEADER,
};

const PRIME_7: u64 = 7;
const PRIME_31: u64 = 31;
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

/// GF(2^32) with the Conway polynomial `f_{2,32}` of `@/citation/Lubeck2024`,
/// `x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1`; the leading term is implicit.
struct EmitterGf2m32Cfg;
impl Gf2mWideConfig<1> for EmitterGf2m32Cfg {
    const M: usize = 32;
    const MODULUS: [u64; 1] = [0x0000_8299];
    const NAME: &'static str = "Gf2m32Conway";
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

const SQUARE_SIZES: &[usize] = &[64, 256, 1024, 4096];
/// Equals `RECT_SHAPES` in `benches/fieldmatrix_gemm.rs`; shape `rsi` has
/// `size_idx = SQUARE_SIZES.len() + rsi`.
const RECT_SHAPES: &[(usize, usize, usize)] = &[(1024, 1024, 32), (1024, 1024, 8)];
const CHARPOLY_SIZES: &[usize] = &[32, 128, 512];
/// Subset of `CHARPOLY_SIZES` without `n = 512`.
const MINPOLY_SIZES: &[usize] = &[32, 128];
const SPMV_SIZES: &[usize] = &[256, 1024, 4096];
const SPMV_DENSITIES: &[(f64, &str)] = &[(0.01, "0.01"), (0.05, "0.05")];

fn run_fp<const P: u64>(args: &Args, sink: &mut CsvSink, field_label: &str) -> std::io::Result<()> {
    let warmup = args.warmup;
    let iters = args.iters;
    let master = args.master_seed;
    let filter = &args.filter;

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

    for (op_idx, op) in [("pluq", 1u64), ("echelon", 2), ("invert", 3), ("solve", 4)] {
        let _ = op_idx;
        run_fp_factorisation::<P>(args, sink, field_label, op_idx, op)?;
    }

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

    for (si, &n) in MINPOLY_SIZES.iter().enumerate() {
        let key = cell_key("minpoly", field_label, n, "uniform");
        if !cell_passes(filter, &key) {
            continue;
        }
        let seed_a = derive_seed(master, "minpoly", 10, si as u64, 0);
        let a = fp_matrix_from_seed::<P>(n, n, seed_a);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(a.minpoly());
            },
            warmup,
            iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "minpoly",
            field_label,
            n,
            n,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_quartic(n), wall_ns),
        )?;
    }

    for (di, &(density, density_label)) in SPMV_DENSITIES.iter().enumerate() {
        for (si, &n) in SPMV_SIZES.iter().enumerate() {
            let regime = format!("density_{density_label}");
            let key = cell_key("spmv", field_label, n, &regime);
            if !cell_passes(filter, &key) {
                continue;
            }
            let row_seed = derive_seed(master, "spmv", 11, si as u64, di as u64);
            let vec_seed = derive_seed(master, "spmv_vec", 11, si as u64, di as u64);
            let a = fp_sparse_from_seed::<P>(n, n, density, row_seed);
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

fn run_gf2m<C: Gf2mWideConfig<1>>(
    args: &Args,
    sink: &mut CsvSink,
    field_label: &str,
) -> std::io::Result<()> {
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

    for (op_idx, op) in [("pluq", 1u64), ("echelon", 2), ("invert", 3), ("solve", 4)] {
        let _ = op_idx;
        run_gf2m_factorisation::<C>(args, sink, field_label, op_idx, op)?;
    }

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

    for (si, &n) in MINPOLY_SIZES.iter().enumerate() {
        let key = cell_key("minpoly", field_label, n, "uniform");
        if !cell_passes(&args.filter, &key) {
            continue;
        }
        let seed_a = derive_seed(args.master_seed, "minpoly", 10, si as u64, 0);
        let a: FieldMatrix<Gf2mWide<1, C>> = gf2m_wide_1_matrix_from_seed::<C>(n, n, seed_a);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let _ = std::hint::black_box(a.minpoly());
            },
            args.warmup,
            args.iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "minpoly",
            field_label,
            n,
            n,
            n,
            "uniform",
            seed_a,
            wall_ns,
            tput(ops_quartic(n), wall_ns),
        )?;
    }

    for (di, &(density, density_label)) in SPMV_DENSITIES.iter().enumerate() {
        for (si, &n) in SPMV_SIZES.iter().enumerate() {
            let regime = format!("density_{density_label}");
            let key = cell_key("spmv", field_label, n, &regime);
            if !cell_passes(&args.filter, &key) {
                continue;
            }
            let row_seed = derive_seed(args.master_seed, "spmv", 11, si as u64, di as u64);
            let vec_seed = derive_seed(args.master_seed, "spmv_vec", 11, si as u64, di as u64);
            let a = gf2m_wide_1_sparse_from_seed::<C>(n, n, density, row_seed);
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

/// GF(2) `BitMatrix` cells. Matmul and echelon sizes follow
/// `benchmarks/reference/m4ri_bench.c` (`@/citation/AlbrechtBard2026`).
fn run_bitmatrix(args: &Args, sink: &mut CsvSink) -> std::io::Result<()> {
    let warmup = args.warmup;
    let iters = args.iters;
    let master = args.master_seed;
    let filter = &args.filter;
    let field_label = "GF(2)";

    const MATMUL_SIZES: &[usize] = &[64, 256, 1024, 4096];
    for (si, &n) in MATMUL_SIZES.iter().enumerate() {
        for (ri, &(regime, deficient)) in
            [("uniform", false), ("deficient", true)].iter().enumerate()
        {
            let key = cell_key("matmul", field_label, n, regime);
            if !cell_passes(filter, &key) {
                continue;
            }
            let seed_a = derive_seed(master, "matmul", 12, si as u64, ri as u64);
            let seed_b = derive_seed(master, "matmul_b", 12, si as u64, ri as u64);
            let a = if deficient {
                bitmatrix_rank_deficient_from_seed(n, n, n / 2, seed_a)
            } else {
                bitmatrix_from_seed(n, n, seed_a)
            };
            let b = bitmatrix_from_seed(n, n, seed_b);
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = time_op(
                || {
                    let _ = std::hint::black_box(gf2_core::alg::m4rm::multiply(&a, &b));
                },
                warmup,
                iters,
            );
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            sink.emit(
                "matmul",
                field_label,
                n,
                n,
                n,
                regime,
                seed_a,
                wall_ns,
                tput(ops_gemm(n, n, n), wall_ns),
            )?;
        }
    }

    const ECHELON_SIZES: &[usize] = &[64, 256, 1024];
    for (si, &n) in ECHELON_SIZES.iter().enumerate() {
        for (ri, &(regime, deficient)) in
            [("uniform", false), ("deficient", true)].iter().enumerate()
        {
            let key = cell_key("echelon", field_label, n, regime);
            if !cell_passes(filter, &key) {
                continue;
            }
            let seed_a = derive_seed(master, "echelon", 13, si as u64, ri as u64);
            let a = if deficient {
                bitmatrix_rank_deficient_from_seed(n, n, n / 2, seed_a)
            } else {
                bitmatrix_from_seed(n, n, seed_a)
            };
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = time_op(
                || {
                    let r = std::hint::black_box(gf2_core::alg::rref::rref(
                        std::hint::black_box(&a),
                        false,
                    ));
                    std::hint::black_box(r);
                },
                warmup,
                iters,
            );
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            sink.emit(
                "echelon",
                field_label,
                n,
                n,
                n,
                regime,
                seed_a,
                wall_ns,
                tput(ops_cubic(n), wall_ns),
            )?;
        }
    }

    // Uniform only: the deficient regime is non-invertible by construction.
    const INVERT_SIZES: &[usize] = &[64, 256, 1024];
    for (si, &n) in INVERT_SIZES.iter().enumerate() {
        let key = cell_key("invert", field_label, n, "uniform");
        if !cell_passes(filter, &key) {
            continue;
        }
        let seed_a = derive_seed(master, "invert", 14, si as u64, 0);
        let a = bitmatrix_from_seed(n, n, seed_a);
        eprintln!("[gf2-csv] {key}");
        let (wall_ns, early) = time_op(
            || {
                let r =
                    std::hint::black_box(gf2_core::alg::gauss::invert(std::hint::black_box(&a)));
                std::hint::black_box(r);
            },
            warmup,
            iters,
        );
        if early {
            eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
        }
        sink.emit(
            "invert",
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

    for (di, &(density, density_label)) in SPMV_DENSITIES.iter().enumerate() {
        for (si, &n) in SPMV_SIZES.iter().enumerate() {
            let regime = format!("density_{density_label}");
            let key = cell_key("spmv", field_label, n, &regime);
            if !cell_passes(filter, &key) {
                continue;
            }
            let row_seed = derive_seed(master, "spmv", 11, si as u64, di as u64);
            let vec_seed = derive_seed(master, "spmv_vec", 11, si as u64, di as u64);
            let a = bitmatrix_sparse_from_seed(n, n, density, row_seed);
            let nnz = a.nnz() as f64;
            let x = bitvec_from_seed(n, vec_seed);
            eprintln!("[gf2-csv] {key}");
            let (wall_ns, early) = time_op(
                || {
                    let y = std::hint::black_box(&a).matvec(std::hint::black_box(&x));
                    std::hint::black_box(y);
                },
                warmup,
                iters,
            );
            if early {
                eprintln!("[gf2-csv] WARN early_exit {key} wall_ns={wall_ns}");
            }
            sink.emit(
                "spmv",
                field_label,
                n,
                n,
                1,
                &regime,
                row_seed,
                wall_ns,
                tput(nnz, wall_ns),
            )?;
        }
    }

    Ok(())
}

/// GF(2^32) square matmul cells. The master seed is XORed with `0x77` before
/// per-cell derivation, as in `benchmarks/reference/ntl_bench.cpp`
/// (`@/citation/Shoup2025`).
fn run_gf2m32_matmul(args: &Args, sink: &mut CsvSink) -> std::io::Result<()> {
    const GF2M32_SIZES: &[usize] = &[64, 256, 1024];
    let field_label = "GF(2^32)";
    let salted_master = args.master_seed ^ 0x77u64;
    for (si, &n) in GF2M32_SIZES.iter().enumerate() {
        let key = cell_key("matmul", field_label, n, "uniform");
        if !cell_passes(&args.filter, &key) {
            continue;
        }
        let seed_a = derive_seed(salted_master, "matmul", 0, si as u64, 0);
        let seed_b = seed_a ^ 0x1111_1111_1111_1111u64;
        let a: FieldMatrix<Gf2mWide<1, EmitterGf2m32Cfg>> =
            gf2m_wide_1_matrix_from_seed::<EmitterGf2m32Cfg>(n, n, seed_a);
        let b: FieldMatrix<Gf2mWide<1, EmitterGf2m32Cfg>> =
            gf2m_wide_1_matrix_from_seed::<EmitterGf2m32Cfg>(n, n, seed_b);
        eprintln!("[gf2-csv] {key} seed={seed_a}");
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
            "matmul",
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
    Ok(())
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
    run_fp::<PRIME_31>(&args, &mut sink, "GF(31)")?;
    run_fp::<PRIME_251>(&args, &mut sink, "GF(251)")?;
    run_fp::<PRIME_65521>(&args, &mut sink, "GF(65521)")?;
    run_fp::<MERSENNE_31>(&args, &mut sink, "GF(2^31-1)")?;
    run_gf2m::<EmitterGf2m8Cfg>(&args, &mut sink, "GF(2^8)")?;
    run_gf2m::<EmitterGf2m16Cfg>(&args, &mut sink, "GF(2^16)")?;
    run_gf2m32_matmul(&args, &mut sink)?;
    run_bitmatrix(&args, &mut sink)?;

    eprintln!("[gf2-csv] wrote {}", args.output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the seed derivation at master seed 0. `tests/bench_seed_compat.rs`
    /// checks the same derivation against a port of the C reference.
    #[test]
    fn first_row_hash_pinned_at_seed_0() {
        let row_seed = derive_seed(0, "fgemm", 0, 0, 0);
        let mut st = row_seed;
        let outs: [u64; 4] = [
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
        ];
        assert_eq!(row_seed, 0xa1f5_dbf0_5125_7436);
        assert_eq!(outs[0], 0xc17b_957b_cba3_b185);
        assert_eq!(outs[1], 0x09c2_e9a9_f50d_f92d);
        assert_eq!(outs[2], 0xc8a2_51f5_c85e_fb2e);
        assert_eq!(outs[3], 0xab69_df17_63cf_f87a);
    }
}
