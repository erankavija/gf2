//! Canonical Tanner-graph edge indexing for LDPC message passing.
//!
//! [`EdgeLayout`] is the one precomputed edge indexing the LDPC decoders in
//! this crate use. It numbers every edge of a parity-check matrix once, in
//! check-major scan order, and records both directions of the map between a
//! check's view of an edge and the variable's view, so neither node update has
//! to search a neighbour list for the other side of an edge.
//!
//! # Orders
//!
//! Check `c` owns the canonical edge ids `check_range(c)`, in the parity-check
//! matrix's CSR [`row_iter`](gf2_core::SpBitMatrixDual::row_iter) order.
//! Variable `v` owns the variable-major slots `var_range(v)`, in the CSC
//! [`col_iter`](gf2_core::SpBitMatrixDual::col_iter) order.
//!
//! # Storage
//!
//! A consumer that keeps its messages in one flat check-major array indexes
//! them by canonical edge id: a check update walks a contiguous run, and a
//! variable update walks the variable's slots and follows
//! [`EdgeLayout::var_edge_to_check_edge`] into the same array. A consumer that
//! keeps a second, variable-major array follows
//! [`EdgeLayout::check_edge_to_var_edge`] the other way; the GPU decode stage
//! in `gf2-sim` uploads exactly these arrays.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::ldpc::{EdgeLayout, LdpcCode};
//!
//! let code = LdpcCode::from_edges(2, 3, &[(0, 0), (0, 1), (1, 1), (1, 2)]);
//! let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
//!
//! assert_eq!(layout.edges(), 4);
//! assert_eq!(layout.max_check_degree(), 2);
//! // Check 0's edges are the canonical ids 0 and 1, at variables 0 and 1.
//! assert_eq!(layout.check_range(0), 0..2);
//! assert_eq!(layout.check_edge_var()[0], 0);
//! assert_eq!(layout.check_edge_var()[1], 1);
//! // Variable 1 sits on both checks, so its two slots resolve to one edge each.
//! let slots = layout.var_range(1);
//! let edges: Vec<u32> = slots.map(|slot| layout.var_edge_to_check_edge()[slot]).collect();
//! assert_eq!(edges, vec![1, 2]);
//! ```

use gf2_core::SpBitMatrixDual;

/// Precomputed edge indexing of a Tanner graph.
///
/// Built once per decoder from a parity-check matrix. Construction is linear
/// in the edge count and allocates; no decoding operation allocates or
/// searches through it.
///
/// See the module documentation for the edge orders and how a consumer
/// stores messages against them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeLayout {
    n: usize,
    m: usize,
    /// `m + 1` prefix sums: check `c` owns edges `check_offsets[c]..check_offsets[c + 1]`.
    check_offsets: Vec<u32>,
    /// The variable of each canonical (check-major) edge.
    check_edge_var: Vec<u32>,
    /// `n + 1` prefix sums: variable `v` owns slots `var_offsets[v]..var_offsets[v + 1]`.
    var_offsets: Vec<u32>,
    /// The canonical edge id of each variable-major slot.
    var_edge_to_check_edge: Vec<u32>,
    /// The variable-major slot of each canonical edge.
    check_edge_to_var_edge: Vec<u32>,
    max_check_degree: usize,
}

impl EdgeLayout {
    /// Builds the layout of a parity-check matrix.
    ///
    /// # Panics
    ///
    /// Panics if the matrix's CSR and CSC views disagree on the edge set, or if
    /// either view lists a node's edges out of ascending order. Both are
    /// invariants of [`SpBitMatrixDual`]; the checks make a violation a
    /// construction failure rather than a silently misindexed decoder.
    #[must_use]
    pub fn from_parity_check(h: &SpBitMatrixDual) -> Self {
        let m = h.rows();
        let n = h.cols();

        let mut check_offsets = Vec::with_capacity(m + 1);
        let mut check_edge_var: Vec<u32> = Vec::new();
        check_offsets.push(0u32);
        let mut max_check_degree = 0usize;
        for check in 0..m {
            let before = check_edge_var.len();
            for var in h.row_iter(check) {
                check_edge_var.push(var as u32);
            }
            max_check_degree = max_check_degree.max(check_edge_var.len() - before);
            check_offsets.push(check_edge_var.len() as u32);
        }
        let edges = check_edge_var.len();

        // The variable-major view, used to assign slots and to check that the
        // two views agree edge for edge.
        let mut var_offsets = Vec::with_capacity(n + 1);
        let mut var_edge_check: Vec<u32> = Vec::with_capacity(edges);
        var_offsets.push(0u32);
        for var in 0..n {
            for check in h.col_iter(var) {
                var_edge_check.push(check as u32);
            }
            var_offsets.push(var_edge_check.len() as u32);
        }
        assert_eq!(
            var_edge_check.len(),
            edges,
            "parity-check CSR and CSC views disagree on the edge count"
        );

        // Both views list a node's edges in ascending order, so the k-th
        // check-major occurrence of a variable is its k-th variable-major slot.
        let mut cursor: Vec<u32> = var_offsets[..n].to_vec();
        let mut var_edge_to_check_edge = vec![0u32; edges];
        let mut check_edge_to_var_edge = vec![0u32; edges];
        for check in 0..m {
            let range = check_offsets[check] as usize..check_offsets[check + 1] as usize;
            for edge in range {
                let var = check_edge_var[edge] as usize;
                let slot = cursor[var] as usize;
                cursor[var] += 1;
                assert_eq!(
                    var_edge_check[slot] as usize, check,
                    "parity-check CSR and CSC views disagree at variable {var}"
                );
                var_edge_to_check_edge[slot] = edge as u32;
                check_edge_to_var_edge[edge] = slot as u32;
            }
        }

        Self {
            n,
            m,
            check_offsets,
            check_edge_var,
            var_offsets,
            var_edge_to_check_edge,
            check_edge_to_var_edge,
            max_check_degree,
        }
    }

