//! Generalized LDPC (GLDPC) codes with quasi-cyclic structure.
//!
//! This module implements GLDPC codes where check nodes use component codes
//! (e.g., BCH codes) instead of simple single-parity-check constraints. The
//! construction follows Lentmaier (2010), using circulant permutation matrices
//! to build the adjacency matrix.
//!
//! # QC-GLDPC Construction
//!
//! The adjacency matrix uses circulant permutation matrices:
//!
//! ```text
//! Gamma = [[I_n^(0), I_n^(0), ..., I_n^(0)],
//!          [I_n^(0), I_n^(1), ..., I_n^(n-1)]]
//! ```
//!
//! where `I_n^(i)` is an `n x n` identity matrix right-rotated by `i` positions.
//!
//! For a component code with parameters `(n_c, k_c)`, the resulting GLDPC code
//! has length `n_c^2` and dimension `n_c^2 - rank(H)`, where H is the expanded
//! parity-check matrix. The QC structure may introduce rank deficiency, so the
//! actual dimension is computed from the rank of H.
//!
//! # Decoding
//!
//! BP decoding where each check node runs a SISO decoder on the component code.
//! Variable node updates are standard BP (sum of all incoming minus self).
//! Check node updates invoke the component SISO decoder with incoming
//! variable-to-check messages as input LLRs.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::gldpc::{QcGldpcCode, GldpcDecoder};
//! use gf2_coding::traits::BlockEncoder;
//! use gf2_core::BitVec;
//!
//! // Create a small QC-GLDPC code from BCH(7, 4, 1) component
//! let code = QcGldpcCode::lentmaier(7, 4, 1);
//! assert_eq!(code.code_n(), 49);
//! // Dimension is n - rank(H), computed from the actual parity-check matrix
//! assert!(code.code_k() > 0);
//! ```

use crate::grand::{OrbGrand, OrbGrandConfig, SoGrand};
use crate::llr::Llr;
use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::{BitMatrix, BitVec};

/// A component code used at GLDPC check nodes.
///
/// Each check node in a GLDPC code corresponds to an `(n_c, k_c)` block code.
/// The component code must support:
/// - Hard-decision validation (syndrome check)
/// - SISO decoding: given input LLRs, produce output LLRs with extrinsic info
///
/// # Parameters
///
/// - `n_c`: component codeword length (number of variables per check node)
/// - `k_c`: component message dimension
/// - `t`: error correction capability
#[derive(Clone, Debug)]
pub struct BchComponentCode {
    /// Component code parameters.
    n_c: usize,
    k_c: usize,
    t: usize,
    /// Parity-check matrix H of the component code (r x n_c where r = n_c - k_c).
    /// Stored as dense rows for fast syndrome computation.
    h_rows: Vec<Vec<usize>>,
    /// Number of parity checks (n_c - k_c).
    num_checks: usize,
}

impl BchComponentCode {
    /// Creates a BCH component code with parameters `(n, k, t)`.
    ///
    /// Constructs the component code and extracts its parity-check matrix for
    /// use in SISO check node processing.
    ///
    /// # Arguments
    ///
    /// * `n` - Component codeword length
    /// * `k` - Component message dimension
    /// * `t` - Error correction capability
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// assert_eq!(comp.n(), 7);
    /// assert_eq!(comp.k(), 4);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the BCH code parameters are invalid.
    pub fn new(n: usize, k: usize, t: usize) -> Self {
        use crate::bch::BchCode;
        use crate::traits::GeneratorMatrixAccess;
        use gf2_core::gf2m::Gf2mField;

        // Find appropriate m such that 2^m - 1 >= n
        let m = (2usize..)
            .find(|&m| (1usize << m) > n)
            .expect("Could not find suitable extension field degree");

        // Use a standard primitive polynomial for GF(2^m)
        let prim_poly = gf2_core::primitive_polys::PrimitivePolynomialDatabase::standard(m)
            .expect("No primitive polynomial available for this m");

        let field = Gf2mField::new(m, prim_poly).with_tables();
        let bch = BchCode::new(n, k, t, field);

        // Compute generator matrix G (k x n), then derive H from it.
        // For systematic G = [I_k | P], H = [-P^T | I_r] = [P^T | I_r] over GF(2).
        let g = bch.generator_matrix();
        let r = n - k;

        // Extract parity part P from G = [I_k | P]
        // H rows: for each parity check i (0..r), the row has 1s at columns
        // where P[j][k+i] = 1 for each j, plus a 1 at column (k+i).
        let mut h_rows = Vec::with_capacity(r);
        for i in 0..r {
            let mut row_ones = Vec::new();
            // P^T part: column j of H row i has a 1 if G[j][k+i] is 1
            for j in 0..k {
                if g.get(j, k + i) {
                    row_ones.push(j);
                }
            }
            // Identity part: position k+i
            row_ones.push(k + i);
            h_rows.push(row_ones);
        }

        Self {
            n_c: n,
            k_c: k,
            t,
            h_rows,
            num_checks: r,
        }
    }

    /// Returns the component codeword length.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// assert_eq!(comp.n(), 7);
    /// ```
    pub fn n(&self) -> usize {
        self.n_c
    }

    /// Returns the component message dimension.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// assert_eq!(comp.k(), 4);
    /// ```
    pub fn k(&self) -> usize {
        self.k_c
    }

    /// Returns the error correction capability.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// assert_eq!(comp.t(), 1);
    /// ```
    pub fn t(&self) -> usize {
        self.t
    }

    /// Returns the number of parity checks (n - k) in the component code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// assert_eq!(comp.num_checks(), 3);
    /// ```
    pub fn num_checks(&self) -> usize {
        self.num_checks
    }

    /// Checks whether a word satisfies all parity checks of the component code.
    ///
    /// # Arguments
    ///
    /// * `word` - Bit vector of length `n_c`
    ///
    /// # Returns
    ///
    /// `true` if the syndrome is all-zero (valid codeword).
    ///
    /// # Panics
    ///
    /// Panics if `word.len() != n_c`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    /// use gf2_core::BitVec;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// let zero = BitVec::zeros(7);
    /// assert!(comp.is_valid_codeword(&zero));
    /// ```
    pub fn is_valid_codeword(&self, word: &BitVec) -> bool {
        assert_eq!(
            word.len(),
            self.n_c,
            "Word length must equal n_c = {}",
            self.n_c
        );
        for row in &self.h_rows {
            let parity: usize = row.iter().filter(|&&col| word.get(col)).count();
            if parity % 2 != 0 {
                return false;
            }
        }
        true
    }

    /// Returns the parity-check matrix of the component code as a dense `BitMatrix`.
    ///
    /// # Returns
    ///
    /// A `BitMatrix` with `num_checks` rows and `n_c` columns.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::BchComponentCode;
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// let h = comp.h_matrix();
    /// assert_eq!(h.rows(), 3);
    /// assert_eq!(h.cols(), 7);
    /// ```
    pub fn h_matrix(&self) -> BitMatrix {
        let mut h = BitMatrix::zeros(self.num_checks, self.n_c);
        for (row_idx, row_ones) in self.h_rows.iter().enumerate() {
            for &col in row_ones {
                h.set(row_idx, col, true);
            }
        }
        h
    }
}

