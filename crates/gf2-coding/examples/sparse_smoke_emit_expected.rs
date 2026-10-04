//! Ground-truth emitter for `benchmarks/reference/sparse_smoke.cpp`: for each
//! `(op, field)` cell it builds the seeded input of the C++ harness, runs the
//! gf2-core operation, and writes input and expected output to a binary file
//! that the harness compares byte for byte.
//!
//! # File format (little-endian)
//!
//! ```text
//! magic   : 8 bytes ASCII "GF2SMK01"
//! n_cells : u32
//! for each cell:
//!   tag_len : u16
//!   tag     : tag_len bytes UTF-8 (e.g. "spmv,GF(2)")
//!   seed    : u64
//!   in_len  : u32
//!   in      : in_len bytes (canonical input — see per-op layout below)
//!   out_len : u32
//!   out     : out_len bytes (canonical expected output)
//! ```
//!
//! Per-op input/output layout (LE u64 for every value field):
//!
//! - `spmv,<F>` :
//!   - `in`  : `nnz: u64`, `nnz × {row: u64, col: u64, val: u64}`,
//!     `n: u64`, `n × val: u64` (RHS vector x).
//!   - `out` : `n: u64`, `n × val: u64` (output vector y = A·x).
//!
//! - `sparse_dense,<F>` :
//!   - `in`  : `nnz: u64`, `nnz × {row: u64, col: u64, val: u64}`,
//!     `n: u64`, `cols_b: u64`, `n*cols_b × val: u64` (B row-major).
//!   - `out` : `m: u64`, `cols_c: u64`, `m*cols_c × val: u64` (C row-major).
//!
//! - `sparse_matmul,<F>` :
//!   - `in`  : `nnz_a: u64`, `nnz_a × {row: u64, col: u64, val: u64}`,
//!     `nnz_b: u64`, `nnz_b × {row: u64, col: u64, val: u64}`.
//!   - `out` : `m: u64`, `n: u64`, `m*n × val: u64` (dense, row-major).
//!
//! - `sparse_elim,<F>` :
//!   - `in`  : `nnz: u64`, `nnz × {row: u64, col: u64, val: u64}`.
//!   - `out` : `rank: u64`, `m: u64`, `n: u64`,
//!     `m*n × val: u64` (full RREF dense, row-major).
//!
//! For GF(2^m) cells the `val` field carries the canonical word-0 of
//! `Gf2mWide<1, _>` — i.e. the `M`-bit polynomial coefficient packed
//! into the LSBs of a u64. For GF(2) the `val` field is 0 or 1
//! (the bit value). For GF(p) the `val` field is the canonical
//! representative in `[0, p)`.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use gf2_core::bench_seed::{derive_seed, splitmix64};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SpBitMatrix;
use gf2_core::BitVec;

// The moduli match `GF2M8` and `GF2M16` in `sparse_smoke.cpp`.

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

/// Default of `sparse_smoke.cpp` and the value in `benchmarks/seeds/seed.txt`.
const DEFAULT_MASTER_SEED: u64 = 0x6F73_AC91_D31E_4A7C;

/// Cell parameters shared with the C++ smoke harness.
const CELL_N: usize = 16;
const CELL_DENSITY: f64 = 0.25;

const MAGIC: &[u8; 8] = b"GF2SMK01";

#[derive(Clone)]
struct Args {
    master_seed: u64,
    output: PathBuf,
}

