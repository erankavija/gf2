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
//! either [`SoGrand`](crate::grand::SoGrand) or [`BcjrDecoder`](crate::bcjr::BcjrDecoder)
//! as the component SISO decoder (selected via [`TurboDecoderConfig::use_bcjr`]).
//! Extrinsic information is exchanged between row and column steps with a scaling
//! factor alpha (typically 0.5). Early termination occurs when the hard-decision
//! matrix forms a valid product codeword or when the average list-BLER drops
//! below a configurable threshold.
//!
//! # Generic Component Support
//!
//! The [`ProductComponent`] trait abstracts over component codes. Any code that
//! provides a parity-check matrix, n/k dimensions, an even-code flag, and a
//! [`BlockEncoder`] implementation can be used as a component. Built-in
//! implementations exist for [`ExtendedBchCode`](crate::bch::extended::ExtendedBchCode)
//! and [`CrcCode`](crate::crc::CrcCode).
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

use crate::bcjr::BcjrDecoder;
use crate::grand::{OrbGrand, OrbGrandConfig, SisoResult, SoGrand};
use crate::llr::Llr;
use crate::traits::BlockEncoder;
use gf2_core::{BitMatrix, BitVec};

/// Internal SISO engine dispatch: either SOGRAND or BCJR.
enum SisoEngine {
    SoGrand(SoGrand),
    Bcjr(BcjrDecoder),
}

impl SisoEngine {
    fn decode_siso(&self, input: &[Llr]) -> SisoResult {
        match self {
            SisoEngine::SoGrand(s) => s.decode_siso(input),
            SisoEngine::Bcjr(b) => b.decode_siso(input),
        }
    }
}

/// Trait abstracting a component code for use in product code constructions.
///
/// Any linear block code that provides a parity-check matrix, code dimensions,
/// an even-weight flag, and encoding can serve as a product code component.
///
/// # Examples
///
/// ```
/// use gf2_coding::product::ProductComponent;
/// use gf2_coding::bch::extended::ExtendedBchCode;
///
/// let code = ExtendedBchCode::ebch_16_11();
/// assert_eq!(ProductComponent::comp_n(&code), 16);
/// assert_eq!(ProductComponent::comp_k(&code), 11);
/// assert!(ProductComponent::comp_is_even(&code));
/// ```
pub trait ProductComponent: BlockEncoder {
    /// Returns the codeword length of the component code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductComponent;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.comp_n(), 16);
    /// ```
    fn comp_n(&self) -> usize;

    /// Returns the message length of the component code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductComponent;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert_eq!(code.comp_k(), 11);
    /// ```
    fn comp_k(&self) -> usize;

    /// Returns `true` if all codewords have even Hamming weight.
    ///
    /// This flag enables ORBGRAND's even-code optimization, which skips
    /// odd-weight noise patterns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductComponent;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// assert!(code.comp_is_even());
    /// ```
    fn comp_is_even(&self) -> bool;

    /// Returns a reference to the parity-check matrix H.
    ///
    /// The matrix has dimensions (n - k) x n.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::ProductComponent;
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let code = ExtendedBchCode::ebch_16_11();
    /// let h = code.comp_parity_check();
    /// assert_eq!(h.rows(), 5);
    /// assert_eq!(h.cols(), 16);
    /// ```
    fn comp_parity_check(&self) -> &BitMatrix;
}

impl ProductComponent for crate::bch::extended::ExtendedBchCode {
    fn comp_n(&self) -> usize {
        self.n()
    }

    fn comp_k(&self) -> usize {
        self.k()
    }

    fn comp_is_even(&self) -> bool {
        self.is_even()
    }

    fn comp_parity_check(&self) -> &BitMatrix {
        self.parity_check()
    }
}

impl ProductComponent for crate::crc::CrcCode {
    fn comp_n(&self) -> usize {
        self.n()
    }

    fn comp_k(&self) -> usize {
        self.k()
    }

    fn comp_is_even(&self) -> bool {
        self.is_even()
    }

    fn comp_parity_check(&self) -> &BitMatrix {
        self.parity_check()
    }
}

impl ProductComponent for crate::drm::DrmCode {
    fn comp_n(&self) -> usize {
        self.n()
    }

    fn comp_k(&self) -> usize {
        self.k()
    }

    fn comp_is_even(&self) -> bool {
        self.is_even()
    }