    /// The number of variable nodes.
    #[inline]
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }

    /// The number of check nodes.
    #[inline]
    #[must_use]
    pub fn m(&self) -> usize {
        self.m
    }

    /// The number of Tanner-graph edges.
    #[inline]
    #[must_use]
    pub fn edges(&self) -> usize {
        self.check_edge_var.len()
    }

    /// The largest check-node degree, the size a per-check scratch buffer needs.
    #[inline]
    #[must_use]
    pub fn max_check_degree(&self) -> usize {
        self.max_check_degree
    }

    /// The canonical edge ids of check `check`, a contiguous range.
    ///
    /// # Panics
    ///
    /// Panics if `check >= m`.
    #[inline]
    #[must_use]
    pub fn check_range(&self, check: usize) -> std::ops::Range<usize> {
        self.check_offsets[check] as usize..self.check_offsets[check + 1] as usize
    }

    /// The variable-major slots of variable `var`, a contiguous range.
    ///
    /// # Panics
    ///
    /// Panics if `var >= n`.
    #[inline]
    #[must_use]
    pub fn var_range(&self, var: usize) -> std::ops::Range<usize> {
        self.var_offsets[var] as usize..self.var_offsets[var + 1] as usize
    }

    /// The `m + 1` check-major prefix sums.
    #[inline]
    #[must_use]
    pub fn check_offsets(&self) -> &[u32] {
        &self.check_offsets
    }

    /// The variable of each canonical edge, in check-major order.
    #[inline]
    #[must_use]
    pub fn check_edge_var(&self) -> &[u32] {
        &self.check_edge_var
    }

    /// The `n + 1` variable-major prefix sums.
    #[inline]
    #[must_use]
    pub fn var_offsets(&self) -> &[u32] {
        &self.var_offsets
    }

    /// The canonical edge id of each variable-major slot.
    #[inline]
    #[must_use]
    pub fn var_edge_to_check_edge(&self) -> &[u32] {
        &self.var_edge_to_check_edge
    }

    /// The variable-major slot of each canonical edge.
    #[inline]
    #[must_use]
    pub fn check_edge_to_var_edge(&self) -> &[u32] {
        &self.check_edge_to_var_edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::LdpcCode;

    #[test]
    fn orders_match_the_matrix_views() {
        let code = LdpcCode::dvb_t2_short(crate::CodeRate::Rate1_2);
        let h = code.parity_check_matrix();
        let layout = EdgeLayout::from_parity_check(h);

        for check in 0..code.m() {
            let recorded: Vec<usize> = layout
                .check_range(check)
                .map(|edge| layout.check_edge_var()[edge] as usize)
                .collect();
            let direct: Vec<usize> = h.row_iter(check).collect();
            assert_eq!(recorded, direct, "check {check}");
        }
    }

    /// Both cross-maps invert each other over the whole edge set, and a
    /// variable's slots resolve to edges that name that variable.
    #[test]
    fn cross_maps_invert_each_other() {
        let code = LdpcCode::dvb_t2_short(crate::CodeRate::Rate1_2);
        let h = code.parity_check_matrix();
        let layout = EdgeLayout::from_parity_check(h);

        for edge in 0..layout.edges() {
            let slot = layout.check_edge_to_var_edge()[edge] as usize;
            assert_eq!(layout.var_edge_to_check_edge()[slot] as usize, edge);
        }
        for var in 0..code.n() {
            let checks: Vec<usize> = layout
                .var_range(var)
                .map(|slot| {
                    let edge = layout.var_edge_to_check_edge()[slot] as usize;
                    assert_eq!(layout.check_edge_var()[edge] as usize, var);
                    // Recover the check from the offsets by binary search.
                    layout
                        .check_offsets()
                        .partition_point(|&o| o as usize <= edge)
                        - 1
                })
                .collect();
            let direct: Vec<usize> = h.col_iter(var).collect();
            assert_eq!(checks, direct, "variable {var}");
        }
    }

    #[test]
    fn degrees_match_the_matrix() {
        let code = LdpcCode::from_edges(3, 4, &[(0, 0), (0, 1), (1, 1), (1, 2), (1, 3), (2, 3)]);
        let layout = EdgeLayout::from_parity_check(code.parity_check_matrix());
        assert_eq!(layout.edges(), 6);
        assert_eq!(layout.max_check_degree(), 3);
        assert_eq!(layout.check_range(1).len(), 3);
        assert_eq!(layout.var_range(1).len(), 2);
        assert_eq!(layout.n(), 4);
        assert_eq!(layout.m(), 3);
    }
}