impl Args {
    fn parse() -> Self {
        let argv: Vec<String> = std::env::args().collect();
        let mut master_seed = DEFAULT_MASTER_SEED;
        let mut output = PathBuf::from("benchmarks/expected/sparse_smoke_n16.bin");
        let mut i = 1;
        while i < argv.len() {
            match argv[i].as_str() {
                "--output" => {
                    output = PathBuf::from(
                        argv.get(i + 1)
                            .expect("--output requires an argument")
                            .clone(),
                    );
                    i += 2;
                }
                "--master-seed" => {
                    let s = argv.get(i + 1).expect("--master-seed requires an argument");
                    master_seed = parse_u64(s);
                    i += 2;
                }
                "--help" | "-h" => {
                    eprintln!(
                        "Usage: sparse_smoke_emit_expected \
                         [--output PATH] [--master-seed N]\n\
                         \n\
                         Defaults: master_seed=0x{DEFAULT_MASTER_SEED:016x}, \
                         output=benchmarks/expected/sparse_smoke_n16.bin"
                    );
                    std::process::exit(0);
                }
                other => panic!("Unknown argument: {other}"),
            }
        }
        Args {
            master_seed,
            output,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Triple {
    row: u64,
    col: u64,
    val: u64,
}

/// Mirrors `build_csr` in `benchmarks/reference/sparse_smoke.cpp` draw for
/// draw; the value draw is taken for `card = 2` too, where it is always 1.
fn build_csr_cpp_walk(seed: u64, n: usize, density: f64, card: u64) -> Vec<Triple> {
    let mut st = seed;
    let mut triples = Vec::new();
    let threshold = (density * 1.844_674_407_370_955e19) as u64;
    for i in 0..n {
        for j in 0..n {
            let draw = splitmix64(&mut st);
            if draw < threshold {
                let v_raw = splitmix64(&mut st);
                let v = (v_raw % (card - 1)) + 1;
                triples.push(Triple {
                    row: i as u64,
                    col: j as u64,
                    val: v,
                });
            }
        }
    }
    triples
}

/// Mirrors the `x` initialiser of `oracle_spmv` in `sparse_smoke.cpp`.
fn build_dense_vec_cpp_walk(seed: u64, n: usize, card: u64) -> Vec<u64> {
    let mut st = seed ^ 0xCAFE_BABE_u64;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let r = splitmix64(&mut st);
        v.push(r % card);
    }
    v
}

/// Mirrors the `B` initialiser of `oracle_sparse_dense` in `sparse_smoke.cpp`.
fn build_dense_mat_cpp_walk(seed: u64, rows: usize, cols: usize, card: u64) -> Vec<u64> {
    let mut st = seed ^ 0xDEAD_BEEF_u64;
    let mut v = Vec::with_capacity(rows * cols);
    for _ in 0..(rows * cols) {
        let r = splitmix64(&mut st);
        v.push(r % card);
    }
    v
}

/// GF(2^m) variant of [`build_csr_cpp_walk`]; a zero masked draw becomes 1.
/// Mirrors `build_csr_gf2m` in `sparse_smoke.cpp` and
/// `gf2_core::bench_seed::gf2m_wide_1_sparse_from_seed`.
fn build_csr_gf2m_walk(seed: u64, n: usize, density: f64, m_bits: usize) -> Vec<Triple> {
    let mask: u64 = if m_bits >= 64 {
        u64::MAX
    } else {
        (1u64 << m_bits) - 1
    };
    let mut st = seed;
    let mut triples = Vec::new();
    let threshold = (density * 1.844_674_407_370_955e19) as u64;
    for i in 0..n {
        for j in 0..n {
            let draw = splitmix64(&mut st);
            if draw < threshold {
                let v_raw = splitmix64(&mut st) & mask;
                let v = if v_raw == 0 { 1 } else { v_raw };
                triples.push(Triple {
                    row: i as u64,
                    col: j as u64,
                    val: v,
                });
            }
        }
    }
    triples
}

/// GF(2^m) variant of [`build_dense_mat_cpp_walk`]; zero draws stay zero.
fn build_dense_mat_gf2m_walk(seed: u64, rows: usize, cols: usize, m_bits: usize) -> Vec<u64> {
    let mask: u64 = if m_bits >= 64 {
        u64::MAX
    } else {
        (1u64 << m_bits) - 1
    };
    let mut st = seed ^ 0xDEAD_BEEF_u64;
    let mut v = Vec::with_capacity(rows * cols);
    for _ in 0..(rows * cols) {
        let r = splitmix64(&mut st);
        v.push(r & mask);
    }
    v
}

struct Cell {
    tag: String,
    seed: u64,
    input: Vec<u8>,
    output: Vec<u8>,
}

fn write_cells(path: &PathBuf, cells: &[Cell]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(MAGIC)?;
    out.write_all(&(cells.len() as u32).to_le_bytes())?;
    for cell in cells {
        let tag_bytes = cell.tag.as_bytes();
        let tag_len: u16 = tag_bytes
            .len()
            .try_into()
            .expect("cell tag length must fit in u16");
        out.write_all(&tag_len.to_le_bytes())?;
        out.write_all(tag_bytes)?;
        out.write_all(&cell.seed.to_le_bytes())?;
        let in_len: u32 = cell
            .input
            .len()
            .try_into()
            .expect("cell input length must fit in u32");
        out.write_all(&in_len.to_le_bytes())?;
        out.write_all(&cell.input)?;
        let out_len: u32 = cell
            .output
            .len()
            .try_into()
            .expect("cell output length must fit in u32");
        out.write_all(&out_len.to_le_bytes())?;
        out.write_all(&cell.output)?;
    }
    out.flush()?;
    Ok(())
}

fn append_triples(buf: &mut Vec<u8>, triples: &[Triple]) {
    buf.extend_from_slice(&(triples.len() as u64).to_le_bytes());
    for t in triples {
        buf.extend_from_slice(&t.row.to_le_bytes());
        buf.extend_from_slice(&t.col.to_le_bytes());
        buf.extend_from_slice(&t.val.to_le_bytes());
    }
}

fn append_vec(buf: &mut Vec<u8>, vals: &[u64]) {
    buf.extend_from_slice(&(vals.len() as u64).to_le_bytes());
    for v in vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

fn append_dense_mat(buf: &mut Vec<u8>, rows: usize, cols: usize, vals: &[u64]) {
    assert_eq!(vals.len(), rows * cols);
    buf.extend_from_slice(&(rows as u64).to_le_bytes());
    buf.extend_from_slice(&(cols as u64).to_le_bytes());
    for v in vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

/// The value field is ignored: every triple is the bit 1.
fn gf2_csr_from_triples(rows: usize, cols: usize, triples: &[Triple]) -> SpBitMatrix {
    let coo: Vec<(usize, usize)> = triples
        .iter()
        .map(|t| (t.row as usize, t.col as usize))
        .collect();
    SpBitMatrix::from_coo(rows, cols, &coo)
}

fn fp_csr_from_triples<const P: u64>(
    rows: usize,
    cols: usize,
    triples: &[Triple],
) -> SparseFieldMatrix<Fp<P>> {
    let triplets = triples
        .iter()
        .map(|t| (t.row as usize, t.col as usize, Fp::<P>::new(t.val)));
    SparseFieldMatrix::<Fp<P>>::from_triplets(rows, cols, triplets)
}

fn fp_dense_to_u64<const P: u64>(m: &FieldMatrix<Fp<P>>) -> Vec<u64> {
    let rows = m.rows();
    let cols = m.cols();
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            out.push(m.get(r, c).value());
        }
    }
    out
}

fn fp_sparse_to_dense_u64<const P: u64>(m: &SparseFieldMatrix<Fp<P>>) -> Vec<u64> {
    let rows = m.rows();
    let cols = m.cols();
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            out.push(m.get(r, c).value());
        }
    }
    out
}

