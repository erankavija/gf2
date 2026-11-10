//! Word-wise encoder of the rate-matched 5G NR LDPC code.
//!
//! The parity part of the lifted parity-check matrix is $[B, 0; C, I]$ with a
//! $4Z \times 4Z$ core $B$ (`@/citation/ThreeGpp2017` Section 5.3.2), so the
//! codeword with systematic part $s$ has core parity $p_c = B^{-1} A s$ and
//! extension parity $p_e = C' [s; p_c]$. Every product is a sum of circulants,
//! and a circulant of shift $t$ applied to a $Z$-bit block is the $Z$-bit
//! window at bit $t$ of the block written twice in a row. The encoder keeps
//! each information and core-parity block in that doubled form, so one
//! circulant costs $\lceil Z/64 \rceil$ shifted word XORs, and computes only
//! the extension blocks rate matching transmits.

use gf2_core::BitVec;

/// A circulant of shift `shift` applied to doubled source block `source`.
#[derive(Clone, Copy, Debug)]
struct Edge {
    source: usize,
    shift: usize,
}

/// One back-substitution step of the core solve: base row `row` determines
/// core parity block `col`, which it reaches through a circulant of shift
/// `shift`; `known` are the row's circulants on core blocks already solved.
#[derive(Clone, Debug)]
struct CoreStep {
    row: usize,
    col: usize,
    shift: usize,
    known: Vec<Edge>,
}

/// Encoding plan of one rate-matched NR code: circulant lists per base row
/// and the core-solve schedule, both derived from the base matrix.
#[derive(Clone, Debug)]
pub(super) struct NrQcEncoder {
    z: usize,
    kb: usize,
    /// Words of one $Z$-bit block.
    block_words: usize,
    /// Words of one doubled block, with slack for the last shifted read.
    doubled_words: usize,
    /// Information blocks holding message bits; the rest are filler zeros.
    info_blocks: usize,
    /// Information circulants of the four core rows.
    core_info: [Vec<Edge>; 4],
    /// Core block whose shifts survive the sum of the four core rows, and the
    /// one shift that survives.
    first_col: usize,
    first_shift: usize,
    steps: Vec<CoreStep>,
    /// Circulants of the transmitted extension rows over information and
    /// core-parity sources, core block `c` being source `kb + c`.
    extension: Vec<Vec<Edge>>,
    target_k: usize,
    punctured: usize,
    parity_kept: usize,
}

impl NrQcEncoder {
    /// Derives the plan from `base` (shifts reduced modulo `z`, $-1$ for no
    /// circulant) for a code of `target_k` message bits, `punctured` leading
    /// punctured bits and `parity_kept` transmitted parity bits.
    ///
    /// # Panics
    ///
    /// Panics if the parity part of `base` is not the NR $[B, 0; C, I]$ form
    /// with a core whose row sum isolates one block, which holds for both
    /// base graphs at every lifting size.
    pub(super) fn new(
        base: &[Vec<i32>],
        kb: usize,
        z: usize,
        target_k: usize,
        punctured: usize,
        parity_kept: usize,
    ) -> Self {
        let shift = |row: usize, col: usize| usize::try_from(base[row][col]).ok();
        let info_blocks = target_k.div_ceil(z).min(kb);
        let info_edges = |row: usize| -> Vec<Edge> {
            (0..info_blocks)
                .filter_map(|col| shift(row, col).map(|shift| Edge { source: col, shift }))
                .collect()
        };
        let core_info = std::array::from_fn(info_edges);

        // Summed over the four core rows, a block's circulants cancel in
        // equal-shift pairs; exactly one block keeps exactly one circulant.
        let mut survivors = Vec::new();
        for col in 0..4 {
            let mut shifts: Vec<usize> = (0..4).filter_map(|row| shift(row, kb + col)).collect();
            shifts.sort_unstable();
            let mut odd = Vec::new();
            for group in shifts.chunk_by(|a, b| a == b) {
                if group.len() % 2 == 1 {
                    odd.push(group[0]);
                }
            }
            if !odd.is_empty() {
                survivors.push((col, odd));
            }
        }
        let [(first_col, odd)] = survivors.as_slice() else {
            panic!("the NR core row sum must isolate one parity block, got {survivors:?}");
        };
        let [first_shift] = odd.as_slice() else {
            panic!("the isolated NR core block must keep one circulant, got {odd:?}");
        };
        let (first_col, first_shift) = (*first_col, *first_shift);

        let mut solved = [false; 4];
        solved[first_col] = true;
        let mut used = [false; 4];
        let mut steps = Vec::with_capacity(3);
        while steps.len() < 3 {
            let step = (0..4).filter(|&row| !used[row]).find_map(|row| {
                let unknown: Vec<usize> = (0..4)
                    .filter(|&col| !solved[col] && shift(row, kb + col).is_some())
                    .collect();
                let [col] = unknown.as_slice() else {
                    return None;
                };
                let known = (0..4)
                    .filter(|&c| solved[c])
                    .filter_map(|c| {
                        shift(row, kb + c).map(|shift| Edge {
                            source: kb + c,
                            shift,
                        })
                    })
                    .collect();
                Some(CoreStep {
                    row,
                    col: *col,
                    shift: shift(row, kb + *col).expect("an unknown block has a circulant"),
                    known,
                })
            });
            let step = step.expect("the NR core must back-substitute one block per row");
            used[step.row] = true;
            solved[step.col] = true;
            steps.push(step);
        }

        let extension_rows = parity_kept.saturating_sub(4 * z).div_ceil(z);
        let extension = (4..4 + extension_rows)
            .map(|row| {
                let mut edges = info_edges(row);
                edges.extend((0..4).filter_map(|c| {
                    shift(row, kb + c).map(|shift| Edge {
                        source: kb + c,
                        shift,
                    })
                }));
                edges
            })
            .collect();

        let block_words = z.div_ceil(64);
        Self {
            z,
            kb,
            block_words,
            doubled_words: (2 * z).div_ceil(64) + 1,
            info_blocks,
            core_info,
            first_col,
            first_shift,
            steps,
            extension,
            target_k,
            punctured,
            parity_kept,
        }
    }