    fn comp_parity_check(&self) -> &BitMatrix {
        self.parity_check()
    }
}

/// A product code constructed from a component (n, k) linear block code.
///
/// The product code has parameters (n^2, k^2) and is formed by encoding rows
/// and columns of a k x k information matrix with the component code.
///
/// The type parameter `C` is the component code, which must implement
/// [`ProductComponent`].
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
pub struct ProductCode<C: ProductComponent> {
    /// The component (n, k) code used for row and column encoding.
    component: C,
    /// Component code length.
    comp_n: usize,
    /// Component message length.
    comp_k: usize,
}

impl<C: ProductComponent> ProductCode<C> {
    /// Creates a new product code from the given component code.
    ///
    /// # Arguments
    ///
    /// * `component` - The component (n, k) code implementing [`ProductComponent`].
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
    pub fn new(component: C) -> Self {
        let comp_n = component.comp_n();
        let comp_k = component.comp_k();
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
    /// use gf2_coding::product::ProductComponent;
    ///
    /// let product = ProductCode::new(ExtendedBchCode::ebch_16_11());
    /// assert_eq!(product.component().comp_n(), 16);
    /// ```
    pub fn component(&self) -> &C {
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
        let h = self.component.comp_parity_check();

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

impl<C: ProductComponent> BlockEncoder for ProductCode<C> {
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
/// factor, the ORBGRAND configuration for the component SISO decoder, and
/// an optional list-BLER early-termination threshold.
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
/// assert!(config.list_bler_threshold.is_none());
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

    /// Optional list-BLER threshold for early termination.
    ///
    /// When set, if the average predicted list-BLER across all row/column
    /// SISO decodings in a half-iteration drops below this threshold, the
    /// decoder terminates early. Uses [`SisoResult::list_bler_prediction`]
    /// from SOGRAND.
    ///
    /// A typical value might be `Some(1e-6)`.
    pub list_bler_threshold: Option<f64>,

    /// Use BCJR trellis decoder instead of SOGRAND for component SISO.
    ///
    /// When `true`, the turbo decoder uses a forward-backward (BCJR) algorithm
    /// on the code trellis for exact APP LLR computation. The `list_size` and
    /// `max_queries` fields are ignored in BCJR mode.
    ///
    /// BCJR is recommended for component codes with n-k <= 16 (up to 2^16 = 64K
    /// trellis states). For dRM(32,21) (n-k=11, 2048 states) it is significantly
    /// faster and more accurate than SOGRAND.
    pub use_bcjr: bool,
}

impl Default for TurboDecoderConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            alpha: 0.5,
            list_size: 4,
            max_queries: 1_000_000,
            list_bler_threshold: None,
            use_bcjr: false,
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
///     queries_per_bit: 500.0 / 121.0,
/// };
/// assert!(result.converged);
/// assert_eq!(result.iterations, 3);
/// assert!((result.queries_per_bit - 500.0 / 121.0).abs() < 1e-10);
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

    /// Average number of ORBGRAND queries per information bit.
    ///
    /// Computed as `total_queries as f64 / (k * k) as f64` where k is the
    /// component message length.
    pub queries_per_bit: f64,
}

impl From<TurboDecoderResult> for crate::traits::DecoderResult {
    /// Converts a [`TurboDecoderResult`] into a generic [`DecoderResult`].
    ///
    /// Field mapping:
    /// - `decoded_bits` maps directly.
    /// - `iterations` maps directly.
    /// - `converged` maps to both `converged` and `syndrome_check_passed`.
    /// - `total_queries` maps to `queries`.
    fn from(t: TurboDecoderResult) -> Self {
        crate::traits::DecoderResult {
            decoded_bits: t.decoded_bits,
            iterations: t.iterations,
            converged: t.converged,
            syndrome_check_passed: t.converged,
            queries: Some(t.total_queries),
        }
    }
}

/// Iterative block turbo decoder using SOGRAND or BCJR as the component SISO decoder.
///
/// The turbo decoder alternates between row-wise and column-wise SISO decoding,
/// exchanging extrinsic information between steps. It uses early termination
/// when the hard-decision matrix forms a valid product codeword or when the
/// average list-BLER drops below a configured threshold.
///
/// The component SISO engine is selected via [`TurboDecoderConfig::use_bcjr`]:
/// - `false` (default): uses [`SoGrand`](crate::grand::SoGrand) (query-based)
/// - `true`: uses [`BcjrDecoder`](crate::bcjr::BcjrDecoder) (trellis-based, exact APP)
///
/// The type parameter `C` is the component code, which must implement
/// [`ProductComponent`] and [`Clone`].
///
/// # Algorithm
///
/// 1. Initialize: L_Ch = n x n channel LLR matrix, L_A = 0
/// 2. **Row step**: for each row, decode with SISO(L_Ch + L_A), compute
///    L_E = L_APP - L_A - L_Ch. Check if hard decision is valid -> early exit.
/// 3. Set L_A = alpha * L_E
/// 4. **Column step**: for each column, decode with SISO(L_Ch + L_A),
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
/// O(I * n * S) where I is the number of iterations, n is the component code
/// length, and S is the per-component SISO cost (ORBGRAND queries for SOGRAND,
/// or O(n * 2^(n-k)) for BCJR).
/// Each iteration performs 2n component SISO decodes (n rows + n columns).
pub struct TurboDecoder<C: ProductComponent> {
    /// Component code for encoding/validity checks.
    component: C,
    /// Decoder configuration.
    config: TurboDecoderConfig,
    /// SISO engine: either SOGRAND or BCJR trellis decoder.
    siso: SisoEngine,
    /// Product code for validity checking.
    product_code: ProductCode<C>,
}

impl<C: ProductComponent + Clone> TurboDecoder<C> {
    /// Creates a new turbo decoder for the given component code.
    ///
    /// # Arguments
    ///
    /// * `component` - The component (n, k) code implementing [`ProductComponent`].
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
    pub fn new(component: C, config: TurboDecoderConfig) -> Self {
        let siso = if config.use_bcjr {
            SisoEngine::Bcjr(BcjrDecoder::new(component.comp_parity_check()))
        } else {
            let h = component.comp_parity_check().clone();
            let orb_config = OrbGrandConfig {
                list_size: config.list_size,
                max_queries: config.max_queries,
                even_code: component.comp_is_even(),
                systematic: true,
            };
            let orbgrand = OrbGrand::new(h, orb_config);
            SisoEngine::SoGrand(SoGrand::new(orbgrand))
        };
        let product_code = ProductCode::new(component.clone());
        Self {
            component,
            config,
            siso,
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
    /// O(I * n * S) where I is the number of iterations, n is the component code
    /// length, and S is the per-component SISO cost (ORBGRAND queries for SOGRAND,
    /// or O(n * 2^(n-k)) for BCJR).
    pub fn decode(&self, channel_llrs: &[Llr]) -> TurboDecoderResult {
        let n = self.component.comp_n();
        let k = self.component.comp_k();
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
            let mut row_bler_sum: f64 = 0.0;
            for i in 0..n {
                // Input to SISO: L_Ch + L_A for this row
                let input: Vec<Llr> = (0..n).map(|j| Llr::new(l_ch[i][j] + l_a[i][j])).collect();
                let siso_result = self.siso.decode_siso(&input);
                total_queries += siso_result.query_count;
                row_bler_sum += siso_result.list_bler_prediction;
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
                    queries_per_bit: total_queries as f64 / (k * k) as f64,
                };
            }

            // Check list-BLER threshold for early termination
            if let Some(threshold) = self.config.list_bler_threshold {
                let avg_bler = row_bler_sum / n as f64;
                if avg_bler < threshold {
                    let valid = self.check_early_termination(&l_app_row);
                    let decoded = self.extract_decoded_message(&l_app_row);
                    return TurboDecoderResult {
                        decoded_bits: decoded,
                        iterations: iteration + 1,
                        converged: valid,
                        total_queries,
                        queries_per_bit: total_queries as f64 / (k * k) as f64,
                    };
                }
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
            let mut col_bler_sum: f64 = 0.0;
            for j in 0..n {
                // Input to SISO: L_Ch + L_A for this column
                let input: Vec<Llr> = (0..n).map(|i| Llr::new(l_ch[i][j] + l_a[i][j])).collect();
                let siso_result = self.siso.decode_siso(&input);
                total_queries += siso_result.query_count;
                col_bler_sum += siso_result.list_bler_prediction;
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
                    queries_per_bit: total_queries as f64 / (k * k) as f64,
                };
            }

            // Check list-BLER threshold for early termination
            if let Some(threshold) = self.config.list_bler_threshold {
                let avg_bler = col_bler_sum / n as f64;
                if avg_bler < threshold {
                    let valid = self.check_early_termination(&l_app_col);
                    let decoded = self.extract_decoded_message(&l_app_col);
                    return TurboDecoderResult {
                        decoded_bits: decoded,
                        iterations: iteration + 1,
                        converged: valid,
                        total_queries,
                        queries_per_bit: total_queries as f64 / (k * k) as f64,
                    };
                }
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
            queries_per_bit: total_queries as f64 / (k * k) as f64,
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
        let n = self.component.comp_n();
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
        let k = self.component.comp_k();
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
    /// # Panics
    ///
    /// Panics if the decoder was constructed with `use_bcjr = true`.
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
        match &self.siso {
            SisoEngine::SoGrand(s) => s,
            SisoEngine::Bcjr(_) => panic!("decoder configured with BCJR, not SOGRAND"),
        }
    }

    /// Returns a reference to the underlying BCJR decoder.
    ///
    /// # Panics
    ///
    /// Panics if the decoder was constructed with `use_bcjr = false` (default).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::product::{TurboDecoder, TurboDecoderConfig};
    /// use gf2_coding::bch::extended::ExtendedBchCode;
    ///
    /// let component = ExtendedBchCode::ebch_16_11();
    /// let config = TurboDecoderConfig { use_bcjr: true, ..TurboDecoderConfig::default() };
    /// let decoder = TurboDecoder::new(component, config);
    /// assert_eq!(decoder.bcjr().n(), 16);
    /// ```
    pub fn bcjr(&self) -> &BcjrDecoder {
        match &self.siso {
            SisoEngine::Bcjr(b) => b,
            SisoEngine::SoGrand(_) => panic!("decoder configured with SOGRAND, not BCJR"),
        }
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
    use crate::crc::CrcCode;
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
        assert_eq!(product.component().comp_n(), 16);
        assert_eq!(product.component().comp_k(), 11);
    }

    #[test]
    fn test_product_code_parameters_16_7() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 256);
        assert_eq!(product.k(), 49);
    }

    #[test]
    fn test_product_code_parameters_crc_25_15() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 625);
        assert_eq!(product.k(), 225);
        assert_eq!(product.component().comp_n(), 25);
        assert_eq!(product.component().comp_k(), 15);
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
    // eBCH(16,7) encoding tests
    // =====================================================================

    #[test]
    fn test_encode_ebch_16_7_all_zeros() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(product.k());
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_ebch_16_7_produces_valid_codeword() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(3, true);
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "eBCH(16,7) product codeword must be valid"
        );
    }

    #[test]
    fn test_encode_ebch_16_7_systematic_roundtrip() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(2) {
            msg.set(i, true);
        }
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        let recovered = product.extract_message(&matrix);
        assert_eq!(recovered, msg, "eBCH(16,7) systematic roundtrip must match");
    }

    // =====================================================================
    // CRC(25,15) encoding tests
    // =====================================================================

    #[test]
    fn test_encode_crc_25_15_all_zeros() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(product.k());
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_crc_25_15_produces_valid_codeword() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(7, true);
        msg.set(14, true);
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "CRC(25,15) product codeword must be valid"
        );
    }

    #[test]
    fn test_encode_crc_25_15_systematic_roundtrip() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(3) {
            msg.set(i, true);
        }
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        let recovered = product.extract_message(&matrix);
        assert_eq!(recovered, msg, "CRC(25,15) systematic roundtrip must match");
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
        assert!(config.list_bler_threshold.is_none());
        assert!(!config.use_bcjr);
    }

    #[test]
    fn test_turbo_config_list_bler_threshold() {
        let config = TurboDecoderConfig {
            list_bler_threshold: Some(1e-6),
            ..TurboDecoderConfig::default()
        };
        assert_eq!(config.list_bler_threshold, Some(1e-6));
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

    #[test]
    fn test_turbo_decoder_bcjr_construction() {
        let component = ExtendedBchCode::ebch_16_11();
        let config = TurboDecoderConfig {
            max_iterations: 5,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);
        assert_eq!(decoder.bcjr().n(), 16);
        assert_eq!(decoder.bcjr().k(), 11);
    }

    // =====================================================================
    // TurboDecoder BCJR decoding tests
    // =====================================================================

    #[test]
    fn test_decode_bcjr_all_zeros_high_snr() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "BCJR should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "Decoded message should be all zeros"
        );
        // BCJR reports 0 queries (trellis-based, not query-based)
        assert_eq!(result.total_queries, 0);
    }

    #[test]
    fn test_decode_bcjr_nonzero_message_high_snr() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(1, true);
        msg.set(10, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "BCJR should converge for high-SNR nonzero message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "BCJR decoded message should match input"
        );
    }

    #[test]
    fn test_decode_bcjr_drm_32_21_high_snr() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 10,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        // All-zero codeword with high-SNR LLRs
        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "BCJR+dRM should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "dRM decoded message should be all zeros"
        );
    }

    // =====================================================================
    // TurboDecoder decoding tests — eBCH(16,11)
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
        assert!(
            result.queries_per_bit > 0.0,
            "queries_per_bit should be positive"
        );
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

    // =====================================================================
    // queries_per_bit tests
    // =====================================================================

    #[test]
    fn test_queries_per_bit_computed_correctly() {
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

        let expected = result.total_queries as f64 / product.k() as f64;
        assert!(
            (result.queries_per_bit - expected).abs() < 1e-10,
            "queries_per_bit should equal total_queries / k^2: got {}, expected {}",
            result.queries_per_bit,
            expected
        );
    }

    // =====================================================================
    // List-BLER threshold tests
    // =====================================================================

    #[test]
    fn test_list_bler_threshold_early_termination() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());

        // Without threshold: many iterations allowed
        let config_no_thresh = TurboDecoderConfig {
            max_iterations: 20,
            list_size: 2,
            max_queries: 10_000,
            list_bler_threshold: None,
            ..TurboDecoderConfig::default()
        };
        let decoder_no_thresh = TurboDecoder::new(component.clone(), config_no_thresh);

        // With a very generous threshold: should still converge
        let config_with_thresh = TurboDecoderConfig {
            max_iterations: 20,
            list_size: 2,
            max_queries: 10_000,
            list_bler_threshold: Some(0.5), // generous threshold
            ..TurboDecoderConfig::default()
        };
        let decoder_with_thresh = TurboDecoder::new(component, config_with_thresh);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];

        let result_no = decoder_no_thresh.decode(&llrs);
        let result_with = decoder_with_thresh.decode(&llrs);

        // Both should converge at high SNR
        assert!(result_no.converged);
        assert!(result_with.converged);

        // With threshold, should use at most as many iterations
        assert!(
            result_with.iterations <= result_no.iterations,
            "BLER threshold should enable equal or earlier termination: \
             with={}, without={}",
            result_with.iterations,
            result_no.iterations,
        );
    }

    // =====================================================================
    // TurboDecoder with eBCH(16,7) tests
    // =====================================================================

    #[test]
    fn test_decode_ebch_16_7_all_zeros_high_snr() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "eBCH(16,7) should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(result.decoded_bits.count_ones(), 0);
        assert!(result.queries_per_bit > 0.0);
    }

    #[test]
    fn test_decode_ebch_16_7_nonzero_message() {
        let component = ExtendedBchCode::ebch_16_7();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(3, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(
            result.converged,
            "eBCH(16,7) should converge for high-SNR message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "eBCH(16,7) decoded must match original"
        );
    }

    // =====================================================================
    // TurboDecoder with CRC(25,15) tests
    // =====================================================================

    #[test]
    fn test_decode_crc_25_15_all_zeros_high_snr() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "CRC(25,15) should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(result.decoded_bits.count_ones(), 0);
        assert!(result.queries_per_bit > 0.0);
    }

    #[test]
    fn test_decode_crc_25_15_nonzero_message() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(7, true);
        msg.set(14, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(
            result.converged,
            "CRC(25,15) should converge for high-SNR message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "CRC(25,15) decoded must match original"
        );
    }

    // =====================================================================
    // BER improvement over iterations test
    // =====================================================================

    #[test]
    fn test_ber_improves_over_iterations() {
        let component = ExtendedBchCode::ebch_16_11();
        let product = ProductCode::new(component.clone());

        // Encode a known message
        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(5) {
            msg.set(i, true);
        }
        let cw = product.encode(&msg);

        // Create moderate-SNR LLRs with some noise to prevent instant convergence.
        // Use a deterministic pattern: flip sign on specific positions.
        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                let base = if cw.get(i) { -2.0_f32 } else { 2.0 };
                // Add systematic perturbation (not random, for reproducibility)
                let perturbation = if (i * 7 + 3) % 11 < 3 { -0.5 } else { 0.0 };
                Llr::new(base + perturbation)
            })
            .collect();

        // Run decoder with increasing max_iterations and collect BER at each level
        let mut prev_ber = f64::MAX;
        for max_iter in 1..=5 {
            let config = TurboDecoderConfig {
                max_iterations: max_iter,
                list_size: 2,
                max_queries: 10_000,
                ..TurboDecoderConfig::default()
            };
            let decoder = TurboDecoder::new(component.clone(), config);
            let result = decoder.decode(&llrs);

            // Compute BER
            let mut bit_errors = 0;
            for i in 0..product.k() {
                if result.decoded_bits.get(i) != msg.get(i) {
                    bit_errors += 1;
                }
            }
            let ber = bit_errors as f64 / product.k() as f64;

            // BER should be monotonically non-increasing with more iterations
            assert!(
                ber <= prev_ber + 1e-10,
                "BER should not increase with more iterations: \
                 iter={}, BER={}, prev_BER={}",
                max_iter,
                ber,
                prev_ber
            );
            prev_ber = ber;
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::bch::extended::ExtendedBchCode;
    use crate::crc::CrcCode;
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

        /// eBCH(16,7) product code: encoding roundtrips for random messages.
        #[test]
        fn prop_ebch_16_7_encode_roundtrip(
            msg_bits in prop::collection::vec(any::<bool>(), 49)
        ) {
            let component = ExtendedBchCode::ebch_16_7();
            let product = ProductCode::new(component);
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = product.encode(&msg);
            let matrix = product.flat_to_matrix(&cw);
            prop_assert!(product.is_valid_codeword(&matrix), "eBCH(16,7) codeword must be valid");
            let extracted = product.extract_message(&matrix);
            prop_assert_eq!(extracted, msg, "eBCH(16,7) message must roundtrip");
        }

        /// CRC(25,15) product code: encoding roundtrips for random messages.
        #[test]
        fn prop_crc_25_15_encode_roundtrip(
            msg_bits in prop::collection::vec(any::<bool>(), 225)
        ) {
            let component = CrcCode::crc_25_15();
            let product = ProductCode::new(component);
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = product.encode(&msg);
            let matrix = product.flat_to_matrix(&cw);
            prop_assert!(product.is_valid_codeword(&matrix), "CRC(25,15) codeword must be valid");
            let extracted = product.extract_message(&matrix);
            prop_assert_eq!(extracted, msg, "CRC(25,15) message must roundtrip");
        }
    }
}

