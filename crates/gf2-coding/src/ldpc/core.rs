//! LDPC codes over sparse parity-check matrices: code construction,
//! belief-propagation decoding and systematic encoding.

use crate::ldpc::edge_layout::EdgeLayout;
use crate::ldpc::min_sum::{min_sum_check_row, MinSumRule};
use crate::llr::Llr;
pub use crate::traits::DecodeOutcome;
use crate::traits::{DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;

/// An LDPC code defined by its sparse m × n parity-check matrix.
#[derive(Debug, Clone)]
pub struct LdpcCode {
    /// Sparse parity-check matrix in dual representation
    h: SpBitMatrixDual,
    qc: Option<std::sync::Arc<QuasiCyclicLdpc>>,
    /// Number of variable nodes (codeword length)
    n: usize,
    /// Number of check nodes (parity checks)
    m: usize,
    /// Cached generator matrix (computed lazily)
    #[allow(clippy::type_complexity)]
    cached_generator: std::sync::Arc<std::sync::Mutex<Option<gf2_core::BitMatrix>>>,
    /// Cached systematic column positions: the k column indices in the full
    /// codeword that carry message bits.
    cached_systematic_cols: std::sync::Arc<std::sync::Mutex<Option<Vec<usize>>>>,
}

impl LdpcCode {
    /// Creates an LDPC code from the (check, variable) edges of an m × n
    /// parity-check matrix.
    pub fn from_edges(m: usize, n: usize, edges: &[(usize, usize)]) -> Self {
        let h = SpBitMatrixDual::from_coo(m, n, edges);
        Self {
            h,
            qc: None,
            n,
            m,
            cached_generator: std::sync::Arc::new(std::sync::Mutex::new(None)),
            cached_systematic_cols: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Returns the codeword length (number of variable nodes).
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the number of check nodes.
    pub fn m(&self) -> usize {
        self.m
    }

    /// Returns the message dimension (for full-rank H).
    pub fn k(&self) -> usize {
        self.n.saturating_sub(self.m)
    }

    /// Returns the code rate k/n.
    pub fn rate(&self) -> f64 {
        self.k() as f64 / self.n as f64
    }

    /// Computes the syndrome of a codeword: s = H × c over GF(2).
    ///
    /// Returns a zero vector if c is a valid codeword.
    pub fn syndrome(&self, codeword: &BitVec) -> BitVec {
        assert_eq!(codeword.len(), self.n, "Codeword length must equal n");
        self.h.matvec(codeword)
    }

    /// Checks if a codeword is valid (syndrome is zero).
    pub fn is_valid_codeword(&self, codeword: &BitVec) -> bool {
        let syndrome = self.syndrome(codeword);
        syndrome.count_ones() == 0
    }

    /// Returns the parity-check matrix H (m × n).
    pub fn parity_check_matrix(&self) -> &SpBitMatrixDual {
        &self.h
    }

    /// Creates an LDPC code from a quasi-cyclic structure.
    pub fn from_quasi_cyclic(qc: &QuasiCyclicLdpc) -> Self {
        let edges = qc.to_edges();
        let m = qc.expanded_rows();
        let n = qc.expanded_cols();
        let mut code = Self::from_edges(m, n, &edges);
        code.qc = Some(std::sync::Arc::new(qc.clone()));
        code
    }

    /// The QC description retained by [`Self::from_quasi_cyclic`],
    /// [`Self::attach_quasi_cyclic`] or the DVB-T2 constructors.
    ///
    /// Returns `None` for other codes; structure is not inferred from edges.
    pub fn quasi_cyclic(&self) -> Option<&QuasiCyclicLdpc> {
        self.qc.as_deref()
    }

    /// Drops the retained QC structure; the parity-check matrix is unchanged,
    /// and the batch decoder routes the code as one without the structure.
    #[must_use]
    pub fn without_quasi_cyclic(mut self) -> Self {
        self.qc = None;
        self
    }

    /// Retains `qc` when its expansion under its row and column orders is
    /// exactly this code's parity-check matrix, and returns whether it does;
    /// a mismatch leaves the code unchanged. Takes O(B + E log E) time for B
    /// circulants and E edges.
    #[must_use]
    pub fn attach_quasi_cyclic(&mut self, qc: &QuasiCyclicLdpc) -> bool {
        if qc.expanded_rows() != self.m || qc.expanded_cols() != self.n {
            return false;
        }
        let lifted = SpBitMatrixDual::from_coo(self.m, self.n, &qc.to_edges());
        let equal = (0..self.m).all(|row| {
            let sorted = |matrix: &SpBitMatrixDual| {
                let mut columns: Vec<_> = matrix.row_iter(row).collect();
                columns.sort_unstable();
                columns
            };
            sorted(&lifted) == sorted(&self.h)
        });
        if equal {
            self.qc = Some(std::sync::Arc::new(qc.clone()));
        }
        equal
    }

    /// Creates a DVB-T2 short frame LDPC code (n=16200) from the
    /// `@/citation/Etsi2015` tables, retaining the block description of
    /// [`Self::quasi_cyclic`] with Z = 360.
    pub fn dvb_t2_short(rate: crate::bch::CodeRate) -> Self {
        use crate::ldpc::dvb_t2::{builder, dvb_t2_matrices, params};

        let params = params::DvbParams::for_code(params::FrameSize::Short, rate);
        let table = match rate {
            crate::bch::CodeRate::Rate1_2 => dvb_t2_matrices::SHORT_RATE_1_2_TABLE,
            crate::bch::CodeRate::Rate3_5 => dvb_t2_matrices::SHORT_RATE_3_5_TABLE,
            crate::bch::CodeRate::Rate2_3 => dvb_t2_matrices::SHORT_RATE_2_3_TABLE,
            crate::bch::CodeRate::Rate3_4 => dvb_t2_matrices::SHORT_RATE_3_4_TABLE,
            crate::bch::CodeRate::Rate4_5 => dvb_t2_matrices::SHORT_RATE_4_5_TABLE,
            crate::bch::CodeRate::Rate5_6 => dvb_t2_matrices::SHORT_RATE_5_6_TABLE,
        };

        let edges = builder::build_dvb_edges(table, &params);
        let mut code = Self::from_edges(params.m, params.n, &edges);
        code.qc = Some(std::sync::Arc::new(builder::build_dvb_quasi_cyclic(
            table, &params,
        )));
        code
    }

    /// Creates a DVB-T2 normal frame LDPC code (n=64800) from the
    /// `@/citation/Etsi2015` tables, retaining the block description of
    /// [`Self::quasi_cyclic`] with Z = 360.
    pub fn dvb_t2_normal(rate: crate::bch::CodeRate) -> Self {
        use crate::ldpc::dvb_t2::{builder, dvb_t2_matrices, params};

        let params = params::DvbParams::for_code(params::FrameSize::Normal, rate);
        let table = match rate {
            crate::bch::CodeRate::Rate1_2 => dvb_t2_matrices::NORMAL_RATE_1_2_TABLE,
            crate::bch::CodeRate::Rate3_5 => dvb_t2_matrices::NORMAL_RATE_3_5_TABLE,
            crate::bch::CodeRate::Rate2_3 => dvb_t2_matrices::NORMAL_RATE_2_3_TABLE,
            crate::bch::CodeRate::Rate3_4 => dvb_t2_matrices::NORMAL_RATE_3_4_TABLE,
            crate::bch::CodeRate::Rate4_5 => dvb_t2_matrices::NORMAL_RATE_4_5_TABLE,
            crate::bch::CodeRate::Rate5_6 => dvb_t2_matrices::NORMAL_RATE_5_6_TABLE,
        };

        let edges = builder::build_dvb_edges(table, &params);
        let mut code = Self::from_edges(params.m, params.n, &edges);
        code.qc = Some(std::sync::Arc::new(builder::build_dvb_quasi_cyclic(
            table, &params,
        )));
        code
    }

    /// Computes the k × n generator matrix whose row `i` is the codeword
    /// [`LdpcEncoder`] gives the unit message `i`, from the
    /// [`SystematicReduction`](crate::ldpc::encoding::SystematicReduction) of H.
    ///
    /// Returns None if H is not full rank.
    fn compute_generator_matrix(&self) -> Option<gf2_core::BitMatrix> {
        use crate::ldpc::encoding::SystematicReduction;
        use gf2_core::BitMatrix;

        let k = self.k();

        if k == 0 {
            return Some(BitMatrix::zeros(0, self.n));
        }

        let SystematicReduction {
            reduced,
            parity_cols,
            message_cols,
        } = SystematicReduction::of(&self.h.to_dense()).ok()?;

        let mut g = BitMatrix::zeros(k, self.n);
        for (i, &message_col) in message_cols.iter().enumerate() {
            g.set(i, message_col, true);
            for (row, &parity_col) in parity_cols.iter().enumerate() {
                if reduced.get(row, message_col) {
                    g.set(i, parity_col, true);
                }
            }
        }

        Some(g)
    }

    /// Returns the codeword positions of the message bits, cached after the
    /// first call: [`LdpcEncoder`] places message bit `i` at
    /// `systematic_cols[i]`.
    ///
    /// # Panics
    ///
    /// Panics if H is rank deficient.
    ///
    /// # Complexity
    ///
    /// First call: that of [`message_cols`](crate::ldpc::encoding::message_cols).
    pub(crate) fn systematic_cols(&self) -> Vec<usize> {
        let mut cache = self.cached_systematic_cols.lock().unwrap();
        if let Some(ref cols) = *cache {
            cols.clone()
        } else {
            let cols = crate::ldpc::encoding::message_cols(&self.h)
                .expect("LDPC parity-check matrix is not full rank");
            *cache = Some(cols.clone());
            cols
        }
    }
}

impl crate::traits::GeneratorMatrixAccess for LdpcCode {
    fn k(&self) -> usize {
        self.k()
    }

    fn n(&self) -> usize {
        self.n
    }

    fn generator_matrix(&self) -> gf2_core::BitMatrix {
        let mut cache = self.cached_generator.lock().unwrap();
        if let Some(ref g) = *cache {
            g.clone()
        } else {
            let g = self
                .compute_generator_matrix()
                .expect("LDPC parity-check matrix is not full rank");
            *cache = Some(g.clone());
            g
        }
    }

    /// Reports whether the message bits sit at codeword positions `0..k`,
    /// that is, whether the generator's first `k` columns are the identity.
    ///
    /// # Panics
    ///
    /// Panics if `k > 0` and H is rank deficient.
    ///
    /// # Complexity
    ///
    /// First call: O(nnz + k) for a dual-diagonal parity part, else
    /// O(k + m·n·min(m, n)) for the m × n matrix H; later calls O(k).
    fn is_systematic(&self) -> bool {
        self.systematic_cols()
            .iter()
            .enumerate()
            .all(|(i, &col)| i == col)
    }
}

/// A circulant matrix for quasi-cyclic LDPC codes.
///
/// For a Z×Z circulant with shift s, the first row has a single 1 in column s,
/// and each subsequent row shifts right by one position (with wraparound).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CirculantMatrix {
    /// Right-shift amount (0 = identity)
    shift: usize,
    /// Size of the circulant (Z×Z)
    size: usize,
}

impl CirculantMatrix {
    /// Creates a `size × size` circulant with right shift `shift`.
    pub fn new(shift: usize, size: usize) -> Self {
        Self { shift, size }
    }

    /// Returns the shift value.
    pub fn shift(&self) -> usize {
        self.shift
    }

    /// Returns the size of the circulant.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Generates the (row, col) edges of this circulant placed at block
    /// (`base_row`, `base_col`) of a larger matrix.
    pub fn to_edges(&self, base_row: usize, base_col: usize) -> Vec<(usize, usize)> {
        let row_offset = base_row * self.size;
        let col_offset = base_col * self.size;

        (0..self.size)
            .map(|i| {
                let row = row_offset + i;
                let col = col_offset + ((i + self.shift) % self.size);
                (row, col)
            })
            .collect()
    }
}

/// One circulant of a [`QuasiCyclicLdpc`] block description.
///
/// Lifted row `i` of block (`row_block`, `col_block`) joins lifted column
/// `(i + shift) mod Z` of that block, except at the masked rows, which have no
/// entry in this circulant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QcCirculant {
    row_block: usize,
    col_block: usize,
    shift: usize,
    masked_rows: Vec<usize>,
}

impl QcCirculant {
    /// A full circulant permutation block.
    #[must_use]
    pub fn new(row_block: usize, col_block: usize, shift: usize) -> Self {
        Self {
            row_block,
            col_block,
            shift,
            masked_rows: Vec::new(),
        }
    }

    /// Removes the entries of the given lifted rows of the block; the rows are
    /// kept sorted and unique.
    #[must_use]
    pub fn with_masked_rows(mut self, mut rows: Vec<usize>) -> Self {
        rows.sort_unstable();
        rows.dedup();
        self.masked_rows = rows;
        self
    }

    /// The base row of the block.
    #[must_use]
    pub fn row_block(&self) -> usize {
        self.row_block
    }

    /// The base column of the block.
    #[must_use]
    pub fn col_block(&self) -> usize {
        self.col_block
    }

    /// The right shift of the circulant.
    #[must_use]
    pub fn shift(&self) -> usize {
        self.shift
    }

    /// The lifted rows without an entry, ascending.
    #[must_use]
    pub fn masked_rows(&self) -> &[usize] {
        &self.masked_rows
    }
}

/// Quasi-cyclic LDPC code structure.
///
/// The lifted matrix has `base_rows · Z` rows and `base_cols · Z` columns and
/// is the GF(2) sum of its [`QcCirculant`]s, which are distinct and
/// non-overlapping. Lifted row `r` is parity check [`Self::row_order`]`[r]` and
/// lifted column `c` is variable [`Self::col_order`]`[c]` of the code; both
/// orders are the identity unless set by [`Self::with_orders`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuasiCyclicLdpc {
    base_rows: usize,
    base_cols: usize,
    expansion_factor: usize,
    /// Sorted by row block, column block and shift.
    circulants: Vec<QcCirculant>,
    /// `None` is the identity.
    row_order: Option<Vec<usize>>,
    col_order: Option<Vec<usize>>,
}