/// Creates an extended BCH component code by taking a BCH(n, k, t) code and
/// appending an overall parity-check bit, yielding an eBCH(n+1, k, t+?) component.
///
/// The extension adds one row to H: the all-ones row (overall even parity).
///
/// # Arguments
///
/// * `n_bch` - BCH codeword length (must be 2^m - 1)
/// * `k_bch` - BCH message dimension
/// * `t_bch` - BCH error correction capability
///
/// # Returns
///
/// A `BchComponentCode` with parameters (n_bch + 1, k_bch, t_bch).
///
/// # Examples
///
/// ```
/// use gf2_coding::gldpc::extended_bch_component;
///
/// // eBCH(32, 26) from BCH(31, 26, 1)
/// let comp = extended_bch_component(31, 26, 1);
/// assert_eq!(comp.n(), 32);
/// assert_eq!(comp.k(), 26);
/// assert_eq!(comp.num_checks(), 6);
/// ```
pub fn extended_bch_component(n_bch: usize, k_bch: usize, t_bch: usize) -> BchComponentCode {
    use crate::bch::BchCode;
    use crate::traits::GeneratorMatrixAccess;
    use gf2_core::gf2m::Gf2mField;

    let n_ext = n_bch + 1;
    let r_ext = n_ext - k_bch;

    // Build the underlying BCH code
    let m = (2usize..)
        .find(|&m| (1usize << m) > n_bch)
        .expect("Could not find suitable extension field degree");

    let prim_poly = gf2_core::primitive_polys::PrimitivePolynomialDatabase::standard(m)
        .expect("No primitive polynomial available for this m");

    let field = Gf2mField::new(m, prim_poly).with_tables();
    let bch = BchCode::new(n_bch, k_bch, t_bch, field);
    let g = bch.generator_matrix();

    let r_bch = n_bch - k_bch;

    // Build H rows from BCH parity-check matrix: H_bch = [P^T | I_r]
    let mut h_rows = Vec::with_capacity(r_ext);

    for i in 0..r_bch {
        let mut row_ones = Vec::new();
        for j in 0..k_bch {
            if g.get(j, k_bch + i) {
                row_ones.push(j);
            }
        }
        row_ones.push(k_bch + i);
        h_rows.push(row_ones);
    }

    // Add the overall parity-check row: sum of all n_ext bits = 0
    // This is the all-ones row of length n_ext
    let parity_row: Vec<usize> = (0..n_ext).collect();
    h_rows.push(parity_row);

    BchComponentCode {
        n_c: n_ext,
        k_c: k_bch,
        t: t_bch,
        h_rows,
        num_checks: r_ext,
    }
}

/// A quasi-cyclic Generalized LDPC code.
///
/// Constructed using circulant permutation matrices following the Lentmaier (2010)
/// construction. Each check node corresponds to a component code rather than a
/// single parity check.
///
/// # Code Parameters
///
/// For component code `(n_c, k_c)`:
/// - Code length: `n_c^2`
/// - Dimension: `n_c^2 - rank(H)` (computed from the expanded parity-check matrix)
/// - Number of check nodes: `2 * n_c` (each of degree `n_c`)
///
/// # Structure
///
/// The adjacency matrix `Gamma` is a `2 x n_c` block matrix of `n_c x n_c`
/// circulant permutation matrices:
///
/// ```text
/// Row 0: [I^0, I^0, ..., I^0]        (n_c identity blocks)
/// Row 1: [I^0, I^1, ..., I^(n_c-1)]  (n_c circulant blocks with shifts 0..n_c-1)
/// ```
///
/// # Examples
///
/// ```
/// use gf2_coding::gldpc::QcGldpcCode;
/// use gf2_coding::traits::BlockEncoder;
/// use gf2_core::BitVec;
///
/// let code = QcGldpcCode::lentmaier(7, 4, 1);
/// assert_eq!(code.code_n(), 49);
/// assert!(code.code_k() > 0);
///
/// let msg = BitVec::zeros(code.code_k());
/// let cw = code.encode(&msg);
/// assert_eq!(cw.len(), 49);
/// ```
#[derive(Clone, Debug)]
pub struct QcGldpcCode {
    /// Component code.
    component: BchComponentCode,
    /// Total code length (n_c^2).
    n: usize,
    /// Code dimension (n - rank(H)).
    k: usize,
    /// Number of check nodes (2 * n_c).
    num_check_nodes: usize,
    /// For each check node, the indices of its n_c connected variable nodes.
    /// check_node_vars[i] has length n_c, listing global variable indices.
    check_node_vars: Vec<Vec<usize>>,
    /// For each variable node, the list of (check_node_idx, position_within_check)
    /// pairs indicating which check nodes it belongs to and at what position.
    var_check_map: Vec<Vec<(usize, usize)>>,
    /// Non-pivot column indices from RREF, used as systematic (information) positions.
    /// Message bits map to/from these column positions in the codeword.
    systematic_positions: Vec<usize>,
    /// Generator matrix (computed lazily).
    cached_generator: std::sync::Arc<std::sync::Mutex<Option<gf2_core::BitMatrix>>>,
}

impl QcGldpcCode {
    /// Creates a QC-GLDPC code using the Lentmaier (2010) construction.
    ///
    /// Builds a GLDPC code with length `n_c^2` from a BCH
    /// component code with parameters `(n_c, k_c, t)`.
    ///
    /// # Arguments
    ///
    /// * `n_c` - Component code codeword length
    /// * `k_c` - Component code message dimension
    /// * `t` - Component code error correction capability
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// // BCH(7,4,1) component -> (49, 7) GLDPC code
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// assert_eq!(code.code_n(), 49);
    /// assert!(code.code_k() > 0);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the component code parameters are invalid.
    pub fn lentmaier(n_c: usize, k_c: usize, t: usize) -> Self {
        let component = BchComponentCode::new(n_c, k_c, t);
        Self::from_component(component)
    }

    /// Creates a (1024, 646) QC-GLDPC code using eBCH(32, 26) as the component code.
    ///
    /// This is the target construction from Lentmaier (2010) for comparison
    /// with 5G NR LDPC codes in the SO-GRAND paper (Fig. 7).
    ///
    /// - Component: extended BCH(32, 26) derived from BCH(31, 26, 1)
    /// - Code length: 32² = 1024
    /// - Ideal dimension: 640 (formula k = n² − 2n(n−k) = 1024 − 384)
    /// - **Actual dimension: 646** — the QC circulant adjacency structure
    ///   introduces 6 linearly dependent rows in H, so rank(H) = 378
    ///   instead of 384, giving k = 1024 − 378 = 646. This is an inherent
    ///   property of the circulant construction and matches the behavior
    ///   reported in QC-LDPC literature. The effective rate (0.631) is
    ///   close to the ideal (0.625).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier_1024();
    /// assert_eq!(code.code_n(), 1024);
    /// assert_eq!(code.code_k(), 646);
    /// ```
    ///
    /// # Complexity
    ///
    /// Construction involves RREF on a matrix with up to 384 rows and 1024 columns.
    /// This takes a few seconds on a modern CPU.
    pub fn lentmaier_1024() -> Self {
        let component = extended_bch_component(31, 26, 1);
        Self::from_component(component)
    }

