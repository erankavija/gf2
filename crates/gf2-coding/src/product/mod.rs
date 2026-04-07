//! Product code construction and iterative block turbo decoder.
//!
//! A product code is formed by arranging information bits in a matrix and encoding
//! rows and columns independently with a component code. Given a component (n, k)
//! code, the product code has parameters (n^2, k^2).
//!
//! # Construction
//!
//! 1. Arrange k^2 information bits as a k x k matrix.
//! 2. Encode each row with the component encoder to produce a k x n matrix.
//! 3. Encode each column of the k x n matrix to produce an n x n codeword matrix.
//!
//! # Turbo Decoding
//!
//! The block turbo decoder iterates between row and column SISO decoding using
//! [`SoGrand`](crate::grand::SoGrand) as the component decoder. Extrinsic
//! information is exchanged between row and column steps with a scaling factor
//! alpha (typically 0.5). Early termination occurs when the hard-decision matrix
//! forms a valid product codeword.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
//! use gf2_coding::bch::extended::ExtendedBchCode;
//! use gf2_coding::traits::BlockEncoder;
//! use gf2_core::BitVec;
//!
//! let component = ExtendedBchCode::ebch_16_11();
//! let product = ProductCode::new(component);
//! assert_eq!(product.n(), 16 * 16);
//! assert_eq!(product.k(), 11 * 11);
//!
//! let msg = BitVec::zeros(product.k());
//! let codeword = product.encode(&msg);
//! assert_eq!(codeword.len(), product.n());
//! ```
//!
//! # References
//!
//! - Pyndiah, R.M. (1998). "Near-optimum decoding of product codes: Block turbo
//!   codes." *IEEE Trans. Commun.*
//! - Chase, D. (1972). "A class of algorithms for decoding block codes with channel
//!   measurement information." *IEEE Trans. Inform. Theory.*

use crate::grand::{OrbGrand, OrbGrandConfig, SoGrand};
use crate::llr::Llr;
use crate::traits::BlockEncoder;
use gf2_core::{BitMatrix, BitVec};

/// A product code constructed from a component (n, k) linear block code.
///
/// The product code has parameters (n^2, k^2) and is formed by encoding rows
/// and columns of a k x k information matrix with the component code.
///
/// # Arguments
///
/// Constructed with a component code that implements [`BlockEncoder`] and
/// provides a parity-check matrix via [`parity_check()`](Self::parity_check).
///
/// # Examples
///
/// ```
/// use gf2_coding::product::ProductCode;
/// use gf2_coding::bch::extended::ExtendedBchCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// let component = ExtendedBchCode::ebch_16_11();
/// let product = ProductCode::new(component);
///
/// assert_eq!(product.n(), 256);
/// assert_eq!(product.k(), 121);
///
/// let msg = BitVec::zeros(121);
/// let cw = product.encode(&msg);
/// assert_eq!(cw.len(), 256);
/// ```
#[derive(Debug, Clone)]
pub struct ProductCode {
    /// The component (n, k) code used for row and column encoding.
    component: crate::bch::extended::ExtendedBchCode,
    /// Component code length.
    comp_n: usize,
    /// Component message length.
    comp_k: usize,
}

impl ProductCode {
    /// Creates a new product code from the given component code.
    ///
    /// # Arguments
    ///
    /// * `component` - The component (n, k) code. Must implement [`BlockEncoder`]
    ///   and have an accessible parity-check matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// assert_eq!(product.n(), 256);
    /// assert_eq!(product.k(), 121);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1) — the constructor just stores the component code.
    pub fn new(component: crate::bch::extended::ExtendedBchCode) -> Self {
        let comp_n = component.n();
        let comp_k = component.k();
        Self {
            component,
            comp_n,
            comp_k,
        }
    }

    /// Returns the product code codeword length (n^2).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// assert_eq!(product.n(), 256);
    /// ```
    pub fn n(&self) -> usize {
        self.comp_n * self.comp_n
    }

    /// Returns the product code message length (k^2).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// assert_eq!(product.k(), 121);
    /// ```
    pub fn k(&self) -> usize {
        self.comp_k * self.comp_k
    }

    /// Returns a reference to the component code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// assert_eq!(product.component().n(), 16);
    /// ```
    pub fn component(&self) -> &crate::bch::extended::ExtendedBchCode {
        &self.component
    }