impl QuasiCyclicLdpc {
    /// Creates the structure of a base matrix whose entries are `-1` for a zero
    /// block or a circulant shift in `0..expansion_factor`.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - Base matrix is empty
    /// - Rows have inconsistent lengths
    /// - Expansion factor is zero
    /// - A shift value is neither -1 nor in range
    pub fn new(base_matrix: Vec<Vec<i32>>, expansion_factor: usize) -> Self {
        assert!(
            !base_matrix.is_empty(),
            "Base matrix must have at least one row"
        );
        assert!(expansion_factor > 0, "Expansion factor must be positive");

        let cols = base_matrix[0].len();
        assert!(
            base_matrix.iter().all(|row| row.len() == cols),
            "All rows in base matrix must have the same length"
        );

        let mut circulants = Vec::new();
        for (base_row, row) in base_matrix.iter().enumerate() {
            for (base_col, &shift) in row.iter().enumerate() {
                if shift == -1 {
                    continue;
                }
                assert!(
                    shift >= 0 && (shift as usize) < expansion_factor,
                    "Shift value {} at position ({},{}) must be -1 or in range [0,{})",
                    shift,
                    base_row,
                    base_col,
                    expansion_factor
                );
                circulants.push(QcCirculant::new(base_row, base_col, shift as usize));
            }
        }
        Self::from_circulants(base_matrix.len(), cols, expansion_factor, circulants)
    }

    /// Creates the structure of a list of circulants, in any order; a block may
    /// hold several circulants of distinct shifts.
    ///
    /// # Panics
    ///
    /// Panics if `base_rows` or `expansion_factor` is zero, a circulant lies
    /// outside the base matrix, a shift or masked row is at least the
    /// expansion factor, or two circulants share a block and shift.
    #[must_use]
    pub fn from_circulants(
        base_rows: usize,
        base_cols: usize,
        expansion_factor: usize,
        mut circulants: Vec<QcCirculant>,
    ) -> Self {
        assert!(base_rows > 0, "Base matrix must have at least one row");
        assert!(expansion_factor > 0, "Expansion factor must be positive");
        for circulant in &circulants {
            assert!(
                circulant.row_block < base_rows && circulant.col_block < base_cols,
                "Circulant block ({},{}) outside the {base_rows}x{base_cols} base matrix",
                circulant.row_block,
                circulant.col_block
            );
            assert!(
                circulant.shift < expansion_factor,
                "Shift value {} at position ({},{}) must be in range [0,{})",
                circulant.shift,
                circulant.row_block,
                circulant.col_block,
                expansion_factor
            );
            assert!(
                circulant
                    .masked_rows
                    .last()
                    .is_none_or(|&row| row < expansion_factor),
                "Masked row out of range [0,{expansion_factor})"
            );
        }
        circulants.sort_unstable();
        assert!(
            circulants.windows(2).all(|pair| {
                (pair[0].row_block, pair[0].col_block, pair[0].shift)
                    != (pair[1].row_block, pair[1].col_block, pair[1].shift)
            }),
            "Two circulants share a block and shift"
        );
        Self {
            base_rows,
            base_cols,
            expansion_factor,
            circulants,
            row_order: None,
            col_order: None,
        }
    }

    /// Maps lifted row `r` to parity check `row_order[r]` and lifted column
    /// `c` to variable `col_order[c]`.
    ///
    /// # Panics
    ///
    /// Panics unless the orders are permutations of the lifted rows and columns.
    #[must_use]
    pub fn with_orders(mut self, row_order: Vec<usize>, col_order: Vec<usize>) -> Self {
        fn normalized(order: Vec<usize>, len: usize, what: &str) -> Option<Vec<usize>> {
            assert_eq!(order.len(), len, "{what} order length");
            let mut seen = vec![false; len];
            for &index in &order {
                assert!(
                    index < len && !std::mem::replace(&mut seen[index], true),
                    "{what} order is not a permutation"
                );
            }
            (!order.iter().enumerate().all(|(i, &index)| i == index)).then_some(order)
        }
        self.row_order = normalized(row_order, self.expanded_rows(), "Row");
        self.col_order = normalized(col_order, self.expanded_cols(), "Column");
        self
    }

    /// Returns the number of rows in the base matrix.
    pub fn base_rows(&self) -> usize {
        self.base_rows
    }

    /// Returns the number of columns in the base matrix.
    pub fn base_cols(&self) -> usize {
        self.base_cols
    }

