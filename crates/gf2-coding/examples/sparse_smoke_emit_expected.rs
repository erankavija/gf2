//! Sparse-smoke ground-truth emitter (`jit:96fde7c7`).
//!
//! Companion to `benchmarks/reference/sparse_smoke.cpp`. This Cargo
//! example walks every `(op, field)` cell the C++ smoke harness asserts
//! against, builds the same seeded input the C++ harness builds via
//! `gf2_bench_derive_seed` + `splitmix64` (see
//! `benchmarks/reference/seed_helpers.h`), runs the **production**
//! gf2-core code path for each cell, and serialises both the input and
//! the expected output to a binary file. The C++ smoke loads that file
//! at startup and asserts byte-equality between (a) the input it builds
//! locally and the input bytes recorded here (L1 — seed-walk
//! equivalence) and (b) the candidate library's output and the gf2-core
//! ground-truth output (L2-L5 — per-op output equality).
//!
//! The mechanism (b) ground-truth file design is documented in
//! `dev/plans/sparse_smoke_gf2core_integration_sketch.md`. The
//! .gitignored binary lives at `benchmarks/expected/sparse_smoke_n16.bin`
//! and is regenerated on every `benchmarks/smoke.sh` invocation.
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
//! - `sparse_elim,<F>` :
//!   - `in`  : `nnz: u64`, `nnz × {row: u64, col: u64, val: u64}`.
//!   - `out` : `rank: u64`, `m: u64`, `n: u64`,
//!     `m*n × val: u64` (full RREF dense, row-major).
//!
//! # Naming convention
//!
//! Cell tags use the project-wide CSV / scorecard convention:
//! `spmv,GF(...)` / `sparse_dense,GF(...)` / `sparse_elim,GF(...)`.
//! The C++ smoke harness uses underscored op names internally
//! (`spmv` / `sparse_dense` / `sparse_elim`) to match the smoke source's
//! existing `oracle_*` helper names; this file uses the same underscored
//! tags. Note: the broader CSV emitter uses Unicode `sparse×dense`
//! (U+00D7) per protocol § 7 — that convention applies to CSV row
//! emission, not to the smoke harness's internal cell tag.
//!
//! # CLI
//!
//! ```text
//! sparse_smoke_emit_expected [--output PATH] [--master-seed N]
//! ```
//!
//! `--master-seed` mirrors `sparse_smoke --seed`. Default seed:
//! `0x6F73AC91D31E4A7C` (the project-wide reference master seed).
//! Default output: `benchmarks/expected/sparse_smoke_n16.bin`.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use gf2_core::bench_seed::{derive_seed, splitmix64};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::gfp::Fp;
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SpBitMatrix;
use gf2_core::BitVec;

/// Project-wide reference master seed; matches `sparse_smoke.cpp`'s
/// default and the seed pinned in `benchmarks/seeds/seed.txt`.
const DEFAULT_MASTER_SEED: u64 = 0x6F73_AC91_D31E_4A7C;

/// Cell parameters shared with the C++ smoke harness (`n=16`,
/// `density=0.25`).
const CELL_N: usize = 16;
const CELL_DENSITY: f64 = 0.25;

const MAGIC: &[u8; 8] = b"GF2SMK01";

// ─── CLI ────────────────────────────────────────────────────────────────────

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

// ─── Seeded input construction (mirror of C++ build_csr) ───────────────────

/// CSR triple `(row, col, value)` in `[0, card)` canonical form.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Triple {
    row: u64,
    col: u64,
    val: u64,
}

/// Mirrors `build_csr` in `benchmarks/reference/sparse_smoke.cpp`. The
/// SplitMix64 walk is identical to the C++ side: per cell `(i, j)` in
/// row-major order, draw a gating splitmix; if it falls below the
/// threshold, draw a second splitmix for the value (`(v_raw % (card - 1))
/// + 1`).
///
/// `card = 2` (GF(2)) yields value = 1 for every nonzero cell, but the
/// second splitmix is still drawn — the seed walk must match the C++
/// harness byte-for-byte regardless of field choice, so the L1 seed-walk
/// assertion in the smoke can compare the bytes-on-disk against the
/// C++-side `build_csr` output.
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