fn bitmatrix_to_u64(m: &BitMatrix) -> Vec<u64> {
    let rows = m.rows();
    let cols = m.cols();
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            out.push(if m.get(r, c) { 1 } else { 0 });
        }
    }
    out
}

fn bitvec_to_u64(v: &BitVec) -> Vec<u64> {
    let n = v.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(if v.get(i) { 1 } else { 0 });
    }
    out
}

fn fp_vec_to_u64<const P: u64>(v: &FieldVec<Fp<P>>) -> Vec<u64> {
    let mut out = Vec::with_capacity(v.len());
    for i in 0..v.len() {
        out.push(v.get(i).value());
    }
    out
}

fn u64_to_bitmatrix(rows: usize, cols: usize, vals: &[u64]) -> BitMatrix {
    let mut m = BitMatrix::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if vals[r * cols + c] != 0 {
                m.set(r, c, true);
            }
        }
    }
    m
}

fn u64_to_bitvec(vals: &[u64]) -> BitVec {
    let mut v = BitVec::zeros(vals.len());
    for (i, &x) in vals.iter().enumerate() {
        if x != 0 {
            v.set(i, true);
        }
    }
    v
}

fn u64_to_fp_matrix<const P: u64>(rows: usize, cols: usize, vals: &[u64]) -> FieldMatrix<Fp<P>> {
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(vals[r * cols + c]));
        }
    }
    m
}

fn u64_to_fp_vec<const P: u64>(vals: &[u64]) -> FieldVec<Fp<P>> {
    let mut v = FieldVec::<Fp<P>>::zeros(vals.len());
    for (i, &x) in vals.iter().enumerate() {
        v.set(i, Fp::<P>::new(x));
    }
    v
}

fn gf2m_csr_from_triples<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    triples: &[Triple],
) -> SparseFieldMatrix<Gf2mWide<1, C>> {
    let triplets = triples.iter().map(|t| {
        (
            t.row as usize,
            t.col as usize,
            Gf2mWide::<1, C>::new([t.val]),
        )
    });
    SparseFieldMatrix::<Gf2mWide<1, C>>::from_triplets(rows, cols, triplets)
}

fn u64_to_gf2m_matrix<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    vals: &[u64],
) -> FieldMatrix<Gf2mWide<1, C>> {
    let mut m = FieldMatrix::<Gf2mWide<1, C>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Gf2mWide::<1, C>::new([vals[r * cols + c]]));
        }
    }
    m
}

fn gf2m_sparse_to_dense_u64<C: Gf2mWideConfig<1>>(
    m: &SparseFieldMatrix<Gf2mWide<1, C>>,
) -> Vec<u64> {
    let rows = m.rows();
    let cols = m.cols();
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            out.push(m.get(r, c).words()[0]);
        }
    }
    out
}

fn gf2m_dense_to_u64<C: Gf2mWideConfig<1>>(m: &FieldMatrix<Gf2mWide<1, C>>) -> Vec<u64> {
    let rows = m.rows();
    let cols = m.cols();
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            out.push(m.get(r, c).words()[0]);
        }
    }
    out
}

fn emit_spmv_gf2(seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, 2);
    let x_vals = build_dense_vec_cpp_walk(seed, CELL_N, 2);

    let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples);
    let x = u64_to_bitvec(&x_vals);
    let y = a.matvec(&x);
    let y_vals = bitvec_to_u64(&y);

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    append_vec(&mut input, &x_vals);
    let mut output = Vec::new();
    append_vec(&mut output, &y_vals);

    Cell {
        tag: "spmv,GF(2)".to_string(),
        seed,
        input,
        output,
    }
}

fn emit_spmv_fp<const P: u64>(field_label: &str, seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, P);
    let x_vals = build_dense_vec_cpp_walk(seed, CELL_N, P);

    let a = fp_csr_from_triples::<P>(CELL_N, CELL_N, &triples);
    let x = u64_to_fp_vec::<P>(&x_vals);
    let y = a.matvec(&x);
    let y_vals = fp_vec_to_u64::<P>(&y);

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    append_vec(&mut input, &x_vals);
    let mut output = Vec::new();
    append_vec(&mut output, &y_vals);

    Cell {
        tag: format!("spmv,{field_label}"),
        seed,
        input,
        output,
    }
}