    /// Returns the expansion factor.
    pub fn expansion_factor(&self) -> usize {
        self.expansion_factor
    }

    /// The circulants, sorted by row block, column block and shift.
    #[must_use]
    pub fn circulants(&self) -> &[QcCirculant] {
        &self.circulants
    }

    /// The parity check of each lifted row, or `None` for the identity.
    #[must_use]
    pub fn row_order(&self) -> Option<&[usize]> {
        self.row_order.as_deref()
    }

    /// The variable of each lifted column, or `None` for the identity.
    #[must_use]
    pub fn col_order(&self) -> Option<&[usize]> {
        self.col_order.as_deref()
    }

    /// The base matrix of shift values, `-1` for a zero block, when the
    /// structure is one; `None` when a block holds several circulants, a
    /// circulant is masked, a shift exceeds `i32::MAX` or an order is not the
    /// identity. Takes O(B) time
    /// for B base-matrix entries.
    #[must_use]
    pub fn base_matrix(&self) -> Option<Vec<Vec<i32>>> {
        if self.row_order.is_some() || self.col_order.is_some() {
            return None;
        }
        let mut matrix = vec![vec![-1i32; self.base_cols]; self.base_rows];
        for circulant in &self.circulants {
            let entry = &mut matrix[circulant.row_block][circulant.col_block];
            if *entry != -1 || !circulant.masked_rows.is_empty() {
                return None;
            }
            *entry = i32::try_from(circulant.shift).ok()?;
        }
        Some(matrix)
    }

    /// Returns the number of rows in the expanded matrix.
    pub fn expanded_rows(&self) -> usize {
        self.base_rows * self.expansion_factor
    }

    /// Returns the number of columns in the expanded matrix.
    pub fn expanded_cols(&self) -> usize {
        self.base_cols * self.expansion_factor
    }

    /// The (check, variable) edges of the lifted matrix under the row and
    /// column orders.
    pub fn to_edges(&self) -> Vec<(usize, usize)> {
        let z = self.expansion_factor;
        let row = |r: usize| self.row_order.as_ref().map_or(r, |order| order[r]);
        let col = |c: usize| self.col_order.as_ref().map_or(c, |order| order[c]);
        let mut edges = Vec::new();
        for circulant in &self.circulants {
            let mut masked = circulant.masked_rows.iter().peekable();
            for i in 0..z {
                if masked.next_if_eq(&&i).is_some() {
                    continue;
                }
                edges.push((
                    row(circulant.row_block * z + i),
                    col(circulant.col_block * z + (i + circulant.shift) % z),
                ));
            }
        }
        edges
    }
}

/// Decoder algorithm selection for LDPC belief propagation.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum DecoderAlgorithm {
    /// Standard min-sum approximation.
    ///
    /// Check-to-variable message:
    /// $$\lambda_{m \to n} = \prod \text{sign} \cdot \min |L_i|$$
    #[default]
    MinSum,
    /// Normalized min-sum: scales the min-sum output by a factor $\alpha \in (0, 1]$.
    ///
    /// # Valid range
    ///
    /// `alpha` must be finite and in `(0.0, 1.0]`.
    NormalizedMinSum(f32),
    /// Offset min-sum: subtracts a non-negative offset $\beta$ from the min magnitude.
    ///
    /// # Valid range
    ///
    /// `beta` must be finite and `>= 0.0`.
    OffsetMinSum(f32),
    /// Exact sum-product algorithm (box-plus).
    ///
    /// Uses $\tanh / \text{atanh}$ computations.
    SumProduct,
}

/// Configuration for the LDPC belief propagation decoder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecoderConfig {
    /// The check-node update algorithm
    algorithm: DecoderAlgorithm,
    /// Whether to stop early when syndrome check passes before max iterations
    early_termination: bool,
}

impl DecoderConfig {
    /// Creates a new decoder configuration; with `early_termination`, decoding
    /// stops at the first iteration whose syndrome check passes.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `NormalizedMinSum(alpha)` has `alpha` that is not finite or not in `(0.0, 1.0]`
    /// - `OffsetMinSum(beta)` has `beta` that is not finite or is negative
    pub fn new(algorithm: DecoderAlgorithm, early_termination: bool) -> Self {
        match algorithm {
            DecoderAlgorithm::NormalizedMinSum(alpha) => {
                assert!(
                    alpha.is_finite() && alpha > 0.0 && alpha <= 1.0,
                    "NormalizedMinSum alpha must be finite and in (0.0, 1.0], got {}",
                    alpha
                );
            }
            DecoderAlgorithm::OffsetMinSum(beta) => {
                assert!(
                    beta.is_finite() && beta >= 0.0,
                    "OffsetMinSum beta must be finite and >= 0.0, got {}",
                    beta
                );
            }
            _ => {}
        }
        Self {
            algorithm,
            early_termination,
        }
    }

    /// Returns the configured algorithm.
    pub fn algorithm(&self) -> DecoderAlgorithm {
        self.algorithm
    }

    /// Returns whether early termination is enabled.
    pub fn early_termination(&self) -> bool {
        self.early_termination
    }
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            algorithm: DecoderAlgorithm::MinSum,
            early_termination: true,
        }
    }
}

/// Belief propagation decoder for LDPC codes.
///
/// Implements the sum-product algorithm (SPA) and min-sum approximations
/// for iterative soft-decision decoding, with configurable algorithm variants
/// and optional early termination.
///
/// # Decoding Algorithm
///
/// The decoder maintains two types of messages:
/// - **Check-to-variable**: $\lambda_{m \to n}$ from check $m$ to variable $n$
/// - **Variable-to-check**: $\mu_{n \to m}$ from variable $n$ to check $m$
///
/// ## Update Rules (Sum-Product Algorithm)
///
/// Check-to-variable update:
/// $$
/// \lambda_{m \to n} = 2 \cdot \text{atanh}\left(\prod_{n' \in N(m) \setminus n} \tanh\left(\frac{\mu_{n' \to m}}{2}\right)\right)
/// $$
///
/// Variable-to-check update:
/// $$
/// \mu_{n \to m} = L_n + \sum_{m' \in M(n) \setminus m} \lambda_{m' \to n}
/// $$
///
/// where $L_n$ is the channel LLR for variable node $n$.
#[derive(Debug)]
pub struct LdpcDecoder {
    code: LdpcCode,
    /// The canonical edge indexing both node updates and the syndrome read.
    layout: EdgeLayout,
    /// Current variable node beliefs (posterior LLRs)
    beliefs: Vec<Llr>,
    /// Check-to-variable messages, one entry per canonical edge.
    check_to_var: Vec<Llr>,
    /// Variable-to-check messages, one entry per canonical edge.
    var_to_check: Vec<Llr>,
    /// Gather buffer for the sum-product check update, sized at construction to
    /// the largest check degree so no update allocates.
    temp_inputs: Vec<Llr>,
    /// Hard decisions of the current beliefs, reused by the syndrome check.
    hard_bits: Vec<bool>,
    /// Number of iterations in last decode
    last_iterations: usize,
    config: DecoderConfig,
    /// [`LdpcCode::systematic_cols`], resolved on the first `decode_iterative`.
    systematic_cols: Option<Vec<usize>>,
}

impl LdpcDecoder {
    /// Creates a new LDPC decoder for the given code with default configuration:
    /// [`DecoderAlgorithm::MinSum`] with early termination enabled.
    pub fn new(code: LdpcCode) -> Self {
        Self::with_config(code, DecoderConfig::default())
    }

    /// Creates a new LDPC decoder with the given configuration.
    pub fn with_config(code: LdpcCode, config: DecoderConfig) -> Self {
        let n = code.n();
        let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
        let edges = layout.edges();
        let max_check_degree = layout.max_check_degree();

        Self {
            code,
            layout,
            beliefs: vec![Llr::zero(); n],
            check_to_var: vec![Llr::zero(); edges],
            var_to_check: vec![Llr::zero(); edges],
            temp_inputs: Vec::with_capacity(max_check_degree),
            hard_bits: vec![false; n],
            last_iterations: 0,
            config,
            systematic_cols: None,
        }
    }

    /// The canonical edge indexing this decoder passes messages over.
    #[inline]
    #[must_use]
    pub fn edge_layout(&self) -> &EdgeLayout {
        &self.layout
    }

    /// Returns the current posterior LLR for each variable node.
    ///
    /// The returned slice has exactly [`LdpcCode::n`] entries and is read-only;
    /// it does not expose the decoder's message-passing state for mutation.
    /// A newly constructed decoder and a decoder after [`IterativeSoftDecoder::reset`]
    /// return zero beliefs. After decoding, the slice contains the beliefs from
    /// the final BP iteration, including when the final hard word fails its
    /// syndrome check, so callers can use it for immediate post-processing.
    /// After a decode with a cap of zero they equal the frame's channel LLRs.
    pub fn posterior_llrs(&self) -> &[Llr] {
        &self.beliefs
    }