    /// Creates a QC-GLDPC code from an existing component code.
    ///
    /// # Arguments
    ///
    /// * `component` - The component code for check nodes
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::{BchComponentCode, QcGldpcCode};
    ///
    /// let comp = BchComponentCode::new(7, 4, 1);
    /// let code = QcGldpcCode::from_component(comp);
    /// assert_eq!(code.code_n(), 49);
    /// ```
    pub fn from_component(component: BchComponentCode) -> Self {
        let n_c = component.n();
        let num_check_nodes = 2 * n_c;
        let n = n_c * n_c;

        // Build the adjacency structure.
        //
        // Variables are indexed 0..n_c^2, laid out as n_c blocks of n_c.
        // Variable (block_col * n_c + offset) for block_col in 0..n_c, offset in 0..n_c.
        //
        // Check node (row_block * n_c + row_offset):
        //   row_block = 0: Gamma[0][j] = I_n^0 for all j
        //     -> check (0 * n_c + row_offset) connects to variable (j * n_c + row_offset)
        //        for each block column j, at position j within the check node.
        //   row_block = 1: Gamma[1][j] = I_n^j
        //     -> check (1 * n_c + row_offset) connects to
        //        variable (j * n_c + (row_offset + j) mod n_c)
        //        for each block column j, at position j within the check node.
        let mut check_node_vars = Vec::with_capacity(num_check_nodes);

        // Row block 0: all identity shifts (shift = 0)
        for row_offset in 0..n_c {
            let mut vars = Vec::with_capacity(n_c);
            for block_col in 0..n_c {
                // I_n^0: variable at (block_col * n_c + row_offset)
                vars.push(block_col * n_c + row_offset);
            }
            check_node_vars.push(vars);
        }

        // Row block 1: circulant shifts (shift = block_col)
        for row_offset in 0..n_c {
            let mut vars = Vec::with_capacity(n_c);
            for block_col in 0..n_c {
                // I_n^block_col: variable at (block_col * n_c + (row_offset + block_col) % n_c)
                let col_offset = (row_offset + block_col) % n_c;
                vars.push(block_col * n_c + col_offset);
            }
            check_node_vars.push(vars);
        }

        // Build reverse map: var -> list of (check_node, position_in_check)
        let mut var_check_map: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (check_idx, vars) in check_node_vars.iter().enumerate() {
            for (pos, &var_idx) in vars.iter().enumerate() {
                var_check_map[var_idx].push((check_idx, pos));
            }
        }

        // Compute the actual code dimension and systematic positions from RREF of H.
        // The formula k = n - 2*n_c*(n_c - k_c) assumes full rank, but the QC
        // structure introduces linear dependencies among check rows. We compute
        // the true rank via RREF.
        let (k, systematic_positions) = {
            let r_c = component.num_checks();
            let total_rows = num_check_nodes * r_c;
            let mut edges = Vec::new();
            for (check_idx, vars) in check_node_vars.iter().enumerate() {
                let row_base = check_idx * r_c;
                for (local_check, h_row) in component.h_rows.iter().enumerate() {
                    let global_row = row_base + local_check;
                    for &local_col in h_row {
                        let global_col = vars[local_col];
                        edges.push((global_row, global_col));
                    }
                }
            }
            let h_sparse = SpBitMatrixDual::from_coo(total_rows, n, &edges);
            let h_dense = h_sparse.to_dense();
            let rref_result = gf2_core::alg::rref::rref(&h_dense, false);
            let k = n - rref_result.rank;
            let sys_pos: Vec<usize> = (0..n)
                .filter(|c| !rref_result.pivot_cols.contains(c))
                .collect();
            assert_eq!(sys_pos.len(), k);
            (k, sys_pos)
        };

        Self {
            component,
            n,
            k,
            num_check_nodes,
            check_node_vars,
            var_check_map,
            systematic_positions,
            cached_generator: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Returns the total code length (n_c^2).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// assert_eq!(code.code_n(), 49);
    /// ```
    pub fn code_n(&self) -> usize {
        self.n
    }

    /// Returns the code dimension.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// assert!(code.code_k() > 0);
    /// ```
    pub fn code_k(&self) -> usize {
        self.k
    }

    /// Returns the number of check nodes (2 * n_c).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// assert_eq!(code.num_check_nodes(), 14);
    /// ```
    pub fn num_check_nodes(&self) -> usize {
        self.num_check_nodes
    }

    /// Returns a reference to the component code.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// assert_eq!(code.component().n(), 7);
    /// ```
    pub fn component(&self) -> &BchComponentCode {
        &self.component
    }

    /// Returns the systematic (information) positions in the codeword.
    ///
    /// These are the non-pivot columns from RREF of H. Message bit `i` is placed
    /// at codeword position `systematic_positions()[i]` during encoding, and
    /// extracted from the same position during decoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// assert_eq!(code.systematic_positions().len(), code.code_k());
    /// ```
    pub fn systematic_positions(&self) -> &[usize] {
        &self.systematic_positions
    }

    /// Returns the variable indices connected to a given check node.
    ///
    /// # Arguments
    ///
    /// * `check` - Check node index (0..num_check_nodes)
    ///
    /// # Panics
    ///
    /// Panics if `check >= num_check_nodes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let vars = code.check_variables(0);
    /// assert_eq!(vars.len(), 7); // Each check node has degree n_c
    /// ```
    pub fn check_variables(&self, check: usize) -> &[usize] {
        assert!(
            check < self.num_check_nodes,
            "Check node index {} out of range (num_check_nodes = {})",
            check,
            self.num_check_nodes
        );
        &self.check_node_vars[check]
    }

    /// Checks if a codeword satisfies all GLDPC check node constraints.
    ///
    /// Each check node extracts its connected variable bits and verifies them
    /// against the component code.
    ///
    /// # Arguments
    ///
    /// * `codeword` - Bit vector of length `code_n()`
    ///
    /// # Returns
    ///
    /// `true` if all check nodes are satisfied.
    ///
    /// # Panics
    ///
    /// Panics if `codeword.len() != code_n()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let msg = BitVec::zeros(code.code_k());
    /// let cw = code.encode(&msg);
    /// assert!(code.is_valid_codeword(&cw));
    /// ```
    pub fn is_valid_codeword(&self, codeword: &BitVec) -> bool {
        assert_eq!(codeword.len(), self.n, "Codeword length must be {}", self.n);

        for check_vars in &self.check_node_vars {
            let mut local = BitVec::with_capacity(self.component.n());
            for &var_idx in check_vars {
                local.push_bit(codeword.get(var_idx));
            }
            if !self.component.is_valid_codeword(&local) {
                return false;
            }
        }
        true
    }

    /// Builds the full parity-check matrix of the GLDPC code.
    ///
    /// Each check node contributes `(n_c - k_c)` rows (the component H matrix
    /// applied to the check node's connected variables). The full H has
    /// `2 * n_c * (n_c - k_c)` rows and `n_c^2` columns.
    ///
    /// # Returns
    ///
    /// Sparse parity-check matrix as `(rows, cols, edges)`.
    ///
    /// # Complexity
    ///
    /// O(num_check_nodes * r_c * n_c) where r_c = n_c - k_c.
    fn build_parity_check_edges(&self) -> (usize, usize, Vec<(usize, usize)>) {
        let r_c = self.component.num_checks();
        let total_rows = self.num_check_nodes * r_c;
        let total_cols = self.n;
        let mut edges = Vec::new();

        for (check_idx, check_vars) in self.check_node_vars.iter().enumerate() {
            let row_base = check_idx * r_c;
            for (local_check, h_row) in self.component.h_rows.iter().enumerate() {
                let global_row = row_base + local_check;
                for &local_col in h_row {
                    let global_col = check_vars[local_col];
                    edges.push((global_row, global_col));
                }
            }
        }

        (total_rows, total_cols, edges)
    }

    /// Computes the generator matrix using RREF on the parity-check matrix.
    ///
    /// Uses the stored `systematic_positions` (non-pivot columns from RREF)
    /// to ensure encoding and decoding use exactly the same information positions.
    ///
    /// # Complexity
    ///
    /// O(m^2 * n) for RREF where m = number of parity checks, n = code length.
    fn compute_generator_matrix(&self) -> BitMatrix {
        use gf2_core::alg::rref::rref;

        let (m, n, edges) = self.build_parity_check_edges();
        let h_sparse = SpBitMatrixDual::from_coo(m, n, &edges);
        let h_dense = h_sparse.to_dense();

        // RREF to find systematic form
        let rref_result = rref(&h_dense, false);
        let rank = rref_result.rank;
        let h_rref = rref_result.reduced;
        let pivot_cols = rref_result.pivot_cols;

        let k = n - rank;
        assert_eq!(
            k, self.k,
            "Computed dimension {} does not match expected {}",
            k, self.k
        );

        // Use the stored systematic positions (non-pivot columns, computed at construction)
        let systematic_positions = &self.systematic_positions;
        assert_eq!(systematic_positions.len(), k);

        // Build G (k x n): G = [I_k columns at systematic_positions, parity elsewhere]
        let mut g = BitMatrix::zeros(k, n);

        // Identity part
        for (i, &sys_col) in systematic_positions.iter().enumerate() {
            g.set(i, sys_col, true);
        }

        // Parity part
        for (msg_idx, &sys_col) in systematic_positions.iter().enumerate() {
            for (check_idx, &parity_col) in pivot_cols.iter().enumerate() {
                if h_rref.get(check_idx, sys_col) {
                    g.set(msg_idx, parity_col, true);
                }
            }
        }

        g
    }

    /// Returns or computes the generator matrix (cached).
    fn generator_matrix_cached(&self) -> gf2_core::BitMatrix {
        let mut cache = self.cached_generator.lock().unwrap();
        if let Some(ref g) = *cache {
            g.clone()
        } else {
            let g = self.compute_generator_matrix();
            *cache = Some(g.clone());
            g
        }
    }
}

impl BlockEncoder for QcGldpcCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    /// Encodes a message into a GLDPC codeword.
    ///
    /// Uses the generator matrix `G` (computed via RREF of H). The codeword is
    /// `c = m * G` over GF(2).
    ///
    /// # Arguments
    ///
    /// * `message` - Bit vector of length `k`
    ///
    /// # Returns
    ///
    /// Codeword of length `n`.
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != k`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::BitVec;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let msg = BitVec::zeros(code.code_k());
    /// let cw = code.encode(&msg);
    /// assert_eq!(cw.len(), 49);
    /// ```
    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.k,
            "Message must have length k = {}",
            self.k
        );

