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
//! `dev/plans/96fde7c7/sparse_smoke_gf2core_integration_sketch.md`. The
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
//! - `sparse_matmul,<F>` :
//!   - `in`  : `nnz_a: u64`, `nnz_a × {row: u64, col: u64, val: u64}`,
//!     `nnz_b: u64`, `nnz_b × {row: u64, col: u64, val: u64}`.
//!   - `out` : `m: u64`, `n: u64`, `m*n × val: u64`
//!     (gf2-core sparse matmul output materialised as a dense
//!     row-major matrix — internal-consistency check: candidate
//!     side computes `A.to_dense() · B.to_dense()` via the same
//!     field's dense matmul and asserts byte-equality).
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
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::SpBitMatrix;
use gf2_core::BitVec;

// ─── GF(2^m) configs (mirror of `bench_sparse_csv_emitter.rs`) ─────────────
//
// Same configs the bench emitter uses for CSV-row emission. The
// MODULUS values are the standard primitive polynomials for GF(2^8)
// (`x^8 + x^4 + x^3 + x + 1` = 0x1B) and GF(2^16) (`x^16 + x^5 + x^3 +
// x^2 + 1` = 0x002D). The C++ smoke side mirrors these constants in
// `scalar_gf2m_mul` / `scalar_gf2m_dense_matmul` so the round-trip
// oracle agrees byte-for-byte.

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

/// GF(2^m) variant of [`build_csr_cpp_walk`]. The mask = `(1 << M) - 1`
/// (or `u64::MAX` for M ≥ 64); each non-zero cell's value is
/// `(splitmix64 & mask)`, bumped to 1 if the masked draw is 0 so the
/// CSR bucket survives `from_triplets` deduplication.
///
/// Mirrors `gf2m_wide_1_sparse_from_seed` in
/// `crates/gf2-core/src/bench_seed.rs:378` so the byte-level seed walk
/// stays consistent with the bench emitter, AND mirrors the C++
/// `build_csr_gf2m` helper that the smoke harness calls — the L1
/// in-bytes assertion catches drift between any of the three.
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

/// GF(2^m) variant of [`build_dense_mat_cpp_walk`]. Per-cell draw is
/// `splitmix64 & mask` (zero is allowed for the dense matrix; only the
/// sparse CSR walk bumps zeros to 1 to keep the support count stable).
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

// ─── GF(2^m) helpers ───────────────────────────────────────────────────────

/// Build a `SparseFieldMatrix<Gf2mWide<1, C>>` (CSR) from triples whose
/// `val` field carries the canonical word-0 packed polynomial coefficient.
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

/// Build a dense `FieldMatrix<Gf2mWide<1, C>>` from a row-major `u64`
/// slice. Each entry is interpreted as the LSBs of word 0.
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

/// Materialise a sparse `Gf2mWide` matrix as a row-major `u64` buffer.
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

/// Materialise a dense `FieldMatrix<Gf2mWide>` as a row-major `u64` buffer.
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

/// Emit `sparse_dense,GF(2^m)`. Production path:
/// `SparseFieldMatrix::<Gf2mWide<1, C>>::matmat(&FieldMatrix) -> FieldMatrix`.
///
/// Internal-consistency: per § 6 of the design sketch, GF(2^m)
/// sparse×dense is `semantics-mismatch` for fflas / LinBox; the smoke
/// validates that gf2-core's sparse → dense `matmat` agrees with
/// gf2-core's pure dense `gemm` over the same field. Both paths are
/// gf2-core, so the candidate-vs-gf2-core framing here is "dense
/// round-trip vs sparse path" — a regression in either path that
/// breaks byte-equivalence is caught.
///
/// We record the gf2-core sparse output as the ground-truth `out`
/// bytes; the C++ smoke side recomputes `A_dense · B_dense` via a
/// scalar GF(2^m) matmul (see `scalar_gf2m_matmul` in
/// `sparse_smoke.cpp`) and asserts byte-equality.
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

/// Append two CSR-triple blocks (A then B) to a `sparse_matmul` input
/// buffer. Format: `nnz_a: u64`, A triples; `nnz_b: u64`, B triples.
fn append_two_triples(buf: &mut Vec<u8>, a: &[Triple], b: &[Triple]) {
    append_triples(buf, a);
    append_triples(buf, b);
}

