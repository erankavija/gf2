//! Export gf2-coding LDPC parity-check matrices to MacKay AList format.
//!
//! Part of the external-library comparison harness (issue `18e69a1a`,
//! `dev/benchmarks/gf2-sim/comparison/`). The AList file lets aff3ct decode
//! the **identical** parity-check matrix `gf2-coding` decodes, so the
//! side-by-side BLER comparison isolates channel + decoder behaviour and
//! cannot drift because of a code-construction mismatch (aff3ct
//! `--dec-h-path <file.alist>`).
//!
//! Two codes are exported, matching the two comparison configurations:
//!
//! * **DVB-T2 r1/2 Normal LDPC** — `LdpcCode::dvb_t2_normal(Rate1_2)`,
//!   the plain (un-rate-matched) ETSI EN 302 755 code (N = 64800, K = 32400).
//! * **5G NR BG1 r1/2 LDPC** — the *mother* code of
//!   `QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448)` (Z = 384). The
//!   comparison decodes the mother code directly (no puncturing/shortening)
//!   so both sides see one fixed `H`; the rate-matching column bookkeeping
//!   that `Nr5gRateMatchedCode` layers on top is a `gf2-coding`-internal
//!   concern not expressible to aff3ct and is therefore excluded from the
//!   isolated-decoder comparison (documented in the comparison README).
//!
//! # AList format (MacKay)
//!
//! For an `M × N` GF(2) parity-check matrix `H` (M rows = checks, N columns =
//! variable nodes):
//!
//! ```text
//! line 1: N M
//! line 2: dc_max dv_max          (max column weight, max row weight)
//! line 3: dc[0] dc[1] ... dc[N-1]   (per-column weights)
//! line 4: dv[0] dv[1] ... dv[M-1]   (per-row weights)
//! next N lines: for each column, the 1-indexed row indices of its nonzeros,
//!               zero-padded on the right to width dc_max
//! next M lines: for each row, the 1-indexed column indices of its nonzeros,
//!               zero-padded on the right to width dv_max
//! ```
//!
//! This is exactly the format aff3ct's `tools::AList::read` consumes.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p gf2-sim --release --bin export_alist -- \
//!     --code dvb-t2-r12 --output dvb_t2_r12.alist
//! cargo run -p gf2-sim --release --bin export_alist -- \
//!     --code nr-bg1-r12 --output nr_bg1_r12.alist
//! ```
//!
//! The harness driver (`run.sh`) invokes this once per code before sweeping.

use std::io::Write;
use std::path::PathBuf;

use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::{CodeRate, LdpcCode};
use gf2_core::sparse::SpBitMatrixDual;

/// Which committed comparison code to export.
#[derive(Clone, Copy)]
enum Code {
    /// DVB-T2 r1/2 Normal LDPC (ETSI EN 302 755): N = 64800, K = 32400.
    DvbT2R12,
    /// 5G NR BG1 r1/2 mother code (Z = 384): N = 68·384, K = 22·384.
    NrBg1R12,
}

impl Code {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "dvb-t2-r12" => Ok(Self::DvbT2R12),
            "nr-bg1-r12" => Ok(Self::NrBg1R12),
            other => Err(format!(
                "unknown --code '{other}' (expected 'dvb-t2-r12' or 'nr-bg1-r12')"
            )),
        }
    }

    /// Builds the parity-check matrix `H` (dual sparse) for this code.
    fn parity_check(self) -> SpBitMatrixDual {
        match self {
            Self::DvbT2R12 => {
                let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
                code.parity_check_matrix().clone()
            }
            Self::NrBg1R12 => {
                // The mother code: nr_5g_rate_matched(1, 16896, 8448) selects
                // Z = 384 for BG1; we export its full (un-rate-matched) H.
                let rm = QuasiCyclicLdpc::nr_5g_rate_matched(1, 16896, 8448);
                rm.mother_code().parity_check_matrix().clone()
            }
        }
    }
}

/// Writes `h` to `path` in MacKay AList format.
fn write_alist(h: &SpBitMatrixDual, path: &PathBuf) -> std::io::Result<()> {
    let m = h.rows();
    let n = h.cols();

    // Per-column nonzero row indices (1-indexed) and per-row nonzero column
    // indices (1-indexed).
    let cols: Vec<Vec<usize>> = (0..n)
        .map(|c| h.col_iter(c).map(|r| r + 1).collect())
        .collect();
    let rows: Vec<Vec<usize>> = (0..m)
        .map(|r| h.row_iter(r).map(|c| c + 1).collect())
        .collect();

    let dc_max = cols.iter().map(Vec::len).max().unwrap_or(0);
    let dv_max = rows.iter().map(Vec::len).max().unwrap_or(0);

    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);

    writeln!(f, "{n} {m}")?;
    writeln!(f, "{dc_max} {dv_max}")?;

    // Column weights.
    let col_w: Vec<String> = cols.iter().map(|c| c.len().to_string()).collect();
    writeln!(f, "{}", col_w.join(" "))?;
    // Row weights.
    let row_w: Vec<String> = rows.iter().map(|r| r.len().to_string()).collect();
    writeln!(f, "{}", row_w.join(" "))?;

    // Per-column entries, right-padded to dc_max with zeros.
    for c in &cols {
        write_padded(&mut f, c, dc_max)?;
    }
    // Per-row entries, right-padded to dv_max with zeros.
    for r in &rows {
        write_padded(&mut f, r, dv_max)?;
    }

    f.flush()
}

/// Writes one AList entry line: the indices, then `(width - len)` zeros.
fn write_padded<W: Write>(f: &mut W, indices: &[usize], width: usize) -> std::io::Result<()> {
    let mut parts: Vec<String> = indices.iter().map(usize::to_string).collect();
    parts.resize(width, "0".to_string());
    writeln!(f, "{}", parts.join(" "))
}

fn main() {
    let mut code: Option<Code> = None;
    let mut output: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--code" => {
                let v = args.next().expect("--code requires a value");
                code = Some(Code::parse(&v).unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(2);
                }));
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().expect("--output requires a value"),
                ));
            }
            "-h" | "--help" => {
                println!(
                    "export_alist --code <dvb-t2-r12|nr-bg1-r12> --output <file.alist>\n\
                     Exports a gf2-coding LDPC parity-check matrix to MacKay AList format."
                );
                return;
            }
            other => {
                eprintln!("error: unknown argument '{other}'");
                std::process::exit(2);
            }
        }
    }

    let code = code.unwrap_or_else(|| {
        eprintln!("error: --code is required");
        std::process::exit(2);
    });
    let output = output.unwrap_or_else(|| {
        eprintln!("error: --output is required");
        std::process::exit(2);
    });

    let h = code.parity_check();
    let (m, n, nnz) = (h.rows(), h.cols(), h.nnz());
    write_alist(&h, &output).unwrap_or_else(|e| {
        eprintln!("error: failed to write {}: {e}", output.display());
        std::process::exit(1);
    });

    println!(
        "wrote {} : H is {m} x {n} ({nnz} nonzeros), rate ~ {:.4}",
        output.display(),
        1.0 - (m as f64) / (n as f64)
    );
}
