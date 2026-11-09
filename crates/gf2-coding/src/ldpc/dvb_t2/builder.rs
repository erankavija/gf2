//! Sparse parity-check matrix edge lists from the `@/citation/Etsi2015` tables.

use super::params::DvbParams;
use crate::ldpc::{QcCirculant, QuasiCyclicLdpc};

/// Validate DVB-T2 table against parameters.
///
/// # Panics
///
/// Panics if:
/// - Table row count != num_info_blocks
/// - Any parity index >= m (out of range)
/// - Table is a single row with a single element
fn validate_table(table: &[&[usize]], params: &DvbParams) {
    // Checked first for the more specific message.
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
///    - Add edge (p, k + p - 1) if p > 0 // Sub-diagonal (NO wrap at p=0)
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

    for (block_idx, base_indices) in table.iter().enumerate() {
        for &base_parity in base_indices.iter() {
            for j in 0..z {
                let info_bit = block_idx * z + j;
                let parity_bit = (base_parity + j * q) % m;
                edges.push((parity_bit, info_bit));
            }
        }
    }

    for p in 0..m {
        edges.push((p, k + p));

        // Row 0 has no sub-diagonal entry.
        if p > 0 {
            edges.push((p, k + p - 1));
        }
    }

    edges
}

/// The block description of the code of [`build_dvb_edges`] with
/// Z = `expansion_factor`.
///
/// With q = `step_size`, lifted row `360·s + t` of row block s is check
/// `q·t + s`, information column `360·i + j` is variable `360·i + j`, and parity
/// column `k + 360·s + t` is variable `k + q·t + s`, the parity-interleaver
/// order of `@/citation/Etsi2015` §6.1.3. Table entry x of row i is a circulant
/// at (x mod q, i) of shift `(Z - ⌊x/q⌋) mod Z`; entries of one row with equal
/// residues share a block. Parity block s has the identity at (s, s) and at
/// (s + 1, s) for s < q - 1, and block q - 1 the shift Z - 1 at (0, q - 1)
/// with row 0 masked, since check 0 has no sub-diagonal entry.
///
/// # Panics
///
/// Panics if table validation fails (see `validate_table`).
pub fn build_dvb_quasi_cyclic(table: &[&[usize]], params: &DvbParams) -> QuasiCyclicLdpc {
    validate_table(table, params);
    let z = params.expansion_factor;
    let q = params.step_size;
    let info_blocks = params.num_info_blocks;
    let mut circulants = Vec::new();
    for (block, row) in table.iter().enumerate() {
        for &address in row.iter() {
            circulants.push(QcCirculant::new(address % q, block, (z - address / q) % z));
        }
    }
    for s in 0..q {
        circulants.push(QcCirculant::new(s, info_blocks + s, 0));
        if s + 1 < q {
            circulants.push(QcCirculant::new(s + 1, info_blocks + s, 0));
        }
    }
    circulants.push(QcCirculant::new(0, info_blocks + q - 1, z - 1).with_masked_rows(vec![0]));

    let row_order = (0..params.m).map(|r| q * (r % z) + r / z).collect();
    let col_order = (0..params.n)
        .map(|c| {
            if c < params.k {
                c
            } else {
                let p = c - params.k;
                params.k + q * (p % z) + p / z
            }
        })
        .collect();
    QuasiCyclicLdpc::from_circulants(q, info_blocks + q, z, circulants)
        .with_orders(row_order, col_order)
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
        let table: &[&[usize]] = &[&[0]];
        validate_table(table, &params);
    }

    #[test]
    #[should_panic(expected = "must be <")]
    fn test_validate_out_of_range_index() {
        let params = DvbParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
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

        assert!(!edges.is_empty());

        for (check, var) in &edges {
            assert!(*check < params.m, "Check index {} out of range", check);
            assert!(*var < params.n, "Variable index {} out of range", var);
        }
    }
}