/// Emit `sparse_matmul,GF(2)`. Production path:
/// `SpBitMatrix::matmul(&Self) -> SpBitMatrix`.
///
/// Internal-consistency: per § 6 of the design sketch, sparse×sparse
/// has `no-independent-oracle` (no external library exposes a directly
/// comparable sparse · sparse product). The smoke validates gf2-core's
/// sparse path against the dense round-trip:
///   `A.matmul(B).to_dense() == A.to_dense().matmul(&B.to_dense())`
/// where the RHS uses gf2-core's `m4rm::multiply` for GF(2). Both
/// sides are gf2-core; a regression in either path that breaks byte-
/// equivalence fails the smoke at L4.
///
/// We record the gf2-core sparse-matmul output as the dense
/// row-major `u64` buffer; the C++ smoke side computes
/// `A.to_dense() · B.to_dense()` via fflas-ffpack `fgemm` over
/// `Modular<int64_t>(2)` and asserts byte-equality (the dense paths
/// are independent since fflas's `fgemm` is a separate
/// implementation from gf2-core's `m4rm::multiply`).
fn emit_sparse_matmul_gf2(seed: u64) -> Cell {
    // Two distinct seed-walks for A and B so the matrices are
    // independent (otherwise A · A would only exercise a degenerate
    // case). The C++ smoke side mirrors this by deriving two
    // sub-seeds from the cell seed via `splitmix64`.
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

/// Emit `sparse_matmul,GF(P)`. Production path:
/// `SparseFieldMatrix::<Fp<P>>::matmul(&Self) -> SparseFieldMatrix<Fp<P>>`.
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

/// Emit `sparse_matmul,GF(2^m)`. Production path:
/// `SparseFieldMatrix::<Gf2mWide<1, C>>::matmul(&Self) ->
/// SparseFieldMatrix<Gf2mWide<1, C>>`.
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
//   - GF(2^8)    : master_seed ^ 0x66  (added by code-review R1)
//   - GF(2^16)   : master_seed ^ 0x77  (added by code-review R1)
//
// The op tag is `smoke-spmv` for spmv, `smoke-spmm` for sparse_dense,
// `smoke-spmatmul` for sparse_matmul (distinct from sparse_dense per
// `dev/plans/96fde7c7/sparse_smoke_gf2core_integration_sketch.md` § 6 — sparse
// × sparse vs sparse × dense are separate ops), and `smoke-spelim`
// for sparse_elim.

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
        // ── spmv ────────────────────────────────────────────────────────
        // 5 cells: all GF(p) primes + GF(2). GF(2^m) is `semantics-mismatch`
        // for fflas/LinBox (no comparable polynomial-coefficient ABI) and is
        // not part of the spmv smoke per the design sketch § 6.
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
        // ── sparse_matmul (sparse × sparse) ─────────────────────────────
        // 7 cells: all 5 prime/GF(2) fields + GF(2^8) + GF(2^16) per
        // protocol § 6's sparse-matmul × {GF(2), GF(p), GF(2^m)}
        // requirement. Internal-consistency check: gf2-core sparse path
        // `matmul` vs the dense round-trip `A.to_dense() · B.to_dense()`.
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
        // ── sparse_dense ────────────────────────────────────────────────
        // 7 cells: 5 fflas-canonical (GF(p) + GF(2)) + 2 GF(2^m)
        // internal-consistency cells per protocol § 6's sparse×dense ×
        // {GF(p), GF(2^m)} requirement (GF(2) included as a canonical
        // candidate-vs-gf2-core cell since `SpBitMatrix::matmat` landed
        // by 521390db enables it).
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
        // ── sparse_elim ─────────────────────────────────────────────────
        // 5 cells: GF(p) + GF(2). GF(2^m) sparse-elim is `not-yet-
        // harnessed` for LinBox (no `Method::SparseElimination` over the
        // gf2-core polynomial ABI) and stays out of the smoke per § 6.
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

        // spmv: 5 fields (GF(p) + GF(2)).
        for f in ["GF(2^31-1)", "GF(65521)", "GF(251)", "GF(7)", "GF(2)"] {
            let want = format!("spmv,{f}");
            assert!(
                tags.contains(&want.as_str()),
                "missing cell {want}; tags={tags:?}"
            );
        }

        // sparse_dense: 5 GF(p)/GF(2) + 2 GF(2^m) = 7 fields.
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

        // sparse_matmul: same 7 fields (sparse-matmul × {GF(2), GF(p),
        // GF(2^m)} per protocol § 6).
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

        // sparse_elim: 5 fields (GF(p) + GF(2)).
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

    /// Internal-consistency check: the sparse-matmul output bytes the
    /// emitter records must equal the dense round-trip of the same
    /// matrices. This is the same invariant the C++ smoke side
    /// asserts at runtime — exercising it in-language here gives a
    /// fast pre-flight signal during `cargo nextest run`, before the
    /// container build is necessary.
    #[test]
    fn sparse_matmul_matches_dense_round_trip_gf2() {
        let cell = emit_sparse_matmul_gf2(0xDEAD_BEEF_C0FF_EE00);

        // Re-derive A and B the same way the emitter did so we can
        // build the dense round-trip.
        let seed_a = 0xDEAD_BEEF_C0FF_EE00_u64;
        let mut s_b = seed_a;
        let _ = splitmix64(&mut s_b);
        let _ = splitmix64(&mut s_b);
        let seed_b = splitmix64(&mut s_b);

        let triples_a = build_csr_cpp_walk(seed_a, CELL_N, CELL_DENSITY, 2);
        let triples_b = build_csr_cpp_walk(seed_b, CELL_N, CELL_DENSITY, 2);
        let a = gf2_csr_from_triples(CELL_N, CELL_N, &triples_a);
        let b = gf2_csr_from_triples(CELL_N, CELL_N, &triples_b);

        // Dense round-trip via gf2-core's M4RM multiply.
        let a_dense = a.to_dense();
        let b_dense = b.to_dense();
        let c_round_trip = m4rm_multiply(&a_dense, &b_dense);
        let c_round_trip_vals = bitmatrix_to_u64(&c_round_trip);

        // Parse back the cell's recorded output; skip the 16-byte
        // dense-matrix header (rows: u64, cols: u64).
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
        // Internal-consistency for sparse_dense × GF(2^m): the gf2-core
        // sparse `matmat` must agree with the dense `gemm` over the
        // same field. C++ side will assert the same invariant.
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
