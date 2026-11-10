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

        Self {
            z,
            kb,
            block_words: z.div_ceil(64),
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

    /// Encodes `message` (`target_k` bits) into the rate-matched codeword:
    /// message bits `punctured..target_k`, then the first `parity_kept`
    /// parity bits.
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != target_k`.
    pub(super) fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.target_k,
            "message length must equal target_k"
        );
        match self.block_words {
            1 => self.encode_blocks::<1>(message),
            2 => self.encode_blocks::<2>(message),
            3 => self.encode_blocks::<3>(message),
            4 => self.encode_blocks::<4>(message),
            5 => self.encode_blocks::<5>(message),
            6 => self.encode_blocks::<6>(message),
            words => unreachable!("NR lifting sizes span at most six words, got {words}"),
        }
    }

    /// [`Self::encode`] for $\lceil Z/64 \rceil = W$. A doubled block takes
    /// $2W + 1$ words, the last one slack for the shifted read; the slot
    /// after the core-parity sources is the scratch of the core solve.
    fn encode_blocks<const W: usize>(&self, message: &BitVec) -> BitVec {
        let (z, kb) = (self.z, self.kb);
        let stride = 2 * W + 1;
        SOURCES.with_borrow_mut(|sources| {
            if sources.len() < (kb + 5) * stride {
                sources.resize((kb + 5) * stride, 0);
            }
            let words = message.words();
            for j in 0..self.info_blocks {
                let block = std::array::from_fn(|w| gf2_core::packed_window(words, j * z + 64 * w));
                store_doubled::<W>(z, block, &mut sources[j * stride..(j + 1) * stride]);
            }

            let mut lambda = [[0u64; W]; 4];
            let mut sum = [0u64; W];
            for (acc, edges) in lambda.iter_mut().zip(&self.core_info) {
                for edge in edges {
                    xor_circulant::<W>(acc, sources, stride, *edge);
                }
                xor_into(&mut sum, acc);
            }
            self.solve_core::<W>(sum, self.first_shift, self.first_col, sources);
            for step in &self.steps {
                let mut rhs = lambda[step.row];
                for edge in &step.known {
                    xor_circulant::<W>(&mut rhs, sources, stride, *edge);
                }
                self.solve_core::<W>(rhs, step.shift, step.col, sources);
            }

            let systematic = self.target_k.saturating_sub(self.punctured);
            let len = systematic + self.parity_kept;
            let start = self.punctured.min(self.target_k);
            let mut out: Vec<u64> = (0..len.div_ceil(64))
                .map(|w| gf2_core::packed_window(words, start + 64 * w))
                .collect();
            // Words past the systematic run picked up message bits beyond
            // it; the parity ORs into zeros.
            let (q, r) = (systematic / 64, systematic % 64);
            if q < out.len() {
                if r != 0 {
                    out[q] &= (1u64 << r) - 1;
                    out[q + 1..].fill(0);
                } else {
                    out[q..].fill(0);
                }
            }

            let mut offset = systematic;
            let mut remaining = self.parity_kept;
            for c in 0..4 {
                if remaining == 0 {
                    break;
                }
                let slot = (kb + c) * stride;
                let block = std::array::from_fn(|w| sources[slot + w]);
                let count = remaining.min(z);
                or_bits::<W>(&mut out, offset, block, count);
                offset += count;
                remaining -= count;
            }
            for edges in &self.extension {
                let mut block = [0u64; W];
                for edge in edges {
                    xor_circulant::<W>(&mut block, sources, stride, *edge);
                }
                let count = remaining.min(z);
                or_bits::<W>(&mut out, offset, block, count);
                offset += count;
                remaining -= count;
            }
            debug_assert_eq!(remaining, 0);
            BitVec::from_words(out, len)
        })
    }

    /// Solves $P^{t} p = $ `rhs` for core block `col` and stores $p$ doubled
    /// as source `kb + col`.
    fn solve_core<const W: usize>(
        &self,
        rhs: [u64; W],
        shift: usize,
        col: usize,
        sources: &mut [u64],
    ) {
        let (z, kb) = (self.z, self.kb);
        let stride = 2 * W + 1;
        let scratch = (kb + 4) * stride;
        store_doubled::<W>(z, rhs, &mut sources[scratch..scratch + stride]);
        let inverse = (z - shift) % z;
        let block = rotate::<W>(&sources[scratch..scratch + stride], inverse);
        let slot = (kb + col) * stride;
        store_doubled::<W>(z, block, &mut sources[slot..slot + stride]);
    }
}

thread_local! {
    /// Doubled information and core-parity blocks of the calling thread's
    /// encode; every slot an encode reads it writes first.
    static SOURCES: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[inline(always)]
fn xor_into<const W: usize>(acc: &mut [u64; W], other: &[u64; W]) {
    for (a, o) in acc.iter_mut().zip(other) {
        *a ^= *o;
    }
}

/// The $Z$-bit window at bit `shift` of a doubled block, the circulant of
/// that shift applied to the block. Bits at or above $Z$ of the result are
/// garbage that [`store_doubled`] and [`or_bits`] mask.
#[inline(always)]
fn rotate<const W: usize>(slot: &[u64], shift: usize) -> [u64; W] {
    let (q, r) = (shift / 64, shift % 64);
    let window = &slot[q..q + W + 1];
    // `(hi << 1) << (63 - r)` is `hi << (64 - r)` and zero at `r == 0`.
    std::array::from_fn(|w| (window[w] >> r) | ((window[w + 1] << 1) << (63 - r)))
}

/// XORs circulant `edge` of its doubled source into `acc`.
#[inline(always)]
fn xor_circulant<const W: usize>(acc: &mut [u64; W], sources: &[u64], stride: usize, edge: Edge) {
    let slot = &sources[edge.source * stride..(edge.source + 1) * stride];
    xor_into(acc, &rotate::<W>(slot, edge.shift));
}

/// Masks `block` to $Z$ bits and writes it twice in a row into `slot`.
#[inline(always)]
fn store_doubled<const W: usize>(z: usize, mut block: [u64; W], slot: &mut [u64]) {
    mask_bits(&mut block, z);
    slot.fill(0);
    slot[..W].copy_from_slice(&block);
    let (q, r) = (z / 64, z % 64);
    for (w, &word) in block.iter().enumerate() {
        slot[q + w] |= word << r;
        if r != 0 {
            slot[q + w + 1] |= word >> (64 - r);
        }
    }
}

/// Clears the bits of `block` at or above `count`.
#[inline(always)]
fn mask_bits<const W: usize>(block: &mut [u64; W], count: usize) {
    for (w, word) in block.iter_mut().enumerate() {
        let low = 64 * w;
        if count <= low {
            *word = 0;
        } else if count < low + 64 {
            *word &= (1u64 << (count - low)) - 1;
        }
    }
}

/// ORs the low `count` bits of `block` into `out` from bit `offset` up;
/// the target bits are zero.
#[inline(always)]
fn or_bits<const W: usize>(out: &mut [u64], offset: usize, mut block: [u64; W], count: usize) {
    mask_bits(&mut block, count);
    let (q, r) = (offset / 64, offset % 64);
    for (w, &word) in block.iter().enumerate().take(count.div_ceil(64)) {
        out[q + w] |= word << r;
        if r != 0 {
            if let Some(next) = out.get_mut(q + w + 1) {
                *next |= word >> (64 - r);
            }
        }
    }
}
