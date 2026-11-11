//! Opt-in cache of preprocessed LDPC encoding matrices.

use super::{PreprocessError, RuEncodingMatrices};
use gf2_core::io::IoError;
use gf2_core::{sparse::SpBitMatrixDual, BitMatrix};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Errors that can occur during cache I/O operations.
#[derive(Debug)]
pub enum CacheIoError {
    /// Standard I/O error (file not found, permissions, etc.)
    IoError(io::Error),
    /// GF(2) serialization error
    Gf2IoError(IoError),
}

impl std::fmt::Display for CacheIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "I/O error: {}", e),
            Self::Gf2IoError(e) => write!(f, "GF(2) serialization error: {}", e),
        }
    }
}

impl std::error::Error for CacheIoError {}

/// Cache key for LDPC encoding matrices: the code dimensions and a hash of the
/// parity-check matrix's dimensions, edge count and first 100 edges.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CacheKey {
    /// Codeword length
    n: usize,
    /// Message dimension
    k: usize,
    /// Structural hash of parity-check matrix
    matrix_hash: u64,
}

impl CacheKey {
    /// Create cache key from code parameters and parity-check matrix.
    pub fn from_params(n: usize, k: usize, h: &SpBitMatrixDual) -> Self {
        Self {
            n,
            k,
            matrix_hash: compute_matrix_hash(h),
        }
    }

    /// The files of this key's entry in the cache directory `dir`, named
    /// `n{codeword_length}_k{message_length}_h{hash}` with the extension of
    /// each file kind.
    pub fn entry_files(&self, dir: &Path) -> CacheEntryFiles {
        let stem = dir.join(format!("n{}_k{}_h{:x}", self.n, self.k, self.matrix_hash));
        CacheEntryFiles {
            parity: stem.with_extension(PARITY_EXTENSION),
            columns: stem.with_extension(COLUMNS_EXTENSION),
        }
    }
}

const PARITY_EXTENSION: &str = "gf2";
const COLUMNS_EXTENSION: &str = "cols";

/// The files one [`EncodingCache`] entry occupies in a cache directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntryFiles {
    /// The k × (n − k) parity part, in the binary `BitMatrix` format.
    pub parity: PathBuf,
    /// The column selection: the k message columns on the first line and the
    /// n − k parity columns on the second, as space-separated decimals.
    pub columns: PathBuf,
}

/// Cache of preprocessed LDPC encoding matrices, keyed by [`CacheKey`].
///
/// Thread-safe; each entry is an `Arc<RuEncodingMatrices>`.
#[derive(Default)]
pub struct EncodingCache {
    cache: RwLock<HashMap<CacheKey, Arc<RuEncodingMatrices>>>,
}