        let g = self.generator_matrix_cached();

        // c = m * G over GF(2): for each column j of G, c[j] = sum_i m[i]*G[i][j]
        let mut codeword = BitVec::zeros(self.n);
        for i in 0..self.k {
            if message.get(i) {
                // XOR row i of G into codeword
                for j in 0..self.n {
                    if g.get(i, j) {
                        codeword.set(j, !codeword.get(j));
                    }
                }
            }
        }
        codeword
    }
}

impl crate::traits::GeneratorMatrixAccess for QcGldpcCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    /// Returns the generator matrix of the GLDPC code.
    ///
    /// Computed lazily via RREF of the parity-check matrix and cached.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::QcGldpcCode;
    /// use gf2_coding::traits::GeneratorMatrixAccess;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let g = code.generator_matrix();
    /// assert_eq!(g.rows(), code.code_k());
    /// assert_eq!(g.cols(), code.code_n());
    /// ```
    fn generator_matrix(&self) -> gf2_core::BitMatrix {
        self.generator_matrix_cached()
    }
}

/// Belief-propagation decoder for QC-GLDPC codes with component SISO check nodes.
///
/// Unlike standard LDPC BP decoding (where check nodes compute box-plus), GLDPC
/// check nodes run a full SISO decoder on the component code. Variable node
/// updates remain standard BP.
///
/// # Algorithm
///
/// 1. **Initialize**: Variable-to-check messages = channel LLRs
/// 2. **Check node update**: Collect variable-to-check LLRs for each check node,
///    run component SISO decoder, distribute extrinsic output as check-to-variable
///    messages
/// 3. **Variable node update**: For each variable, posterior = channel LLR +
///    sum of incoming check-to-variable messages. Outgoing variable-to-check =
///    posterior - incoming from that check.
/// 4. **Convergence check**: Hard-decide posteriors and verify all check nodes
///
/// # Examples
///
/// ```
/// use gf2_coding::gldpc::{QcGldpcCode, GldpcDecoder};
/// use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
/// use gf2_coding::llr::Llr;
/// use gf2_core::BitVec;
///
/// let code = QcGldpcCode::lentmaier(7, 4, 1);
/// let mut decoder = GldpcDecoder::new(code.clone());
///
/// // Encode the all-zeros codeword and create perfect channel LLRs
/// let msg = BitVec::zeros(code.code_k());
/// let cw = code.encode(&msg);
/// let llrs: Vec<Llr> = (0..code.code_n())
///     .map(|i| if cw.get(i) { Llr::new(-5.0) } else { Llr::new(5.0) })
///     .collect();
///
/// let result = decoder.decode_iterative(&llrs, 20);
/// assert!(result.converged);
/// ```
pub struct GldpcDecoder {
    /// The GLDPC code being decoded.
    code: QcGldpcCode,
    /// Current posterior beliefs for each variable node.
    beliefs: Vec<Llr>,
    /// Check-to-variable messages: check_to_var[check][position] -> Llr.
    check_to_var: Vec<Vec<Llr>>,
    /// Variable-to-check messages: var_to_check[var][idx_in_var_check_map] -> Llr.
    var_to_check: Vec<Vec<Llr>>,
    /// Prebuilt SOGRAND decoder for the component code (reused across check nodes).
    component_sogrand: SoGrand,
    /// Lookup table: var_check_idx[var][check_idx] -> index in var_check_map[var].
    /// Avoids O(degree) linear scan in the hot check-node update path.
    var_check_idx: Vec<std::collections::HashMap<usize, usize>>,
    /// Number of iterations in the last decode call.
    last_iterations: usize,
}

impl std::fmt::Debug for GldpcDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GldpcDecoder")
            .field("code", &self.code)
            .field("beliefs_len", &self.beliefs.len())
            .field("last_iterations", &self.last_iterations)
            .finish_non_exhaustive()
    }
}

impl GldpcDecoder {
    /// Creates a new GLDPC decoder for the given code.
    ///
    /// Builds a SOGRAND decoder from the component code's parity-check matrix,
    /// used at each check node during BP iterations.
    ///
    /// # Arguments
    ///
    /// * `code` - The QC-GLDPC code to decode
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::{QcGldpcCode, GldpcDecoder};
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let decoder = GldpcDecoder::new(code);
    /// ```
    pub fn new(code: QcGldpcCode) -> Self {
        // Detect if the component code is an even code (all codewords have
        // even Hamming weight). This is the case for extended BCH codes,
        // which have an all-ones parity-check row. Enabling even_code halves
        // the ORBGRAND search space by skipping impossible parity patterns.
        let n_c = code.component().n();
        let is_even = code
            .component()
            .h_rows
            .iter()
            .any(|row| row.len() == n_c && (0..n_c).all(|i| row.contains(&i)));

        let config = OrbGrandConfig {
            // list_size=4 gives SOGRAND enough codeword diversity for
            // meaningful soft output (APP LLRs) at check nodes.
            list_size: 4,
            even_code: is_even,
            // For n=32 component codes, 100K queries covers the high-probability
            // patterns sufficiently for good soft output while keeping per-frame
            // runtime bounded (~0.2 fps at n=1024 with 1M queries, ~2 fps with 100K).
            max_queries: 100_000,
            ..OrbGrandConfig::default()
        };
        Self::with_sogrand_config(code, config)
    }