#[cfg(test)]
mod additional_component_tests {
    use super::*;
    use crate::bch::extended::ExtendedBchCode;
    use crate::drm::DrmCode;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_product_code_parameters_ebch_32_26() {
        let comp = ExtendedBchCode::ebch_32_26();
        let product = ProductCode::new(comp);
        assert_eq!(product.n(), 32 * 32);
        assert_eq!(product.k(), 26 * 26);
    }

    #[test]
    fn test_encode_ebch_32_26_all_zeros() {
        let comp = ExtendedBchCode::ebch_32_26();
        let product = ProductCode::new(comp);
        let msg = BitVec::zeros(26 * 26);
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), 32 * 32);
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_product_code_parameters_ebch_64_57() {
        let comp = ExtendedBchCode::ebch_64_57();
        let product = ProductCode::new(comp);
        assert_eq!(product.n(), 64 * 64);
        assert_eq!(product.k(), 57 * 57);
    }

    #[test]
    fn test_encode_ebch_64_57_all_zeros() {
        let comp = ExtendedBchCode::ebch_64_57();
        let product = ProductCode::new(comp);
        let msg = BitVec::zeros(57 * 57);
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), 64 * 64);
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_product_code_parameters_drm_32_21() {
        let comp = DrmCode::drm_32_21();
        let product = ProductCode::new(comp);
        assert_eq!(product.n(), 32 * 32);
        assert_eq!(product.k(), 21 * 21);
    }

    #[test]
    fn test_encode_drm_32_21_all_zeros() {
        let comp = DrmCode::drm_32_21();
        let product = ProductCode::new(comp);
        let msg = BitVec::zeros(21 * 21);
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), 32 * 32);
        assert_eq!(cw.count_ones(), 0);
    }
}
