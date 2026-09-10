//! Shared input-bundle contract for the LDPC decoder baseline survey
//! (jit:c077a88b).
//!
//! Every arm of the survey — the gf2 arm and each external adapter — reads
//! one immutable *input bundle*: the parity-check matrix in MacKay AList
//! form, the transmitted codewords, and the recorded channel LLRs. Fixing
//! the bundle by content digest is what makes the matched-algorithm arms
//! comparable: both sides decode bit-identical LLRs over a bit-identical
//! `H`.

pub mod arm;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::Path;

/// Schema identifier of [`BundleManifest`].
pub const BUNDLE_SCHEMA: &str = "ldpc-survey-input-bundle-v1";

/// File name of the AList parity-check matrix inside a bundle.
pub const ALIST_FILE: &str = "h.alist";
/// File name of the transmitted codewords inside a bundle.
pub const CODEWORDS_FILE: &str = "codewords.bin";
/// File name of the recorded channel LLRs inside a bundle.
pub const LLRS_FILE: &str = "llrs.f32";
/// File name of the manifest inside a bundle.
pub const MANIFEST_FILE: &str = "manifest.json";

/// How a frame's transmitted codeword was produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodewordSource {
    /// Every frame transmits the all-zero codeword.
    AllZero,
    /// Every frame transmits an encoded uniformly random message.
    Random,
    /// Even frames are all-zero, odd frames are encoded random messages.
    Both,
}

/// Immutable description of one input bundle.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    pub schema: String,
    /// `ComparisonCode` name: `dvb-t2-r12` or `nr-bg1-r12`.
    pub code: String,
    pub n: usize,
    pub k: usize,
    pub m: usize,
    /// Number of nonzeros in `H`.
    pub nnz: usize,
    /// SHA-256 of `h.alist`; the code identity the addendum pins.
    pub h_sha256: String,
    /// SHA-256 of `codewords.bin`.
    pub codewords_sha256: String,
    /// SHA-256 of `llrs.f32`.
    pub llrs_sha256: String,
    pub frames: usize,
    pub seed: u64,
    pub esn0_db: f64,
    /// AWGN noise standard deviation, `sqrt(1 / (2 * 10^(esn0_db / 10)))`.
    pub sigma: f64,
    pub codeword_source: CodewordSource,
    /// Number of leading codeword positions that carry no transmitted LLR.
    ///
    /// The recorded LLR file always holds `n` values per frame; a punctured
    /// prefix means the first `punctured_prefix` of them are exactly zero and
    /// an adapter whose decoder takes the shortened codeblock passes only the
    /// remaining `n - punctured_prefix` values.
    pub punctured_prefix: usize,
}

/// Reads and validates a bundle from `dir`.
///
/// # Errors
///
/// Returns an error when a bundle file is missing, when the manifest does not
/// decode, or when a recorded digest does not match the bytes on disk.
pub fn load_manifest(dir: &Path) -> io::Result<BundleManifest> {
    let bytes = fs::read(dir.join(MANIFEST_FILE))?;
    let manifest: BundleManifest = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if manifest.schema != BUNDLE_SCHEMA {
        return Err(io::Error::other(format!(
            "bundle schema {:?} is not {BUNDLE_SCHEMA}",
            manifest.schema
        )));
    }
    Ok(manifest)
}

/// Returns the lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Returns the lowercase hexadecimal SHA-256 of the file at `path`.
///
/// # Errors
///
/// Returns an error when the file cannot be read.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    Ok(sha256_hex(&fs::read(path)?))
}

/// Verifies that every recorded digest in `manifest` matches `dir`.
///
/// # Errors
///
/// Returns an error naming the first file whose digest differs.
pub fn verify_digests(dir: &Path, manifest: &BundleManifest) -> io::Result<()> {
    for (file, want) in [
        (ALIST_FILE, &manifest.h_sha256),
        (CODEWORDS_FILE, &manifest.codewords_sha256),
        (LLRS_FILE, &manifest.llrs_sha256),
    ] {
        let got = sha256_file(&dir.join(file))?;
        if &got != want {
            return Err(io::Error::other(format!(
                "{file}: recorded {want}, found {got}"
            )));
        }
    }
    Ok(())
}

/// Reads `frames * n` little-endian `f32` LLRs.
///
/// # Errors
///
/// Returns an error when the file length is not `4 * frames * n` bytes.
pub fn read_llrs(dir: &Path, frames: usize, n: usize) -> io::Result<Vec<f32>> {
    let bytes = fs::read(dir.join(LLRS_FILE))?;
    if bytes.len() != 4 * frames * n {
        return Err(io::Error::other(format!(
            "{LLRS_FILE} holds {} bytes, expected {}",
            bytes.len(),
            4 * frames * n
        )));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect())
}