    /// Creates a new GLDPC decoder with a custom ORBGRAND configuration for
    /// the component SOGRAND check node decoder.
    ///
    /// # Arguments
    ///
    /// * `code` - The QC-GLDPC code to decode
    /// * `orbgrand_config` - Configuration for the underlying ORBGRAND decoder
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::{QcGldpcCode, GldpcDecoder};
    /// use gf2_coding::grand::OrbGrandConfig;
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let config = OrbGrandConfig {
    ///     list_size: 8,
    ///     ..OrbGrandConfig::default()
    /// };
    /// let decoder = GldpcDecoder::with_sogrand_config(code, config);
    /// ```
    pub fn with_sogrand_config(code: QcGldpcCode, orbgrand_config: OrbGrandConfig) -> Self {
        let n = code.code_n();

        // Build the component H matrix and create the SOGRAND decoder
        let h_matrix = code.component().h_matrix();
        let orbgrand = OrbGrand::new(h_matrix, orbgrand_config);
        let component_sogrand = SoGrand::new(orbgrand);

        let check_to_var: Vec<Vec<Llr>> = code
            .check_node_vars
            .iter()
            .map(|vars| vec![Llr::zero(); vars.len()])
            .collect();

        let var_to_check: Vec<Vec<Llr>> = code
            .var_check_map
            .iter()
            .map(|checks| vec![Llr::zero(); checks.len()])
            .collect();

        let var_check_idx: Vec<std::collections::HashMap<usize, usize>> = code
            .var_check_map
            .iter()
            .map(|var_checks| {
                var_checks
                    .iter()
                    .enumerate()
                    .map(|(vc_idx, &(ci, _))| (ci, vc_idx))
                    .collect()
            })
            .collect();

        Self {
            code,
            beliefs: vec![Llr::zero(); n],
            check_to_var,
            var_to_check,
            component_sogrand,
            var_check_idx,
            last_iterations: 0,
        }
    }

    /// Performs check node update using full SISO-SOGRAND component decoding.
    ///
    /// For each check node, collects variable-to-check messages, runs
    /// SOGRAND to obtain extrinsic LLRs, and stores them as
    /// check-to-variable messages.
    fn check_node_update(&mut self) {
        let n_c = self.code.component().n();

        for (check_idx, check_vars) in self.code.check_node_vars.iter().enumerate() {
            // Collect input LLRs from variable-to-check messages
            let mut input_llrs = Vec::with_capacity(n_c);
            for &var_idx in check_vars.iter() {
                // O(1) lookup via prebuilt index (replaces O(degree) linear scan)
                let vc_idx = self.var_check_idx[var_idx][&check_idx];
                input_llrs.push(self.var_to_check[var_idx][vc_idx]);
            }

            // Run SOGRAND SISO decoder on the component code
            let siso_result = self.component_sogrand.decode_siso(&input_llrs);

            // Store extrinsic LLRs as check-to-variable messages
            for (pos, ext) in siso_result.extrinsic_llrs.into_iter().enumerate() {
                self.check_to_var[check_idx][pos] = ext;
            }
        }
    }

    /// Performs standard BP variable node update.
    ///
    /// For each variable: posterior = channel_llr + sum(incoming check-to-var).
    /// Outgoing variable-to-check = posterior - incoming from that check.
    fn variable_node_update(&mut self, channel_llrs: &[Llr]) {
        for (var, checks) in self.code.var_check_map.iter().enumerate() {
            // Compute total belief
            let mut total = channel_llrs[var].value();
            for &(check_idx, pos) in checks {
                total += self.check_to_var[check_idx][pos].value();
            }
            self.beliefs[var] = Llr::new(total);

            // Compute outgoing variable-to-check messages
            for (idx, &(check_idx, pos)) in checks.iter().enumerate() {
                let incoming = self.check_to_var[check_idx][pos].value();
                self.var_to_check[var][idx] = Llr::new(total - incoming);
            }
        }
    }

    /// Makes hard decisions based on current beliefs.
    fn hard_decode(&self) -> BitVec {
        let mut decoded = BitVec::with_capacity(self.code.code_n());
        for &belief in &self.beliefs {
            decoded.push_bit(belief.hard_decision());
        }
        decoded
    }
}

impl SoftDecoder for GldpcDecoder {
    fn k(&self) -> usize {
        self.code.k
    }

    fn n(&self) -> usize {
        self.code.n
    }

    fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
        assert_eq!(llrs.len(), self.n());
        let mut decoded = BitVec::with_capacity(self.k());
        for &sys_col in self.code.systematic_positions() {
            decoded.push_bit(llrs[sys_col].hard_decision());
        }
        decoded
    }
}

impl IterativeSoftDecoder for GldpcDecoder {
    /// Decodes using iterative BP with SISO component check nodes.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Channel LLRs of length `n`
    /// * `max_iterations` - Maximum outer BP iterations
    ///
    /// # Returns
    ///
    /// Decoded message bits with convergence metadata.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n`.
    fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
        assert_eq!(
            llrs.len(),
            self.n(),
            "LLR length must equal n = {}",
            self.n()
        );

        // Reset messages
        for check_msgs in &mut self.check_to_var {
            for msg in check_msgs.iter_mut() {
                *msg = Llr::zero();
            }
        }

        // Initialize variable-to-check with channel LLRs
        for (var, checks) in self.code.var_check_map.iter().enumerate() {
            for idx in 0..checks.len() {
                self.var_to_check[var][idx] = llrs[var];
            }
        }

        // Initialize beliefs
        for (var, &llr) in llrs.iter().enumerate() {
            self.beliefs[var] = llr;
        }

        let mut iterations = 0;
        let mut converged = false;

        for iter in 0..max_iterations {
            iterations = iter + 1;

            // Check node update (SISO component decoding)
            self.check_node_update();

            // Variable node update
            self.variable_node_update(llrs);

            // Convergence check
            let decoded = self.hard_decode();
            if self.code.is_valid_codeword(&decoded) {
                converged = true;
                break;
            }
        }

        self.last_iterations = iterations;
        let decoded_codeword = self.hard_decode();
        let syndrome_passed = self.code.is_valid_codeword(&decoded_codeword);

        // Extract message bits from the systematic (non-pivot) positions,
        // matching the positions used by the encoder.
        let k = self.code.code_k();
        let mut message = BitVec::with_capacity(k);
        for &sys_col in self.code.systematic_positions() {
            message.push_bit(decoded_codeword.get(sys_col));
        }