    /// Performs check node update (sum-product algorithm).
    ///
    /// Computes check-to-variable messages using the exact box-plus operation.
    /// Each output gathers the check's other inputs into the preallocated
    /// buffer through the canonical layout, so the `tanh` product
    /// accumulates in the check's own edge order.
    fn check_node_update_spa(&mut self) {
        for check in 0..self.layout.m() {
            let range = self.layout.check_range(check);
            for position in range.clone() {
                self.temp_inputs.clear();
                for other in range.clone() {
                    if other != position {
                        self.temp_inputs.push(self.var_to_check[other]);
                    }
                }

                let message = if self.temp_inputs.is_empty() {
                    Llr::zero()
                } else {
                    Llr::boxplus_n(&self.temp_inputs)
                };

                self.check_to_var[position] = message;
            }
        }
    }

    /// Performs a check node update of the min-sum family.
    ///
    /// Every check writes all of its outgoing messages through
    /// [`min_sum_check_row`], whose numerical contract holds whatever the
    /// `simd` cargo feature and the host's SIMD capabilities are; the update
    /// does not go through the AVX2 kernel of `Llr::boxplus_minsum_n`.
    fn check_node_update_min_sum(&mut self, rule: MinSumRule) {
        for check in 0..self.layout.m() {
            let range = self.layout.check_range(check);
            min_sum_check_row(
                rule,
                &self.var_to_check[range.clone()],
                &mut self.check_to_var[range],
            );
        }
    }

    fn check_node_update(&mut self) {
        match self.config.algorithm.min_sum_rule() {
            Some(rule) => self.check_node_update_min_sum(rule),
            None => self.check_node_update_spa(),
        }
    }

    /// Performs variable node update.
    ///
    /// Updates beliefs and variable-to-check messages. A variable's edges are
    /// its canonical layout slots, so both passes read the incoming messages in
    /// the parity-check matrix's column order without a neighbour search.
    fn variable_node_update(&mut self, channel_llrs: &[Llr]) {
        let slots = self.layout.var_edge_to_check_edge();
        for (var, &channel_llr) in channel_llrs.iter().enumerate().take(self.code.n()) {
            let range = self.layout.var_range(var);

            let mut belief = channel_llr;
            for slot in range.clone() {
                let edge = slots[slot] as usize;
                belief = Llr::new(belief.value() + self.check_to_var[edge].value());
            }
            self.beliefs[var] = belief;

            for slot in range {
                // Message = belief - incoming message from this check
                let edge = slots[slot] as usize;
                let incoming = self.check_to_var[edge];
                self.var_to_check[edge] = Llr::new(belief.value() - incoming.value());
            }
        }
    }

    /// Whether the hard decisions of the current beliefs satisfy every check.
    ///
    /// This is the predicate [`LdpcCode::is_valid_codeword`] computes for the
    /// hard-decided beliefs, evaluated over the canonical layout: each check's
    /// parity is the exclusive or of its edges' decisions, and the scan stops at
    /// the first unsatisfied check. Neither the hard-decision word nor the
    /// syndrome is materialised.
    fn syndrome_passes(&mut self) -> bool {
        for (bit, belief) in self.hard_bits.iter_mut().zip(self.beliefs.iter()) {
            *bit = belief.hard_decision();
        }
        let edge_var = self.layout.check_edge_var();
        for check in 0..self.layout.m() {
            let mut parity = false;
            for edge in self.layout.check_range(check) {
                parity ^= self.hard_bits[edge_var[edge] as usize];
            }
            if parity {
                return false;
            }
        }
        true
    }

    /// Runs iterative BP and returns the full decoded codeword (all n bits).
    ///
    /// Unlike [`IterativeSoftDecoder::decode_iterative`] which extracts k
    /// message bits from systematic positions, this returns the hard-decided
    /// codeword at all n positions. Useful when the caller knows the
    /// systematic column mapping (e.g., 5G NR rate-matched codes that use
    /// natural column ordering per `docs/reference/standards-conformance.md`).
    ///
    /// The cap of zero behaves as in [`LdpcDecoder::decode_codeword_into`].
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len()` differs from the code's `n`.
    pub fn decode_to_codeword(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
        let mut codeword = BitVec::with_capacity(self.code.n());
        let outcome = self.decode_codeword_into(llrs, max_iterations, &mut codeword);
        DecoderResult::new(
            codeword,
            outcome.iterations,
            outcome.converged,
            outcome.syndrome_check_passed,
        )
    }

    /// Runs iterative BP and writes the decoded codeword into `codeword`.
    ///
    /// This is the allocation-free entry point: with a decoder that has already
    /// decoded once and a `codeword` buffer that already holds `n` bits of
    /// storage, a call performs no heap allocation at all. Message storage,
    /// the check-update scratch buffer and the syndrome's hard-decision buffer
    /// are all sized at construction from the code's edge layout, and the
    /// syndrome is evaluated without materialising a hard-decision word.
    /// [`LdpcDecoder::decode_to_codeword`] and
    /// [`IterativeSoftDecoder::decode_iterative`] delegate here and allocate
    /// only the vectors they return.
    ///
    /// `codeword` is overwritten with all `n` hard-decided codeword bits; its
    /// previous contents and length are discarded and its capacity is kept.
    ///
    /// With `max_iterations == 0` no iteration runs: the posterior equals the
    /// channel LLRs, the codeword is their hard decision, `iterations` is 0, and
    /// `converged` equals the syndrome test of that word.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len()` differs from the code's `n`.
    ///
    /// # Complexity
    ///
    /// Each iteration is linear in the edge count for the min-sum family; the
    /// sum-product check update reads `d_c (d_c - 1)` messages per check of
    /// degree `d_c`.
    pub fn decode_codeword_into(
        &mut self,
        llrs: &[Llr],
        max_iterations: usize,
        codeword: &mut BitVec,
    ) -> DecodeOutcome {
        assert_eq!(llrs.len(), self.code.n(), "LLR length must equal n");

        // Reset all messages: check-to-variable to zero, and every
        // variable-to-check message on an edge to that edge's channel LLR.
        self.check_to_var.fill(Llr::zero());
        let edge_var = self.layout.check_edge_var();
        for (message, &var) in self.var_to_check.iter_mut().zip(edge_var.iter()) {
            *message = llrs[var as usize];
        }
        // The first iteration overwrites every belief; only a zero cap reads them.
        if max_iterations == 0 {
            self.beliefs.copy_from_slice(llrs);
        }

        let mut iterations = 0;
        let mut converged = false;
        let early_termination = self.config.early_termination;

        for iter in 0..max_iterations {
            iterations = iter + 1;
            self.check_node_update();
            self.variable_node_update(llrs);

            if early_termination && self.syndrome_passes() {
                converged = true;
                break;
            }
        }

        self.last_iterations = iterations;
        let syndrome_check_passed = self.syndrome_passes();
        if !early_termination || max_iterations == 0 {
            converged = syndrome_check_passed;
        }

        // `syndrome_passes` leaves the current hard decisions in the reused
        // buffer, so the codeword is written from it without a second pass
        // over the beliefs.
        codeword.clear();
        for &bit in &self.hard_bits {
            codeword.push_bit(bit);
        }

        DecodeOutcome {
            iterations,
            converged,
            syndrome_check_passed,
            queries: None,
        }
    }
}

impl SoftDecoder for LdpcDecoder {
    fn k(&self) -> usize {
        self.code.k()
    }

    fn n(&self) -> usize {
        self.code.n()
    }

    /// Returns the hard decisions of all `n` LLRs.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n`.
    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        assert_eq!(llrs.len(), self.n());
        let mut decoded = BitVec::with_capacity(self.n());
        for &llr in llrs {
            decoded.push_bit(llr.hard_decision());
        }
        decoded
    }

    fn soft_decoded_len(&self) -> usize {
        self.n()
    }

    /// Reports no iteration; `converged` and `syndrome_check_passed` are the
    /// parity test of the returned word.
    fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
        let decoded = self.decode_soft(llrs);
        let is_codeword = self.code.is_valid_codeword(&decoded);
        DecoderResult::new(decoded, 0, is_codeword, is_codeword)
    }
}

impl IterativeSoftDecoder for LdpcDecoder {
    /// The cap of zero behaves as in [`LdpcDecoder::decode_codeword_into`].
    fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
        let mut decoded_codeword = BitVec::with_capacity(self.code.n());
        let outcome = self.decode_codeword_into(llrs, max_iterations, &mut decoded_codeword);
        let DecodeOutcome {
            iterations,
            converged,
            syndrome_check_passed: syndrome_passed,
            queries: _,
        } = outcome;

        // Resolved here so that construction never runs a row reduction.
        let sys_cols = self
            .systematic_cols
            .get_or_insert_with(|| self.code.systematic_cols());
        let mut message = BitVec::with_capacity(sys_cols.len());
        for &col in sys_cols.iter() {
            message.push_bit(decoded_codeword.get(col));
        }

        DecoderResult::new(message, iterations, converged, syndrome_passed)
    }

    fn last_iteration_count(&self) -> usize {
        self.last_iterations
    }

    fn reset(&mut self) {
        self.check_to_var.fill(Llr::zero());
        self.var_to_check.fill(Llr::zero());
        self.beliefs.fill(Llr::zero());
        self.hard_bits.fill(false);
        self.last_iterations = 0;
    }
}

/// Internal encoder implementation selected at construction time.
enum EncoderImpl {
    /// Staircase accumulator for dual-diagonal IRA codes (e.g. DVB-T2).
    /// O(edges) construction and encoding; no matrix densification.
    Ira(std::sync::Arc<crate::ldpc::encoding::IraEncoder>),
    /// RREF-based encoder, used when the parity region is not dual-diagonal.
    Ru(std::sync::Arc<crate::ldpc::encoding::RuEncodingMatrices>),
}