/// Reads `frames * n` transmitted bits, one byte per bit.
///
/// # Errors
///
/// Returns an error when the file length is not `frames * n` bytes or a byte
/// is neither 0 nor 1.
pub fn read_codewords(dir: &Path, frames: usize, n: usize) -> io::Result<Vec<u8>> {
    let bytes = fs::read(dir.join(CODEWORDS_FILE))?;
    if bytes.len() != frames * n {
        return Err(io::Error::other(format!(
            "{CODEWORDS_FILE} holds {} bytes, expected {}",
            bytes.len(),
            frames * n
        )));
    }
    if bytes.iter().any(|bit| *bit > 1) {
        return Err(io::Error::other(format!(
            "{CODEWORDS_FILE} is not 0/1 bytes"
        )));
    }
    Ok(bytes)
}

/// Reads the recorded MacKay AList into a decoder code without building an encoder.
///
/// For T input bytes and E edges, loading takes O(T + E log E) time and
/// O(T + E + N + M) auxiliary space.
///
/// Uses the padded, one-indexed row and column sections emitted by
/// `gf2-sim::export_alist`; both sections must describe the same graph.
///
/// # Errors
///
/// Returns an error for unreadable input or inconsistent dimensions, degrees,
/// adjacency or padding. Sparse code construction follows `LdpcCode::from_edges`.
pub fn read_alist_code(path: &Path) -> io::Result<gf2_coding::ldpc::LdpcCode> {
    parse_alist_code(&fs::read_to_string(path)?)
}

fn parse_alist_code(text: &str) -> io::Result<gf2_coding::ldpc::LdpcCode> {
    let bad = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let mut tokens = text.split_whitespace();
    let mut next = || -> io::Result<usize> {
        tokens
            .next()
            .ok_or_else(|| bad("truncated AList"))?
            .parse()
            .map_err(|_| bad("AList contains a non-integer"))
    };
    let n = next()?;
    let m = next()?;
    let max_column = next()?;
    let max_row = next()?;
    if m > n {
        return Err(bad("AList has more checks than variables"));
    }
    let column_weights: Vec<_> = (0..n).map(|_| next()).collect::<io::Result<_>>()?;
    let row_weights: Vec<_> = (0..m).map(|_| next()).collect::<io::Result<_>>()?;
    if column_weights.iter().any(|&d| d > max_column) || row_weights.iter().any(|&d| d > max_row) {
        return Err(bad("AList degree exceeds its declared maximum"));
    }
    let mut rows = vec![Vec::new(); m];
    let mut edges = Vec::new();
    for (column, &degree) in column_weights.iter().enumerate() {
        for slot in 0..max_column {
            let entry = next()?;
            if slot < degree {
                if entry == 0 || entry > m {
                    return Err(bad("invalid AList row index"));
                }
                rows[entry - 1].push(column);
                edges.push((entry - 1, column));
            } else if entry != 0 {
                return Err(bad("nonzero AList column padding"));
            }
        }
    }
    for (row, &degree) in row_weights.iter().enumerate() {
        let mut observed = Vec::new();
        for slot in 0..max_row {
            let entry = next()?;
            if slot < degree {
                if entry == 0 || entry > n {
                    return Err(bad("invalid AList column index"));
                }
                observed.push(entry - 1);
            } else if entry != 0 {
                return Err(bad("nonzero AList row padding"));
            }
        }
        observed.sort_unstable();
        if observed != rows[row] || observed.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(bad(
                "AList row and column graphs disagree or repeat an edge",
            ));
        }
    }
    if tokens.next().is_some() {
        return Err(bad("trailing AList values"));
    }
    Ok(gf2_coding::ldpc::LdpcCode::from_edges(m, n, &edges))
}

#[cfg(test)]
mod alist_tests {
    use super::parse_alist_code;
    use gf2_core::BitVec;

    #[test]
    fn alist_preserves_parity_at_word_boundaries() {
        for n in [0, 1, 63, 64, 65] {
            let input = if n == 0 {
                "0 0\n0 0\n\n\n".to_owned()
            } else {
                let degree = n.min(2);
                let weights: Vec<_> = (0..n)
                    .map(|i| usize::from(i == 0 || i == n - 1).to_string())
                    .collect();
                let columns = weights.join("\n");
                let row = if n == 1 {
                    "1".to_owned()
                } else {
                    format!("1 {n}")
                };
                format!(
                    "{n} 1\n1 {degree}\n{}\n{degree}\n{columns}\n{row}\n",
                    weights.join(" ")
                )
            };
            let code = parse_alist_code(&input).unwrap();
            let mut word = BitVec::zeros(n);
            assert!(code.is_valid_codeword(&word));
            if n > 1 {
                word.set(0, true);
                word.set(n - 1, true);
                assert!(code.is_valid_codeword(&word));
            }
            if n != 0 {
                word.set(n - 1, !word.get(n - 1));
                assert!(!code.is_valid_codeword(&word));
            }
        }
    }

    #[test]
    fn alist_rejects_disagreeing_graph_sections() {
        let input = "3 2\n2 2\n1 2 1\n2 2\n1 0\n1 2\n2 0\n1 2\n2 3\n";
        assert!(parse_alist_code(input).is_ok());
        assert!(parse_alist_code(&input.replace("2 3\n", "1 3\n")).is_err());
    }
}