    /// Encodes a product code message into a flat codeword vector.
    ///
    /// The encoding procedure:
    /// 1. Arrange k^2 message bits as a k x k matrix (row-major).
    /// 2. Encode each row to produce a k x n matrix.
    /// 3. Encode each column to produce an n x n codeword matrix.
    /// 4. Flatten the n x n matrix to a length-n^2 vector (row-major).
    ///
    /// # Arguments
    ///
    /// * `message` - A bit vector of length k^2 containing the message bits.
    ///
    /// # Returns
    ///
    /// A bit vector of length n^2 containing the encoded product codeword.
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != k^2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// let msg = BitVec::zeros(121);
    /// let cw = product.encode(&msg);
    /// assert_eq!(cw.len(), 256);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n * k + n * n) where n and k are the component code parameters.
    /// Specifically, k row encodings of length n plus n column encodings of length n.
    pub fn encode_product(&self, message: &BitVec) -> BitVec {
        let k = self.comp_k;
        let n = self.comp_n;
        assert_eq!(
            message.len(),
            k * k,
            "Message length {} must equal k^2 = {}",
            message.len(),
            k * k
        );

        // Step 1: Arrange into k x k matrix and encode rows -> k x n
        let mut row_encoded = BitMatrix::zeros(k, n);
        for i in 0..k {
            let mut row_msg = BitVec::with_capacity(k);
            for j in 0..k {
                row_msg.push_bit(message.get(i * k + j));
            }
            let row_cw = self.component.encode(&row_msg);
            for j in 0..n {
                row_encoded.set(i, j, row_cw.get(j));
            }
        }

        // Step 2: Encode columns -> n x n
        let mut codeword_matrix = BitMatrix::zeros(n, n);
        for j in 0..n {
            // Extract column j from row_encoded (k elements)
            let mut col = BitVec::with_capacity(k);
            for i in 0..k {
                col.push_bit(row_encoded.get(i, j));
            }
            let col_cw = self.component.encode(&col);
            for i in 0..n {
                codeword_matrix.set(i, j, col_cw.get(i));
            }
        }

        // Step 3: Flatten to vector (row-major)
        let mut result = BitVec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                result.push_bit(codeword_matrix.get(i, j));
            }
        }
        result
    }

    /// Checks whether a given n x n bit matrix is a valid product codeword.
    ///
    /// Verifies that every row and every column has zero syndrome under the
    /// component code's parity-check matrix.
    ///
    /// # Arguments
    ///
    /// * `matrix` - An n x n matrix of hard-decision bits.
    ///
    /// # Returns
    ///
    /// `true` if every row and column is a valid component codeword.
    ///
    /// # Panics
    ///
    /// Panics if `matrix` dimensions are not n x n.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// let msg = BitVec::zeros(121);
    /// let cw = product.encode(&msg);
    ///
    /// // Reshape to matrix and check validity
    /// let matrix = product.flat_to_matrix(&cw);
    /// assert!(product.is_valid_codeword(&matrix));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n^2 * r) where r = n - k is the number of parity checks per component.
    pub fn is_valid_codeword(&self, matrix: &BitMatrix) -> bool {
        let n = self.comp_n;
        assert_eq!(matrix.rows(), n);
        assert_eq!(matrix.cols(), n);
        let h = self.component.parity_check();

        // Check all rows
        for i in 0..n {
            let mut row = BitVec::with_capacity(n);
            for j in 0..n {
                row.push_bit(matrix.get(i, j));
            }
            let syn = h.matvec(&row);
            if syn.count_ones() > 0 {
                return false;
            }
        }

        // Check all columns
        for j in 0..n {
            let mut col = BitVec::with_capacity(n);
            for i in 0..n {
                col.push_bit(matrix.get(i, j));
            }
            let syn = h.matvec(&col);
            if syn.count_ones() > 0 {
                return false;
            }
        }

        true
    }

    /// Converts a flat codeword vector to an n x n matrix.
    ///
    /// # Arguments
    ///
    /// * `flat` - A bit vector of length n^2 (row-major order).
    ///
    /// # Returns
    ///
    /// An n x n `BitMatrix`.
    ///
    /// # Panics
    ///
    /// Panics if `flat.len() != n^2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_core::BitVec;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// let flat = BitVec::zeros(256);
    /// let matrix = product.flat_to_matrix(&flat);
    /// assert_eq!(matrix.rows(), 16);
    /// assert_eq!(matrix.cols(), 16);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n^2).
    pub fn flat_to_matrix(&self, flat: &BitVec) -> BitMatrix {
        let n = self.comp_n;
        assert_eq!(
            flat.len(),
            n * n,
            "Flat vector length {} must equal n^2 = {}",
            flat.len(),
            n * n
        );
        let mut matrix = BitMatrix::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                matrix.set(i, j, flat.get(i * n + j));
            }
        }
        matrix
    }

    /// Converts an n x n matrix to a flat codeword vector (row-major).
    ///
    /// # Arguments
    ///
    /// * `matrix` - An n x n `BitMatrix`.
    ///
    /// # Returns
    ///
    /// A bit vector of length n^2.
    ///
    /// # Panics
    ///
    /// Panics if `matrix` dimensions are not n x n.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_core::BitMatrix;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// let matrix = BitMatrix::zeros(16, 16);
    /// let flat = product.matrix_to_flat(&matrix);
    /// assert_eq!(flat.len(), 256);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n^2).
    pub fn matrix_to_flat(&self, matrix: &BitMatrix) -> BitVec {
        let n = self.comp_n;
        assert_eq!(matrix.rows(), n);
        assert_eq!(matrix.cols(), n);
        let mut result = BitVec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                result.push_bit(matrix.get(i, j));
            }
        }
        result
    }

    /// Extracts message bits from a valid product codeword matrix.
    ///
    /// For a systematic code, the message bits occupy the top-left k x k submatrix.
    ///
    /// # Arguments
    ///
    /// * `matrix` - An n x n codeword matrix.
    ///
    /// # Returns
    ///
    /// A bit vector of length k^2 containing the extracted message.
    ///
    /// # Panics
    ///
    /// Panics if `matrix` dimensions are not n x n.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductCode;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// let msg = BitVec::zeros(121);
    /// let cw = product.encode(&msg);
    /// let matrix = product.flat_to_matrix(&cw);
    /// let extracted = product.extract_message(&matrix);
    /// assert_eq!(extracted, msg);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(k^2).
    pub fn extract_message(&self, matrix: &BitMatrix) -> BitVec {
        let n = self.comp_n;
        let k = self.comp_k;
        assert_eq!(matrix.rows(), n);
        assert_eq!(matrix.cols(), n);
        let mut msg = BitVec::with_capacity(k * k);
        for i in 0..k {
            for j in 0..k {
                msg.push_bit(matrix.get(i, j));
            }
        }
        msg
    }
}