impl EncodingCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Look up encoding matrices without computing them on a miss, for a
    /// caller that treats a miss as an error, e.g. when verifying a cache
    /// loaded from disk with [`Self::from_directory`].
    pub fn get(&self, key: &CacheKey) -> Option<Arc<RuEncodingMatrices>> {
        let cache_read = self.cache.read().unwrap();
        cache_read.get(key).map(Arc::clone)
    }

    /// Returns the cached matrices for `key`; on a miss, preprocesses `h` at
    /// the cost of [`RuEncodingMatrices::preprocess`] and caches the result.
    ///
    /// # Errors
    ///
    /// Returns `PreprocessError` if Gaussian elimination fails (e.g., rank deficient matrix).
    pub fn get_or_compute(
        &self,
        key: CacheKey,
        h: &SpBitMatrixDual,
    ) -> Result<Arc<RuEncodingMatrices>, PreprocessError> {
        {
            let cache_read = self.cache.read().unwrap();
            if let Some(matrices) = cache_read.get(&key) {
                return Ok(Arc::clone(matrices));
            }
        }

        let matrices = Arc::new(RuEncodingMatrices::preprocess(h)?);

        let mut cache_write = self.cache.write().unwrap();
        cache_write.insert(key, Arc::clone(&matrices));

        Ok(matrices)
    }

    /// Preprocesses the encoding matrices of every DVB-T2 frame size and code
    /// rate into the cache.
    pub fn precompute_dvb_t2(&self) {
        use crate::bch::CodeRate;
        use crate::ldpc::dvb_t2::FrameSize;
        use crate::ldpc::LdpcCode;

        let configs = [
            (FrameSize::Short, CodeRate::Rate1_2),
            (FrameSize::Short, CodeRate::Rate3_5),
            (FrameSize::Short, CodeRate::Rate2_3),
            (FrameSize::Short, CodeRate::Rate3_4),
            (FrameSize::Short, CodeRate::Rate4_5),
            (FrameSize::Short, CodeRate::Rate5_6),
            (FrameSize::Normal, CodeRate::Rate1_2),
            (FrameSize::Normal, CodeRate::Rate3_5),
            (FrameSize::Normal, CodeRate::Rate2_3),
            (FrameSize::Normal, CodeRate::Rate3_4),
            (FrameSize::Normal, CodeRate::Rate4_5),
            (FrameSize::Normal, CodeRate::Rate5_6),
        ];

        eprintln!("Preprocessing all DVB-T2 LDPC configurations...");
        let total_start = std::time::Instant::now();

        for (i, (frame_size, rate)) in configs.iter().enumerate() {
            eprint!("[{:2}/12] {:?} {:?}... ", i + 1, frame_size, rate);
            let start = std::time::Instant::now();

            let code = match frame_size {
                FrameSize::Short => LdpcCode::dvb_t2_short(*rate),
                FrameSize::Normal => LdpcCode::dvb_t2_normal(*rate),
            };

            let key = CacheKey::from_params(code.n(), code.k(), code.parity_check_matrix());
            let _ = self.get_or_compute(key, code.parity_check_matrix());

            let elapsed = start.elapsed();
            eprintln!("done in {:.1}s", elapsed.as_secs_f64());
        }

        let total_elapsed = total_start.elapsed();
        eprintln!(
            "\nAll configurations preprocessed in {:.1}s",
            total_elapsed.as_secs_f64()
        );
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let cache_read = self.cache.read().unwrap();
        CacheStats {
            entries: cache_read.len(),
        }
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        let mut cache_write = self.cache.write().unwrap();
        cache_write.clear();
    }

    /// Saves each entry to the directory `path` (created if missing) as the
    /// files [`CacheKey::entry_files`] names.
    ///
    /// # Errors
    ///
    /// Returns error if directory creation or file writing fails.
    pub fn save_to_directory(&self, path: &Path) -> Result<(), CacheIoError> {
        std::fs::create_dir_all(path).map_err(CacheIoError::IoError)?;

        let cache_read = self.cache.read().unwrap();

        for (key, matrices) in cache_read.iter() {
            let files = key.entry_files(path);
            matrices
                .parity_part()
                .save_to_file(&files.parity)
                .map_err(CacheIoError::Gf2IoError)?;
            std::fs::write(
                &files.columns,
                format_columns(matrices.systematic_cols(), matrices.parity_cols()),
            )
            .map_err(CacheIoError::IoError)?;
        }

        Ok(())
    }

    /// Loads every entry of the directory `path` whose
    /// [`CacheKey::entry_files`] are both present. Other files and a parity
    /// file without its column selection are skipped, so
    /// [`Self::get_or_compute`] rebuilds that entry.
    ///
    /// # Errors
    ///
    /// Returns error if the directory does not exist or cannot be read, or a
    /// file is corrupted.
    pub fn from_directory(path: &Path) -> Result<Self, CacheIoError> {
        let cache = Self::new();

        if !path.exists() {
            return Err(CacheIoError::IoError(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory does not exist: {}", path.display()),
            )));
        }

        let entries = std::fs::read_dir(path).map_err(CacheIoError::IoError)?;

        for entry in entries {
            let entry = entry.map_err(CacheIoError::IoError)?;
            let filepath = entry.path();

            if filepath.extension().and_then(|s| s.to_str()) != Some(PARITY_EXTENSION) {
                continue;
            }

            if let Some(filename) = filepath.file_stem().and_then(|s| s.to_str()) {
                if let Some((n, k, hash)) = parse_cache_filename(filename) {
                    let key = CacheKey {
                        n,
                        k,
                        matrix_hash: hash,
                    };
                    let files = key.entry_files(path);
                    let columns = match std::fs::read_to_string(&files.columns) {
                        Ok(text) => text,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                        Err(error) => return Err(CacheIoError::IoError(error)),
                    };
                    let (systematic_cols, parity_cols) =
                        parse_columns(&columns, n, k).ok_or_else(|| {
                            CacheIoError::IoError(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("invalid column selection for {filename}"),
                            ))
                        })?;
                    let parity_matrix = BitMatrix::load_from_file(&files.parity)
                        .map_err(CacheIoError::Gf2IoError)?;

                    let matrices = Arc::new(RuEncodingMatrices::from_components(
                        k,
                        n,
                        parity_matrix,
                        systematic_cols,
                        parity_cols,
                    ));

                    let mut cache_write = cache.cache.write().unwrap();
                    cache_write.insert(key, matrices);
                }
            }
        }

        Ok(cache)
    }

    /// Runs [`Self::precompute_dvb_t2`] on a fresh cache and saves it to
    /// `output_dir` (created if missing) for [`Self::from_directory`].
    ///
    /// # Errors
    ///
    /// Returns error if file writing fails.
    pub fn precompute_and_save_dvb_t2(output_dir: &Path) -> Result<(), CacheIoError> {
        let cache = Self::new();
        cache.precompute_dvb_t2();
        cache.save_to_directory(output_dir)?;
        Ok(())
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached entries
    pub entries: usize,
}