        DecoderResult::new(message, iterations, converged, syndrome_passed)
    }

    fn last_iteration_count(&self) -> usize {
        self.last_iterations
    }

    fn reset(&mut self) {
        for check_msgs in &mut self.check_to_var {
            for msg in check_msgs.iter_mut() {
                *msg = Llr::zero();
            }
        }
        for var_msgs in &mut self.var_to_check {
            for msg in var_msgs.iter_mut() {
                *msg = Llr::zero();
            }
        }
        for belief in &mut self.beliefs {
            *belief = Llr::zero();
        }
        self.last_iterations = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- BchComponentCode tests ---

    #[test]
    fn test_component_code_parameters() {
        let comp = BchComponentCode::new(7, 4, 1);
        assert_eq!(comp.n(), 7);
        assert_eq!(comp.k(), 4);
        assert_eq!(comp.t(), 1);
        assert_eq!(comp.num_checks(), 3);
    }

    #[test]
    fn test_component_code_zero_is_codeword() {
        let comp = BchComponentCode::new(7, 4, 1);
        let zero = BitVec::zeros(7);
        assert!(comp.is_valid_codeword(&zero));
    }

    #[test]
    fn test_component_code_validates_bch_codewords() {
        use crate::bch::{BchCode, BchEncoder};
        use gf2_core::gf2m::Gf2mField;

        let field = Gf2mField::new(3, 0b1011).with_tables();
        let bch = BchCode::new(7, 4, 1, field);
        let encoder = BchEncoder::new(bch);

        let comp = BchComponentCode::new(7, 4, 1);

        // Encode several messages and verify the component code accepts them
        for pattern in 0u8..16 {
            let mut msg = BitVec::with_capacity(4);
            for bit in 0..4 {
                msg.push_bit((pattern >> bit) & 1 == 1);
            }
            let cw = encoder.encode(&msg);
            assert!(
                comp.is_valid_codeword(&cw),
                "BCH codeword for pattern {} should be valid",
                pattern
            );
        }
    }

    #[test]
    fn test_component_code_rejects_errors() {
        use crate::bch::{BchCode, BchEncoder};
        use gf2_core::gf2m::Gf2mField;

        let field = Gf2mField::new(3, 0b1011).with_tables();
        let bch = BchCode::new(7, 4, 1, field);
        let encoder = BchEncoder::new(bch);
        let comp = BchComponentCode::new(7, 4, 1);

        let msg = BitVec::ones(4);
        let cw = encoder.encode(&msg);
        assert!(comp.is_valid_codeword(&cw));

        // Flip one bit -> should fail
        let mut corrupted = cw.clone();
        corrupted.set(0, !corrupted.get(0));
        assert!(!comp.is_valid_codeword(&corrupted));
    }

    #[test]
    fn test_component_h_matrix_dimensions() {
        let comp = BchComponentCode::new(7, 4, 1);
        let h = comp.h_matrix();
        assert_eq!(h.rows(), 3); // num_checks = 7 - 4 = 3
        assert_eq!(h.cols(), 7);
    }

    #[test]
    fn test_sogrand_siso_extrinsic_for_valid_codeword() {
        let comp = BchComponentCode::new(7, 4, 1);
        let h = comp.h_matrix();
        let orbgrand = OrbGrand::new(h, OrbGrandConfig::default());
        let sogrand = SoGrand::new(orbgrand);

        // Strong LLRs for all-zeros codeword (positive = bit 0 likely)
        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let result = sogrand.decode_siso(&llrs);

        // Extrinsic should be non-negative (reinforcing the decision)
        for ext in &result.extrinsic_llrs {
            assert!(
                ext.value() >= -0.01,
                "Extrinsic {} should be non-negative for valid all-zeros",
                ext.value()
            );
        }
    }

    // --- QcGldpcCode construction tests ---

    #[test]
    fn test_lentmaier_7_4_1_parameters() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        assert_eq!(code.code_n(), 49);
        // Dimension is n - rank(H); rank deficiency means k > n - 2*n_c*r_c
        assert!(code.code_k() > 0);
        assert!(code.code_k() < code.code_n());
        assert_eq!(code.num_check_nodes(), 14);
    }

    #[test]
    fn test_check_node_degree() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        // Each check node should connect to exactly n_c = 7 variables
        for check in 0..code.num_check_nodes() {
            assert_eq!(
                code.check_variables(check).len(),
                7,
                "Check {} should have degree 7",
                check
            );
        }
    }

    #[test]
    fn test_variable_node_degree() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        // Each variable participates in exactly 2 check nodes (one from each row block)
        for var in 0..code.code_n() {
            assert_eq!(
                code.var_check_map[var].len(),
                2,
                "Variable {} should participate in 2 checks",
                var
            );
        }
    }

    #[test]
    fn test_check_node_vars_distinct() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        for check in 0..code.num_check_nodes() {
            let vars = code.check_variables(check);
            let mut sorted = vars.to_vec();
            sorted.sort();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                vars.len(),
                "Check {} should have distinct variable connections",
                check
            );
        }
    }

    #[test]
    fn test_all_variables_covered() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut covered = vec![false; code.code_n()];
        for check in 0..code.num_check_nodes() {
            for &var in code.check_variables(check) {
                covered[var] = true;
            }
        }
        assert!(
            covered.iter().all(|&c| c),
            "All variables should be covered"
        );
    }

    // --- Encoding tests ---

    #[test]
    fn test_encode_zero_message() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let msg = BitVec::zeros(code.code_k());
        let cw = code.encode(&msg);
        assert_eq!(cw.len(), code.code_n());
        // Zero message should produce zero codeword
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_produces_valid_codeword() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);

        // Test several message patterns (up to 128 patterns)
        let num_patterns = 1u32 << code.code_k().min(7);
        for pattern in 0u32..num_patterns {
            let mut msg = BitVec::with_capacity(code.code_k());
            for bit in 0..code.code_k() {
                msg.push_bit(if bit < 32 {
                    (pattern >> bit) & 1 == 1
                } else {
                    false
                });
            }
            let cw = code.encode(&msg);
            assert!(
                code.is_valid_codeword(&cw),
                "Codeword for pattern {} should be valid",
                pattern
            );
        }
    }

    #[test]
    fn test_encode_all_codewords_valid() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let k = code.code_k();
        // Exhaustive check: sample all 2^k messages if k <= 16, else sample 1024
        let num_patterns: u32 = if k <= 16 { 1u32 << k } else { 1024 };
        for pattern in 0u32..num_patterns {
            let mut msg = BitVec::with_capacity(k);
            for bit in 0..k {
                msg.push_bit(if bit < 32 {
                    (pattern >> bit) & 1 == 1
                } else {
                    false
                });
            }
            let cw = code.encode(&msg);
            assert!(
                code.is_valid_codeword(&cw),
                "Codeword for pattern {} should be valid",
                pattern
            );
        }
    }

    #[test]
    fn test_encode_distinct_codewords() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let k = code.code_k();
        // If k is too large for exhaustive enumeration, test a subset
        let num_patterns: u32 = if k <= 16 { 1u32 << k } else { 1024 };
        let mut codewords = std::collections::HashSet::new();

        for pattern in 0u32..num_patterns {
            let mut msg = BitVec::with_capacity(k);
            for bit in 0..k {
                msg.push_bit(if bit < 32 {
                    (pattern >> bit) & 1 == 1
                } else {
                    false
                });
            }
            let cw = code.encode(&msg);
            let key: Vec<bool> = (0..code.code_n()).map(|i| cw.get(i)).collect();
            codewords.insert(key);
        }

        assert_eq!(
            codewords.len(),
            num_patterns as usize,
            "All tested codewords should be distinct"
        );
    }

    #[test]
    #[should_panic(expected = "Message must have length k")]
    fn test_encode_wrong_length() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let msg = BitVec::zeros(3); // wrong length
        code.encode(&msg);
    }

    // --- Generator matrix tests ---

    #[test]
    fn test_generator_matrix_dimensions() {
        use crate::traits::GeneratorMatrixAccess;
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let g = code.generator_matrix();
        assert_eq!(g.rows(), code.code_k());
        assert_eq!(g.cols(), code.code_n());
    }

    // --- Decoder tests ---

    #[test]
    fn test_decoder_creation() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let decoder = GldpcDecoder::new(code.clone());
        assert_eq!(decoder.k(), code.code_k());
        assert_eq!(decoder.n(), code.code_n());
    }

    #[test]
    fn test_decode_all_zeros() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());

        // All-zeros codeword with high-confidence LLRs (positive = bit 0)
        let llrs: Vec<Llr> = vec![Llr::new(10.0); code.code_n()];
        let result = decoder.decode_iterative(&llrs, 20);

        assert!(result.converged, "Should converge for clean all-zeros");
        assert!(result.syndrome_check_passed);
        assert_eq!(result.decoded_bits.len(), code.code_k());
        // All-zeros message
        for i in 0..code.code_k() {
            assert!(!result.decoded_bits.get(i), "Bit {} should be 0", i);
        }
    }

    #[test]
    fn test_decode_encoded_message() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());

        // Encode a non-trivial message
        let mut msg = BitVec::with_capacity(code.code_k());
        for i in 0..code.code_k() {
            msg.push_bit(i % 2 == 0);
        }
        let cw = code.encode(&msg);

        // Create LLRs from codeword (high confidence, no noise)
        let llrs: Vec<Llr> = (0..code.code_n())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-10.0) // bit 1 -> negative LLR
                } else {
                    Llr::new(10.0) // bit 0 -> positive LLR
                }
            })
            .collect();

        let result = decoder.decode_iterative(&llrs, 20);
        assert!(result.converged, "Should converge with clean LLRs");
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_decode_with_noise() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());

        // Encode all-zeros message
        let msg = BitVec::zeros(code.code_k());
        let cw = code.encode(&msg);

        // Add moderate noise: most bits have correct sign, a few are uncertain
        let mut llrs: Vec<Llr> = (0..code.code_n())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-3.0)
                } else {
                    Llr::new(3.0)
                }
            })
            .collect();

        // Make one bit uncertain but not flipped
        llrs[0] = Llr::new(0.5);

        let result = decoder.decode_iterative(&llrs, 30);
        // With moderate noise and iterative decoding, should still converge
        assert!(
            result.converged,
            "Should converge with moderate noise (iterations: {})",
            result.iterations
        );
    }

    #[test]
    fn test_decoder_reset() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());

        let llrs: Vec<Llr> = vec![Llr::new(10.0); code.code_n()];
        decoder.decode_iterative(&llrs, 5);

        assert!(decoder.last_iteration_count() > 0);

        decoder.reset();
        assert_eq!(decoder.last_iteration_count(), 0);
    }

    #[test]
    fn test_decoder_max_iterations_respected() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());

        // All uncertain -> may not converge
        let llrs: Vec<Llr> = vec![Llr::new(0.01); code.code_n()];
        let result = decoder.decode_iterative(&llrs, 3);
        assert!(result.iterations <= 3);
    }

    #[test]
    #[should_panic(expected = "LLR length must equal n")]
    fn test_decode_wrong_llr_length() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());
        let llrs: Vec<Llr> = vec![Llr::new(1.0); 10]; // wrong length
        decoder.decode_iterative(&llrs, 5);
    }

    // --- QC structure property tests ---

    #[test]
    fn test_row_block_0_is_identity() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let n_c = 7;

        // Row block 0, check i should connect to variables {0*7+i, 1*7+i, ..., 6*7+i}
        for i in 0..n_c {
            let vars = code.check_variables(i);
            let expected: Vec<usize> = (0..n_c).map(|j| j * n_c + i).collect();
            assert_eq!(vars, &expected, "Row block 0, check {} mismatch", i);
        }
    }

    #[test]
    fn test_row_block_1_has_circulant_shifts() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let n_c = 7;

        // Row block 1, check i connects to variable (j*n_c + (i+j)%n_c) for j=0..n_c-1
        for i in 0..n_c {
            let check = n_c + i; // offset by row block 0
            let vars = code.check_variables(check);
            let expected: Vec<usize> = (0..n_c).map(|j| j * n_c + (i + j) % n_c).collect();
            assert_eq!(vars, &expected, "Row block 1, check {} mismatch", i);
        }
    }

    // --- Simulation framework compatibility ---

    #[test]
    fn test_block_encoder_trait() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        // Verify BlockEncoder trait methods
        assert_eq!(BlockEncoder::k(&code), code.code_k());
        assert_eq!(BlockEncoder::n(&code), code.code_n());

        let msg = BitVec::zeros(code.code_k());
        let cw = BlockEncoder::encode(&code, &msg);
        assert_eq!(cw.len(), code.code_n());
    }

    #[test]
    fn test_soft_decoder_trait() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let decoder = GldpcDecoder::new(code.clone());

        assert_eq!(SoftDecoder::k(&decoder), code.code_k());
        assert_eq!(SoftDecoder::n(&decoder), code.code_n());
    }

    // --- Proptest for mathematical invariants ---

    // --- Extended BCH component tests ---

    #[test]
    fn test_extended_bch_component_parameters() {
        let comp = extended_bch_component(31, 26, 1);
        assert_eq!(comp.n(), 32);
        assert_eq!(comp.k(), 26);
        assert_eq!(comp.num_checks(), 6); // 32 - 26
    }

    #[test]
    fn test_extended_bch_zero_is_valid() {
        let comp = extended_bch_component(31, 26, 1);
        let zero = BitVec::zeros(32);
        assert!(comp.is_valid_codeword(&zero));
    }

    #[test]
    fn test_extended_bch_overall_parity() {
        let comp = extended_bch_component(31, 26, 1);
        // The last H row is all-ones (overall even parity).
        // A word with odd weight should fail the overall parity check.
        let mut odd_weight = BitVec::zeros(32);
        odd_weight.set(0, true); // weight 1 = odd
        assert!(!comp.is_valid_codeword(&odd_weight));
    }

    // --- Lentmaier 1024 construction test ---

    #[test]
    fn test_lentmaier_1024_parameters() {
        let code = QcGldpcCode::lentmaier_1024();
        assert_eq!(code.code_n(), 1024);
        // eBCH(32,26): n_c=32, k_c=26, r_c=6
        // Full-rank formula: k = 1024 - 2*32*6 = 640.
        // The Lentmaier QC circulant construction introduces rank deficiency
        // of 6: the 2*32*6 = 384 check rows have only 378 independent rows,
        // giving k = 1024 - 378 = 646. This is inherent to the QC structure
        // (the circulant block rows in row block 0 sum to zero modulo each
        // component check, creating 6 linear dependencies).
        assert_eq!(
            code.code_k(),
            646,
            "Dimension must be 646 (full-rank 640 + 6 rank-deficient)"
        );
        assert_eq!(code.num_check_nodes(), 64); // 2 * 32
        assert_eq!(code.component().n(), 32);
        assert_eq!(code.component().k(), 26);
    }

    #[test]
    fn test_lentmaier_1024_encode_zero() {
        let code = QcGldpcCode::lentmaier_1024();
        let msg = BitVec::zeros(code.code_k());
        let cw = code.encode(&msg);
        assert_eq!(cw.len(), 1024);
        assert_eq!(cw.count_ones(), 0);
        assert!(code.is_valid_codeword(&cw));
    }

    #[test]
    fn test_lentmaier_1024_encode_valid() {
        let code = QcGldpcCode::lentmaier_1024();
        // Encode a non-zero message
        let mut msg = BitVec::zeros(code.code_k());
        msg.set(0, true);
        msg.set(1, true);
        let cw = code.encode(&msg);
        assert_eq!(cw.len(), 1024);
        assert!(code.is_valid_codeword(&cw));
    }

    #[test]
    fn test_lentmaier_1024_decode_clean() {
        let code = QcGldpcCode::lentmaier_1024();
        let mut decoder = GldpcDecoder::new(code.clone());

        let msg = BitVec::zeros(code.code_k());
        let cw = code.encode(&msg);

        let llrs: Vec<Llr> = (0..1024)
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-8.0)
                } else {
                    Llr::new(8.0)
                }
            })
            .collect();

        let result = decoder.decode_iterative(&llrs, 10);
        assert!(result.converged, "Should converge with clean LLRs");
    }

    // --- Roundtrip tests: encode -> noiseless decode -> extract == original ---

    #[test]
    fn test_roundtrip_encode_decode_small() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let mut decoder = GldpcDecoder::new(code.clone());
        let k = code.code_k();

        // Test multiple non-zero message patterns
        let num_patterns = 1u32 << k.min(7);
        for pattern in 1u32..num_patterns {
            let mut msg = BitVec::with_capacity(k);
            for bit in 0..k {
                msg.push_bit(if bit < 32 {
                    (pattern >> bit) & 1 == 1
                } else {
                    false
                });
            }

            let cw = code.encode(&msg);

            // Create noiseless LLRs from codeword
            let llrs: Vec<Llr> = (0..code.code_n())
                .map(|i| {
                    if cw.get(i) {
                        Llr::new(-10.0)
                    } else {
                        Llr::new(10.0)
                    }
                })
                .collect();

            decoder.reset();
            let result = decoder.decode_iterative(&llrs, 20);
            assert!(
                result.converged,
                "Pattern {}: should converge with noiseless LLRs",
                pattern
            );
            assert_eq!(
                result.decoded_bits, msg,
                "Pattern {}: decode(encode(msg)) must equal msg",
                pattern
            );
        }
    }

    #[test]
    fn test_roundtrip_encode_decode_1024() {
        let code = QcGldpcCode::lentmaier_1024();
        let mut decoder = GldpcDecoder::new(code.clone());
        let k = code.code_k();

        // Test a few non-zero random-ish patterns
        for seed in &[1u64, 42, 0xDEADBEEF, 0x12345678] {
            let mut msg = BitVec::with_capacity(k);
            // Simple PRNG based on seed to create a deterministic pattern
            let mut state = *seed;
            for _ in 0..k {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                msg.push_bit((state >> 33) & 1 == 1);
            }

            let cw = code.encode(&msg);
            assert!(
                code.is_valid_codeword(&cw),
                "Seed {}: encoded codeword must be valid",
                seed
            );

            // Noiseless LLRs
            let llrs: Vec<Llr> = (0..code.code_n())
                .map(|i| {
                    if cw.get(i) {
                        Llr::new(-10.0)
                    } else {
                        Llr::new(10.0)
                    }
                })
                .collect();

            decoder.reset();
            let result = decoder.decode_iterative(&llrs, 20);
            assert!(
                result.converged,
                "Seed {}: should converge with noiseless LLRs",
                seed
            );
            assert_eq!(
                result.decoded_bits, msg,
                "Seed {}: decode(encode(msg)) must equal msg",
                seed
            );
        }
    }

    #[test]
    fn test_systematic_positions_used_consistently() {
        let code = QcGldpcCode::lentmaier(7, 4, 1);
        let k = code.code_k();
        let sys_pos = code.systematic_positions();
        assert_eq!(sys_pos.len(), k);

        // Encode a message and verify message bits appear at systematic positions
        let mut msg = BitVec::with_capacity(k);
        for i in 0..k {
            msg.push_bit(i % 3 == 0);
        }
        let cw = code.encode(&msg);

        // The systematic encoding G has identity columns at systematic_positions,
        // so cw[sys_pos[i]] == msg[i] for each i
        for (i, &col) in sys_pos.iter().enumerate() {
            assert_eq!(
                cw.get(col),
                msg.get(i),
                "Systematic position {}: cw[{}] should equal msg[{}]",
                i,
                col,
                i
            );
        }
    }

    /// Diagnostic: measure SOGRAND cumulative probability and extrinsic gain
    /// for the eBCH(32,26) component code used by GLDPC (1024,646).
    ///
    /// Tests with the ACTUAL default config (list_size=1, even_code=false)
    /// used by GldpcDecoder::new(), and with various post_list_budget sizes.
    #[test]
    fn test_sogrand_ebch32_cumulative_probability_diagnostic() {
        use crate::grand::{OrbGrand, OrbGrandConfig, SoGrand};

        // Build the eBCH(32,26) component used by GLDPC(1024,646)
        let comp = extended_bch_component(31, 26, 1);
        assert_eq!(comp.n(), 32);
        assert_eq!(comp.k(), 26);
        let h = comp.h_matrix();

        // Test at multiple LLR magnitudes:
        for (label, llr_mag) in [
            ("strong (3dB channel)", 4.0_f32),
            ("moderate (BP early)", 1.5),
            ("weak (BP struggling)", 0.5),
        ] {
            eprintln!(
                "\n=== eBCH(32,26) SOGRAND Diagnostic: {} (|LLR|={:.1}) ===",
                label, llr_mag
            );

            let llrs: Vec<Llr> = vec![Llr::new(llr_mag); 32];

            // --- Test with different max_queries to see scaling ---
            for max_q in [1_000_usize, 65_536, 1_000_000] {
                let config = OrbGrandConfig {
                    max_queries: max_q,
                    list_size: 4,
                    even_code: true,
                    systematic: false,
                };
                let orb = OrbGrand::new(h.clone(), config.clone());
                let sogrand = SoGrand::new(orb);
                let result = sogrand.decode_siso(&llrs);

                let orb2 = OrbGrand::new(h.clone(), config);
                let orb_result = orb2.decode(&llrs);

                let cum_prob = orb_result.cumulative_probability();
                let avg_ext = result
                    .extrinsic_llrs
                    .iter()
                    .map(|l| l.value().abs() as f64)
                    .sum::<f64>()
                    / 32.0;

                eprintln!("  max_queries={:>9}: cum_prob={:.6e}, P(C\\L)={:.6e}, avg_ext={:.4}, queries={}",
                    max_q, cum_prob, result.list_bler_prediction, avg_ext, orb_result.query_count);
            }
        }
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_linearity_sum_of_codewords(a_pat in 0u32..128, b_pat in 0u32..128) {
                let code = QcGldpcCode::lentmaier(7, 4, 1);
                let k = code.code_k();

                // Create two messages
                let mut msg_a = BitVec::with_capacity(k);
                let mut msg_b = BitVec::with_capacity(k);
                for bit in 0..k {
                    msg_a.push_bit((a_pat >> bit) & 1 == 1);
                    msg_b.push_bit((b_pat >> bit) & 1 == 1);
                }

                let cw_a = code.encode(&msg_a);
                let cw_b = code.encode(&msg_b);

                // XOR of two codewords should also be a valid codeword (linearity)
                let mut cw_sum = cw_a.clone();
                cw_sum.bit_xor_into(&cw_b);

                prop_assert!(
                    code.is_valid_codeword(&cw_sum),
                    "XOR of two codewords must be a valid codeword"
                );
            }

            #[test]
            fn test_encoding_preserves_message_bits_structure(pattern in 0u32..128) {
                let code = QcGldpcCode::lentmaier(7, 4, 1);
                let k = code.code_k();

                let mut msg = BitVec::with_capacity(k);
                for bit in 0..k {
                    msg.push_bit((pattern >> bit) & 1 == 1);
                }

                let cw = code.encode(&msg);
                // Codeword length must be n
                prop_assert_eq!(cw.len(), code.code_n());
                // Must be a valid codeword
                prop_assert!(code.is_valid_codeword(&cw));
            }
        }
    }
}

#[cfg(test)]
mod simulation_integration {
    use super::*;
    use crate::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};

    /// Verify GLDPC code works with the simulation framework.
    #[test]
    fn test_gldpc_with_simulation_runner() {
        // Use the small BCH(7,4) component for speed
        let component = BchComponentCode::new(7, 4, 1);
        let code = QcGldpcCode::from_component(component.clone());
        let mut decoder = GldpcDecoder::new(code.clone());

        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![8.0];
        config.max_frames = 5;
        config.min_errors = 1;

        let results = SimulationRunner::run_coded_iterative(&code, &mut decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
    }
}
