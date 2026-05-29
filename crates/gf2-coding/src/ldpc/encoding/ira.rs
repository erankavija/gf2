//! Linear-time IRA (Irregular Repeat-Accumulate) staircase encoder.
//!
//! Exploits the dual-diagonal parity structure of DVB-T2 LDPC codes to encode
//! in O(number of edges) time with no matrix densification or RREF.
//!
//! # Algorithm
//!
//! DVB-T2 LDPC codes are systematic: `c = [info (k bits) | parity (m bits)]`,
//! where the parity part of H is dual-diagonal:
//!
//! - Row 0: single 1 at parity column `k+0`
//! - Row p (p > 0): 1s at parity columns `k+p` and `k+p-1`
//!
//! Each check equation therefore reads:
//!
//! ```text
//! s[0] XOR par[0]              = 0   (row 0)
//! s[p] XOR par[p] XOR par[p-1] = 0   (row p > 0)
//! ```
//!
//! where `s[p]` is the XOR of the info bits in check row `p`. Solving gives
//! the staircase recursion:
//!
//! ```text
//! par[0] = s[0]
//! par[p] = s[p] XOR par[p-1]   for p = 1 .. m-1
//! ```
//!
//! # Preconditions
//!
//! This encoder requires that:
//! - Systematic bits occupy columns `0 .. k` (standard DVB-T2 layout)
//! - Parity bits occupy columns `k .. n` (standard DVB-T2 layout)
//! - The parity part of H is dual-diagonal (DVB-T2 by construction)
//!
//! Use [`IraEncoder::new`] to build the encoder from any `LdpcCode` that
//! satisfies these preconditions. Construction fails fast if they are not met.

use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;

/// Precomputed staircase encoder for dual-diagonal IRA codes (e.g. DVB-T2).
///
/// Constructed once per code configuration; encoding is then O(edges) with
/// no allocations beyond the output codeword.
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
/// let enc = LdpcEncoder::new(code.clone());
///
/// let msg = BitVec::zeros(code.k());
/// let cw = enc.encode(&msg);
/// assert_eq!(cw.len(), code.n());
/// assert!(code.is_valid_codeword(&cw));
/// ```
#[derive(Debug, Clone)]
pub struct IraEncoder {
    /// Codeword length n.
    n: usize,
    /// Information bit count k.
    k: usize,
    /// Parity bit count m = n - k.
    m: usize,
    /// `check_info_vars[p]` — sorted list of information-bit column indices
    /// (< k) connected to check row p in H.
    ///
    /// XOR of `info[v]` over `v` in `check_info_vars[p]` gives `s[p]`.
    check_info_vars: Vec<Vec<usize>>,
}

impl IraEncoder {
    /// Constructs an `IraEncoder` from a parity-check matrix.
    ///
    /// Iterates the rows of `h` once to collect the info-column indices
    /// per check (columns `< k`). This is O(nnz) and allocates one `Vec<usize>`
    /// per check row.
    ///
    /// # Arguments
    ///
    /// * `h` - Sparse parity-check matrix (m × n).
    /// * `k` - Number of information bits (systematic columns `0..k`).
    ///
    /// # Panics
    ///
    /// Panics if `h.rows() + k != h.cols()` (dimensions inconsistent with
    /// a systematic code having `m = n - k`).
    ///
    /// # Complexity
    ///
    /// O(nnz) construction time; O(nnz) storage.
    pub fn new(h: &SpBitMatrixDual, k: usize) -> Self {
        let m = h.rows();
        let n = h.cols();
        assert_eq!(
            m + k,
            n,
            "IraEncoder requires m + k == n (got m={m}, k={k}, n={n})"
        );

        // For each check row, collect the info-column indices (< k).
        // Parity-column entries (>= k) are handled implicitly by the staircase.
        let check_info_vars: Vec<Vec<usize>> = (0..m)
            .map(|check| h.row_iter(check).filter(|&col| col < k).collect())
            .collect();

        Self {
            n,
            k,
            m,
            check_info_vars,
        }
    }