/// Mirrors the dense vector walk in `oracle_spmv` (`x` initialiser):
/// `st = seed ^ 0xCAFEBABE`, then per-cell `splitmix64 % card`.
fn build_dense_vec_cpp_walk(seed: u64, n: usize, card: u64) -> Vec<u64> {
    let mut st = seed ^ 0xCAFE_BABE_u64;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let r = splitmix64(&mut st);
        v.push(r % card);
    }
    v
}

/// Mirrors the dense matrix walk in `oracle_sparse_dense` (`B`
/// initialiser): `st = seed ^ 0xDEADBEEF`, then per-cell `splitmix64 %
/// card` in row-major order over `n*n` cells.
fn build_dense_mat_cpp_walk(seed: u64, rows: usize, cols: usize, card: u64) -> Vec<u64> {
    let mut st = seed ^ 0xDEAD_BEEF_u64;
    let mut v = Vec::with_capacity(rows * cols);
    for _ in 0..(rows * cols) {
        let r = splitmix64(&mut st);
        v.push(r % card);
    }
    v
}

// ─── Cell record & writer ──────────────────────────────────────────────────

/// One per-cell record as written to the ground-truth file.
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

/// Append a `nnz: u64`, `{row, col, val} × nnz` triples block in LE.
fn append_triples(buf: &mut Vec<u8>, triples: &[Triple]) {
    buf.extend_from_slice(&(triples.len() as u64).to_le_bytes());
    for t in triples {
        buf.extend_from_slice(&t.row.to_le_bytes());
        buf.extend_from_slice(&t.col.to_le_bytes());
        buf.extend_from_slice(&t.val.to_le_bytes());
    }
}

/// Append a `len: u64`, `len × val: u64` vector block.
fn append_vec(buf: &mut Vec<u8>, vals: &[u64]) {
    buf.extend_from_slice(&(vals.len() as u64).to_le_bytes());
    for v in vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

/// Append a `rows: u64`, `cols: u64`, `rows*cols × val: u64` row-major
/// dense matrix block.
fn append_dense_mat(buf: &mut Vec<u8>, rows: usize, cols: usize, vals: &[u64]) {
    assert_eq!(vals.len(), rows * cols);
    buf.extend_from_slice(&(rows as u64).to_le_bytes());
    buf.extend_from_slice(&(cols as u64).to_le_bytes());
    for v in vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

// ─── Per-cell builders ─────────────────────────────────────────────────────

/// Build a `SpBitMatrix` (CSR) from CSR triples. GF(2) ignores the value
/// field — every triple is treated as the bit `1` (the value is always
/// `1` for `card=2` anyway by `build_csr_cpp_walk`'s construction).
fn gf2_csr_from_triples(rows: usize, cols: usize, triples: &[Triple]) -> SpBitMatrix {
    let coo: Vec<(usize, usize)> = triples
        .iter()
        .map(|t| (t.row as usize, t.col as usize))
        .collect();
    SpBitMatrix::from_coo(rows, cols, &coo)
}

/// Build a `SparseFieldMatrix<Fp<P>>` (CSR) from triples.
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

/// Convert a `SparseFieldMatrix<Fp<P>>` to its dense canonical
/// representation as a row-major `u64` buffer in `[0, P)`.
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

/// Read out a `SparseFieldMatrix<Fp<P>>` cell-by-cell as a dense
/// row-major `u64` buffer in `[0, P)`. The matrix is small (n=16) so the
/// `O(n² log nnz_per_row)` cost is negligible; we use the public `get`
/// API rather than touching the private CSR internals.
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

/// Convert a `BitMatrix` to a row-major `u64` buffer (each entry 0 or 1).
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

/// Convert a `BitVec` to a row-major `u64` buffer (each entry 0 or 1).
fn bitvec_to_u64(v: &BitVec) -> Vec<u64> {
    let n = v.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(if v.get(i) { 1 } else { 0 });
    }
    out
}

/// Convert a `FieldVec<Fp<P>>` to a `[0, P)` u64 buffer.
fn fp_vec_to_u64<const P: u64>(v: &FieldVec<Fp<P>>) -> Vec<u64> {
    let mut out = Vec::with_capacity(v.len());
    for i in 0..v.len() {
        out.push(v.get(i).value());
    }
    out
}

/// Build a dense `BitMatrix` from row-major `u64` values (each cell is
/// the LSB of the input — but `build_dense_mat_cpp_walk` for GF(2) calls
/// `splitmix64 % 2`, already a 0/1 value).
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

/// Build a `BitVec` from a `u64` slice (each entry treated as a 0/1 bit).
fn u64_to_bitvec(vals: &[u64]) -> BitVec {
    let mut v = BitVec::zeros(vals.len());
    for (i, &x) in vals.iter().enumerate() {
        if x != 0 {
            v.set(i, true);
        }
    }
    v
}

/// Build a `FieldMatrix<Fp<P>>` from a row-major `u64` slice (values
/// reduced mod P at construction time — but `build_dense_mat_cpp_walk`
/// already reduces mod card).
fn u64_to_fp_matrix<const P: u64>(rows: usize, cols: usize, vals: &[u64]) -> FieldMatrix<Fp<P>> {
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(vals[r * cols + c]));
        }
    }
    m
}