impl BlockEncoder for ProductCode {
    fn k(&self) -> usize {
        self.comp_k * self.comp_k
    }

    fn n(&self) -> usize {
        self.comp_n * self.comp_n
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        self.encode_product(message)
    }
}

/// Configuration for the iterative block turbo decoder.
///
/// Controls the maximum number of turbo iterations, the extrinsic scaling
/// factor, and the ORBGRAND configuration for the component SISO decoder.
///
/// # Examples
///
/// ```
/// use gf2_coding::product::TurboDecoderConfig;
///
/// let config = TurboDecoderConfig::default();
/// assert_eq!(config.max_iterations, 20);
/// assert!((config.alpha - 0.5).abs() < 1e-10);
/// assert_eq!(config.list_size, 4);
/// ```
#[derive(Debug, Clone)]
pub struct TurboDecoderConfig {
    /// Maximum number of row-column iteration pairs.
    pub max_iterations: usize,

    /// Extrinsic information scaling factor (typically 0.5).
    ///
    /// The scaled extrinsic LLRs `alpha * L_E` are fed as a-priori
    /// information to the next decoder step. Smaller alpha provides
    /// more stable convergence at the cost of slower convergence.
    pub alpha: f32,

    /// ORBGRAND list size for the component SISO decoder.
    ///
    /// Larger list sizes improve soft-output quality but increase
    /// decoding complexity.
    pub list_size: usize,

    /// Maximum ORBGRAND queries per component decode.
    pub max_queries: usize,
}

impl Default for TurboDecoderConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            alpha: 0.5,
            list_size: 4,
            max_queries: 1_000_000,
        }
    }
}

