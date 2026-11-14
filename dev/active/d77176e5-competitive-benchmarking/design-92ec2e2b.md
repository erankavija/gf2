# Design: BCH generator materialization in the declared layout

Design for @/issue/92ec2e2b: workload W2, the systematic generator
$[\,I_k \mid P\,]$ of every row, against M4RI `mzd_echelonize_m4ri`
@/citation/AlbrechtBard2026 and AFF3CT basis-vector encoding
@/citation/Cassagne2019.

## Measurement basis

Indication, host fraktaali (AMD Ryzen 9 5900X), `taskset -c 2-5`,
2026-11-12, gf2 at commit `540483f2b`, `--release` with
`-C target-cpu=native`. The gf2 side ran a driver built outside the tracked
tree that times the call of the cutover adapter
`experiments/encode-compare/src/cutover/current.rs`
(`LayoutView::generator_matrix` in the descending layout,
`DvbT2BchCode::generator_matrix` for the DVB-T2 rows) and, for comparison,
`BchCode::generator_matrix` in the default ascending layout. The comparator
side is one round of the external arm of
`experiments/encode-compare/cutover.sh` with
`CUTOVER_CELLS=W2/B1,W2/B2,W2/B3,W2/T2S`; T2N was not run (call budget). The
night-window table of @/issue/fd9d5416 supersedes the times.

The faster comparator per row is AFF3CT `Encoder_BCH_inter` on B1
(0.160 $\mu$s per call) and M4RI `mzd_echelonize_m4ri` on B2, B3 and T2S
(6.31 $\mu$s, 34.6 $\mu$s, 36.2 ms). gf2's adapter call takes 0.566 $\mu$s,
63.0 $\mu$s, 439 $\mu$s and 2.05 s on those rows, while
`BchCode::generator_matrix` in the ascending layout takes 0.072 $\mu$s,
0.481 $\mu$s and 1.49 $\mu$s on B1 to B3, 13 to 23 times faster than M4RI on
B2 and B3: the declared-layout and shortened paths are what lose.

## Profile

- B2, adapter call: 63 % `LayoutView::generator_matrix_into` (with
  `permute_matrix` and `gather_in_place` inlined), 36 % `BitMatrix::set`. The
  fill itself is below 1 %.
- T2S: 66 % and 30 % in the same two symbols, 3.6 % in the restriction copy of
  `Shortened::generator_matrix_into`.

Two causes:

1. `LayoutView::generator_matrix_into` (`crates/gf2-coding/src/bch/matrix.rs`)
   writes the ascending-layout matrix, then applies the layout correspondence
   with `permute_matrix`, one `get` and one bounds-checked `set` per cell per
   axis: $2kn$ single-bit moves. For the descending layout the correspondence
   reverses the row order and each of the two column blocks.
2. `Shortened::generator_matrix_into` (`crates/gf2-coding/src/transform/mod.rs`),
   derivation `SystematicRestriction`, materializes the whole mother generator
   and copies the kept rows and columns out cell by cell. For DVB-T2 the
   mother is the length-$2^m - 1$ code: $16\,215 \times 16\,383$ bits for T2S
   and $65\,343 \times 65\,535$ bits (535 MB) for T2N, each permuted cell by
   cell by cause 1 first. T2N is projected from T2S at over 30 s per call.

## Why the comparators are faster

- M4RI (`$GF2_EXT_ROOT/6c6b09b1/m4ri-src/m4ri/echelonform.c`,
  `mzd_echelonize_m4ri`) reduces the $k \times n$ cyclic matrix of shifts of
  $g$ by Method-of-Four-Russians elimination, $O(k^2 n / (64 \log k))$ word
  operations, all word-parallel. It is asymptotically above gf2's
  $O(k \lceil r/64 \rceil)$ recurrence; it wins only because gf2 moves single
  bits in cause 1 and materializes the mother in cause 2.
- AFF3CT encodes the $k$ unit messages, $k$ LFSR runs of $k r$ element
  operations over 8 or 32 frames per SIMD wave: $O(k^2 r)$ work, unpacked,
  also above gf2's recurrence.

## Proposed change