impl Clone for EncoderImpl {
    fn clone(&self) -> Self {
        match self {
            Self::Ira(a) => Self::Ira(a.clone()),
            Self::Ru(a) => Self::Ru(a.clone()),
        }
    }
}

impl std::fmt::Debug for EncoderImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ira(_) => f.write_str("EncoderImpl::Ira"),
            Self::Ru(_) => f.write_str("EncoderImpl::Ru"),
        }
    }
}

/// LDPC encoder for systematic encoding.
///
/// For DVB-T2 LDPC codes (and any code whose parity-check matrix has
/// systematic columns `0..k` and a dual-diagonal parity region), construction
/// uses the linear-time IRA staircase accumulator in O(nnz), with no matrix
/// densification and no RREF. Any other code uses the RREF-based
/// preprocessing of [`RuEncodingMatrices`](crate::ldpc::encoding::RuEncodingMatrices).
///
/// The codeword carries the message, in order, at the columns that
/// right-to-left pivoting of H leaves without a pivot: columns `0..k` when the
/// last `m` columns of H are linearly independent. [`LdpcDecoder`]'s
/// `decode_iterative` returns the bits at those columns.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_coding::CodeRate;
/// use gf2_core::BitVec;
///
/// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
/// let encoder = LdpcEncoder::new(code.clone());
///
/// let message = BitVec::zeros(encoder.k());
/// let codeword = encoder.encode(&message);
///
/// assert_eq!(codeword.len(), encoder.n());
/// assert!(code.is_valid_codeword(&codeword));
/// ```
pub struct LdpcEncoder {
    code: LdpcCode,
    impl_: EncoderImpl,
}

impl LdpcEncoder {
    /// Returns `true` when this encoder uses the IRA staircase accumulator.
    pub fn is_ira(&self) -> bool {
        matches!(self.impl_, EncoderImpl::Ira(_))
    }

    /// Creates a new LDPC encoder: the IRA staircase accumulator, constructed
    /// in O(nnz), when the parity region is dual-diagonal, and RREF
    /// preprocessing otherwise.
    ///
    /// For multiple encoders of the same non-IRA configuration, prefer
    /// [`LdpcEncoder::with_cache`] to amortise the RREF cost.
    ///
    /// # Panics
    ///
    /// Panics if the parity-check matrix preprocessing fails (non-IRA path).
    pub fn new(code: LdpcCode) -> Self {
        let impl_ = Self::build_impl(&code);
        Self { code, impl_ }
    }

    /// If the code has standard systematic layout (cols 0..k) and
    /// dual-diagonal parity, build the linear-time IRA encoder. Returns
    /// `None` for codes that need the generic RREF path.
    ///
    /// Shared by [`Self::new`] and [`Self::with_cache`].
    fn try_build_ira(code: &LdpcCode) -> Option<EncoderImpl> {
        let h = code.parity_check_matrix();
        let k = code.k();
        if crate::ldpc::encoding::ira::has_dual_diagonal_parity(h, k) {
            let ira = crate::ldpc::encoding::IraEncoder::new(h, k);
            Some(EncoderImpl::Ira(std::sync::Arc::new(ira)))
        } else {
            None
        }
    }

    /// Build the encoder implementation for a code: prefer the IRA path when
    /// it applies, else fall through to a freshly-preprocessed RREF encoder.
    fn build_impl(code: &LdpcCode) -> EncoderImpl {
        Self::try_build_ira(code).unwrap_or_else(|| {
            let h = code.parity_check_matrix();
            let ru = crate::ldpc::encoding::RuEncodingMatrices::preprocess(h)
                .expect("Failed to preprocess LDPC code for encoding");
            EncoderImpl::Ru(std::sync::Arc::new(ru))
        })
    }

    /// Creates a new LDPC encoder that takes RREF preprocessing results from
    /// `cache`, computing and inserting them on a miss.
    ///
    /// Codes on the IRA path (e.g. DVB-T2) skip the RREF; the cache is only
    /// consulted for other codes.
    ///
    /// # Panics
    ///
    /// Panics if the parity-check matrix preprocessing fails.
    pub fn with_cache(code: LdpcCode, cache: &crate::ldpc::encoding::EncodingCache) -> Self {
        let impl_ = Self::try_build_ira(&code).unwrap_or_else(|| {
            let h = code.parity_check_matrix();
            let key = crate::ldpc::encoding::CacheKey::from_params(code.n(), code.k(), h);
            let ru = cache
                .get_or_compute(key, h)
                .expect("Failed to preprocess LDPC code for encoding");
            EncoderImpl::Ru(ru)
        });

        Self { code, impl_ }
    }
}

impl LdpcEncoder {
    /// Encodes multiple messages in batch: sequentially on the IRA path, and
    /// through `CpuBackend::batch_matvec_transpose` on the RREF path.
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        match &self.impl_ {
            EncoderImpl::Ira(ira) => messages.iter().map(|m| ira.encode(m)).collect(),
            EncoderImpl::Ru(ru) => {
                let backend = gf2_core::compute::CpuBackend::new();
                ru.encode_batch(messages, &backend)
            }
        }
    }
}

impl crate::traits::BlockEncoder for LdpcEncoder {
    fn k(&self) -> usize {
        self.code.k()
    }

    fn n(&self) -> usize {
        self.code.n()
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k(),
            "Message length {} must equal k = {}",
            message.len(),
            self.k()
        );

        match &self.impl_ {
            EncoderImpl::Ira(ira) => ira.encode(message),
            EncoderImpl::Ru(ru) => ru.encode(message),
        }
    }
}

#[cfg(test)]
mod decoder_tests {
    use super::*;
    use crate::test_support::pooled_decode;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_decoder_creation() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let decoder = LdpcDecoder::new(code);