/// Result of a turbo decoding operation.
///
/// Contains the decoded message bits, convergence information, and
/// performance statistics.
///
/// # Examples
///
/// ```
/// use gf2_coding::product::TurboDecoderResult;
/// use gf2_core::BitVec;
///
/// let result = TurboDecoderResult {
///     decoded_bits: BitVec::zeros(121),
///     iterations: 3,
///     converged: true,
///     total_queries: 500,
/// };
/// assert!(result.converged);
/// assert_eq!(result.iterations, 3);
/// ```
#[derive(Debug, Clone)]
pub struct TurboDecoderResult {
    /// The decoded message bits (length k^2).
    pub decoded_bits: BitVec,

    /// Number of row-column iteration pairs performed.
    pub iterations: usize,

    /// Whether the decoder converged to a valid product codeword.
    pub converged: bool,

    /// Total number of ORBGRAND queries across all component decodes.
    pub total_queries: usize,
}

/// Iterative block turbo decoder using SOGRAND as the component SISO decoder.
///
/// The turbo decoder alternates between row-wise and column-wise SISO decoding,
/// exchanging extrinsic information between steps. It uses early termination
/// when the hard-decision matrix forms a valid product codeword.
///
/// # Algorithm
///
/// 1. Initialize: L_Ch = n x n channel LLR matrix, L_A = 0
/// 2. **Row step**: for each row, decode with SISO-SOGRAND(L_Ch + L_A), compute
///    L_E = L_APP - L_A - L_Ch. Check if hard decision is valid -> early exit.
/// 3. Set L_A = alpha * L_E
/// 4. **Column step**: for each column, decode with SISO-SOGRAND(L_Ch + L_A),
///    compute L_E = L_APP - L_A - L_Ch. Check validity -> early exit.
/// 5. Set L_A = alpha * L_E, go to step 2.
/// 6. Repeat up to `max_iterations` pairs.
///
/// # Examples
///
/// ```
/// use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
/// use gf2_coding::bch::extended::ExtendedBchCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_coding::llr::Llr;
/// use gf2_core::BitVec;
///
/// let component = ExtendedBchCode::ebch_16_11();
/// let product = ProductCode::new(component.clone());
///
/// let config = TurboDecoderConfig {
///     max_iterations: 5,
///     list_size: 2,
///     max_queries: 10_000,
///     ..TurboDecoderConfig::default()
/// };
/// let decoder = TurboDecoder::new(component, config);
///
/// // Encode all-zeros and create high-confidence LLRs
/// let msg = BitVec::zeros(product.k());
/// let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
/// let result = decoder.decode(&llrs);
/// assert!(result.converged);
/// ```
///
/// # Complexity
///
/// O(I * n * Q) where I is the number of iterations, n is the component code
/// length, and Q is the average ORBGRAND query count per component decode.
/// Each iteration performs 2n component SISO decodes (n rows + n columns).
pub struct TurboDecoder {
    /// Component code for encoding/validity checks.
    component: crate::bch::extended::ExtendedBchCode,
    /// Decoder configuration.
    config: TurboDecoderConfig,
    /// SOGRAND instance for component SISO decoding.
    sogrand: SoGrand,
    /// Product code for validity checking.
    product_code: ProductCode,
}

impl TurboDecoder {
    /// Creates a new turbo decoder for the given component code.
    ///
    /// # Arguments
    ///
    /// * `component` - The component (n, k) extended BCH code.
    /// * `config` - Decoder configuration controlling iterations, scaling, and
    ///   ORBGRAND parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{TurboDecoder, TurboDecoderConfig};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let decoder = TurboDecoder::new(component, TurboDecoderConfig::default());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n^2) for constructing the sparse parity-check matrix used by ORBGRAND.
    pub fn new(
        component: crate::bch::extended::ExtendedBchCode,
        config: TurboDecoderConfig,
    ) -> Self {
        let h = component.parity_check().clone();
        let orb_config = OrbGrandConfig {
            list_size: config.list_size,
            max_queries: config.max_queries,
            even_code: component.is_even(),
            systematic: true,
        };
        let orbgrand = OrbGrand::new(h, orb_config);
        let sogrand = SoGrand::new(orbgrand);
        let product_code = ProductCode::new(component.clone());
        Self {
            component,
            config,
            sogrand,
            product_code,
        }
    }