/// Hashes the matrix dimensions, edge count and first 100 edges in row-major
/// order; matrices that agree on those collide.
fn compute_matrix_hash(h: &SpBitMatrixDual) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    h.rows().hash(&mut hasher);
    h.cols().hash(&mut hasher);
    h.nnz().hash(&mut hasher);

    let mut edge_count = 0;
    'outer: for row in 0..h.rows() {
        for col in h.row_iter(row) {
            (row, col).hash(&mut hasher);
            edge_count += 1;
            if edge_count >= 100 {
                break 'outer;
            }
        }
    }

    hasher.finish()
}

/// The systematic columns on the first line and the parity columns on the
/// second, space-separated.
fn format_columns(systematic: &[usize], parity: &[usize]) -> String {
    let line = |columns: &[usize]| {
        columns
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("{}\n{}\n", line(systematic), line(parity))
}

/// Parses [`format_columns`] output; `None` unless it lists `k` systematic and
/// `n - k` parity columns that together cover `0..n` once.
fn parse_columns(text: &str, n: usize, k: usize) -> Option<(Vec<usize>, Vec<usize>)> {
    let mut lines = text.lines();
    let mut column_line = || -> Option<Vec<usize>> {
        lines
            .next()?
            .split_whitespace()
            .map(|word| word.parse().ok())
            .collect()
    };
    let (systematic, parity) = (column_line()?, column_line()?);
    let mut seen = vec![false; n];
    for &column in systematic.iter().chain(&parity) {
        if column >= n || std::mem::replace(&mut seen[column], true) {
            return None;
        }
    }
    (systematic.len() == k && parity.len() == n - k).then_some((systematic, parity))
}

/// Parses `n{n}_k{k}_h{hash}` with the hash in hexadecimal.
fn parse_cache_filename(filename: &str) -> Option<(usize, usize, u64)> {
    let parts: Vec<&str> = filename.split('_').collect();
    if parts.len() != 3 {
        return None;
    }

    let n = parts[0].strip_prefix('n')?.parse().ok()?;
    let k = parts[1].strip_prefix('k')?.parse().ok()?;
    let hash = u64::from_str_radix(parts[2].strip_prefix('h')?, 16).ok()?;

    Some((n, k, hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::sparse::SpBitMatrixDual;

    fn simple_hamming_h() -> SpBitMatrixDual {
        let edges = vec![
            (0, 0),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 1),
            (1, 3),
            (1, 5),
            (2, 2),
            (2, 3),
            (2, 6),
        ];
        SpBitMatrixDual::from_coo(3, 7, &edges)
    }

    #[test]
    fn test_cache_creation() {
        let cache = EncodingCache::new();
        let stats = cache.stats();
        assert_eq!(stats.entries, 0);
    }

    #[test]
    fn test_cache_hit() {
        let cache = EncodingCache::new();
        let h = simple_hamming_h();
        let key = CacheKey::from_params(7, 4, &h);

        let m1 = cache.get_or_compute(key.clone(), &h).unwrap();
        assert_eq!(cache.stats().entries, 1);

        let m2 = cache.get_or_compute(key, &h).unwrap();
        assert_eq!(cache.stats().entries, 1);

        assert!(Arc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn test_cache_different_codes() {
        let cache = EncodingCache::new();

        let h1 = simple_hamming_h();
        let k1 = CacheKey::from_params(7, 4, &h1);

        let edges2 = vec![
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 4),
            (0, 5),
            (0, 6),
            (0, 7),
            (1, 0),
            (1, 1),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (1, 8),
            (2, 0),
            (2, 2),
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 7),
            (2, 8),
            (3, 1),
            (3, 2),
            (3, 3),
            (3, 4),
            (3, 6),
            (3, 7),
            (3, 8),
        ];
        let h2 = SpBitMatrixDual::from_coo(4, 15, &edges2);
        let k2 = CacheKey::from_params(15, 11, &h2);

        let _m1 = cache.get_or_compute(k1, &h1).unwrap();
        let _m2 = cache.get_or_compute(k2, &h2).unwrap();

        assert_eq!(cache.stats().entries, 2);
    }

    #[test]
    fn test_cache_clear() {
        let cache = EncodingCache::new();
        let h = simple_hamming_h();
        let key = CacheKey::from_params(7, 4, &h);

        let _m = cache.get_or_compute(key, &h).unwrap();
        assert_eq!(cache.stats().entries, 1);

        cache.clear();
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn test_matrix_hash_deterministic() {
        let h1 = simple_hamming_h();
        let h2 = simple_hamming_h();

        let hash1 = compute_matrix_hash(&h1);
        let hash2 = compute_matrix_hash(&h2);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_matrix_hash_different() {
        let h1 = simple_hamming_h();

        let edges2 = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let h2 = SpBitMatrixDual::from_coo(2, 3, &edges2);

        let hash1 = compute_matrix_hash(&h1);
        let hash2 = compute_matrix_hash(&h2);

        assert_ne!(hash1, hash2);
    }
}