1. **Fill in the declared layout** (gf2-coding, `crates/gf2-coding/src/bch/matrix.rs`).
   `MatrixFill::fill_generator` and `MatrixFill::fill_parity_check` take the
   `SystematicPlan` (dimension, redundancy, layout) in place of the bare
   dimension, and `LayoutView::generator_matrix_into` and
   `LayoutView::parity_check_matrix_into` each make one fill call with the
   view's plan. `permute_matrix` leaves both `LayoutView` methods.
   - `BitMatrix` generator arm: the existing recurrence
     $P_{i+1} = x P_i \bmod g$, $P_0 = x^r \bmod g$, runs in a scratch
     register of $\lceil r/64 \rceil$ words and writes row $\sigma(i)$
     directly: the identity bit at the message coordinate the layout assigns to
     degree $r + i$, and the parity through the parity-placement step the
     encoder uses (`packed_place_codeword`'s parity half, factored into one
     shared function so the generator and the codewords cannot diverge, under
     @/inv/convention-convergence). Each row costs $O(\lceil r/64 \rceil)$
     word operations; zeroing the matrix is the only $O(kn/64)$ term.
   - `BitMatrix` parity-check arm: $H = [\,-P^{\mathsf T} \mid I_r\,]$ in the
     plan's layout. The same recurrence writes the placed parity rows into a
     $k \times r$ scratch `BitMatrix`, `BitMatrix::transpose` (gf2-core's
     block transpose) turns it into the $r \times k$ block, and each of its rows
     is copied word-wise into $H$ at the layout's message columns, with the
     identity at the layout's parity columns. This replaces today's per-cell
     column recurrence, $O(kr)$ `get`/`set` calls, with
     $O(k \lceil r/64 \rceil)$ word operations plus the transpose.
   - The provided bodies, which serve `FieldMatrix` and any opt-in matrix type
     such as the trait's `RowMajor` rustdoc example, write the ascending fill
     and then apply the layout correspondence with `permute_matrix`, which
     moves into `crates/gf2-coding/src/bch/matrix.rs` as the provided bodies' helper. `FieldMatrix`
     overrides that step with row swaps (`FieldMatrix::swap_rows`) and
     in-place reversal of the two contiguous column blocks of each row slice,
     the shape every `SystematicLayout` variant's correspondence has.
2. **Restriction without the mother** (gf2-coding, `crates/gf2-coding/src/traits.rs` and
   `crates/gf2-coding/src/transform/mod.rs`). `GeneratorMatrixAccess` gains the provided method
   `systematic_restriction_into(&self, kept_rows: &[usize], kept_columns: &[usize], out)`,
   whose default body is today's materialize-and-copy, so every existing
   implementor keeps its behavior; `Shortened`'s `SystematicRestriction` arm
   calls it. The `BchCode` and `LayoutView` implementations of the trait
   override it when the removed coordinates are message coordinates of the
   highest degrees and every parity coordinate is kept, which is the
   shortening of a polynomial code: the result is then change 1's fill for the
   plan with dimension $k' = |\text{kept rows}|$ and length $n' = k' + r$ in
   the code's layout. The override is the trait method and calls the
   `MatrixFill` fill, so there is no type-specific route beside the trait.
   DVB-T2 removes the leading $K_{\text{mother}} - K_{\text{bch}}$ message
   coordinates of the descending layout, the highest degrees, so both DVB-T2
   rows take the override. Other restrictions fall to the default.
3. **Word-level copy in the default restriction.** The default body copies
   row runs with `BitMatrix` word operations where the kept columns form
   contiguous runs, instead of a `get`/`set` per cell. This is the residual
   path for restrictions change 2 does not recognize.

Selection is structural: the representation picks the fill arm, and the
shortening's shape picks the override (@/inv/structural-performance-defaults);
a caller of `generator_matrix` passes nothing new (@/inv/ergonomic-defaults).
All three changes are safe Rust in gf2-coding; no kernel is involved, so the
scalar path is the only path and no fallback is needed.

## Implementor sweep

`rg 'impl.*\b(MatrixFill|GeneratorMatrixAccess|ParityCheckMatrixAccess)\b.* for' crates experiments`
at `540483f2b` lists every implementor the change touches; `experiments/`
has none. The change edits each one as follows, and sweeps the rustdoc that
states the old behavior.

`MatrixFill` (signature change, every implementor edited):

- `BitMatrix`, `FieldMatrix` (`crates/gf2-coding/src/bch/matrix.rs`): new
  arms as in change 1.
- `RowMajor<F>` in the trait's rustdoc example and `RowMajorMatrix<F>` in the
  `crates/gf2-coding/src/bch/matrix.rs` tests: empty opt-ins that take the provided bodies; the
  example's code is unchanged and its prose states the plan argument.

`GeneratorMatrixAccess` (provided method; implementors compile unchanged):

- Override: `BchCode<X, S, M>` and `LayoutView<X, S, M>`
  (`crates/gf2-coding/src/bch/matrix.rs`).
- Forward to the wrapped code, so a wrapped BCH mother keeps the override:
  `CachedMatrices<C>` (`crates/gf2-coding/src/bch/matrix.rs`), the `CountingCode<C>` test wrapper
  (`crates/gf2-coding/src/bch/matrix.rs`).
