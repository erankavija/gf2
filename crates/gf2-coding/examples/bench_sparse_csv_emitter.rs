//! Emits gf2's sparse benchmark CSV rows (`spmv`, `sparse-matmul`, `sparse×dense`, `sparse-elim`)
//! over GF(2), GF(p) and GF(2^m) in the `gf2_core::bench_seed::CSV_HEADER` schema, with inputs
//! drawn from the `gf2_core::bench_seed` helpers. The `rank_regime` column carries
//! `density_<d>_<layout>` for random cells, `structured_<name>` for `--structured` cells and
//! `coding-theory_<name>` for `--coding-theory` cells.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
use gf2_coding::CodeRate;
use gf2_core::bench_seed::{
    bitmatrix_from_seed, bitmatrix_sparse_from_seed, bitvec_from_seed, derive_seed,
    fp_sparse_from_seed, fp_vec_from_seed, gf2m_wide_1_sparse_from_seed, gf2m_wide_1_vec_from_seed,
    splitmix64, tput, CSV_HEADER,
};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::sparse::SpBitMatrix;

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
    full: bool,
    coding_theory: bool,
    structured: bool,
    filter: Option<String>,
}

impl Args {
    fn parse() -> Self {
        let argv: Vec<String> = std::env::args().collect();
        let mut master_seed: u64 = 0x6F73_AC91_D31E_4A7C;
        let mut warmup: u32 = 1;
        let mut iters: u32 = 3;
        let mut output: Option<PathBuf> = None;
        let mut full = false;
        let mut coding_theory = false;
        let mut structured = false;
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
                "--full" => {
                    full = true;
                    i += 1;
                }
                "--coding-theory" => {
                    coding_theory = true;
                    i += 1;
                }
                "--structured" => {
                    structured = true;
                    i += 1;
                }
                "--filter" => {
                    filter = Some(argv[i + 1].clone());
                    i += 2;
                }
                "--quick" => {
                    full = false;
                    structured = false;
                    coding_theory = false;
                    i += 1;
                }
                "--help" | "-h" => {
                    eprintln!(
                        "Usage: bench_sparse_csv_emitter \
                         [--seed N] [--warmup K] [--iters K] \
                         [--output PATH] [--filter SUBSTRING] \
                         [--quick | --full] \
                         [--structured] [--coding-theory]"
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
            full,
            coding_theory,
            structured,
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
    PathBuf::from(format!("bench_results/gf2-sparse-{secs}.csv"))
}

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

fn cell_passes(filter: &Option<String>, key: &str) -> bool {
    match filter {
        Some(f) => key.contains(f),
        None => true,
    }
}

/// Formats `density` as C `%.6e` (two-digit exponent) so regime strings byte-match the C++
/// harnesses' `snprintf` output; Rust's `{:.6e}` writes `e-3` where C writes `e-03`.
fn fmt_density_c(density: f64) -> String {
    let raw = format!("{density:.6e}");
    if let Some((m, e)) = raw.split_once('e') {
        let (sign, digits) = if let Some(stripped) = e.strip_prefix('-') {
            ("-", stripped)
        } else if let Some(stripped) = e.strip_prefix('+') {
            ("+", stripped)
        } else {
            ("+", e)
        };
        let digits_padded = if digits.len() < 2 {
            format!("0{digits}")
        } else {
            digits.to_string()
        };
        format!("{m}e{sign}{digits_padded}")
    } else {
        raw
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

fn run_gf2_random_er(args: &Args, sink: &mut CsvSink, sizes: &[usize]) -> std::io::Result<()> {
    let field = "GF(2)";
    // sparse-elim sizes and seeds match `linbox_sparse_bench --quick`, independent of `sizes`.
    for (si, &elim_n) in [256usize, 1024].iter().enumerate() {
        let elim_density = 10.0 / (elim_n as f64);
        let elim_key = format!("sparse-elim/{field}/{elim_n}/csr");
        if !cell_passes(&args.filter, &elim_key) {
            continue;
        }
        let elim_seed = derive_seed(args.master_seed, "spelim-er", 3, si as u64, 1);
        let m = bitmatrix_sparse_from_seed(elim_n, elim_n, elim_density, elim_seed);
        eprintln!("[gf2-sparse] {elim_key}");
        let (wall_e, _) = time_op(
            || {
                let _ = std::hint::black_box(m.rref());
            },
            args.warmup,
            args.iters,
        );
        sink.emit(
            "sparse-elim",
            field,
            elim_n,
            elim_n,
            elim_n,
            &format!("density_{}_csr", fmt_density_c(elim_density)),
            elim_seed,
            wall_e,
            tput((elim_n * elim_n * elim_n) as f64, wall_e),
        )?;
    }
    for (si, &n) in sizes.iter().enumerate() {
        let density = 10.0 / (n as f64);
        let regime = format!("density_{}_csr", fmt_density_c(density));
        let key = format!("spmv/{field}/{n}/csr");
        if !cell_passes(&args.filter, &key) {
            continue;
        }
        let row_seed = derive_seed(args.master_seed, "spmv-er", 0, si as u64, 1);
        let vec_seed = derive_seed(args.master_seed, "spmv-er-vec", 0, si as u64, 1);
        let a = bitmatrix_sparse_from_seed(n, n, density, row_seed);
        let nnz = a.nnz();
        let x = bitvec_from_seed(n, vec_seed);
        eprintln!("[gf2-sparse] {key}");
        let (wall, early) = time_op(
            || {
                let _ = std::hint::black_box(a.matvec(std::hint::black_box(&x)));
            },
            args.warmup,
            args.iters,
        );
        if early {
            eprintln!("[gf2-sparse] WARN early_exit {key} wall_ns={wall}");
        }
        sink.emit(
            "spmv",
            field,
            n,
            n,
            1,
            &regime,
            row_seed,
            wall,
            tput(nnz as f64, wall),
        )?;

        let dual_key = format!("spmv/{field}/{n}/csc");
        if cell_passes(&args.filter, &dual_key) {
            let dual = build_dual_from_csr(&a);
            eprintln!("[gf2-sparse] {dual_key}");
            let (wall_dual, _) = time_op(
                || {
                    let _ = std::hint::black_box(dual.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                &format!("density_{}_csc", fmt_density_c(density)),
                row_seed,
                wall_dual,
                tput(nnz as f64, wall_dual),
            )?;
        }

        let blk_key = format!("spmv/{field}/{n}/block-csr");
        if cell_passes(&args.filter, &blk_key) {
            let blocked = a.to_default_block_csr();
            eprintln!("[gf2-sparse] {blk_key}");
            let (wall_blk, _) = time_op(
                || {
                    let _ = std::hint::black_box(blocked.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                &format!("density_{}_block-csr", fmt_density_c(density)),
                row_seed,
                wall_blk,
                tput(nnz as f64, wall_blk),
            )?;

            let pf_key = format!("spmv/{field}/{n}/prefetch-d8");
            if cell_passes(&args.filter, &pf_key) {
                eprintln!("[gf2-sparse] {pf_key}");
                let (wall_pf, _) = time_op(
                    || {
                        let _ = std::hint::black_box(
                            blocked.matvec_with_prefetch_distance(std::hint::black_box(&x), 8),
                        );
                    },
                    args.warmup,
                    args.iters,
                );
                sink.emit(
                    "spmv",
                    field,
                    n,
                    n,
                    1,
                    &format!("density_{}_prefetch-d8", fmt_density_c(density)),
                    row_seed,
                    wall_pf,
                    tput(nnz as f64, wall_pf),
                )?;
            }
        }

        // The permutation and the permuted input are built outside the timer.
        let rcm_key = format!("spmv/{field}/{n}/rcm-reordered");
        if cell_passes(&args.filter, &rcm_key) {
            let (reordered, perm) = a.reorder_rcm();
            let x_perm = perm.apply_cols(&x);
            eprintln!("[gf2-sparse] {rcm_key}");
            let (wall_rcm, _) = time_op(
                || {
                    let _ = std::hint::black_box(reordered.matvec(std::hint::black_box(&x_perm)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                &format!("density_{}_rcm-reordered", fmt_density_c(density)),
                row_seed,
                wall_rcm,
                tput(nnz as f64, wall_rcm),
            )?;
        }

        let mm_key = format!("sparse-matmul/{field}/{n}/csr");
        if cell_passes(&args.filter, &mm_key) {
            let other_seed = derive_seed(args.master_seed, "spmm-er-b", 1, si as u64, 1);
            let b = bitmatrix_sparse_from_seed(n, n, density, other_seed);
            let nnz_total = (a.nnz() + b.nnz()) as f64;
            eprintln!("[gf2-sparse] {mm_key}");
            let (wall_mm, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matmul(std::hint::black_box(&b)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "sparse-matmul",
                field,
                n,
                n,
                n,
                &format!("density_{}_csr", fmt_density_c(density)),
                row_seed,
                wall_mm,
                tput(nnz_total, wall_mm),
            )?;
        }

        // Throughput counts `nnz(A) · n` operations: each nonzero XORs one n-bit row.
        let sd_key = format!("sparse×dense/{field}/{n}/csr");
        if cell_passes(&args.filter, &sd_key) {
            let dense_seed = derive_seed(args.master_seed, "spdn-b", 2, si as u64, 1);
            let b_dense = bitmatrix_from_seed(n, n, dense_seed);
            eprintln!("[gf2-sparse] {sd_key}");
            let (wall_sd, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matmat(std::hint::black_box(&b_dense)));
                },
                args.warmup,
                args.iters,
            );
            let work_ops = (a.nnz() as f64) * (n as f64);
            sink.emit(
                "sparse×dense",
                field,
                n,
                n,
                n,
                &format!("density_{}_csr", fmt_density_c(density)),
                row_seed,
                wall_sd,
                tput(work_ops, wall_sd),
            )?;
        }
    }

    Ok(())
}

fn build_dual_from_csr(csr: &SpBitMatrix) -> gf2_core::sparse::SpBitMatrixDual {
    let mut entries: Vec<(usize, usize)> = Vec::with_capacity(csr.nnz());
    for r in 0..csr.rows() {
        for c in csr.row_iter(r) {
            entries.push((r, c));
        }
    }
    gf2_core::sparse::SpBitMatrixDual::from_coo(csr.rows(), csr.cols(), &entries)
}

fn run_fp_random_er<const P: u64>(
    args: &Args,
    sink: &mut CsvSink,
    field_label: &str,
    sizes: &[usize],
) -> std::io::Result<()> {
    for (si, &elim_n) in [256usize, 1024].iter().enumerate() {
        let elim_density = 10.0 / (elim_n as f64);
        let elim_key = format!("sparse-elim/{field_label}/{elim_n}/csr");
        if !cell_passes(&args.filter, &elim_key) {
            continue;
        }
        let elim_seed = derive_seed(args.master_seed, "spelim-er", 3, si as u64, 1);
        let m = fp_sparse_from_seed::<P>(elim_n, elim_n, elim_density, elim_seed);
        eprintln!("[gf2-sparse] {elim_key}");
        let (wall_e, _) = time_op(
            || {
                let _ = std::hint::black_box(m.rref());
            },
            args.warmup,
            args.iters,
        );
        sink.emit(
            "sparse-elim",
            field_label,
            elim_n,
            elim_n,
            elim_n,
            &format!("density_{}_csr", fmt_density_c(elim_density)),
            elim_seed,
            wall_e,
            tput((elim_n * elim_n * elim_n) as f64, wall_e),
        )?;
    }
    for (si, &n) in sizes.iter().enumerate() {
        let density = 10.0 / (n as f64);
        let regime = format!("density_{}_csr", fmt_density_c(density));
        let row_seed = derive_seed(args.master_seed, "spmv-er", 0, si as u64, 1);
        let vec_seed = derive_seed(args.master_seed, "spmv-er-vec", 0, si as u64, 1);
        let a = fp_sparse_from_seed::<P>(n, n, density, row_seed);
        let nnz = a.nnz();
        let x = fp_vec_from_seed::<P>(n, vec_seed);

        let key = format!("spmv/{field_label}/{n}/csr");
        if cell_passes(&args.filter, &key) {
            eprintln!("[gf2-sparse] {key}");
            let (wall, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field_label,
                n,
                n,
                1,
                &regime,
                row_seed,
                wall,
                tput(nnz as f64, wall),
            )?;
        }

        let mm_key = format!("sparse-matmul/{field_label}/{n}/csr");
        if cell_passes(&args.filter, &mm_key) {
            let b_seed = derive_seed(args.master_seed, "spmm-er-b", 1, si as u64, 1);
            let b = fp_sparse_from_seed::<P>(n, n, density, b_seed);
            let nnz_total = (a.nnz() + b.nnz()) as f64;
            eprintln!("[gf2-sparse] {mm_key}");
            let (wall_mm, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matmul(std::hint::black_box(&b)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "sparse-matmul",
                field_label,
                n,
                n,
                n,
                &format!("density_{}_csr", fmt_density_c(density)),
                row_seed,
                wall_mm,
                tput(nnz_total, wall_mm),
            )?;
        }

        let sd_key = format!("sparse×dense/{field_label}/{n}/csr");
        if cell_passes(&args.filter, &sd_key) {
            let b_seed = derive_seed(args.master_seed, "spdn-b", 2, si as u64, 1);
            let b = build_fp_dense::<P>(n, n, b_seed);
            eprintln!("[gf2-sparse] {sd_key}");
            let (wall_sd, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matmat(std::hint::black_box(&b)));
                },
                args.warmup,
                args.iters,
            );
            let work_ops = (a.nnz() as f64) * (n as f64);
            sink.emit(
                "sparse×dense",
                field_label,
                n,
                n,
                n,
                &format!("density_{}_csr", fmt_density_c(density)),
                row_seed,
                wall_sd,
                tput(work_ops, wall_sd),
            )?;
        }
    }
    Ok(())
}

fn build_fp_dense<const P: u64>(
    rows: usize,
    cols: usize,
    seed: u64,
) -> FieldMatrix<gf2_core::gfp::Fp<P>> {
    let mut m = FieldMatrix::<gf2_core::gfp::Fp<P>>::zeros(rows, cols);
    let mut st = seed;
    for r in 0..rows {
        for c in 0..cols {
            let v = splitmix64(&mut st) % P;
            m.set(r, c, gf2_core::gfp::Fp::<P>::new(v));
        }
    }
    m
}

fn build_gf2m_dense<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    seed: u64,
) -> FieldMatrix<Gf2mWide<1, C>> {
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    let mut m = FieldMatrix::<Gf2mWide<1, C>>::zeros(rows, cols);
    let mut st = seed;
    for r in 0..rows {
        for c in 0..cols {
            let v = splitmix64(&mut st) & mask;
            m.set(r, c, Gf2mWide::<1, C>::new([v]));
        }
    }
    m
}

fn run_gf2m_random_er<C: Gf2mWideConfig<1>>(
    args: &Args,
    sink: &mut CsvSink,
    field_label: &str,
    sizes: &[usize],
) -> std::io::Result<()> {
    for (si, &n) in sizes.iter().enumerate() {
        let density = 10.0 / (n as f64);
        let regime = format!("density_{}_csr", fmt_density_c(density));
        let row_seed = derive_seed(args.master_seed, "spmv-er", 0, si as u64, 1);
        let vec_seed = derive_seed(args.master_seed, "spmv-er-vec", 0, si as u64, 1);
        let a = gf2m_wide_1_sparse_from_seed::<C>(n, n, density, row_seed);
        let nnz = a.nnz();
        let x: FieldVec<Gf2mWide<1, C>> = gf2m_wide_1_vec_from_seed::<C>(n, vec_seed);

        let key = format!("spmv/{field_label}/{n}/csr");
        if cell_passes(&args.filter, &key) {
            eprintln!("[gf2-sparse] {key}");
            let (wall, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field_label,
                n,
                n,
                1,
                &regime,
                row_seed,
                wall,
                tput(nnz as f64, wall),
            )?;
        }

        let mm_key = format!("sparse-matmul/{field_label}/{n}/csr");
        if cell_passes(&args.filter, &mm_key) {
            let b_seed = derive_seed(args.master_seed, "spmm-er-b", 1, si as u64, 1);
            let b = gf2m_wide_1_sparse_from_seed::<C>(n, n, density, b_seed);
            let nnz_total = (a.nnz() + b.nnz()) as f64;
            eprintln!("[gf2-sparse] {mm_key}");
            let (wall_mm, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matmul(std::hint::black_box(&b)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "sparse-matmul",
                field_label,
                n,
                n,
                n,
                &format!("density_{}_csr", fmt_density_c(density)),
                row_seed,
                wall_mm,
                tput(nnz_total, wall_mm),
            )?;
        }

        let sd_key = format!("sparse×dense/{field_label}/{n}/csr");
        if cell_passes(&args.filter, &sd_key) {
            let b_seed = derive_seed(args.master_seed, "spdn-b", 2, si as u64, 1);
            let b = build_gf2m_dense::<C>(n, n, b_seed);
            eprintln!("[gf2-sparse] {sd_key}");
            let (wall_sd, _) = time_op(
                || {
                    let _ = std::hint::black_box(a.matmat(std::hint::black_box(&b)));
                },
                args.warmup,
                args.iters,
            );
            let work_ops = (a.nnz() as f64) * (n as f64);
            sink.emit(
                "sparse×dense",
                field_label,
                n,
                n,
                n,
                &format!("density_{}_csr", fmt_density_c(density)),
                row_seed,
                wall_sd,
                tput(work_ops, wall_sd),
            )?;
        }

        // GF(2^m) sparse-elim runs under `--full` only; `linbox_sparse_bench.cpp` emits no
        // GF(2^m) sparse-elim reference rows.
        if args.full {
            let elim_n = 256;
            let elim_key = format!("sparse-elim/{field_label}/{elim_n}/csr");
            if cell_passes(&args.filter, &elim_key) {
                let elim_density = 10.0 / (elim_n as f64);
                let elim_seed = derive_seed(args.master_seed, "spelim-er", 3, si as u64, 1);
                let m = gf2m_wide_1_sparse_from_seed::<C>(elim_n, elim_n, elim_density, elim_seed);
                eprintln!("[gf2-sparse] {elim_key}");
                let (wall_e, _) = time_op(
                    || {
                        let _ = std::hint::black_box(m.rref());
                    },
                    args.warmup,
                    args.iters,
                );
                sink.emit(
                    "sparse-elim",
                    field_label,
                    elim_n,
                    elim_n,
                    elim_n,
                    &format!("density_{}_csr", fmt_density_c(elim_density)),
                    elim_seed,
                    wall_e,
                    tput((elim_n * elim_n * elim_n) as f64, wall_e),
                )?;
            }
        }
    }
    Ok(())
}

fn run_coding_theory(args: &Args, sink: &mut CsvSink) -> std::io::Result<()> {
    eprintln!("[gf2-sparse] === coding-theory class ===");

    let field = "GF(2)";

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let h = ldpc_h_to_csr(&code);
    let n_cols = code.n();
    let m_rows = code.m();
    eprintln!(
        "[gf2-sparse] dvb-t2-short-r1_2 H={}x{} nnz={}",
        m_rows,
        n_cols,
        h.nnz()
    );
    let x = bitvec_from_seed(
        n_cols,
        derive_seed(args.master_seed, "ct-x-dvb-short", 0, 0, 0),
    );
    let (wall, _) = time_op(
        || {
            let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
        },
        args.warmup,
        args.iters,
    );
    sink.emit(
        "spmv",
        field,
        m_rows,
        n_cols,
        1,
        "coding-theory_dvb-t2-short-r1_2",
        0,
        wall,
        tput(h.nnz() as f64, wall),
    )?;

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate2_3);
    let h = ldpc_h_to_csr(&code);
    let n_cols = code.n();
    let m_rows = code.m();
    eprintln!(
        "[gf2-sparse] dvb-t2-normal-r2_3 H={}x{} nnz={}",
        m_rows,
        n_cols,
        h.nnz()
    );
    let x = bitvec_from_seed(
        n_cols,
        derive_seed(args.master_seed, "ct-x-dvb-normal", 0, 1, 0),
    );
    let (wall, _) = time_op(
        || {
            let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
        },
        args.warmup,
        args.iters,
    );
    sink.emit(
        "spmv",
        field,
        m_rows,
        n_cols,
        1,
        "coding-theory_dvb-t2-normal-r2_3",
        0,
        wall,
        tput(h.nnz() as f64, wall),
    )?;

    let qc = QuasiCyclicLdpc::nr_5g(1, 384);
    let m_rows = qc.expanded_rows();
    let n_cols = qc.expanded_cols();
    let h = SpBitMatrix::from_coo(m_rows, n_cols, &qc.to_edges());
    eprintln!(
        "[gf2-sparse] nr-5g-bg1-z384 H={}x{} nnz={}",
        m_rows,
        n_cols,
        h.nnz()
    );
    let x = bitvec_from_seed(n_cols, derive_seed(args.master_seed, "ct-x-bg1", 0, 2, 0));
    let (wall, _) = time_op(
        || {
            let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
        },
        args.warmup,
        args.iters,
    );
    sink.emit(
        "spmv",
        field,
        m_rows,
        n_cols,
        1,
        "coding-theory_nr-5g-bg1-z384",
        0,
        wall,
        tput(h.nnz() as f64, wall),
    )?;

    let qc = QuasiCyclicLdpc::nr_5g(2, 208);
    let m_rows = qc.expanded_rows();
    let n_cols = qc.expanded_cols();
    let h = SpBitMatrix::from_coo(m_rows, n_cols, &qc.to_edges());
    eprintln!(
        "[gf2-sparse] nr-5g-bg2-z208 H={}x{} nnz={}",
        m_rows,
        n_cols,
        h.nnz()
    );
    let x = bitvec_from_seed(n_cols, derive_seed(args.master_seed, "ct-x-bg2", 0, 3, 0));
    let (wall, _) = time_op(
        || {
            let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
        },
        args.warmup,
        args.iters,
    );
    sink.emit(
        "spmv",
        field,
        m_rows,
        n_cols,
        1,
        "coding-theory_nr-5g-bg2-z208",
        0,
        wall,
        tput(h.nnz() as f64, wall),
    )?;

    Ok(())
}

/// Recovers H column by column: column j is the syndrome of the unit vector e_j.
fn ldpc_h_to_csr(code: &LdpcCode) -> SpBitMatrix {
    let n_cols = code.n();
    let m_rows = code.m();
    let mut entries: Vec<(usize, usize)> = Vec::new();
    for j in 0..n_cols {
        let mut e = gf2_core::BitVec::zeros(n_cols);
        e.set(j, true);
        let s = code.syndrome(&e);
        for i in 0..m_rows {
            if s.get(i) {
                entries.push((i, j));
            }
        }
    }
    SpBitMatrix::from_coo(m_rows, n_cols, &entries)
}

fn run_structured(args: &Args, sink: &mut CsvSink, sizes: &[usize]) -> std::io::Result<()> {
    eprintln!("[gf2-sparse] === structured class (GF(2) only in --quick) ===");
    let field = "GF(2)";

    for (si, &n) in sizes.iter().enumerate() {
        let key = format!("spmv/{field}/{n}/banded-w8");
        if cell_passes(&args.filter, &key) {
            let h = build_banded(n, 8);
            let x = bitvec_from_seed(
                n,
                derive_seed(args.master_seed, "struct-banded-x", 0, si as u64, 0),
            );
            eprintln!("[gf2-sparse] {key} nnz={}", h.nnz());
            let (wall, _) = time_op(
                || {
                    let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                "structured_banded-w8",
                0,
                wall,
                tput(h.nnz() as f64, wall),
            )?;
        }

        let key = format!("spmv/{field}/{n}/banded-w64");
        if cell_passes(&args.filter, &key) {
            let h = build_banded(n, 64);
            let x = bitvec_from_seed(
                n,
                derive_seed(args.master_seed, "struct-banded64-x", 0, si as u64, 0),
            );
            eprintln!("[gf2-sparse] {key} nnz={}", h.nnz());
            let (wall, _) = time_op(
                || {
                    let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                "structured_banded-w64",
                0,
                wall,
                tput(h.nnz() as f64, wall),
            )?;
        }

        let key = format!("spmv/{field}/{n}/circulant-w8");
        if cell_passes(&args.filter, &key) {
            let h = build_circulant(
                n,
                8,
                derive_seed(args.master_seed, "struct-circulant-c", 0, si as u64, 0),
            );
            let x = bitvec_from_seed(
                n,
                derive_seed(args.master_seed, "struct-circulant-x", 0, si as u64, 0),
            );
            eprintln!("[gf2-sparse] {key} nnz={}", h.nnz());
            let (wall, _) = time_op(
                || {
                    let _ = std::hint::black_box(h.matvec(std::hint::black_box(&x)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                "structured_circulant-w8",
                0,
                wall,
                tput(h.nnz() as f64, wall),
            )?;
        }

        // Same matrix and seeds as the random class's RCM row, under the structured regime label.
        let key = format!("spmv/{field}/{n}/rcm-permuted-er");
        if cell_passes(&args.filter, &key) {
            let density = 10.0 / (n as f64);
            let row_seed = derive_seed(args.master_seed, "spmv-er", 0, si as u64, 1);
            let vec_seed = derive_seed(args.master_seed, "spmv-er-vec", 0, si as u64, 1);
            let a = bitmatrix_sparse_from_seed(n, n, density, row_seed);
            let (reordered, perm) = a.reorder_rcm();
            let x = bitvec_from_seed(n, vec_seed);
            let x_perm = perm.apply_cols(&x);
            eprintln!("[gf2-sparse] {key} nnz={}", reordered.nnz());
            let (wall, _) = time_op(
                || {
                    let _ = std::hint::black_box(reordered.matvec(std::hint::black_box(&x_perm)));
                },
                args.warmup,
                args.iters,
            );
            sink.emit(
                "spmv",
                field,
                n,
                n,
                1,
                "structured_rcm-permuted-er",
                row_seed,
                wall,
                tput(reordered.nnz() as f64, wall),
            )?;
        }
    }
    Ok(())
}

fn build_banded(n: usize, bandwidth: usize) -> SpBitMatrix {
    let mut entries: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        let lo = i.saturating_sub(bandwidth);
        let hi = (i + bandwidth + 1).min(n);
        for j in lo..hi {
            entries.push((i, j));
        }
    }
    SpBitMatrix::from_coo(n, n, &entries)
}

fn build_circulant(n: usize, weight: usize, seed: u64) -> SpBitMatrix {
    let mut st = seed;
    let mut offsets: Vec<usize> = Vec::with_capacity(weight);
    while offsets.len() < weight {
        let o = (splitmix64(&mut st) % (n as u64)) as usize;
        if !offsets.contains(&o) {
            offsets.push(o);
        }
    }
    let mut entries: Vec<(usize, usize)> = Vec::with_capacity(n * weight);
    for i in 0..n {
        for &o in offsets.iter() {
            let j = (i + o) % n;
            entries.push((i, j));
        }
    }
    SpBitMatrix::from_coo(n, n, &entries)
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    eprintln!(
        "[gf2-sparse] master_seed=0x{:016x} warmup={} iters={} output={} mode={}",
        args.master_seed,
        args.warmup,
        args.iters,
        args.output.display(),
        if args.full { "full" } else { "quick" }
    );

    let mut sink = CsvSink::new(&args.output)?;

    let sizes: &[usize] = if args.full {
        &[1024, 4096, 16384]
    } else {
        &[1024]
    };

    run_gf2_random_er(&args, &mut sink, sizes)?;
    run_fp_random_er::<PRIME_7>(&args, &mut sink, "GF(7)", sizes)?;
    run_fp_random_er::<PRIME_251>(&args, &mut sink, "GF(251)", sizes)?;
    run_fp_random_er::<PRIME_65521>(&args, &mut sink, "GF(65521)", sizes)?;
    run_fp_random_er::<MERSENNE_31>(&args, &mut sink, "GF(2^31-1)", sizes)?;
    run_gf2m_random_er::<EmitterGf2m8Cfg>(&args, &mut sink, "GF(2^8)", sizes)?;
    run_gf2m_random_er::<EmitterGf2m16Cfg>(&args, &mut sink, "GF(2^16)", sizes)?;

    if args.structured || args.full {
        run_structured(&args, &mut sink, sizes)?;
    }

    if args.coding_theory || args.full {
        run_coding_theory(&args, &mut sink)?;
    }

    eprintln!("[gf2-sparse] wrote {}", args.output.display());
    Ok(())
}
