//! DVB-T2 sparse matrix builder from table format.
//!
//! Converts DVB-T2 standard tables to sparse parity-check matrix edge lists.

use super::params::DvbParams;

/// Validate DVB-T2 table against parameters.
///
/// # Panics
///
/// Panics if:
/// - Table row count != num_info_blocks
/// - Any parity index >= m (out of range)
/// - Table is placeholder (single row with single element)
fn validate_table(table: &[&[usize]], params: &DvbParams) {
    // Check for placeholder first (provides better error message)
    assert!(
        table.len() > 1 || table[0].len() > 1,
        "DVB-T2 table not yet implemented (placeholder detected)"
    );

    assert_eq!(
        table.len(),
        params.num_info_blocks,
        "Table must have {} rows (info blocks), found {}",
        params.num_info_blocks,
        table.len()
    );

    // Validate all parity indices in range
    for (row_idx, row) in table.iter().enumerate() {
        for &parity_idx in row.iter() {
            assert!(
                parity_idx < params.m,
                "Invalid parity index {} in table row {} (must be < {})",
                parity_idx,
                row_idx,
                params.m
            );
        }
    }
}

/// Build sparse parity-check matrix edges from DVB-T2 table format.
///
/// # Algorithm
///
/// 1. Information bit connections (from table):
///    For each info block i (table row i):
///    - For each base parity index p in table[i]:
///      - For each bit position j in block (0..Z-1):
///        - info_bit = i * Z + j
///        - parity_bit = (p + j * q) mod m
///        - Add edge: (parity_bit, info_bit)
///
/// 2. Dual-diagonal parity structure:
///    For each parity bit p in [0, m):
///    - Add edge (p, k + p)              // Diagonal
///    - Add edge (p-1, k + p) if p > 0   // Sub-diagonal (NO wrap at p=0)
///
/// # Arguments
///
/// * `table` - DVB table with base parity indices per info block
/// * `params` - Code parameters (n, k, m, q, Z)
///
/// # Returns
///
/// Edge list for sparse matrix: Vec<(check_idx, var_idx)>
/// Note: May contain duplicate edges; sparse matrix constructor handles this.
///
/// # Panics
///
/// Panics if table validation fails (see validate_table).
pub fn build_dvb_edges(table: &[&[usize]], params: &DvbParams) -> Vec<(usize, usize)> {
    validate_table(table, params);

    let mut edges = Vec::new();
    let z = params.expansion_factor;
    let q = params.step_size;
    let m = params.m;
    let k = params.k;

    // 1. Information bit connections from table
    for (block_idx, base_indices) in table.iter().enumerate() {
        for &base_parity in base_indices.iter() {
            for j in 0..z {
                let info_bit = block_idx * z + j;
                let parity_bit = (base_parity + j * q) % m;
                edges.push((parity_bit, info_bit));
            }
        }
    }

    // 2. Dual-diagonal parity structure (DVB-T2 standard)
    //
    // DVB-T2 dual-diagonal B matrix (parity-on-parity):
    //   - Row 0: SINGLE 1 at column k+0 (diagonal only, NO sub-diagonal)
    //   - Row p (p>0): TWO 1s at columns k+p (diagonal) and k+(p-1) (sub-diagonal)
    //
    // In edge representation (check, variable):
    //   - All rows p: diagonal edge (p, k+p)
    //   - Rows p>0: sub-diagonal edge (p, k+p-1)
    //
    // This creates a staircase pattern where row p connects to columns k+p and k+p-1,
    // allowing iterative solution: p[0] = s[0], p[i] = s[i] ⊕ p[i-1] for i>0
    for p in 0..m {
        // Diagonal: check p connected to variable k+p
        edges.push((p, k + p));

        // Sub-diagonal: check p connected to variable k+p-1 (ONLY for p > 0)
        // Row 0 has NO sub-diagonal connection (single 1 only)
        if p > 0 {
            edges.push((p, k + p - 1));
        }
    }

    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::CodeRate;
    use crate::ldpc::dvb_t2::params::FrameSize;

    #[test]
    #[should_panic(expected = "must have")]
    fn test_validate_wrong_row_count() {
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
        let table: &[&[usize]] = &[&[0, 100]]; // Only 1 row, should be 90
        validate_table(table, &params);
    }

    #[test]
    #[should_panic(expected = "placeholder")]
    fn test_validate_placeholder() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
        let table: &[&[usize]] = &[&[0]]; // Placeholder
        validate_table(table, &params);
    }

    #[test]
    #[should_panic(expected = "must be <")]
    fn test_validate_out_of_range_index() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
        // Create table with 20 rows but invalid parity index
        const BAD_ROW: &[usize] = &[10000]; // Way out of range (m = 9000)
        let table: &[&[usize]] = &[
            BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW,
            BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW,
            BAD_ROW, BAD_ROW,
        ];
        validate_table(table, &params);
    }

    #[test]
    fn test_normal_rate_1_2_table() {
        use super::super::dvb_t2_matrices::NORMAL_RATE_1_2_TABLE;

        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
        let edges = build_dvb_edges(NORMAL_RATE_1_2_TABLE, &params);

        // Verify we got edges
        assert!(!edges.is_empty());

        // Check all edges are in valid range
        for (check, var) in &edges {
            assert!(*check < params.m, "Check index {} out of range", check);
            assert!(*var < params.n, "Variable index {} out of range", var);
        }
    }
}