- Default body, no edit: `DrmCode` (`crates/gf2-coding/src/drm.rs`), `QcGldpcCode` (`crates/gf2-coding/src/gldpc/mod.rs`),
  `LinearBlockCode` (`crates/gf2-coding/src/linear.rs`), `CrcCode` (`crates/gf2-coding/src/crc.rs`), `LdpcCode`
  (`crates/gf2-coding/src/ldpc/core.rs`), `Extended<C>`, `Shortened<C>`, `Punctured<C>` and the
  `DenseTestCode<F>` test code (`crates/gf2-coding/src/transform/mod.rs`), `RankDerivedMother<C>`
  (`crates/gf2-coding/src/test_support.rs`, which exists to force the rank-derived path),
  `RepetitionCode<F>` and `MockLinearCode` (`crates/gf2-coding/src/traits.rs` tests) and the
  `FixtureCode` test code (`crates/gf2-coding/src/osd/generator.rs`). The blanket
  `compat::binary_v1::GeneratorMatrixAccess` impl in `crates/gf2-coding/src/traits.rs` reaches only
  `generator_matrix` and needs no edit.

`ParityCheckMatrixAccess` (no trait change; the `LayoutView` and `BchCode`
methods change through `MatrixFill::fill_parity_check`): the other
implementors, `LinearBlockCode`, `Extended<C>`, `Shortened<C>`,
`CachedMatrices<C>`, `CountingCode<C>`, `RankDerivedMother<C>` and
`RepetitionCode<F>`, are unchanged. `Shortened`'s parity-check
`SystematicRestriction` arm materializes the mother's parity check the way
its generator arm does today; REQ-01 and REQ-02 name only the generator, so
that arm is outside this change and needs its own issue.

Rustdoc to rewrite in the same change: the `MatrixFill` trait page and both
fill methods (signature, the plan's layout, complexity per arm);
`LayoutView::generator_matrix_into` and `LayoutView::parity_check_matrix_into`
(complexity no longer includes an in-place permutation);
`GeneratorMatrixAccess` (the new method, its default and its contract on
`kept_rows` and `kept_columns`); `Shortened::generator_matrix_into` (the
`SystematicRestriction` complexity, which today states the mother's
materialization); `dvb_t2_bch_code` (whose prose says the shortened code
materializes no generator).

## Conformance tests (REQ-01)

- The generator conformance of `crates/gf2-coding/tests/bch_conformance.rs`
  compares `generator_matrix` against
  `test_support::bch_generator_matrix_by_encoding`, the basis-vector oracle.
  Its per-layout `LayoutView` loop gains the same comparison, with the oracle
  encoding the unit messages through the view's encoder, so each layout's
  rows are pinned to its own codewords.
- `Shortened`: the override's output equals the default
  materialize-and-copy body (reachable from `test_support` as the oracle) on
  small mothers shortened so that $k'$ and $n'$ hit 1, 63, 64 and 65 (for
  example the $m = 8$ narrow-sense mothers with $r \le 64$), under both layouts.
  A restriction that is not a highest-degree shortening asserts that the
  default path ran and still matches.
- The DVB-T2 rows stay above `FAST_TIER_MATRIX_CELLS`; a slow-tier test
  (`#[ignore = "slow: ..."]`) checks T2S and T2N row by row against the
  shortened code's encoder on the unit messages, which change 2 makes cheap.
- `parity_check_matrix` keeps its existing oracle comparison
  (`test_support::bch_parity_check_matrix_by_encoding`), and the per-layout
  `LayoutView` loop gains it too, so the new `BitMatrix` parity arm is pinned
  in both layouts, including codes whose $r$ and $k$ sit at 63, 64 and 65,
  the transpose's block boundaries.

## Expected gain (estimate)

| Row | Estimate after the change | Faster comparator | Ratio |
| --- | --- | --- | --- |
| B1 | 0.08 to 0.10 $\mu$s | AFF3CT 0.160 | 1.6 to 2.0 |
| B2 | 0.5 to 0.7 $\mu$s | M4RI 6.31 | 9 to 13 |
| B3 | 1.6 to 2.0 $\mu$s | M4RI 34.6 | 17 to 22 |
| T2S | 0.5 to 1.5 ms | M4RI 36.2 ms | 25 to 70 |
| T2N | 15 to 60 ms | M4RI, not run; about 3.5 s by cubic scaling from T2S | above 50 |

Reasoning: the ascending-layout fill measured above, plus one parity
placement per row ($O(\lceil r/64 \rceil)$ words, the same placement the
encoder performs). For the DVB-T2 rows the cost is zeroing the
$k' \times n'$ matrix (6.3 MB for T2S, 130 MB for T2N), dominated by first-touch
page faults, plus $k'$ register steps of three words. Both comparators
allocate the same output. B1 is the row with the least margin, since the
$5 \times 15$ matrix's allocation is most of its cost.