fn emit_sparse_dense_gf2(seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, 2);
    let b_vals = build_dense_mat_cpp_walk(seed, CELL_N, CELL_N, 2);

    let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples);
    let b = u64_to_bitmatrix(CELL_N, CELL_N, &b_vals);
    let c = a.matmat(&b);
    let c_vals = bitmatrix_to_u64(&c);

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    append_dense_mat(&mut input, CELL_N, CELL_N, &b_vals);
    let mut output = Vec::new();
    append_dense_mat(&mut output, CELL_N, CELL_N, &c_vals);

    Cell {
        tag: "sparse_dense,GF(2)".to_string(),
        seed,
        input,
        output,
    }
}

fn emit_sparse_dense_fp<const P: u64>(field_label: &str, seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, P);
    let b_vals = build_dense_mat_cpp_walk(seed, CELL_N, CELL_N, P);

    let a = fp_csr_from_triples::<P>(CELL_N, CELL_N, &triples);
    let b = u64_to_fp_matrix::<P>(CELL_N, CELL_N, &b_vals);
    let c = a.matmat(&b);
    let c_vals = fp_dense_to_u64::<P>(&c);

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    append_dense_mat(&mut input, CELL_N, CELL_N, &b_vals);
    let mut output = Vec::new();
    append_dense_mat(&mut output, CELL_N, CELL_N, &c_vals);

    Cell {
        tag: format!("sparse_dense,{field_label}"),
        seed,
        input,
        output,
    }
}

fn emit_sparse_elim_gf2(seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, 2);
    let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples);
    let r = a.rref();

    let mut dense = BitMatrix::zeros(CELL_N, CELL_N);
    for row in 0..r.rows() {
        for col in r.row_iter(row) {
            dense.set(row, col, true);
        }
    }
    let rank = (0..r.rows())
        .filter(|&row| r.row_iter(row).len() > 0)
        .count() as u64;
    let dense_vals = bitmatrix_to_u64(&dense);

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    let mut output = Vec::new();
    output.extend_from_slice(&rank.to_le_bytes());
    append_dense_mat(&mut output, CELL_N, CELL_N, &dense_vals);

    Cell {
        tag: "sparse_elim,GF(2)".to_string(),
        seed,
        input,
        output,
    }
}

fn emit_sparse_elim_fp<const P: u64>(field_label: &str, seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, P);
    let a = fp_csr_from_triples::<P>(CELL_N, CELL_N, &triples);
    let r = a.rref();

    let dense_vals = fp_sparse_to_dense_u64::<P>(&r);
    let mut rank: u64 = 0;
    for row in 0..r.rows() {
        let mut any_nonzero = false;
        for col in 0..r.cols() {
            if r.get(row, col).value() != 0 {
                any_nonzero = true;
                break;
            }
        }
        if any_nonzero {
            rank += 1;
        }
    }

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    let mut output = Vec::new();
    output.extend_from_slice(&rank.to_le_bytes());
    append_dense_mat(&mut output, CELL_N, CELL_N, &dense_vals);

    Cell {
        tag: format!("sparse_elim,{field_label}"),
        seed,
        input,
        output,
    }
}

/// The C++ harness checks this cell against a scalar GF(2^m) product of the
/// dense forms.
fn emit_sparse_dense_gf2m<C: Gf2mWideConfig<1>>(field_label: &str, seed: u64) -> Cell {
    let triples = build_csr_gf2m_walk(seed, CELL_N, CELL_DENSITY, C::M);
    let b_vals = build_dense_mat_gf2m_walk(seed, CELL_N, CELL_N, C::M);

    let a = gf2m_csr_from_triples::<C>(CELL_N, CELL_N, &triples);
    let b = u64_to_gf2m_matrix::<C>(CELL_N, CELL_N, &b_vals);
    let c = a.matmat(&b);
    let c_vals = gf2m_dense_to_u64::<C>(&c);

    let mut input = Vec::new();
    append_triples(&mut input, &triples);
    append_dense_mat(&mut input, CELL_N, CELL_N, &b_vals);
    let mut output = Vec::new();
    append_dense_mat(&mut output, CELL_N, CELL_N, &c_vals);

    Cell {
        tag: format!("sparse_dense,{field_label}"),
        seed,
        input,
        output,
    }
}

fn append_two_triples(buf: &mut Vec<u8>, a: &[Triple], b: &[Triple]) {
    append_triples(buf, a);
    append_triples(buf, b);
}