    /// Decodes a received product codeword from channel LLRs.
    ///
    /// The LLR vector is interpreted as an n x n matrix in row-major order.
    ///
    /// # Arguments
    ///
    /// * `channel_llrs` - Channel LLRs of length n^2. Positive means bit 0
    ///   is more likely; negative means bit 1.
    ///
    /// # Returns
    ///
    /// A [`TurboDecoderResult`] containing decoded message bits and statistics.
    ///
    /// # Panics
    ///
    /// Panics if `channel_llrs.len() != n^2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_coding::llr::Llr;
    /// use gf2_core::BitVec;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let product = ProductCode::new(component.clone());
    /// let config = TurboDecoderConfig {
    ///     max_iterations: 3,
    ///     list_size: 2,
    ///     max_queries: 10_000,
    ///     ..TurboDecoderConfig::default()
    /// };
    /// let decoder = TurboDecoder::new(component, config);
    ///
    /// let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
    /// let result = decoder.decode(&llrs);
    /// assert!(result.converged);
    /// assert_eq!(result.decoded_bits.len(), product.k());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(I * n * Q) where I is the number of iterations, n is the component
    /// code length, and Q is the average ORBGRAND query count per component decode.
    pub fn decode(&self, channel_llrs: &[Llr]) -> TurboDecoderResult {
        let n = self.component.n();
        let n_sq = n * n;
        assert_eq!(
            channel_llrs.len(),
            n_sq,
            "Channel LLR length {} must equal n^2 = {}",
            channel_llrs.len(),
            n_sq
        );

        // Reshape channel LLRs into n x n matrix (row-major)
        let l_ch: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| channel_llrs[i * n + j].value()).collect())
            .collect();

        // Initialize a-priori LLRs to zero
        let mut l_a: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
        let mut total_queries: usize = 0;

        for iteration in 0..self.config.max_iterations {
            // === Row step ===
            let mut l_app_row: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for i in 0..n {
                // Input to SISO: L_Ch + L_A for this row
                let input: Vec<Llr> = (0..n).map(|j| Llr::new(l_ch[i][j] + l_a[i][j])).collect();
                let siso_result = self.sogrand.decode_siso(&input);
                total_queries += siso_result.query_count;
                for (j, app_llr) in siso_result.app_llrs.iter().enumerate() {
                    l_app_row[i][j] = app_llr.value();
                }
            }

            // Compute extrinsic: L_E = L_APP - L_A - L_Ch
            let mut l_e: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for i in 0..n {
                for j in 0..n {
                    l_e[i][j] = l_app_row[i][j] - l_a[i][j] - l_ch[i][j];
                }
            }

            // Check early termination: hard decision on L_APP
            if self.check_early_termination(&l_app_row) {
                let decoded = self.extract_decoded_message(&l_app_row);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries,
                };
            }

            // Set L_A = alpha * L_E
            let alpha = self.config.alpha;
            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha * l_e[i][j];
                }
            }

            // === Column step ===
            let mut l_app_col: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for j in 0..n {
                // Input to SISO: L_Ch + L_A for this column
                let input: Vec<Llr> = (0..n).map(|i| Llr::new(l_ch[i][j] + l_a[i][j])).collect();
                let siso_result = self.sogrand.decode_siso(&input);
                total_queries += siso_result.query_count;
                for (i, app_llr) in siso_result.app_llrs.iter().enumerate() {
                    l_app_col[i][j] = app_llr.value();
                }
            }

            // Compute extrinsic: L_E = L_APP - L_A - L_Ch
            for i in 0..n {
                for j in 0..n {
                    l_e[i][j] = l_app_col[i][j] - l_a[i][j] - l_ch[i][j];
                }
            }

            // Check early termination on column APP
            if self.check_early_termination(&l_app_col) {
                let decoded = self.extract_decoded_message(&l_app_col);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries,
                };
            }

            // Set L_A = alpha * L_E for next iteration
            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha * l_e[i][j];
                }
            }
        }

        // Maximum iterations reached without convergence.
        // Use the last L_APP (from column step if available, else from row step)
        // combined with L_Ch + L_A for a final hard decision.
        let final_llrs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect())
            .collect();
        let decoded = self.extract_decoded_message(&final_llrs);

        TurboDecoderResult {
            decoded_bits: decoded,
            iterations: self.config.max_iterations,
            converged: false,
            total_queries,
        }
    }

    /// Checks if the hard decision on the given LLR matrix forms a valid product codeword.
    ///
    /// # Arguments
    ///
    /// * `llr_matrix` - n x n matrix of LLR values.
    ///
    /// # Returns
    ///
    /// `true` if the hard-decision matrix is a valid product codeword.
    fn check_early_termination(&self, llr_matrix: &[Vec<f32>]) -> bool {
        let n = self.component.n();
        let mut matrix = BitMatrix::zeros(n, n);
        for (i, row) in llr_matrix.iter().enumerate().take(n) {
            for (j, &val) in row.iter().enumerate().take(n) {
                if val < 0.0 {
                    matrix.set(i, j, true);
                }
            }
        }
        self.product_code.is_valid_codeword(&matrix)
    }

    /// Extracts k^2 decoded message bits from the hard decision on an LLR matrix.
    ///
    /// For a systematic code, the message bits are in the top-left k x k submatrix.
    fn extract_decoded_message(&self, llr_matrix: &[Vec<f32>]) -> BitVec {
        let k = self.component.k();
        let mut msg = BitVec::with_capacity(k * k);
        for row in llr_matrix.iter().take(k) {
            for &val in row.iter().take(k) {
                msg.push_bit(val < 0.0);
            }
        }
        msg
    }

    /// Returns a reference to the underlying SOGRAND decoder.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{TurboDecoder, TurboDecoderConfig};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let decoder = TurboDecoder::new(component, TurboDecoderConfig::default());
    /// assert_eq!(decoder.sogrand().n(), 16);
    /// ```
    pub fn sogrand(&self) -> &SoGrand {
        &self.sogrand
    }

    /// Returns the turbo decoder configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{TurboDecoder, TurboDecoderConfig};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let decoder = TurboDecoder::new(component, TurboDecoderConfig::default());
    /// assert_eq!(decoder.config().max_iterations, 20);
    /// ```
    pub fn config(&self) -> &TurboDecoderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::extended::ExtendedBchCode;
    use crate::traits::BlockEncoder;

    // =====================================================================
    // ProductCode construction tests
    // =====================================================================

    #[test]
    fn test_product_code_parameters_16_11() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 256);
        assert_eq!(product.k(), 121);
        assert_eq!(product.component().n(), 16);
        assert_eq!(product.component().k(), 11);
    }

    #[test]
    fn test_product_code_parameters_16_7() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 256);
        assert_eq!(product.k(), 49);
    }

    #[test]
    fn test_product_code_block_encoder_trait() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        // Test through BlockEncoder trait
        let encoder: &dyn BlockEncoder = &product;
        assert_eq!(encoder.k(), 121);
        assert_eq!(encoder.n(), 256);
    }

    // =====================================================================
    // Encoding tests
    // =====================================================================

    #[test]
    fn test_encode_all_zeros() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(product.k());
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());
        // All-zero message should produce all-zero codeword
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_produces_valid_codeword() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        // Encode a message with some ones
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(5, true);
        msg.set(10, true);

        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());

        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "Encoded product codeword must be valid"
        );
    }

    #[test]
    fn test_encode_systematic_message_recovery() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);

        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(3) {
            msg.set(i, true);
        }

        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        let recovered = product.extract_message(&matrix);
        assert_eq!(
            recovered, msg,
            "Extracted message must match original for systematic code"
        );
    }

    #[test]
    fn test_encode_all_ones_message() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        let msg = BitVec::ones(product.k());
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "All-ones message should produce a valid product codeword"
        );
    }

    #[test]
    #[should_panic(expected = "Message length")]
    fn test_encode_wrong_message_length_panics() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(100); // wrong length
        product.encode(&msg);
    }

    // =====================================================================
    // Validity check tests
    // =====================================================================

    #[test]
    fn test_is_valid_codeword_all_zeros() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        let matrix = BitMatrix::zeros(16, 16);
        assert!(product.is_valid_codeword(&matrix));
    }

    #[test]
    fn test_is_valid_codeword_invalid() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        let mut matrix = BitMatrix::zeros(16, 16);
        matrix.set(0, 0, true); // single bit flip invalidates both row 0 and col 0
        assert!(!product.is_valid_codeword(&matrix));
    }

    // =====================================================================
    // Matrix <-> flat conversion tests
    // =====================================================================

    #[test]
    fn test_flat_to_matrix_roundtrip() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);

        let mut flat = BitVec::zeros(256);
        flat.set(0, true);
        flat.set(17, true); // row 1, col 1
        flat.set(255, true);

        let matrix = product.flat_to_matrix(&flat);
        assert!(matrix.get(0, 0));
        assert!(matrix.get(1, 1));
        assert!(matrix.get(15, 15));

        let recovered = product.matrix_to_flat(&matrix);
        assert_eq!(recovered, flat);
    }

    #[test]
    #[should_panic(expected = "Flat vector length")]
    fn test_flat_to_matrix_wrong_length_panics() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component);
        let flat = BitVec::zeros(100);
        product.flat_to_matrix(&flat);
    }

    // =====================================================================
    // TurboDecoderConfig tests
    // =====================================================================

    #[test]
    fn test_turbo_config_default() {
        let config = TurboDecoderConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert!((config.alpha - 0.5).abs() < 1e-10);
        assert_eq!(config.list_size, 4);
        assert_eq!(config.max_queries, 1_000_000);
    }

    // =====================================================================
    // TurboDecoder construction tests
    // =====================================================================

    #[test]
    fn test_turbo_decoder_construction() {
        let component = ExtendedBchCode::ebch_16_11();
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);
        assert_eq!(decoder.sogrand().n(), 16);
        assert_eq!(decoder.config().max_iterations, 5);
    }

    // =====================================================================
    // TurboDecoder decoding tests
    // =====================================================================

    #[test]
    fn test_decode_all_zeros_high_snr() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        // All-zero codeword with strong positive LLRs
        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged, "Should converge for high-SNR all-zeros");
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "Decoded message should be all zeros"
        );
        assert!(result.total_queries > 0, "Should have performed queries");
    }

    #[test]
    fn test_decode_nonzero_message_high_snr() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        // Encode a specific message
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(1, true);
        msg.set(10, true);
        let cw = product.encode(&msg);

        // Create high-SNR LLRs from the codeword
        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0) // bit 1 -> negative LLR
                } else {
                    Llr::new(5.0) // bit 0 -> positive LLR
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(
            result.converged,
            "Should converge for high-SNR encoded message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "Decoded message must match original"
        );
    }

    #[test]
    fn test_decode_tracks_iteration_count() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 3,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        // Should converge early (1 iteration for strong signal)
        assert!(result.iterations >= 1);
        assert!(result.iterations <= 3);
    }

    #[test]
    fn test_decode_early_termination() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());

        // Use many iterations to confirm early termination kicks in
        let config = TurboDecoderConfig {
            max_iterations: 20,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged);
        // With strong signal, should terminate well before 20 iterations
        assert!(
            result.iterations < 20,
            "Should terminate early for strong signal, used {} iterations",
            result.iterations
        );
    }

    #[test]
    #[should_panic(expected = "Channel LLR length")]
    fn test_decode_wrong_llr_length_panics() {
        let component = ExtendedBchCode::ebch_16_11();
        let config = TurboDecoderConfig::default();
        let decoder = TurboDecoder::new(component, config);
        let llrs: Vec<Llr> = vec![Llr::new(1.0); 100];
        decoder.decode(&llrs);
    }

    #[test]
    fn test_decode_queries_increase_with_noise() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 3,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        // High SNR
        let llrs_high: Vec<Llr> = vec![Llr::new(10.0); product.n()];
        let result_high = decoder.decode(&llrs_high);

        // Lower SNR (but still decodable)
        let llrs_low: Vec<Llr> = vec![Llr::new(2.0); product.n()];
        let result_low = decoder.decode(&llrs_low);

        // Lower SNR typically needs more queries (or at least as many)
        assert!(
            result_low.total_queries >= result_high.total_queries,
            "Lower SNR should require at least as many queries: high={}, low={}",
            result_high.total_queries,
            result_low.total_queries,
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::bch::extended::ExtendedBchCode;
    use crate::traits::BlockEncoder;
    use proptest::prelude::*;

    proptest! {
        /// For any random message, encoding produces a valid product codeword
        /// and the extracted message matches the input.
        #[test]
        fn prop_encode_produces_valid_codeword_and_roundtrips(
            msg_bits in prop::collection::vec(any::<bool>(), 121)
        ) {
            let component = ExtendedBchCode::ebch_16_11();
            let product = ProductCode::new(component);
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = product.encode(&msg);
            let matrix = product.flat_to_matrix(&cw);
            prop_assert!(product.is_valid_codeword(&matrix), "Encoded codeword must be valid");
            let extracted = product.extract_message(&matrix);
            prop_assert_eq!(extracted, msg, "Extracted message must match original");
        }
    }
}
