//! Exports a gf2-coding LDPC parity-check matrix to MacKay AList format, so
//! that aff3ct (`@/citation/Cassagne2019`, `--dec-h-path <file.alist>`) decodes
//! the same matrix. The code selection is [`ComparisonCode`], shared with the
//! `ldpc_bler_sweep` binary.
//!
//! # AList format
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

use std::io::Write;
use std::path::PathBuf;

use gf2_core::sparse::SpBitMatrixDual;
use gf2_sim::testutil::ComparisonCode;

fn write_alist(h: &SpBitMatrixDual, path: &PathBuf) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    write_alist_into(h, &mut f)?;
    f.flush()
}

fn write_alist_into<W: Write>(h: &SpBitMatrixDual, f: &mut W) -> std::io::Result<()> {
    let m = h.rows();
    let n = h.cols();

    let cols: Vec<Vec<usize>> = (0..n)
        .map(|c| h.col_iter(c).map(|r| r + 1).collect())
        .collect();
    let rows: Vec<Vec<usize>> = (0..m)
        .map(|r| h.row_iter(r).map(|c| c + 1).collect())
        .collect();

    let dc_max = cols.iter().map(Vec::len).max().unwrap_or(0);
    let dv_max = rows.iter().map(Vec::len).max().unwrap_or(0);

    writeln!(f, "{n} {m}")?;
    writeln!(f, "{dc_max} {dv_max}")?;

    let col_w: Vec<String> = cols.iter().map(|c| c.len().to_string()).collect();
    writeln!(f, "{}", col_w.join(" "))?;
    let row_w: Vec<String> = rows.iter().map(|r| r.len().to_string()).collect();
    writeln!(f, "{}", row_w.join(" "))?;

    for c in &cols {
        write_padded(f, c, dc_max)?;
    }
    for r in &rows {
        write_padded(f, r, dv_max)?;
    }

    Ok(())
}

fn write_padded<W: Write>(f: &mut W, indices: &[usize], width: usize) -> std::io::Result<()> {
    let mut parts: Vec<String> = indices.iter().map(usize::to_string).collect();
    parts.resize(width, "0".to_string());
    writeln!(f, "{}", parts.join(" "))
}

fn main() {
    let mut code: Option<ComparisonCode> = None;
    let mut output: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--code" => {
                let v = args.next().expect("--code requires a value");
                code = Some(ComparisonCode::parse(&v).unwrap_or_else(|e| {
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

    let h: SpBitMatrixDual = code.build().parity_check_matrix().clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn alist_string(h: &SpBitMatrixDual) -> String {
        let mut buf = Vec::new();
        write_alist_into(h, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn test_write_alist_small_matrix_exact() {
        let h = SpBitMatrixDual::from_coo(2, 3, &[(0, 0), (0, 2), (1, 1), (1, 2)]);
        assert_eq!(
            alist_string(&h),
            "3 2\n\
             2 2\n\
             1 1 2\n\
             2 2\n\
             1 0\n\
             2 0\n\
             1 2\n\
             1 3\n\
             2 3\n"
        );
    }

    #[test]
    fn test_write_alist_zero_weight_column_pads_with_zeros() {
        let h = SpBitMatrixDual::from_coo(2, 3, &[(0, 0), (1, 0)]);
        assert_eq!(
            alist_string(&h),
            "3 2\n\
             2 1\n\
             2 0 0\n\
             1 1\n\
             1 2\n\
             0 0\n\
             0 0\n\
             1\n\
             1\n"
        );
    }

    #[test]
    fn test_write_alist_one_indexed() {
        let h = SpBitMatrixDual::from_coo(1, 1, &[(0, 0)]);
        assert_eq!(alist_string(&h), "1 1\n1 1\n1\n1\n1\n1\n");
    }
}