/// The C++ harness checks this cell against fflas-ffpack `fgemm` over the
/// dense forms.
fn emit_sparse_matmul_gf2(seed: u64) -> Cell {
    // B takes its own sub-seed so the product is not A · A; the C++ harness
    // derives it the same way.
    let seed_a = seed;
    let mut s_b = seed;
    let _ = splitmix64(&mut s_b);
    let _ = splitmix64(&mut s_b);
    let seed_b = splitmix64(&mut s_b);

    let triples_a = build_csr_cpp_walk(seed_a, CELL_N, CELL_DENSITY, 2);
    let triples_b = build_csr_cpp_walk(seed_b, CELL_N, CELL_DENSITY, 2);

    let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples_a);
    let b = gf2_csr_from_triples(CELL_N, CELL_N, &triples_b);
    let c = a.matmul(&b);
    let c_dense = c.to_dense();
    let c_vals = bitmatrix_to_u64(&c_dense);

    let mut input = Vec::new();
    append_two_triples(&mut input, &triples_a, &triples_b);
    let mut output = Vec::new();
    append_dense_mat(&mut output, CELL_N, CELL_N, &c_vals);

    Cell {
        tag: "sparse_matmul,GF(2)".to_string(),
        seed,
        input,
        output,
    }
}

fn emit_sparse_matmul_fp<const P: u64>(field_label: &str, seed: u64) -> Cell {
    let seed_a = seed;
    let mut s_b = seed;
    let _ = splitmix64(&mut s_b);
    let _ = splitmix64(&mut s_b);
    let seed_b = splitmix64(&mut s_b);

    let triples_a = build_csr_cpp_walk(seed_a, CELL_N, CELL_DENSITY, P);
    let triples_b = build_csr_cpp_walk(seed_b, CELL_N, CELL_DENSITY, P);

    let a = fp_csr_from_triples::<P>(CELL_N, CELL_N, &triples_a);
    let b = fp_csr_from_triples::<P>(CELL_N, CELL_N, &triples_b);
    let c = a.matmul(&b);
    let c_vals = fp_sparse_to_dense_u64::<P>(&c);

    let mut input = Vec::new();
    append_two_triples(&mut input, &triples_a, &triples_b);
    let mut output = Vec::new();
    append_dense_mat(&mut output, CELL_N, CELL_N, &c_vals);

    Cell {
        tag: format!("sparse_matmul,{field_label}"),
        seed,
        input,
        output,
    }
}

fn emit_sparse_matmul_gf2m<C: Gf2mWideConfig<1>>(field_label: &str, seed: u64) -> Cell {
    let seed_a = seed;
    let mut s_b = seed;
    let _ = splitmix64(&mut s_b);
    let _ = splitmix64(&mut s_b);
    let seed_b = splitmix64(&mut s_b);

    let triples_a = build_csr_gf2m_walk(seed_a, CELL_N, CELL_DENSITY, C::M);
    let triples_b = build_csr_gf2m_walk(seed_b, CELL_N, CELL_DENSITY, C::M);

    let a = gf2m_csr_from_triples::<C>(CELL_N, CELL_N, &triples_a);
    let b = gf2m_csr_from_triples::<C>(CELL_N, CELL_N, &triples_b);
    let c = a.matmul(&b);
    let c_vals = gf2m_sparse_to_dense_u64::<C>(&c);

    let mut input = Vec::new();
    append_two_triples(&mut input, &triples_a, &triples_b);
    let mut output = Vec::new();
    append_dense_mat(&mut output, CELL_N, CELL_N, &c_vals);

    Cell {
        tag: format!("sparse_matmul,{field_label}"),
        seed,
        input,
        output,
    }
}

// The masks and op tags mirror the seed schedule in `main()` of
// `sparse_smoke.cpp`.

const FIELD_XOR_M31: u64 = 0x00;
const FIELD_XOR_65521: u64 = 0x11;
const FIELD_XOR_251: u64 = 0x22;
const FIELD_XOR_7: u64 = 0x33;
const FIELD_XOR_GF2: u64 = 0x55;
const FIELD_XOR_GF2M8: u64 = 0x66;
const FIELD_XOR_GF2M16: u64 = 0x77;

fn cell_seed(master: u64, field_xor: u64, op_tag: &str) -> u64 {
    derive_seed(master ^ field_xor, op_tag, 0, 0, 0)
}

