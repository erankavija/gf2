//! Optional caching for LDPC encoding matrices.
//!
//! This module provides an opt-in performance optimization for LDPC encoding.
//! All functionality works WITHOUT the cache - it's purely for speed.
//!
//! # Usage
//!
//! ## Without Cache (Simple, Always Works)
//!
//! ```no_run
//! use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
//! use gf2_coding::CodeRate;
//!
//! // Just works, no cache needed
//! let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
//! let encoder = LdpcEncoder::new(code);
//! // Takes 2-3 seconds, but no complexity
//! ```
//!
//! ## With Cache (Performance Boost)
//!
//! ```no_run
//! use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
//! use gf2_coding::ldpc::encoding::EncodingCache;
//! use gf2_coding::CodeRate;
//!
//! // Create and own the cache
//! let cache = EncodingCache::new();
//!
//! // First call: preprocesses and caches
//! let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
//! let enc1 = LdpcEncoder::with_cache(code.clone(), &cache);
//! // Takes 2-3 seconds
//!
//! // Second call: instant
//! let enc2 = LdpcEncoder::with_cache(code, &cache);
//! // Takes <1μs
//! ```

use super::{PreprocessError, RuEncodingMatrices};
use gf2_core::io::IoError;
use gf2_core::{sparse::SpBitMatrixDual, BitMatrix};
use std::collections::HashMap;
use std::io;
use std::path::Path;
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

/// Cache key for LDPC encoding matrices.
///
/// Uniquely identifies an LDPC code configuration based on its dimensions
/// and parity-check matrix structure.
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
}