        assert_eq!(decoder.last_iteration_count(), 0);
    }

    #[test]
    fn test_posterior_llrs_pre_decode_and_after_reset() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code.clone());

        let initial_beliefs: &[Llr] = decoder.posterior_llrs();
        assert_eq!(initial_beliefs.len(), code.n());
        assert!(initial_beliefs.iter().all(|belief| belief.value() == 0.0));

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        decoder.decode_to_codeword(&llrs, 10);
        assert!(decoder
            .posterior_llrs()
            .iter()
            .any(|belief| belief.value() != 0.0));

        decoder.reset();

        assert!(decoder
            .posterior_llrs()
            .iter()
            .all(|belief| belief.value() == 0.0));
    }

    #[test]
    fn test_posterior_llrs_match_decode_to_codeword_hard_word() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let mut decoder = LdpcDecoder::new(code.clone());
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];

        let result = decoder.decode_to_codeword(&llrs, 10);

        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(decoder.posterior_llrs().len(), code.n());
        for (index, posterior) in decoder.posterior_llrs().iter().enumerate() {
            assert_eq!(
                posterior.hard_decision(),
                result.decoded_bits.get(index),
                "posterior hard decision differs at variable {index}"
            );
        }
    }

    #[test]
    fn test_posterior_llrs_match_failed_iterative_decode() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let mut decoder = LdpcDecoder::new(code.clone());
        let llrs = vec![Llr::new(10.0), Llr::new(-2.0), Llr::new(1.0)];

        let result = decoder.decode_iterative(&llrs, 1);

        assert!(!result.syndrome_check_passed);
        let mut codeword_decoder = LdpcDecoder::new(code.clone());
        let codeword_result = codeword_decoder.decode_to_codeword(&llrs, 1);
        assert!(!codeword_result.syndrome_check_passed);
        assert_eq!(decoder.posterior_llrs().len(), code.n());
        for (index, posterior) in decoder.posterior_llrs().iter().enumerate() {
            assert_eq!(
                posterior.hard_decision(),
                codeword_result.decoded_bits.get(index),
                "posterior hard decision differs at variable {index}"
            );
        }

        let posterior_for_post_processing = decoder.posterior_llrs().to_vec();
        assert!(posterior_for_post_processing
            .iter()
            .any(|belief| belief.value() != 0.0));
        assert_eq!(decoder.posterior_llrs(), posterior_for_post_processing);
    }

    #[test]
    fn test_posterior_llrs_preserved_after_failed_codeword_decode() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let mut decoder = LdpcDecoder::new(code.clone());
        let llrs = vec![Llr::new(10.0), Llr::new(-2.0), Llr::new(1.0)];

        let result = decoder.decode_to_codeword(&llrs, 1);

        assert!(!result.syndrome_check_passed);
        let posterior = decoder.posterior_llrs().to_vec();
        assert_eq!(posterior.len(), code.n());
        assert_eq!(
            posterior
                .iter()
                .map(|llr| llr.hard_decision())
                .collect::<Vec<_>>(),
            (0..code.n())
                .map(|index| result.decoded_bits.get(index))
                .collect::<Vec<_>>()
        );
        assert!(posterior.iter().any(|llr| llr.value() != 0.0));
        assert_eq!(decoder.posterior_llrs(), posterior);
    }

    #[test]
    fn test_trivial_decode_no_errors() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];

        let result = decoder.decode_iterative(&llrs, 10);

        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
        assert!(result.iterations <= 2);
    }

    #[test]
    fn test_decode_with_single_error() {
        // Single parity check code [3,2]
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        // Two strong 1s, one weak 0 → should decode to [1, 1, 0] (even parity)
        let llrs = vec![Llr::new(-5.0), Llr::new(-5.0), Llr::new(2.0)];

        let result = decoder.decode_iterative(&llrs, 20);

        if result.converged {
            assert!(result.syndrome_check_passed);
            assert_eq!(result.decoded_bits.count_ones(), 2);
        }
    }

    /// Decodes `llrs` at a cap of zero and checks every result field against
    /// values computed from the frame and the check list alone.
    fn assert_zero_cap_decode(decoder: &mut LdpcDecoder, checks: &[&[usize]], llrs: &[Llr]) {
        let mut codeword = BitVec::new();
        let outcome = decoder.decode_codeword_into(llrs, 0, &mut codeword);

        let posterior: Vec<u32> = decoder
            .posterior_llrs()
            .iter()
            .map(|l| l.value().to_bits())
            .collect();
        let channel: Vec<u32> = llrs.iter().map(|l| l.value().to_bits()).collect();
        assert_eq!(posterior, channel);

        let hard: Vec<bool> = llrs.iter().map(|l| l.value() < 0.0).collect();
        assert_eq!(codeword.len(), hard.len());
        for (i, &bit) in hard.iter().enumerate() {
            assert_eq!(codeword.get(i), bit, "codeword bit {i}");
        }

        let syndrome_ok = checks
            .iter()
            .all(|vars| vars.iter().filter(|&&v| hard[v]).count() % 2 == 0);
        assert_eq!(outcome.syndrome_check_passed, syndrome_ok);
        assert_eq!(outcome.converged, syndrome_ok);
        assert_eq!(outcome.iterations, 0);
        assert_eq!(decoder.last_iteration_count(), 0);
    }

    #[test]
    fn test_zero_cap_results_depend_only_on_the_frame() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let checks: [&[usize]; 2] = [&[0, 1], &[1, 2]];
        let code = LdpcCode::from_edges(2, 3, &edges);
        let llr = |v: [f32; 3]| v.map(Llr::new).to_vec();
        let frames = [
            llr([3.0, 2.0, 1.0]),    // hard word 000: satisfies both checks
            llr([-3.0, 2.0, 1.0]),   // hard word 100: violates check 0
            llr([-0.5, -7.25, 4.0]), // hard word 110: violates check 1
        ];

        for order in [[0, 1, 2], [2, 1, 0], [1, 0, 1]] {
            let mut decoder = LdpcDecoder::new(code.clone());
            for &f in &order {
                assert_zero_cap_decode(&mut decoder, &checks, &frames[f]);
            }
        }

        for f in 0..frames.len() {
            let mut decoder = LdpcDecoder::new(code.clone());
            for g in 0..frames.len() {
                decoder.decode_to_codeword(&frames[g], 5);
                assert_zero_cap_decode(&mut decoder, &checks, &frames[f]);
            }
        }
    }

    #[test]
    fn test_consecutive_decodes_no_state_leakage() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        let llrs1 = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result1 = decoder.decode_iterative(&llrs1, 10);
        assert!(result1.converged);
        assert!(result1.syndrome_check_passed);
        assert_eq!(result1.decoded_bits.count_ones(), 0);

        let llrs2 = vec![Llr::new(-10.0), Llr::new(-10.0), Llr::new(10.0)];
        let result2 = decoder.decode_iterative(&llrs2, 10);
        assert!(result2.converged);
        assert!(result2.syndrome_check_passed);
        assert_eq!(result2.decoded_bits.count_ones(), 2);

        let result3 = decoder.decode_iterative(&llrs1, 10);
        assert!(result3.converged);
        assert!(result3.syndrome_check_passed);
        assert_eq!(result3.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_decoder_reset_clears_state() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let mut decoder = LdpcDecoder::new(code);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result1 = decoder.decode_iterative(&llrs, 10);
        assert!(result1.iterations > 0);

        decoder.reset();
        assert_eq!(decoder.last_iteration_count(), 0);

        let result2 = decoder.decode_iterative(&llrs, 10);
        assert!(result2.converged);
        assert_eq!(result2.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_encoder_batch_processing() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let encoder = LdpcEncoder::new(code.clone());

        let messages: Vec<BitVec> = vec![
            BitVec::from_bytes_le(&[0b00]),
            BitVec::from_bytes_le(&[0b01]),
            BitVec::from_bytes_le(&[0b10]),
            BitVec::from_bytes_le(&[0b11]),
        ]
        .into_iter()
        .map(|bv| {
            let mut msg = BitVec::with_capacity(2);
            msg.push_bit(bv.get(0));
            msg.push_bit(bv.get(1));
            msg
        })
        .collect();

        let codewords = encoder.encode_batch(&messages);

        assert_eq!(codewords.len(), 4);
        for (msg, cw) in messages.iter().zip(codewords.iter()) {
            let expected = encoder.encode(msg);
            assert_eq!(cw.len(), expected.len());
            for i in 0..cw.len() {
                assert_eq!(cw.get(i), expected.get(i));
            }
        }
    }

    #[test]
    fn test_decoder_batch_processing() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);

        let llr_blocks: Vec<Vec<Llr>> = vec![
            vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)], // [0,0,0]
            vec![Llr::new(-10.0), Llr::new(-10.0), Llr::new(10.0)], // [1,1,0]
            vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)], // [0,0,0]
        ];

        let results = pooled_decode(&code, &llr_blocks, 10);

        let mut single = LdpcDecoder::new(code.clone());
        for (llrs, result) in llr_blocks.iter().zip(&results) {
            assert_eq!(result, &single.decode_iterative(llrs, 10));
        }
        assert_eq!(results.len(), 3);
        assert!(results[0].converged);
        assert_eq!(results[0].decoded_bits.count_ones(), 0);
        assert!(results[1].converged);
        assert_eq!(results[1].decoded_bits.count_ones(), 2);
        assert!(results[2].converged);
        assert_eq!(results[2].decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_batch_processing_empty_input() {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);
        let encoder = LdpcEncoder::new(code.clone());

        let empty_messages: Vec<BitVec> = vec![];
        let codewords = encoder.encode_batch(&empty_messages);
        assert_eq!(codewords.len(), 0);

        let empty_llrs: Vec<Vec<Llr>> = vec![];
        let results = pooled_decode(&code, &empty_llrs, 10);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_cached_neighbors_correctness() {
        let code = LdpcCode::dvb_t2_normal(crate::CodeRate::Rate3_5);
        let h = code.parity_check_matrix();

        let cached: Vec<Vec<usize>> = (0..code.m())
            .map(|check| h.row_iter(check).collect())
            .collect();

        for (check, cached_neighbors) in cached.iter().enumerate() {
            let dynamic: Vec<usize> = h.row_iter(check).collect();
            assert_eq!(
                cached_neighbors, &dynamic,
                "Cached neighbors must match dynamic iteration for check {}",
                check
            );
        }
    }

    #[test]
    fn test_decoder_equivalence_with_caching() {
        let code = LdpcCode::dvb_t2_normal(crate::CodeRate::Rate3_5);

        let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(10.0)).collect();

        let mut decoder1 = LdpcDecoder::new(code.clone());
        let result1 = decoder1.decode_iterative(&llrs, 50);

        let mut decoder2 = LdpcDecoder::new(code.clone());
        let result2 = decoder2.decode_iterative(&llrs, 50);

        assert_eq!(result1.converged, result2.converged);
        assert_eq!(result1.iterations, result2.iterations);
        assert_eq!(result1.decoded_bits, result2.decoded_bits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::GeneratorMatrixAccess;

    fn first_k_columns_are_identity(code: &LdpcCode) -> bool {
        let g = GeneratorMatrixAccess::generator_matrix(code);
        (0..code.k()).all(|i| (0..code.k()).all(|j| g.get(i, j) == (i == j)))
    }

    #[test]
    fn base_matrix_is_none_when_a_shift_exceeds_i32() {
        let limit = i32::MAX as usize;
        let at_limit =
            QuasiCyclicLdpc::from_circulants(1, 1, limit + 2, vec![QcCirculant::new(0, 0, limit)]);
        assert_eq!(at_limit.base_matrix(), Some(vec![vec![i32::MAX]]));
        let above = QuasiCyclicLdpc::from_circulants(
            1,
            1,
            limit + 2,
            vec![QcCirculant::new(0, 0, limit + 1)],
        );
        assert_eq!(above.base_matrix(), None);
    }

    #[test]
    fn test_is_systematic_message_bits_at_leading_positions() {
        // H = [1 1 1]: right-to-left pivoting gives column 2 the parity.
        let code = LdpcCode::from_edges(1, 3, &[(0, 0), (0, 1), (0, 2)]);
        assert!(GeneratorMatrixAccess::is_systematic(&code));
        assert!(first_k_columns_are_identity(&code));
    }

    #[test]
    fn test_is_systematic_false_when_message_bits_are_displaced() {
        // Column 3 of H carries no parity, so message bit 1 sits at position 3.
        let code = LdpcCode::from_edges(2, 4, &[(0, 0), (0, 1), (1, 1), (1, 2)]);
        assert_eq!(code.systematic_cols(), vec![0, 3]);
        assert!(!GeneratorMatrixAccess::is_systematic(&code));
        assert!(!first_k_columns_are_identity(&code));
    }

    #[test]
    fn test_is_systematic_true_for_dual_diagonal_parity() {
        let edges = [(0, 0), (0, 2), (1, 1), (1, 2), (1, 3)];
        let code = LdpcCode::from_edges(2, 4, &edges);
        assert!(GeneratorMatrixAccess::is_systematic(&code));
        assert!(first_k_columns_are_identity(&code));
    }

    #[test]
    fn test_ldpc_code_creation() {
        let edges = vec![(0, 0), (0, 1), (1, 1), (1, 2), (2, 0), (2, 2)];
        let code = LdpcCode::from_edges(3, 4, &edges);

        assert_eq!(code.n(), 4);
        assert_eq!(code.m(), 3);
        assert_eq!(code.k(), 1);
        assert!((code.rate() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_syndrome_computation() {
        // Single parity check code: H = [1 1 1]
        // Valid codewords have even parity
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        let code = LdpcCode::from_edges(1, 3, &edges);

        let mut valid = BitVec::new();
        for _ in 0..3 {
            valid.push_bit(false);
        }
        assert!(code.is_valid_codeword(&valid));

        let mut valid2 = BitVec::new();
        valid2.push_bit(true);
        valid2.push_bit(true);
        valid2.push_bit(false);
        assert!(code.is_valid_codeword(&valid2));

        let mut invalid = BitVec::new();
        invalid.push_bit(true);
        invalid.push_bit(false);
        invalid.push_bit(false);
        assert!(!code.is_valid_codeword(&invalid));
    }

    #[test]
    fn test_regular_ldpc_structure() {
        // Create a regular (2,4) code: 2 ones per column, 4 ones per row
        // 4 checks × 8 variables
        let mut edges = Vec::new();
        for col in 0..8 {
            let check1 = (col * 2) % 4;
            let check2 = (col * 2 + 1) % 4;
            edges.push((check1, col));
            edges.push((check2, col));
        }

        let code = LdpcCode::from_edges(4, 8, &edges);
        let h = code.parity_check_matrix();

        for col in 0..8 {
            let weight = h.col_iter(col).count();
            assert_eq!(weight, 2, "Column {} should have weight 2", col);
        }

        for row in 0..4 {
            let weight = h.row_iter(row).count();
            assert_eq!(weight, 4, "Row {} should have weight 4", row);
        }
    }
}

#[cfg(test)]
mod generator_matrix_access_tests {
    use super::*;
    use crate::traits::GeneratorMatrixAccess;

    #[test]
    fn test_ldpc_generator_matrix_dimensions() {
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 4),
            (2, 1),
            (2, 2),
            (2, 5),
        ];
        let code = LdpcCode::from_edges(3, 7, &edges);
        let g = code.generator_matrix();
        assert_eq!(g.rows(), 4); // k = n - m = 7 - 3
        assert_eq!(g.cols(), 7);
    }

    #[test]
    fn test_ldpc_generator_parity_orthogonality() {
        let edges = vec![(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (1, 3)];
        let code = LdpcCode::from_edges(2, 4, &edges);
        let g = code.generator_matrix();
        let h = code.parity_check_matrix().to_dense();

        let h_t = h.transpose();
        let product = &g * &h_t;

        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j), "G·H^T must be zero at ({}, {})", i, j);
            }
        }
    }

    #[test]
    fn test_ldpc_encoding_via_generator_produces_valid_codewords() {
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 4),
            (2, 1),
            (2, 2),
            (2, 5),
        ];
        let code = LdpcCode::from_edges(3, 7, &edges);
        let g = code.generator_matrix();

        for i in 0..code.k() {
            let row = g.row_as_bitvec(i);
            assert!(
                code.is_valid_codeword(&row),
                "Row {} of G must be a valid codeword",
                i
            );
        }
    }

    #[test]
    fn test_ldpc_generator_cached() {
        let edges = vec![
            (0, 0),
            (0, 1),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 4),
            (2, 1),
            (2, 2),
            (2, 5),
        ];
        let code = LdpcCode::from_edges(3, 7, &edges);
        let g1 = code.generator_matrix();
        let g2 = code.generator_matrix();
        assert_eq!(g1, g2);
    }

    #[test]
    fn test_ldpc_small_identity_h() {
        // Simple test with H = [I_2 | P]
        // H = [1 0 | 1 1]
        //     [0 1 | 1 0]
        // Then G = [1 1 | 1 0]
        //          [1 0 | 0 1]
        let edges = vec![
            (0, 0),
            (0, 2),
            (0, 3), // First check
            (1, 1),
            (1, 2), // Second check
        ];
        let code = LdpcCode::from_edges(2, 4, &edges);
        let g = code.generator_matrix();

        assert_eq!(g.rows(), 2); // k = 4 - 2 = 2
        assert_eq!(g.cols(), 4);

        for i in 0..code.k() {
            let row = g.row_as_bitvec(i);
            assert!(code.is_valid_codeword(&row));
        }
    }

    #[test]
    fn test_ldpc_regular_3_6() {
        let edges = vec![
            // Each variable node connects to 3 checks
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (1, 1),
            (3, 1),
            (0, 2),
            (2, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
            (0, 4),
            (2, 4),
            (3, 4),
            (1, 5),
            (2, 5),
            (3, 5),
        ];
        let code = LdpcCode::from_edges(4, 6, &edges);
        let g = code.generator_matrix();

        assert_eq!(g.rows(), 2); // k = 6 - 4 = 2
        assert_eq!(g.cols(), 6);

        let h = code.parity_check_matrix().to_dense();
        let h_t = h.transpose();
        let product = &g * &h_t;

        for i in 0..product.rows() {
            for j in 0..product.cols() {
                assert!(!product.get(i, j), "G·H^T must be zero");
            }
        }
    }
}