fn collect_cells(master_seed: u64) -> Vec<Cell> {
    const M31: u64 = 2_147_483_647;
    const PRIME_65521: u64 = 65521;
    const PRIME_251: u64 = 251;
    const PRIME_7: u64 = 7;

    vec![
        emit_spmv_fp::<M31>(
            "GF(2^31-1)",
            cell_seed(master_seed, FIELD_XOR_M31, "smoke-spmv"),
        ),
        emit_spmv_fp::<PRIME_65521>(
            "GF(65521)",
            cell_seed(master_seed, FIELD_XOR_65521, "smoke-spmv"),
        ),
        emit_spmv_fp::<PRIME_251>(
            "GF(251)",
            cell_seed(master_seed, FIELD_XOR_251, "smoke-spmv"),
        ),
        emit_spmv_fp::<PRIME_7>("GF(7)", cell_seed(master_seed, FIELD_XOR_7, "smoke-spmv")),
        emit_spmv_gf2(cell_seed(master_seed, FIELD_XOR_GF2, "smoke-spmv")),
        emit_sparse_matmul_fp::<M31>(
            "GF(2^31-1)",
            cell_seed(master_seed, FIELD_XOR_M31, "smoke-spmatmul"),
        ),
        emit_sparse_matmul_fp::<PRIME_65521>(
            "GF(65521)",
            cell_seed(master_seed, FIELD_XOR_65521, "smoke-spmatmul"),
        ),
        emit_sparse_matmul_fp::<PRIME_251>(
            "GF(251)",
            cell_seed(master_seed, FIELD_XOR_251, "smoke-spmatmul"),
        ),
        emit_sparse_matmul_fp::<PRIME_7>(
            "GF(7)",
            cell_seed(master_seed, FIELD_XOR_7, "smoke-spmatmul"),
        ),
        emit_sparse_matmul_gf2(cell_seed(master_seed, FIELD_XOR_GF2, "smoke-spmatmul")),
        emit_sparse_matmul_gf2m::<EmitterGf2m8Cfg>(
            "GF(2^8)",
            cell_seed(master_seed, FIELD_XOR_GF2M8, "smoke-spmatmul"),
        ),
        emit_sparse_matmul_gf2m::<EmitterGf2m16Cfg>(
            "GF(2^16)",
            cell_seed(master_seed, FIELD_XOR_GF2M16, "smoke-spmatmul"),
        ),
        emit_sparse_dense_fp::<M31>(
            "GF(2^31-1)",
            cell_seed(master_seed, FIELD_XOR_M31, "smoke-spmm"),
        ),
        emit_sparse_dense_fp::<PRIME_65521>(
            "GF(65521)",
            cell_seed(master_seed, FIELD_XOR_65521, "smoke-spmm"),
        ),
        emit_sparse_dense_fp::<PRIME_251>(
            "GF(251)",
            cell_seed(master_seed, FIELD_XOR_251, "smoke-spmm"),
        ),
        emit_sparse_dense_fp::<PRIME_7>("GF(7)", cell_seed(master_seed, FIELD_XOR_7, "smoke-spmm")),
        emit_sparse_dense_gf2(cell_seed(master_seed, FIELD_XOR_GF2, "smoke-spmm")),
        emit_sparse_dense_gf2m::<EmitterGf2m8Cfg>(
            "GF(2^8)",
            cell_seed(master_seed, FIELD_XOR_GF2M8, "smoke-spmm"),
        ),
        emit_sparse_dense_gf2m::<EmitterGf2m16Cfg>(
            "GF(2^16)",
            cell_seed(master_seed, FIELD_XOR_GF2M16, "smoke-spmm"),
        ),
        emit_sparse_elim_fp::<M31>(
            "GF(2^31-1)",
            cell_seed(master_seed, FIELD_XOR_M31, "smoke-spelim"),
        ),
        emit_sparse_elim_fp::<PRIME_65521>(
            "GF(65521)",
            cell_seed(master_seed, FIELD_XOR_65521, "smoke-spelim"),
        ),
        emit_sparse_elim_fp::<PRIME_251>(
            "GF(251)",
            cell_seed(master_seed, FIELD_XOR_251, "smoke-spelim"),
        ),
        emit_sparse_elim_fp::<PRIME_7>(
            "GF(7)",
            cell_seed(master_seed, FIELD_XOR_7, "smoke-spelim"),
        ),
        emit_sparse_elim_gf2(cell_seed(master_seed, FIELD_XOR_GF2, "smoke-spelim")),
    ]
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    eprintln!(
        "[sparse_smoke_emit_expected] master_seed=0x{:016x} output={}",
        args.master_seed,
        args.output.display()
    );
    let cells = collect_cells(args.master_seed);
    eprintln!(
        "[sparse_smoke_emit_expected] emitting {} cells",
        cells.len()
    );
    for c in &cells {
        eprintln!(
            "[sparse_smoke_emit_expected] cell={} seed=0x{:016x} in={}B out={}B",
            c.tag,
            c.seed,
            c.input.len(),
            c.output.len()
        );
    }
    write_cells(&args.output, &cells)?;
    eprintln!(
        "[sparse_smoke_emit_expected] wrote {}",
        args.output.display()
    );
    Ok(())
}

/// Mirrors `load_expected` in `sparse_smoke.cpp`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedCell {
    tag: String,
    seed: u64,
    input: Vec<u8>,
    output: Vec<u8>,
}