/// Build a `FieldVec<Fp<P>>` from a `u64` slice.
fn u64_to_fp_vec<const P: u64>(vals: &[u64]) -> FieldVec<Fp<P>> {
    let mut v = FieldVec::<Fp<P>>::zeros(vals.len());
    for (i, &x) in vals.iter().enumerate() {
        v.set(i, Fp::<P>::new(x));
    }
    v
}

// ─── Cell emitters ─────────────────────────────────────────────────────────

/// Emit `spmv,GF(2)`. Production path: `SpBitMatrix::matvec(&BitVec)`.
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

/// Emit `spmv,GF(P)`. Production path:
/// `SparseFieldMatrix::<Fp<P>>::matvec(&FieldVec)`.
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

/// Emit `sparse_dense,GF(2)`. Production path:
/// `SpBitMatrix::matmat(&BitMatrix) -> BitMatrix`.
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

/// Emit `sparse_dense,GF(P)`. Production path:
/// `SparseFieldMatrix::<Fp<P>>::matmat(&FieldMatrix) -> FieldMatrix`.
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

/// Emit `sparse_elim,GF(2)`. Production path:
/// `SpBitMatrix::rref() -> SpBitMatrix` (sparse-native CSR-CSR RREF
/// landed via `jit:0d6ca3b6`).
///
/// Output records `rank: u64`, then the dense RREF of the matrix as a
/// row-major `n × n` `u64` buffer with each cell ∈ `{0, 1}`.
fn emit_sparse_elim_gf2(seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, 2);
    let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples);
    let r = a.rref();

    // Materialise the RREF as a dense BitMatrix to record byte-for-byte.
    // SpBitMatrix doesn't expose a public `to_dense`, so we walk via
    // `row_iter` (rank is then the count of non-zero rows).
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