#[cfg(test)]
mod algorithm_tests {
    use super::*;
    use crate::traits::IterativeSoftDecoder;

    /// A [3,2] single parity check code.
    fn simple_parity_code() -> LdpcCode {
        let edges = vec![(0, 0), (0, 1), (0, 2)];
        LdpcCode::from_edges(1, 3, &edges)
    }

    #[test]
    fn test_decoder_config_default() {
        let config = DecoderConfig::default();
        assert_eq!(config.algorithm(), DecoderAlgorithm::MinSum);
        assert!(config.early_termination());
    }

    #[test]
    fn test_decoder_config_new_valid_normalized() {
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
        assert_eq!(
            config.algorithm(),
            DecoderAlgorithm::NormalizedMinSum(0.875)
        );
    }

    #[test]
    fn test_decoder_config_new_valid_offset() {
        let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(0.5), false);
        assert_eq!(config.algorithm(), DecoderAlgorithm::OffsetMinSum(0.5));
        assert!(!config.early_termination());
    }

    #[test]
    fn test_decoder_config_new_normalized_alpha_one() {
        // alpha = 1.0 is valid (upper bound inclusive)
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(1.0), true);
        assert_eq!(config.algorithm(), DecoderAlgorithm::NormalizedMinSum(1.0));
    }

    #[test]
    fn test_decoder_config_new_offset_beta_zero() {
        // beta = 0.0 is valid (lower bound inclusive)
        let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(0.0), true);
        assert_eq!(config.algorithm(), DecoderAlgorithm::OffsetMinSum(0.0));
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_zero() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.0), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_negative() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(-0.5), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_greater_than_one() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(1.1), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_nan() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(f32::NAN), true);
    }

    #[test]
    #[should_panic(expected = "NormalizedMinSum alpha must be finite and in (0.0, 1.0]")]
    fn test_decoder_config_rejects_alpha_inf() {
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(f32::INFINITY), true);
    }

    #[test]
    #[should_panic(expected = "OffsetMinSum beta must be finite and >= 0.0")]
    fn test_decoder_config_rejects_beta_negative() {
        DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(-0.1), true);
    }

    #[test]
    #[should_panic(expected = "OffsetMinSum beta must be finite and >= 0.0")]
    fn test_decoder_config_rejects_beta_nan() {
        DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(f32::NAN), true);
    }

    #[test]
    #[should_panic(expected = "OffsetMinSum beta must be finite and >= 0.0")]
    fn test_decoder_config_rejects_beta_inf() {
        DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(f32::INFINITY), true);
    }

    #[test]
    fn test_normalized_minsum_single_error_correction() {
        let code = simple_parity_code();
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.875), true);
        let mut decoder = LdpcDecoder::with_config(code, config);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_offset_minsum_single_error_correction() {
        let code = simple_parity_code();
        let config = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(0.5), true);
        let mut decoder = LdpcDecoder::with_config(code, config);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_sum_product_single_error_correction() {
        let code = simple_parity_code();
        let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);
        let mut decoder = LdpcDecoder::with_config(code, config);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.count_ones(), 0);
    }

    #[test]
    fn test_early_termination_reduces_iterations() {
        let code = simple_parity_code();

        let config_early = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
        let mut decoder_early = LdpcDecoder::with_config(code.clone(), config_early);

        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let result_early = decoder_early.decode_iterative(&llrs, 50);
        assert!(result_early.converged);
        assert!(
            result_early.iterations < 50,
            "Should converge before max iterations"
        );

        let config_no_early = DecoderConfig::new(DecoderAlgorithm::MinSum, false);
        let mut decoder_no_early = LdpcDecoder::with_config(code, config_no_early);
        let result_no_early = decoder_no_early.decode_iterative(&llrs, 50);

        // Without early termination, always runs all max_iterations
        assert_eq!(result_no_early.iterations, 50);
        assert!(result_no_early.syndrome_check_passed);
        assert!(result_no_early.converged); // converged is set based on final syndrome when early_termination=false
    }

    #[test]
    fn test_all_algorithms_agree_on_high_snr() {
        let code = simple_parity_code();
        let llrs = vec![Llr::new(10.0), Llr::new(10.0), Llr::new(10.0)];
        let max_iter = 20;

        let algorithms = [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(0.875),
            DecoderAlgorithm::OffsetMinSum(0.5),
            DecoderAlgorithm::SumProduct,
        ];

        for &algo in &algorithms {
            let config = DecoderConfig::new(algo, true);
            let mut decoder = LdpcDecoder::with_config(code.clone(), config);
            let result = decoder.decode_iterative(&llrs, max_iter);

            assert!(
                result.converged,
                "Algorithm {:?} should converge at high SNR",
                algo
            );
            assert!(
                result.syndrome_check_passed,
                "Algorithm {:?} should pass syndrome check",
                algo
            );
            assert_eq!(
                result.decoded_bits.count_ones(),
                0,
                "Algorithm {:?} should decode all-zero codeword",
                algo
            );
        }
    }

    #[test]
    fn test_sum_product_nr5g_256_121_noiseless() {
        use crate::ldpc::nr_5g::Nr5gRateMatchedDecoder;
        use crate::ldpc::QuasiCyclicLdpc;
        use crate::traits::{BlockEncoder, IterativeSoftDecoder};

        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
        let target_k = rm_code.params().target_k;
        let target_n = rm_code.params().target_n;

        let message = BitVec::zeros(target_k);
        let codeword = rm_code.encode(&message);
        assert_eq!(codeword.len(), target_n);

        let channel_llrs: Vec<Llr> = (0..target_n)
            .map(|i| {
                if codeword.get(i) {
                    Llr::new(-10.0)
                } else {
                    Llr::new(10.0)
                }
            })
            .collect();

        let mut nms_decoder = Nr5gRateMatchedDecoder::new(rm_code.clone());
        let nms_result = nms_decoder.decode_iterative(&channel_llrs, 50);
        assert!(
            nms_result.converged,
            "NMS should converge on noiseless input"
        );
        assert!(
            nms_result.syndrome_check_passed,
            "NMS should pass syndrome check"
        );
        let nms_errors = (0..target_k)
            .filter(|&i| nms_result.decoded_bits.get(i) != message.get(i))
            .count();
        assert_eq!(nms_errors, 0, "NMS should have zero bit errors");

        let mut sp_decoder =
            Nr5gRateMatchedDecoder::with_algorithm(rm_code.clone(), DecoderAlgorithm::SumProduct);
        let sp_result = sp_decoder.decode_iterative(&channel_llrs, 50);

        let sp_errors = (0..target_k)
            .filter(|&i| sp_result.decoded_bits.get(i) != message.get(i))
            .count();

        assert!(
            sp_result.converged,
            "SP should converge on noiseless input (converged={}, syndrome={}, iters={}, errors={})",
            sp_result.converged,
            sp_result.syndrome_check_passed,
            sp_result.iterations,
            sp_errors
        );
        assert!(
            sp_result.syndrome_check_passed,
            "SP should pass syndrome check"
        );
        assert_eq!(sp_errors, 0, "SP should have zero bit errors");
    }

    /// `boxplus_n` stays finite for extreme LLR values, where tanh(x/2)
    /// saturates to exactly 1.0 in f32 and atanh(1.0) = Inf.
    #[test]
    fn test_boxplus_n_numerical_stability() {
        let large_msgs: Vec<Llr> = vec![
            Llr::new(15.0),
            Llr::new(12.0),
            Llr::new(10.0),
            Llr::new(8.0),
            Llr::new(15.0),
        ];
        let result = Llr::boxplus_n(&large_msgs);
        assert!(
            result.is_finite(),
            "boxplus_n should not produce Inf for large positive LLRs, got {}",
            result.value()
        );

        // Mix of large positive and zero (punctured positions)
        let mixed_msgs: Vec<Llr> = vec![
            Llr::new(15.0),
            Llr::new(0.0),
            Llr::new(10.0),
            Llr::new(15.0),
        ];
        let result = Llr::boxplus_n(&mixed_msgs);
        assert!(
            result.is_finite(),
            "boxplus_n with a zero input should be finite, got {}",
            result.value()
        );
        // tanh(0/2) = 0, so product = 0, atanh(0) = 0
        assert_eq!(
            result.value(),
            0.0,
            "boxplus_n with a zero input should return 0"
        );

        let huge_msgs: Vec<Llr> = vec![Llr::new(50.0), Llr::new(30.0), Llr::new(40.0)];
        let result = Llr::boxplus_n(&huge_msgs);
        assert!(
            result.is_finite(),
            "boxplus_n should not produce Inf for very large LLRs, got {}",
            result.value()
        );

        assert!(
            Llr::boxplus_n(&large_msgs).value().abs() < 20.0,
            "boxplus_n of moderate LLRs should produce a bounded result"
        );
    }

    #[test]
    fn test_all_algorithms_converge_on_11_codeword() {
        let code = simple_parity_code();
        // [1,1,0] is a valid codeword (even parity)
        let llrs = vec![Llr::new(-10.0), Llr::new(-10.0), Llr::new(10.0)];
        let max_iter = 20;

        let algorithms = [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(0.875),
            DecoderAlgorithm::OffsetMinSum(0.5),
            DecoderAlgorithm::SumProduct,
        ];

        for &algo in &algorithms {
            let config = DecoderConfig::new(algo, true);
            let mut decoder = LdpcDecoder::with_config(code.clone(), config);
            let result = decoder.decode_iterative(&llrs, max_iter);

            assert!(
                result.converged,
                "Algorithm {:?} should converge for [1,1,0]",
                algo
            );
            assert_eq!(
                result.decoded_bits.count_ones(),
                2,
                "Algorithm {:?} should decode to [1,1,0]",
                algo
            );
        }
    }
}