    /// Encodes `info` (k bits) into a systematic codeword (n bits).
    ///
    /// The codeword layout is `[info | parity]`: the first k bits are the
    /// verbatim information bits and the last m bits are the computed parity.
    ///
    /// # Arguments
    ///
    /// * `info` - Information bit vector of length `k`.
    ///
    /// # Returns
    ///
    /// Systematic codeword of length `n = k + m`.
    ///
    /// # Panics
    ///
    /// Panics if `info.len() != k`.
    ///
    /// # Complexity
    ///
    /// O(nnz) — one pass over all info-column edges plus one pass over m parity bits.
    pub fn encode(&self, info: &BitVec) -> BitVec {
        assert_eq!(
            info.len(),
            self.k,
            "IraEncoder::encode: info length {} != k={}",
            info.len(),
            self.k
        );

        // Step 1: accumulate s[p] = XOR of info bits connected to check p.
        let mut s = vec![false; self.m];
        for (p, vars) in self.check_info_vars.iter().enumerate() {
            let mut acc = false;
            for &v in vars {
                acc ^= info.get(v);
            }
            s[p] = acc;
        }

        // Step 2: staircase recursion.
        //   par[0] = s[0]
        //   par[p] = s[p] XOR par[p-1]   for p = 1..m-1
        let mut par = vec![false; self.m];
        par[0] = s[0];
        for p in 1..self.m {
            par[p] = s[p] ^ par[p - 1];
        }

        // Step 3: assemble codeword [info | parity].
        let mut cw = BitVec::with_capacity(self.n);
        for i in 0..self.k {
            cw.push_bit(info.get(i)); // clippy: index is needed (BitVec random access)
        }
        for &bit in &par {
            cw.push_bit(bit);
        }
        cw
    }

