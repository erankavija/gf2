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

use crate::llr::Llr;
use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;

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

    /// Runs a min-sum SISO decoder on the component code.
    ///
    /// Performs iterative belief propagation on the component code's parity-check
    /// matrix using the min-sum algorithm. Returns extrinsic LLR information for
    /// each bit position.
    ///
    /// # Arguments
    ///
    /// * `input_llrs` - Channel/incoming LLRs of length `n_c`
    /// * `max_iters` - Maximum BP iterations within the component decoder
    ///
    /// # Returns
    ///
    /// A vector of extrinsic LLRs (output minus input) of length `n_c`.
    ///
    /// # Panics
    ///
    /// Panics if `input_llrs.len() != n_c`.
    ///
    /// # Complexity
    ///
    /// O(max_iters * n_c * num_checks) per call.
    pub fn decode_siso(&self, input_llrs: &[Llr], max_iters: usize) -> Vec<Llr> {
        assert_eq!(
            input_llrs.len(),
            self.n_c,
            "Input LLR length must equal n_c = {}",
            self.n_c
        );

        let n = self.n_c;

        // check_to_var[check][position_in_check_row] messages
        let mut check_to_var: Vec<Vec<Llr>> = self
            .h_rows
            .iter()
            .map(|row| vec![Llr::zero(); row.len()])
            .collect();

        // var_to_check: for each variable, for each check it participates in
        // We need a mapping: var -> list of (check_idx, position_in_check_row)
        let mut var_checks: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (check_idx, row) in self.h_rows.iter().enumerate() {
            for (pos, &col) in row.iter().enumerate() {
                var_checks[col].push((check_idx, pos));
            }
        }

        let mut var_to_check: Vec<Vec<Llr>> = var_checks
            .iter()
            .map(|checks| vec![Llr::zero(); checks.len()])
            .collect();

        // Initialize var-to-check with channel LLRs
        for (var, checks) in var_checks.iter().enumerate() {
            for (idx, &(_check_idx, _pos)) in checks.iter().enumerate() {
                var_to_check[var][idx] = input_llrs[var];
            }
        }

        // Posterior beliefs
        let mut beliefs: Vec<Llr> = input_llrs.to_vec();

        for _iter in 0..max_iters {
            // Check-to-variable update (min-sum)
            for (check_idx, row) in self.h_rows.iter().enumerate() {
                for (pos, &_col) in row.iter().enumerate() {
                    // Collect all other var-to-check messages for this check
                    let mut sign_product: f32 = 1.0;
                    let mut min_mag: f32 = f32::INFINITY;

                    for (other_pos, &other_col) in row.iter().enumerate() {
                        if other_pos == pos {
                            continue;
                        }
                        // Find var_to_check message for other_col -> check_idx
                        let vc_idx = var_checks[other_col]
                            .iter()
                            .position(|&(ci, _)| ci == check_idx)
                            .unwrap();
                        let msg = var_to_check[other_col][vc_idx];
                        let val = msg.value();
                        if val < 0.0 {
                            sign_product *= -1.0;
                        }
                        min_mag = min_mag.min(val.abs());
                    }

                    check_to_var[check_idx][pos] = Llr::new(sign_product * min_mag);
                }
            }

            // Variable-to-check update
            for (var, checks) in var_checks.iter().enumerate() {
                // Total belief = channel + sum of all check-to-var
                let mut total = input_llrs[var].value();
                for &(check_idx, pos) in checks {
                    total += check_to_var[check_idx][pos].value();
                }
                beliefs[var] = Llr::new(total);

                // var-to-check = total - incoming from that check
                for (idx, &(check_idx, pos)) in checks.iter().enumerate() {
                    let incoming = check_to_var[check_idx][pos].value();
                    var_to_check[var][idx] = Llr::new(total - incoming);
                }
            }

            // Early termination: check syndrome
            let hard: BitVec = beliefs
                .iter()
                .map(|l| l.hard_decision())
                .collect::<Vec<_>>()
                .into_iter()
                .fold(BitVec::with_capacity(n), |mut bv, bit| {
                    bv.push_bit(bit);
                    bv
                });
            if self.is_valid_codeword(&hard) {
                break;
            }
        }

        // Return extrinsic information: posterior - prior
        beliefs
            .iter()
            .zip(input_llrs.iter())
            .map(|(post, prior)| Llr::new(post.value() - prior.value()))
            .collect()
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

    /// Creates a (1024, k) QC-GLDPC code using eBCH(32, 26) as the component code.
    ///
    /// This is the target construction from Lentmaier (2010):
    /// - Component: extended BCH(32, 26) derived from BCH(31, 26, 1)
    /// - Code length: 32^2 = 1024
    /// - Dimension: computed from rank(H)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::gldpc::QcGldpcCode;
    ///
    /// let code = QcGldpcCode::lentmaier_1024();
    /// assert_eq!(code.code_n(), 1024);
    /// assert!(code.code_k() > 0);
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

        // Compute the actual code dimension from the rank of the parity-check matrix.
        // The formula k = n - 2*n_c*(n_c - k_c) assumes full rank, but the QC
        // structure introduces linear dependencies among check rows. We compute
        // the true rank via RREF.
        let k = {
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
            n - rref_result.rank
        };

        Self {
            component,
            n,
            k,
            num_check_nodes,
            check_node_vars,
            var_check_map,
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
    /// # Complexity
    ///
    /// O(m^2 * n) for RREF where m = number of parity checks, n = code length.
    fn compute_generator_matrix(&self) -> gf2_core::BitMatrix {
        use gf2_core::alg::rref::rref;
        use gf2_core::BitMatrix;

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

        // Non-pivot columns are systematic (information) positions
        let systematic_positions: Vec<usize> = (0..n).filter(|c| !pivot_cols.contains(c)).collect();

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
#[derive(Debug)]
pub struct GldpcDecoder {
    /// The GLDPC code being decoded.
    code: QcGldpcCode,
    /// Current posterior beliefs for each variable node.
    beliefs: Vec<Llr>,
    /// Check-to-variable messages: check_to_var[check][position] -> Llr.
    check_to_var: Vec<Vec<Llr>>,
    /// Variable-to-check messages: var_to_check[var][idx_in_var_check_map] -> Llr.
    var_to_check: Vec<Vec<Llr>>,
    /// Maximum BP iterations within the component SISO decoder.
    component_max_iters: usize,
    /// Number of iterations in the last decode call.
    last_iterations: usize,
}

impl GldpcDecoder {
    /// Creates a new GLDPC decoder for the given code.
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
        let n = code.code_n();
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

        Self {
            code,
            beliefs: vec![Llr::zero(); n],
            check_to_var,
            var_to_check,
            component_max_iters: 5,
            last_iterations: 0,
        }
    }

    /// Creates a new GLDPC decoder with a custom number of component SISO iterations.
    ///
    /// # Arguments
    ///
    /// * `code` - The QC-GLDPC code to decode
    /// * `component_max_iters` - Maximum iterations for the component SISO decoder
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::gldpc::{QcGldpcCode, GldpcDecoder};
    ///
    /// let code = QcGldpcCode::lentmaier(7, 4, 1);
    /// let decoder = GldpcDecoder::with_component_iters(code, 10);
    /// ```
    pub fn with_component_iters(code: QcGldpcCode, component_max_iters: usize) -> Self {
        let mut decoder = Self::new(code);
        decoder.component_max_iters = component_max_iters;
        decoder
    }

    /// Performs check node update using SISO component decoding.
    ///
    /// For each check node, collects variable-to-check messages, runs the
    /// component SISO decoder, and stores the extrinsic output as
    /// check-to-variable messages.
    fn check_node_update(&mut self) {
        let n_c = self.code.component().n();
        let component_max_iters = self.component_max_iters;

        for (check_idx, check_vars) in self.code.check_node_vars.iter().enumerate() {
            // Collect input LLRs from variable-to-check messages
            let mut input_llrs = Vec::with_capacity(n_c);
            for &var_idx in check_vars.iter() {
                // Find which index in var_check_map[var_idx] corresponds to this check
                let vc_idx = self.code.var_check_map[var_idx]
                    .iter()
                    .position(|&(ci, _)| ci == check_idx)
                    .unwrap();
                input_llrs.push(self.var_to_check[var_idx][vc_idx]);
            }

            // Run component SISO decoder
            let extrinsic = self
                .code
                .component
                .decode_siso(&input_llrs, component_max_iters);

            // Store check-to-variable messages
            for (pos, ext) in extrinsic.into_iter().enumerate() {
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
        for &llr in llrs.iter().take(self.k()) {
            decoded.push_bit(llr.hard_decision());
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

        // Extract message bits (first k bits of decoded codeword)
        let k = self.code.code_k();
        let mut message = BitVec::with_capacity(k);
        for i in 0..k {
            message.push_bit(decoded_codeword.get(i));
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
    use crate::traits::BlockEncoder;

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
    fn test_component_siso_extrinsic_for_valid_codeword() {
        let comp = BchComponentCode::new(7, 4, 1);

        // Strong LLRs for all-zeros codeword (positive = bit 0 likely)
        let llrs: Vec<Llr> = vec![Llr::new(5.0); 7];
        let extrinsic = comp.decode_siso(&llrs, 5);

        // Extrinsic should be non-negative (reinforcing the decision)
        for ext in &extrinsic {
            assert!(
                ext.value() >= -0.01,
                "Extrinsic {} should be non-negative for valid all-zeros",
                ext.value()
            );
        }
    }

    #[test]
    #[should_panic(expected = "Input LLR length must equal n_c")]
    fn test_component_siso_wrong_length() {
        let comp = BchComponentCode::new(7, 4, 1);
        comp.decode_siso(&[Llr::new(1.0); 5], 5);
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
        assert!(code.code_k() > 0, "Dimension must be positive");
        assert!(
            code.code_k() < 1024,
            "Dimension must be less than code length"
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