#[cfg(test)]
mod profiling_helpers {
    use super::*;

    #[test]
    #[ignore]
    fn measure_check_node_degrees() {
        let code = LdpcCode::dvb_t2_normal(crate::CodeRate::Rate3_5);
        let h = code.parity_check_matrix();

        let mut degrees = Vec::new();
        for check in 0..code.m() {
            let degree = h.row_iter(check).count();
            degrees.push(degree);
        }

        degrees.sort();

        println!("\nDVB-T2 NORMAL Rate 3/5 check node degrees:");
        println!("  Total checks: {}", code.m());
        println!("  Min:    {}", degrees.iter().min().unwrap());
        println!("  Max:    {}", degrees.iter().max().unwrap());
        println!("  Median: {}", degrees[degrees.len() / 2]);
        println!(
            "  Mean:   {:.2}",
            degrees.iter().sum::<usize>() as f64 / degrees.len() as f64
        );

        let mut histogram = std::collections::HashMap::new();
        for &deg in &degrees {
            *histogram.entry(deg).or_insert(0) += 1;
        }

        println!("\nHistogram:");
        let mut bins: Vec<_> = histogram.iter().collect();
        bins.sort_by_key(|(k, _)| *k);
        for (deg, count) in bins {
            println!(
                "  Degree {:2}: {:5} checks ({:.1}%)",
                deg,
                count,
                100.0 * *count as f64 / code.m() as f64
            );
        }
    }
}

#[cfg(test)]
mod decoder_proptests {
    use super::*;
    use crate::traits::IterativeSoftDecoder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_algorithm_variants_agree_at_high_snr(
            alpha in 0.5f32..=1.0f32,
            beta in 0.0f32..=1.0f32,
            snr_mag in 5.0f32..20.0f32,
        ) {
            // Single parity check code [3,2]
            let edges = vec![(0, 0), (0, 1), (0, 2)];
            let code = LdpcCode::from_edges(1, 3, &edges);

            let llrs = vec![Llr::new(snr_mag), Llr::new(snr_mag), Llr::new(snr_mag)];

            let config_ms = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
            let mut decoder_ms = LdpcDecoder::with_config(code.clone(), config_ms);
            let result_ms = decoder_ms.decode_iterative(&llrs, 20);

            let config_nms = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(alpha), true);
            let mut decoder_nms = LdpcDecoder::with_config(code.clone(), config_nms);
            let result_nms = decoder_nms.decode_iterative(&llrs, 20);

            let config_oms = DecoderConfig::new(DecoderAlgorithm::OffsetMinSum(beta), true);
            let mut decoder_oms = LdpcDecoder::with_config(code, config_oms);
            let result_oms = decoder_oms.decode_iterative(&llrs, 20);

            prop_assert!(result_ms.converged, "MinSum should converge");
            prop_assert!(result_nms.converged, "NormalizedMinSum should converge");
            prop_assert!(result_oms.converged, "OffsetMinSum should converge");

            prop_assert_eq!(result_ms.decoded_bits.count_ones(), 0);
            prop_assert_eq!(result_nms.decoded_bits.count_ones(), 0);
            prop_assert_eq!(result_oms.decoded_bits.count_ones(), 0);
        }
    }
}