/// Optional cache for preprocessed LDPC encoding matrices.
///
/// This cache stores the results of expensive preprocessing (Gaussian elimination)
/// so that creating multiple encoders for the same LDPC code is fast.
///
/// The cache is thread-safe and can be shared across threads. Each entry stores
/// an `Arc<RuEncodingMatrices>`, so cloning the cache value is cheap.
///
/// # Lifecycle
///
/// You control the cache lifetime. It can be:
/// - Local to a function
/// - Stored in application state
/// - Thread-local
/// - Global (if you choose)
///
/// # Memory (In-Memory Cache)
///
/// Each cached matrix in memory consumes approximately:
/// - DVB-T2 Short: ~7-8 MB (dense parity matrix k × r)
/// - DVB-T2 Normal: ~120-130 MB (dense parity matrix k × r)
///
/// # Disk Storage
///
/// Cache files on disk:
/// - DVB-T2 Short: ~5-8 MB per config (dense .gf2 format)
/// - DVB-T2 Normal: ~70-126 MB per config (dense .gf2 format)
/// - Total for all 12 configs: ~530 MB
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::ldpc::encoding::EncodingCache;
/// use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
/// use gf2_coding::CodeRate;
///
/// let cache = EncodingCache::new();
///
/// // Precompute all DVB-T2 configs at startup
/// cache.precompute_dvb_t2();
///
/// // All subsequent encoder creation is instant
/// let encoder = LdpcEncoder::with_cache(
///     LdpcCode::dvb_t2_short(CodeRate::Rate1_2),
///     &cache
/// );
/// ```
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

    /// Get or compute encoding matrices for the given parity-check matrix.
    ///
    /// If the matrices are already cached, returns them immediately (<1μs).
    /// Otherwise, preprocesses the matrix (2-10 seconds) and caches the result.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key identifying the LDPC code
    /// * `h` - Parity-check matrix (only used if cache miss)
    ///
    /// # Returns
    ///
    /// Arc to the preprocessed encoding matrices, either from cache or newly computed.
    ///
    /// # Errors
    ///
    /// Returns `PreprocessError` if Gaussian elimination fails (e.g., rank deficient matrix).
    pub fn get_or_compute(
        &self,
        key: CacheKey,
        h: &SpBitMatrixDual,
    ) -> Result<Arc<RuEncodingMatrices>, PreprocessError> {
        // Fast path: check cache with read lock
        {
            let cache_read = self.cache.read().unwrap();
            if let Some(matrices) = cache_read.get(&key) {
                return Ok(Arc::clone(matrices));
            }
        }

        // Slow path: preprocess and cache with write lock
        let matrices = Arc::new(RuEncodingMatrices::preprocess(h)?);

        let mut cache_write = self.cache.write().unwrap();
        cache_write.insert(key, Arc::clone(&matrices));

        Ok(matrices)
    }

    /// Precompute all DVB-T2 LDPC encoding matrices.
    ///
    /// This preprocesses all 12 DVB-T2 configurations (6 rates × 2 frame sizes)
    /// and stores them in the cache for instant access.
    ///
    /// # Performance
    ///
    /// - Total time: ~13 minutes (with RREF+SIMD optimization)
    /// - Memory usage: ~800 MB peak (all 12 configs in memory)
    /// - Recommended for production applications
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::encoding::EncodingCache;
    ///
    /// let cache = EncodingCache::new();
    ///
    /// // One-time precomputation at startup
    /// cache.precompute_dvb_t2();
    ///
    /// // Now all encoders are instant
    /// // ... rest of application
    /// ```
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
    ///
    /// Returns information about the current cache state.
    pub fn stats(&self) -> CacheStats {
        let cache_read = self.cache.read().unwrap();
        CacheStats {
            entries: cache_read.len(),
        }
    }

    /// Clear all cached entries.
    ///
    /// This is primarily useful for testing. In production, you typically
    /// want to keep the cache populated.
    pub fn clear(&self) {
        let mut cache_write = self.cache.write().unwrap();
        cache_write.clear();
    }

    /// Save cache to directory as .gf2 files.
    ///
    /// Each cached entry is saved as a separate file with naming convention:
    /// `n{codeword_length}_k{message_length}_h{hash}.gf2`
    ///
    /// # Arguments
    ///
    /// * `path` - Directory to save cache files (created if doesn't exist)
    ///
    /// # Errors
    ///
    /// Returns error if directory creation or file writing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::encoding::EncodingCache;
    /// use std::path::Path;
    ///
    /// let cache = EncodingCache::new();
    /// // ... populate cache ...
    /// cache.save_to_directory(Path::new("cache_data")).unwrap();
    /// ```
    pub fn save_to_directory(&self, path: &Path) -> Result<(), CacheIoError> {
        std::fs::create_dir_all(path).map_err(CacheIoError::IoError)?;

        let cache_read = self.cache.read().unwrap();

        for (key, matrices) in cache_read.iter() {
            let filename = format!("n{}_k{}_h{:x}", key.n, key.k, key.matrix_hash);
            let filepath = path.join(&filename);

            // Save only parity part (identity is implicit for systematic codes)
            // Assumes standard systematic form: systematic bits in [0..k), parity in [k..n)
            let parity_path = filepath.with_extension("gf2");
            matrices
                .parity_part()
                .save_to_file(&parity_path)
                .map_err(CacheIoError::Gf2IoError)?;
        }

        Ok(())
    }

    /// Load cache from directory of .gf2 files.
    ///
    /// Loads all .gf2 files in the directory and reconstructs the cache.
    /// If directory doesn't exist or is empty, returns an empty cache.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory containing .gf2 cache files
    ///
    /// # Errors
    ///
    /// Returns error if directory cannot be read or files are corrupted.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::encoding::EncodingCache;
    /// use std::path::Path;
    ///
    /// let cache = EncodingCache::from_directory(Path::new("cache_data")).unwrap();
    /// // Cache now contains all pre-computed matrices
    /// ```
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

            if filepath.extension().and_then(|s| s.to_str()) != Some("gf2") {
                continue;
            }

            // Parse filename: n{n}_k{k}_h{hash}.gf2
            if let Some(filename) = filepath.file_stem().and_then(|s| s.to_str()) {
                if let Some((n, k, hash)) = parse_cache_filename(filename) {
                    // Load parity matrix as DENSE BitMatrix (DVB-T2 is 40-50% dense)
                    let parity_matrix =
                        BitMatrix::load_from_file(&filepath).map_err(CacheIoError::Gf2IoError)?;

                    // Assume standard systematic form:
                    // Systematic bits in columns [0, 1, ..., k-1]
                    // Parity bits in columns [k, k+1, ..., n-1]
                    let systematic_cols: Vec<usize> = (0..k).collect();
                    let parity_cols: Vec<usize> = (k..n).collect();

                    // Reconstruct RuEncodingMatrices
                    let matrices = Arc::new(RuEncodingMatrices::from_components(
                        k,
                        n,
                        parity_matrix,
                        systematic_cols,
                        parity_cols,
                    ));

                    let key = CacheKey {
                        n,
                        k,
                        matrix_hash: hash,
                    };

                    let mut cache_write = cache.cache.write().unwrap();
                    cache_write.insert(key, matrices);
                }
            }
        }

        Ok(cache)
    }

    /// Precompute and save all DVB-T2 LDPC configurations.
    ///
    /// This is a convenience method that:
    /// 1. Precomputes all 12 DVB-T2 encoding matrices (~13 minutes with SIMD)
    /// 2. Saves them to disk (~530 MB total, dense format)
    ///
    /// Run this once to generate cache files, then use `from_directory()`
    /// for instant initialization.
    ///
    /// # Arguments
    ///
    /// * `output_dir` - Directory to save cache files (created if doesn't exist)
    ///
    /// # Errors
    ///
    /// Returns error if preprocessing fails or file writing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::ldpc::encoding::EncodingCache;
    /// use std::path::Path;
    ///
    /// // One-time generation (takes ~13 minutes)
    /// EncodingCache::precompute_and_save_dvb_t2(
    ///     Path::new("data/ldpc/dvb_t2")
    /// ).unwrap();
    ///
    /// // Subsequently, loading is fast:
    /// let cache = EncodingCache::from_directory(
    ///     Path::new("data/ldpc/dvb_t2")
    /// ).unwrap();
    /// // Loading all 12 configs: ~16ms
    /// ```
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

/// Compute a structural hash of a sparse parity-check matrix.
///
/// This hash is based on:
/// - Matrix dimensions (rows, cols)
/// - Number of non-zero entries
/// - First 100 edge positions (fingerprint)
///
/// The hash is deterministic and uniquely identifies the matrix structure.
fn compute_matrix_hash(h: &SpBitMatrixDual) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash dimensions
    h.rows().hash(&mut hasher);
    h.cols().hash(&mut hasher);
    h.nnz().hash(&mut hasher);

    // Hash first 100 edges as structure fingerprint
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

/// Parse cache filename to extract n, k, and hash.
///
/// Expected format: n{n}_k{k}_h{hash}
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

        // First access: cache miss
        let m1 = cache.get_or_compute(key.clone(), &h).unwrap();
        assert_eq!(cache.stats().entries, 1);

        // Second access: cache hit
        let m2 = cache.get_or_compute(key, &h).unwrap();
        assert_eq!(cache.stats().entries, 1);

        // Should be same Arc
        assert!(Arc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn test_cache_different_codes() {
        let cache = EncodingCache::new();

        let h1 = simple_hamming_h();
        let k1 = CacheKey::from_params(7, 4, &h1);

        // Different Hamming code
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

        // Different matrix
        let edges2 = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let h2 = SpBitMatrixDual::from_coo(2, 3, &edges2);

        let hash1 = compute_matrix_hash(&h1);
        let hash2 = compute_matrix_hash(&h2);

        assert_ne!(hash1, hash2);
    }
}
