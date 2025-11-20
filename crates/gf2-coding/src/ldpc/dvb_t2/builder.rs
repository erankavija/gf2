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
///      For each base parity index p in table[i]:
///        For each bit position j in block (0..Z-1):
///          info_bit = i * Z + j
///          parity_bit = (p + j * q) mod m
///          Add edge: (parity_bit, info_bit)
///
/// 2. Dual-diagonal parity structure:
///    For each parity bit p in [0, m):
///      Add edge (p, k + p)              // Diagonal
///      Add edge ((p-1) mod m, k + p)    // Sub-diagonal (wraps at p=0)
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
    
    // 2. Dual-diagonal parity structure
    for p in 0..m {
        // Diagonal: check p connected to variable k+p
        edges.push((p, k + p));
        
        // Sub-diagonal: check (p-1) mod m connected to variable k+p
        let prev_check = if p == 0 { m - 1 } else { p - 1 };
        edges.push((prev_check, k + p));
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
            BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW,
            BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW,
            BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW,
            BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW, BAD_ROW,
        ];
        validate_table(table, &params);
    }

    #[test]
    fn test_small_example() {
        // Hand-crafted small example: Z=3, 2 info blocks, simple structure
        // Info block 0: base parity indices [0, 2]
        // Info block 1: base parity indices [1]
        // Params: n=9, k=6, m=3, q=1, Z=3
        let table: &[&[usize]] = &[
            &[0, 2], // Block 0: bits 0,1,2
            &[1],    // Block 1: bits 3,4,5
        ];
        
        let params = DvbParams {
            n: 9,
            k: 6,
            m: 3,
            step_size: 1,
            expansion_factor: 3,
            num_info_blocks: 2,
        };
        
        let edges = build_dvb_edges(table, &params);
        
        // Expected edges from info bits:
        // Block 0, base 0: (0+0*1)%3=0 -> (0,0), (0+1*1)%3=1 -> (1,1), (0+2*1)%3=2 -> (2,2)
        // Block 0, base 2: (2+0*1)%3=2 -> (2,0), (2+1*1)%3=0 -> (0,1), (2+2*1)%3=1 -> (1,2)
        // Block 1, base 1: (1+0*1)%3=1 -> (1,3), (1+1*1)%3=2 -> (2,4), (1+2*1)%3=0 -> (0,5)
        //
        // Dual-diagonal (k=6, m=3):
        // p=0: (0,6), (2,6)  [prev = (0-1)%3 = 2]
        // p=1: (1,7), (0,7)
        // p=2: (2,8), (1,8)
        
        // Total: 9 info edges + 6 parity edges = 15 edges
        assert_eq!(edges.len(), 15);
        
        // Check some specific info edges
        assert!(edges.contains(&(0, 0)));
        assert!(edges.contains(&(1, 1)));
        assert!(edges.contains(&(2, 2)));
        assert!(edges.contains(&(1, 3)));
        
        // Check parity edges
        assert!(edges.contains(&(0, 6))); // Diagonal
        assert!(edges.contains(&(2, 6))); // Sub-diagonal wrap
    }

    #[test]
    fn test_normal_rate_1_2_table() {
        use super::super::dvb_t2_matrices::NORMAL_RATE_1_2_TABLE;
        
        let params = DvbParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
        let edges = build_dvb_edges(NORMAL_RATE_1_2_TABLE, &params);
        
        // Verify we got edges
        assert!(edges.len() > 0);
        
        // Check all edges are in valid range
        for (check, var) in &edges {
            assert!(*check < params.m, "Check index {} out of range", check);
            assert!(*var < params.n, "Variable index {} out of range", var);
        }
        
        // Expected edge count:
        // Info edges: sum of (row_length * Z) for all rows
        // Parity edges: 2 * m
        let info_edge_count: usize = NORMAL_RATE_1_2_TABLE.iter()
            .map(|row| row.len() * params.expansion_factor)
            .sum();
        let parity_edge_count = 2 * params.m;
        let expected_total = info_edge_count + parity_edge_count;
        
        assert_eq!(edges.len(), expected_total);
    }

    #[test]
    fn test_dual_diagonal_structure() {
        // Test just the dual-diagonal structure
        let params = DvbParams {
            n: 8,
            k: 5,
            m: 3,
            step_size: 1,
            expansion_factor: 1,
            num_info_blocks: 5,
        };
        
        let table: &[&[usize]] = &[&[0], &[0], &[0], &[0], &[0]]; // Minimal info
        let edges = build_dvb_edges(table, &params);
        
        // Check parity edges
        assert!(edges.contains(&(0, 5))); // Diagonal p=0
        assert!(edges.contains(&(2, 5))); // Sub-diagonal p=0, prev=(0-1)%3=2
        assert!(edges.contains(&(1, 6))); // Diagonal p=1
        assert!(edges.contains(&(0, 6))); // Sub-diagonal p=1
        assert!(edges.contains(&(2, 7))); // Diagonal p=2
        assert!(edges.contains(&(1, 7))); // Sub-diagonal p=2
    }
}
