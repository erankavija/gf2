//! Linear-time IRA (Irregular Repeat-Accumulate) staircase encoder.

use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;

/// Precomputed staircase encoder for dual-diagonal IRA codes (e.g. DVB-T2).
///
/// The codeword is systematic, `c = [info (k bits) | parity (m bits)]`, and the
/// parity part of H is dual-diagonal: row 0 has a single 1 at column `k`, and
/// row `p > 0` has 1s at columns `k+p` and `k+p-1`. With `s[p]` the XOR of the
/// info bits in check row `p`, the parity bits follow the staircase recursion:
///
/// ```text
/// par[0] = s[0]
/// par[p] = s[p] XOR par[p-1]   for p = 1 .. m-1
/// ```
///
/// [`IraEncoder::new`] checks only `m + k == n`; the dual-diagonal parity part
/// is the caller's precondition.
#[derive(Debug, Clone)]
pub struct IraEncoder {
    /// Codeword length n.
    n: usize,
    /// Information bit count k.
    k: usize,
    /// Parity bit count m = n - k.
    m: usize,
    /// `check_info_vars[p]`: the information columns (`< k`) of check row `p`,
    /// whose XOR over `info` is `s[p]`.
    check_info_vars: Vec<Vec<usize>>,
}

impl IraEncoder {
    /// Builds the encoder from the parity-check matrix `h` (m × n) of a code
    /// with systematic columns `0..k`, in O(nnz) time and storage.
    ///
    /// # Panics
    ///
    /// Panics if `h.rows() + k != h.cols()`.
    pub fn new(h: &SpBitMatrixDual, k: usize) -> Self {
        let m = h.rows();
        let n = h.cols();
        assert_eq!(
            m + k,
            n,
            "IraEncoder requires m + k == n (got m={m}, k={k}, n={n})"
        );

        // Parity-column entries (>= k) are implicit in the staircase.
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

    /// Encodes `info` (k bits) into the systematic codeword `[info | parity]`
    /// (n bits) in O(nnz).
    ///
    /// # Panics
    ///
    /// Panics if `info.len() != k`.
    pub fn encode(&self, info: &BitVec) -> BitVec {
        assert_eq!(
            info.len(),
            self.k,
            "IraEncoder::encode: info length {} != k={}",
            info.len(),
            self.k
        );

        let mut s = vec![false; self.m];
        for (p, vars) in self.check_info_vars.iter().enumerate() {
            let mut acc = false;
            for &v in vars {
                acc ^= info.get(v);
            }
            s[p] = acc;
        }

        let mut par = vec![false; self.m];
        par[0] = s[0];
        for p in 1..self.m {
            par[p] = s[p] ^ par[p - 1];
        }

        let mut cw = BitVec::with_capacity(self.n);
        for i in 0..self.k {
            cw.push_bit(info.get(i));
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

    /// A dual-diagonal H fixture.
    fn make_mini_ira_h() -> (SpBitMatrixDual, usize) {
        let k = 4usize;
        let m = 4usize;
        let n = k + m;
        let edges: Vec<(usize, usize)> = vec![
            (0, 0),
            (0, 1),
            (1, 1),
            (1, 2),
            (2, 2),
            (2, 3),
            (3, 0),
            (3, 3),
            (0, k),
            (1, k + 1),
            (1, k),
            (2, k + 2),
            (2, k + 1),
            (3, k + 3),
            (3, k + 2),
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

        for msg_val in 0u8..16 {
            let mut info = BitVec::with_capacity(k);
            for bit in 0..k {
                info.push_bit((msg_val >> bit) & 1 == 1);
            }
            let cw = enc.encode(&info);

            assert_eq!(cw.len(), enc.n());

            for i in 0..k {
                assert_eq!(
                    cw.get(i),
                    info.get(i),
                    "msg={msg_val}: systematic bit {i} mismatch"
                );
            }

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

        let info = BitVec::zeros(k);
        let cw = enc.encode(&info);
        assert_eq!(cw.len(), code.n());
        let syn = code.syndrome(&cw);
        assert_eq!(syn.count_ones(), 0, "Zero message: syndrome must be zero");

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