#[allow(dead_code)]
fn parse_file(bytes: &[u8]) -> Result<Vec<ParsedCell>, String> {
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(format!(
            "magic mismatch: expected GF2SMK01, got {:?}",
            &bytes[..bytes.len().min(8)]
        ));
    }
    let n_cells = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut cursor = 12usize;
    let mut cells = Vec::with_capacity(n_cells);
    for idx in 0..n_cells {
        if cursor + 2 > bytes.len() {
            return Err(format!("cell {idx}: truncated tag_len"));
        }
        let tag_len = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;
        if cursor + tag_len > bytes.len() {
            return Err(format!("cell {idx}: truncated tag"));
        }
        let tag = std::str::from_utf8(&bytes[cursor..cursor + tag_len])
            .map_err(|e| format!("cell {idx}: bad UTF-8 tag: {e}"))?
            .to_string();
        cursor += tag_len;
        if cursor + 8 > bytes.len() {
            return Err(format!("cell {idx} ({tag}): truncated seed"));
        }
        let seed = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;
        if cursor + 4 > bytes.len() {
            return Err(format!("cell {idx} ({tag}): truncated in_len"));
        }
        let in_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        if cursor + in_len > bytes.len() {
            return Err(format!("cell {idx} ({tag}): truncated input"));
        }
        let input = bytes[cursor..cursor + in_len].to_vec();
        cursor += in_len;
        if cursor + 4 > bytes.len() {
            return Err(format!("cell {idx} ({tag}): truncated out_len"));
        }
        let out_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        if cursor + out_len > bytes.len() {
            return Err(format!("cell {idx} ({tag}): truncated output"));
        }
        let output = bytes[cursor..cursor + out_len].to_vec();
        cursor += out_len;
        cells.push(ParsedCell {
            tag,
            seed,
            input,
            output,
        });
    }
    if cursor != bytes.len() {
        return Err(format!(
            "trailing bytes: parsed {} of {} bytes",
            cursor,
            bytes.len()
        ));
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::alg::m4rm::multiply as m4rm_multiply;
    use gf2_core::field::matrix::gemm;

    #[test]
    fn round_trip_binary_format() {
        let cells = collect_cells(DEFAULT_MASTER_SEED);
        assert_eq!(
            cells.len(),
            24,
            "expected 5 spmv + 7 sparse_matmul + 7 sparse_dense + 5 sparse_elim cells \
             (post code-review R1 expansion to GF(2^m) + sparse-matmul)"
        );

        let tmp = tempfile::NamedTempFile::new().expect("tmpfile");
        let path = tmp.path().to_path_buf();
        write_cells(&path, &cells).expect("write");
        let bytes = std::fs::read(&path).expect("read");

        let parsed = parse_file(&bytes).expect("parse");
        assert_eq!(parsed.len(), cells.len());
        for (orig, p) in cells.iter().zip(parsed.iter()) {
            assert_eq!(orig.tag, p.tag);
            assert_eq!(orig.seed, p.seed);
            assert_eq!(orig.input, p.input, "input bytes for {}", orig.tag);
            assert_eq!(orig.output, p.output, "output bytes for {}", orig.tag);
        }
    }

    #[test]
    fn cells_cover_all_op_field_tracks() {
        let cells = collect_cells(DEFAULT_MASTER_SEED);
        let tags: Vec<&str> = cells.iter().map(|c| c.tag.as_str()).collect();

        for f in ["GF(2^31-1)", "GF(65521)", "GF(251)", "GF(7)", "GF(2)"] {
            let want = format!("spmv,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
        }

        for f in [
            "GF(2^31-1)",
            "GF(65521)",
            "GF(251)",
            "GF(7)",
            "GF(2)",
            "GF(2^8)",
            "GF(2^16)",
        ] {
            let want = format!("sparse_dense,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
        }

        for f in [
            "GF(2^31-1)",
            "GF(65521)",
            "GF(251)",
            "GF(7)",
            "GF(2)",
            "GF(2^8)",
            "GF(2^16)",
        ] {
            let want = format!("sparse_matmul,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
        }

        for f in ["GF(2^31-1)", "GF(65521)", "GF(251)", "GF(7)", "GF(2)"] {
            let want = format!("sparse_elim,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
        }
    }

    #[test]
    fn cpp_walk_matches_first_n_triples_gf7() {
        let triples = build_csr_cpp_walk(0xCAFE_BABE_DEAD_BEEF, 16, 0.25, 7);
        assert!(
            (32..=96).contains(&triples.len()),
            "unexpected triple count {} (expected ~64)",
            triples.len()
        );
        for t in &triples {
            assert!(t.row < 16);
            assert!(t.col < 16);
            assert!((1..=6).contains(&t.val), "value {} not in [1, 6]", t.val);
        }
    }

    #[test]
    fn sparse_matmul_matches_dense_round_trip_gf2() {
        let cell = emit_sparse_matmul_gf2(0xDEAD_BEEF_C0FF_EE00);

        let seed_a = 0xDEAD_BEEF_C0FF_EE00_u64;
        let mut s_b = seed_a;
        let _ = splitmix64(&mut s_b);
        let _ = splitmix64(&mut s_b);
        let seed_b = splitmix64(&mut s_b);

        let triples_a = build_csr_cpp_walk(seed_a, CELL_N, CELL_DENSITY, 2);
        let triples_b = build_csr_cpp_walk(seed_b, CELL_N, CELL_DENSITY, 2);
        let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples_a);
        let b = gf2_csr_from_triples(CELL_N, CELL_N, &triples_b);

        let a_dense = a.to_dense();
        let b_dense = b.to_dense();
        let c_round_trip = m4rm_multiply(&a_dense, &b_dense);
        let c_round_trip_vals = bitmatrix_to_u64(&c_round_trip);

        // 16 skips the dense-matrix header (rows: u64, cols: u64).
        let mut recorded = Vec::with_capacity(CELL_N * CELL_N);
        for i in 0..(CELL_N * CELL_N) {
            let off = 16 + 8 * i;
            recorded.push(u64::from_le_bytes(
                cell.output[off..off + 8].try_into().unwrap(),
            ));
        }
        assert_eq!(
            recorded, c_round_trip_vals,
            "sparse-matmul (CSR×CSR) ↔ dense round-trip disagree on GF(2)"
        );
    }

    #[test]
    fn sparse_matmul_matches_dense_round_trip_gf7() {
        type F = Fp<7>;
        let cell = emit_sparse_matmul_fp::<7>("GF(7)", 0xCAFE_BABE_FEED_FACE);

        let seed_a = 0xCAFE_BABE_FEED_FACE_u64;
        let mut s_b = seed_a;
        let _ = splitmix64(&mut s_b);
        let _ = splitmix64(&mut s_b);
        let seed_b = splitmix64(&mut s_b);

        let triples_a = build_csr_cpp_walk(seed_a, CELL_N, CELL_DENSITY, 7);
        let triples_b = build_csr_cpp_walk(seed_b, CELL_N, CELL_DENSITY, 7);
        let a: SparseFieldMatrix<F> = fp_csr_from_triples::<7>(CELL_N, CELL_N, &triples_a);
        let b: SparseFieldMatrix<F> = fp_csr_from_triples::<7>(CELL_N, CELL_N, &triples_b);

        let a_dense = a.to_dense();
        let b_dense = b.to_dense();
        let c_round_trip = gemm(&a_dense, &b_dense);
        let c_round_trip_vals = fp_dense_to_u64::<7>(&c_round_trip);

        let mut recorded = Vec::with_capacity(CELL_N * CELL_N);
        for i in 0..(CELL_N * CELL_N) {
            let off = 16 + 8 * i;
            recorded.push(u64::from_le_bytes(
                cell.output[off..off + 8].try_into().unwrap(),
            ));
        }
        assert_eq!(
            recorded, c_round_trip_vals,
            "sparse-matmul (CSR×CSR) ↔ dense round-trip disagree on GF(7)"
        );
    }

    #[test]
    fn sparse_matmul_matches_dense_round_trip_gf2m8() {
        let cell = emit_sparse_matmul_gf2m::<EmitterGf2m8Cfg>("GF(2^8)", 0x1122_3344_5566_7788);

        let seed_a = 0x1122_3344_5566_7788_u64;
        let mut s_b = seed_a;
        let _ = splitmix64(&mut s_b);
        let _ = splitmix64(&mut s_b);
        let seed_b = splitmix64(&mut s_b);

        let triples_a = build_csr_gf2m_walk(seed_a, CELL_N, CELL_DENSITY, 8);
        let triples_b = build_csr_gf2m_walk(seed_b, CELL_N, CELL_DENSITY, 8);
        let a = gf2m_csr_from_triples::<EmitterGf2m8Cfg>(CELL_N, CELL_N, &triples_a);
        let b = gf2m_csr_from_triples::<EmitterGf2m8Cfg>(CELL_N, CELL_N, &triples_b);

        let a_dense = a.to_dense();
        let b_dense = b.to_dense();
        let c_round_trip = gemm(&a_dense, &b_dense);
        let c_round_trip_vals = gf2m_dense_to_u64::<EmitterGf2m8Cfg>(&c_round_trip);

        let mut recorded = Vec::with_capacity(CELL_N * CELL_N);
        for i in 0..(CELL_N * CELL_N) {
            let off = 16 + 8 * i;
            recorded.push(u64::from_le_bytes(
                cell.output[off..off + 8].try_into().unwrap(),
            ));
        }
        assert_eq!(
            recorded, c_round_trip_vals,
            "sparse-matmul (CSR×CSR) ↔ dense round-trip disagree on GF(2^8)"
        );
    }

    #[test]
    fn sparse_dense_gf2m_matches_dense_round_trip() {
        let cell = emit_sparse_dense_gf2m::<EmitterGf2m16Cfg>("GF(2^16)", 0xABCD_1234_DEAD_BEEF);

        let triples = build_csr_gf2m_walk(0xABCD_1234_DEAD_BEEF, CELL_N, CELL_DENSITY, 16);
        let b_vals = build_dense_mat_gf2m_walk(0xABCD_1234_DEAD_BEEF, CELL_N, CELL_N, 16);

        let a = gf2m_csr_from_triples::<EmitterGf2m16Cfg>(CELL_N, CELL_N, &triples);
        let b = u64_to_gf2m_matrix::<EmitterGf2m16Cfg>(CELL_N, CELL_N, &b_vals);

        let a_dense = a.to_dense();
        let c_round_trip = gemm(&a_dense, &b);
        let c_round_trip_vals = gf2m_dense_to_u64::<EmitterGf2m16Cfg>(&c_round_trip);

        let mut recorded = Vec::with_capacity(CELL_N * CELL_N);
        for i in 0..(CELL_N * CELL_N) {
            let off = 16 + 8 * i;
            recorded.push(u64::from_le_bytes(
                cell.output[off..off + 8].try_into().unwrap(),
            ));
        }
        assert_eq!(
            recorded, c_round_trip_vals,
            "sparse_dense (CSR · dense) ↔ dense gemm disagree on GF(2^16)"
        );
    }
}