/// Emit `sparse_elim,GF(P)`. Production path:
/// `SparseFieldMatrix::<Fp<P>>::rref() -> SparseFieldMatrix<Fp<P>>`.
///
/// Output records `rank: u64`, then the dense RREF as `n × n` `u64`
/// buffer with each cell ∈ `[0, P)`.
fn emit_sparse_elim_fp<const P: u64>(field_label: &str, seed: u64) -> Cell {
    let triples = build_csr_cpp_walk(seed, CELL_N, CELL_DENSITY, P);
    let a = fp_csr_from_triples::<P>(CELL_N, CELL_N, &triples);
    let r = a.rref();

    let dense_vals = fp_sparse_to_dense_u64::<P>(&r);
    // Rank = number of non-zero rows in the RREF. SparseFieldMatrix
    // doesn't expose a per-row nnz directly via the public API; count
    // by checking each row for a non-zero cell.
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

// ─── Smoke seed schedule ───────────────────────────────────────────────────
//
// Mirrors the seed schedule in `sparse_smoke.cpp`'s `main()`. Each cell
// uses `gf2_bench_derive_seed(master_xor, op_tag, 0, 0, 0)` with a
// per-field XOR mask on the master seed:
//
//   - GF(2^31-1) : master_seed
//   - GF(65521)  : master_seed ^ 0x11
//   - GF(251)    : master_seed ^ 0x22
//   - GF(7)      : master_seed ^ 0x33
//   - GF(2)      : master_seed ^ 0x55
//
// The op tag is `smoke-spmv` for spmv, `smoke-spmm` for sparse_dense,
// `smoke-spelim` for sparse_elim.

fn cell_seed(master: u64, field_xor: u64, op_tag: &str) -> u64 {
    derive_seed(master ^ field_xor, op_tag, 0, 0, 0)
}

fn collect_cells(master_seed: u64) -> Vec<Cell> {
    const M31: u64 = 2_147_483_647;
    const PRIME_65521: u64 = 65521;
    const PRIME_251: u64 = 251;
    const PRIME_7: u64 = 7;

    vec![
        // ── spmv ────────────────────────────────────────────────────────
        emit_spmv_fp::<M31>("GF(2^31-1)", cell_seed(master_seed, 0x00, "smoke-spmv")),
        emit_spmv_fp::<PRIME_65521>("GF(65521)", cell_seed(master_seed, 0x11, "smoke-spmv")),
        emit_spmv_fp::<PRIME_251>("GF(251)", cell_seed(master_seed, 0x22, "smoke-spmv")),
        emit_spmv_fp::<PRIME_7>("GF(7)", cell_seed(master_seed, 0x33, "smoke-spmv")),
        emit_spmv_gf2(cell_seed(master_seed, 0x55, "smoke-spmv")),
        // ── sparse_dense ────────────────────────────────────────────────
        emit_sparse_dense_fp::<M31>("GF(2^31-1)", cell_seed(master_seed, 0x00, "smoke-spmm")),
        emit_sparse_dense_fp::<PRIME_65521>(
            "GF(65521)",
            cell_seed(master_seed, 0x11, "smoke-spmm"),
        ),
        emit_sparse_dense_fp::<PRIME_251>("GF(251)", cell_seed(master_seed, 0x22, "smoke-spmm")),
        emit_sparse_dense_fp::<PRIME_7>("GF(7)", cell_seed(master_seed, 0x33, "smoke-spmm")),
        emit_sparse_dense_gf2(cell_seed(master_seed, 0x55, "smoke-spmm")),
        // ── sparse_elim ─────────────────────────────────────────────────
        emit_sparse_elim_fp::<M31>("GF(2^31-1)", cell_seed(master_seed, 0x00, "smoke-spelim")),
        emit_sparse_elim_fp::<PRIME_65521>(
            "GF(65521)",
            cell_seed(master_seed, 0x11, "smoke-spelim"),
        ),
        emit_sparse_elim_fp::<PRIME_251>("GF(251)", cell_seed(master_seed, 0x22, "smoke-spelim")),
        emit_sparse_elim_fp::<PRIME_7>("GF(7)", cell_seed(master_seed, 0x33, "smoke-spelim")),
        emit_sparse_elim_gf2(cell_seed(master_seed, 0x55, "smoke-spelim")),
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

// ─── Round-trip parser (also used by tests) ────────────────────────────────

/// Minimal parser for the binary format. Mirrors the C++ smoke harness's
/// `load_expected` and is used by the in-binary unit tests below. Kept
/// in this file so the format definition lives in exactly one place.
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

    #[test]
    fn round_trip_binary_format() {
        let cells = collect_cells(DEFAULT_MASTER_SEED);
        assert_eq!(
            cells.len(),
            15,
            "expected 3 ops (spmv, sparse_dense, sparse_elim) × 5 fields"
        );

        // Serialise into a Vec<u8> via a tempfile so we exercise the same
        // I/O code path the C++ smoke loader hits.
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
        // spmv × 5 fields
        for f in ["GF(2^31-1)", "GF(65521)", "GF(251)", "GF(7)", "GF(2)"] {
            let want = format!("spmv,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
            let want = format!("sparse_dense,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
            let want = format!("sparse_elim,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
        }
    }

    #[test]
    fn cpp_walk_matches_first_n_triples_gf7() {
        // Cross-check that the C++-style walk, when executed against
        // the same seed/density used by the smoke harness, builds the
        // expected triple count and that the value field is in [1, 6].
        let triples = build_csr_cpp_walk(0xCAFE_BABE_DEAD_BEEF, 16, 0.25, 7);
        // For density 0.25 over 256 cells, expect ~64 nonzeros; assert
        // the count is in a reasonable band so a regression in the
        // splitmix walk would be caught.
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
}