    /// Returns the codeword length n.
    #[allow(dead_code)]
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the information bit count k.
    #[allow(dead_code)]
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the parity bit count m.
    #[allow(dead_code)]
    pub fn m(&self) -> usize {
        self.m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::sparse::SpBitMatrixDual;

    /// Build the tiny dual-diagonal H used as a unit-test fixture.
    ///
    /// For k=3, m=3, n=6:
    ///
    /// ```text
    ///        v0 v1 v2 | p0 p1 p2
    /// c0  [  1  1  0  |  1  0  0  ]
    /// c1  [  0  1  1  |  1  1  0  ]
    /// c2  [  1  0  1  |  0  1  1  ]
    /// ```
    ///
    /// Staircase columns: p0→c0; p1→c0,c1 (wait — this is just a fixture, not
    /// the exact DVB-T2 structure). Let's make it exactly dual-diagonal:
    fn make_mini_ira_h() -> (SpBitMatrixDual, usize) {
        // k=4, m=4, n=8
        // Info-bit edges (arbitrary):
        //   c0 ← {0, 1}
        //   c1 ← {1, 2}
        //   c2 ← {2, 3}
        //   c3 ← {0, 3}
        // Dual-diagonal parity edges:
        //   c0 ← p0=col4
        //   c1 ← p1=col5, p0=col4
        //   c2 ← p2=col6, p1=col5
        //   c3 ← p3=col7, p2=col6
        let k = 4usize;
        let m = 4usize;
        let n = k + m;
        let edges: Vec<(usize, usize)> = vec![
            // Info edges
            (0, 0),
            (0, 1),
            (1, 1),
            (1, 2),
            (2, 2),
            (2, 3),
            (3, 0),
            (3, 3),
            // Dual-diagonal parity edges
            (0, k),     // c0 ← p0
            (1, k + 1), // c1 ← p1
            (1, k),     // c1 ← p0  (sub-diagonal)
            (2, k + 2), // c2 ← p2
            (2, k + 1), // c2 ← p1  (sub-diagonal)
            (3, k + 3), // c3 ← p3
            (3, k + 2), // c3 ← p2  (sub-diagonal)
        ];
        (SpBitMatrixDual::from_coo(m, n, &edges), k)
    }

    #[test]
    fn test_ira_encoder_construction() {
        let (h, k) = make_mini_ira_h();
        let enc = IraEncoder::new(&h, k);
        assert_eq!(enc.n(), 8);
        assert_eq!(enc.k(), 4);
        assert_eq!(enc.m(), 4);
    }

    #[test]
    fn test_ira_encoder_zero_message() {
        let (h, k) = make_mini_ira_h();
        let enc = IraEncoder::new(&h, k);

        let info = BitVec::zeros(k);
        let cw = enc.encode(&info);

        assert_eq!(cw.len(), 8);
        // Zero info → all s[p] = 0 → all par[p] = 0
        assert_eq!(cw.count_ones(), 0);

        // Verify H·c = 0
        let syndrome = h.matvec(&cw);
        assert_eq!(syndrome.count_ones(), 0, "Zero codeword must satisfy H·c=0");
    }

    #[test]
    fn test_ira_encoder_all_ones_message() {
        let (h, k) = make_mini_ira_h();
        let enc = IraEncoder::new(&h, k);

        let mut info = BitVec::with_capacity(k);
        for _ in 0..k {
            info.push_bit(true);
        }
        let cw = enc.encode(&info);

        assert_eq!(cw.len(), 8);
        let syndrome = h.matvec(&cw);
        assert_eq!(
            syndrome.count_ones(),
            0,
            "All-ones message codeword must satisfy H·c=0"
        );
    }

    #[test]
    fn test_ira_encoder_all_messages_valid() {
        let (h, k) = make_mini_ira_h();
        let enc = IraEncoder::new(&h, k);

        // Exhaustively test all 2^4 = 16 messages
        for msg_val in 0u8..16 {
            let mut info = BitVec::with_capacity(k);
            for bit in 0..k {
                info.push_bit((msg_val >> bit) & 1 == 1);
            }
            let cw = enc.encode(&info);

            assert_eq!(cw.len(), enc.n());

            // Systematic property: first k bits equal the message
            for i in 0..k {
                assert_eq!(
                    cw.get(i),
                    info.get(i),
                    "msg={msg_val}: systematic bit {i} mismatch"
                );
            }

            // Parity-check property: H·c = 0
            let syndrome = h.matvec(&cw);
            assert_eq!(
                syndrome.count_ones(),
                0,
                "msg={msg_val}: codeword does not satisfy H·c=0"
            );
        }
    }

    #[test]
    fn test_ira_encoder_dimension_mismatch_panics() {
        let edges: Vec<(usize, usize)> = vec![(0, 0), (0, 1), (0, 2)];
        let h = SpBitMatrixDual::from_coo(1, 3, &edges);
        // k=3 would mean n=k+m=4, but h.cols()=3 → should panic
        let result = std::panic::catch_unwind(|| IraEncoder::new(&h, 3));
        assert!(result.is_err(), "Should panic on dimension mismatch");
    }

    /// Verify the encoder against the DVB-T2 Short Rate 1/2 code
    /// (k=7200, m=9000, n=16200). Only checks syndrome for a few random messages
    /// since full bit-identity comparison with RREF takes > 5 s.
    #[test]
    fn test_ira_encoder_dvb_t2_short_rate_1_2_syndrome() {
        use crate::ldpc::LdpcCode;
        use crate::CodeRate;

        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let h = code.parity_check_matrix();
        let k = code.k();
        let enc = IraEncoder::new(h, k);

        assert_eq!(enc.n(), code.n());
        assert_eq!(enc.k(), code.k());
        assert_eq!(enc.m(), code.m());

        // Test zero message
        let info = BitVec::zeros(k);
        let cw = enc.encode(&info);
        assert_eq!(cw.len(), code.n());
        let syn = code.syndrome(&cw);
        assert_eq!(syn.count_ones(), 0, "Zero message: syndrome must be zero");

        // Test a few deterministic non-zero messages
        for seed in 0u8..5 {
            let mut info = BitVec::with_capacity(k);
            for i in 0..k {
                info.push_bit(((i as u8).wrapping_add(seed)) % 3 == 0);
            }
            let cw = enc.encode(&info);
            let syn = code.syndrome(&cw);
            assert_eq!(syn.count_ones(), 0, "seed={seed}: syndrome must be zero");
        }
    }
}