    /// Encodes `message` (`target_k` bits) into `output`, the rate-matched
    /// codeword: message bits `punctured..target_k`, then the first
    /// `parity_kept` parity bits.
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != target_k` or
    /// `output.len() != target_k - punctured + parity_kept` (saturating).
    pub(super) fn encode_into(&self, message: &BitVec, output: &mut BitVec) {
        assert_eq!(
            message.len(),
            self.target_k,
            "message length must equal target_k"
        );
        let systematic = self.target_k.saturating_sub(self.punctured);
        assert_eq!(
            output.len(),
            systematic + self.parity_kept,
            "output length must equal target_n"
        );
        let (z, bw, dw) = (self.z, self.block_words, self.doubled_words);
        let mut sources = vec![0u64; (self.kb + 4) * dw];
        let mut block = vec![0u64; bw];

        let words = message.words();
        for j in 0..self.info_blocks {
            for (w, word) in block.iter_mut().enumerate() {
                *word = gf2_core::packed_window(words, j * z + 64 * w);
            }
            self.store_doubled(&mut block, &mut sources[j * dw..(j + 1) * dw]);
        }

        let mut lambda = [
            vec![0u64; bw],
            vec![0u64; bw],
            vec![0u64; bw],
            vec![0u64; bw],
        ];
        let mut sum = vec![0u64; bw];
        for (row, acc) in lambda.iter_mut().enumerate() {
            for edge in &self.core_info[row] {
                self.xor_circulant(acc, &sources, *edge);
            }
            for (s, a) in sum.iter_mut().zip(acc.iter()) {
                *s ^= *a;
            }
        }

        let mut doubled = vec![0u64; dw];
        self.store_inverse(
            &mut sum,
            self.first_shift,
            &mut doubled,
            &mut sources,
            self.first_col,
        );
        for step in &self.steps {
            let rhs = &mut lambda[step.row];
            for edge in &step.known {
                self.xor_circulant(rhs, &sources, *edge);
            }
            self.store_inverse(rhs, step.shift, &mut doubled, &mut sources, step.col);
        }

        output.copy_bits(0, message, self.punctured.min(self.target_k), systematic);
        let mut offset = systematic;
        let mut remaining = self.parity_kept;
        for c in 0..4 {
            if remaining == 0 {
                return;
            }
            let count = remaining.min(z);
            let start = (self.kb + c) * dw;
            output.write_bits(offset, &sources[start..start + bw], count);
            offset += count;
            remaining -= count;
        }
        for edges in &self.extension {
            block.fill(0);
            for edge in edges {
                self.xor_circulant(&mut block, &sources, *edge);
            }
            let count = remaining.min(z);
            output.write_bits(offset, &block, count);
            offset += count;
            remaining -= count;
        }
        debug_assert_eq!(remaining, 0);
    }

    /// XORs circulant `edge` of its doubled source into the $Z$-bit block
    /// `acc`; bits of `acc` at or above $Z$ take garbage that
    /// [`Self::store_doubled`] and the output writes ignore.
    #[inline]
    fn xor_circulant(&self, acc: &mut [u64], sources: &[u64], edge: Edge) {
        let dw = self.doubled_words;
        let source = &sources[edge.source * dw..(edge.source + 1) * dw];
        let (q, r) = (edge.shift / 64, edge.shift % 64);
        let window = &source[q..q + acc.len() + 1];
        if r == 0 {
            for (a, s) in acc.iter_mut().zip(window) {
                *a ^= *s;
            }
        } else {
            for (a, pair) in acc.iter_mut().zip(window.windows(2)) {
                *a ^= (pair[0] >> r) | (pair[1] << (64 - r));
            }
        }
    }

    /// Masks `block` to $Z$ bits and writes it twice in a row into `doubled`.
    #[inline]
    fn store_doubled(&self, block: &mut [u64], doubled: &mut [u64]) {
        let z = self.z;
        let tail = z % 64;
        if tail != 0 {
            *block.last_mut().expect("a block has a word") &= (1u64 << tail) - 1;
        }
        doubled.fill(0);
        doubled[..block.len()].copy_from_slice(block);
        let (q, r) = (z / 64, z % 64);
        for (w, &word) in block.iter().enumerate() {
            doubled[q + w] |= word << r;
            if r != 0 {
                doubled[q + w + 1] |= word >> (64 - r);
            }
        }
    }

    /// Solves $P^{t} p = $ `rhs` for core block `col` and stores $p$ doubled
    /// as source `kb + col`; `scratch` is a doubled-block buffer.
    fn store_inverse(
        &self,
        rhs: &mut [u64],
        shift: usize,
        scratch: &mut [u64],
        sources: &mut [u64],
        col: usize,
    ) {
        let dw = self.doubled_words;
        self.store_doubled(rhs, scratch);
        let inverse = (self.z - shift) % self.z;
        let (q, r) = (inverse / 64, inverse % 64);
        let mut block = vec![0u64; rhs.len()];
        for (w, word) in block.iter_mut().enumerate() {
            *word = if r == 0 {
                scratch[q + w]
            } else {
                (scratch[q + w] >> r) | (scratch[q + w + 1] << (64 - r))
            };
        }
        let slot = self.kb + col;
        self.store_doubled(&mut block, &mut sources[slot * dw..(slot + 1) * dw]);
    }
}
