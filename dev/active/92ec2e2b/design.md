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

1. **Fill in the declared layout** (gf2-coding, `bch/matrix.rs`).
   `MatrixFill::fill_generator` takes the `SystematicPlan` (dimension,
   redundancy, layout) in place of the bare dimension. The `BitMatrix` arm runs
   the existing recurrence $P_{i+1} = x P_i \bmod g$, $P_0 = x^r \bmod g$, in a
   scratch register of $\lceil r/64 \rceil$ words, and writes row $\sigma(i)$
   directly: the identity bit at the message coordinate the layout assigns to
   degree $r + i$, and the parity through the parity-placement step the
   encoder uses (`packed_place_codeword`'s parity half, factored into one
   shared function so the generator and the codewords cannot diverge, under
   @/inv/convention-convergence). Each row costs $O(\lceil r/64 \rceil)$ word
   operations; the matrix zeroing is the only $O(kn/64)$ term. The
   `FieldMatrix` arm keeps the fill-then-permute route, inside its own
   implementation, so `LayoutView::generator_matrix_into` makes one call for
   every representation and `permute_matrix` stays only where symbols are not
   bit-packed. The parity-check fill gets the same plan argument so the trait
   keeps one shape; its `BitMatrix` arm can follow in the same change.
2. **Restriction without the mother** (gf2-coding, `traits.rs` and
   `transform/mod.rs`). `GeneratorMatrixAccess` gains
   `systematic_restriction_into(&self, kept_rows: &[usize], kept_columns: &[usize], out)`,
   whose default body is today's materialize-and-copy, so every existing
   mother keeps its behavior; `Shortened`'s `SystematicRestriction` arm calls
   it. `LayoutView` over a `BchCode` overrides it when the removed coordinates
   are message coordinates of the highest degrees and every parity coordinate
   is kept, which is the shortening of a polynomial code: the result is then
   exactly change 1's fill for the plan with dimension $k' = |\text{kept rows}|$
   and length $n' = k' + r$ in the mother's layout. DVB-T2 removes the leading
   $K_{\text{mother}} - K_{\text{bch}}$ message coordinates of the descending
   layout, the highest degrees, so both DVB-T2 rows take the override. Other
   restrictions fall to the default.
3. **Word-level copy in the default restriction.** The default body copies
   row runs with `BitMatrix` word operations where the kept columns form
   contiguous runs, instead of a `get`/`set` per cell. This is the residual
   path for restrictions change 2 does not recognize.

Selection is structural: the representation picks the fill arm, and the
shortening's shape picks the override (@/inv/structural-performance-defaults);
a caller of `generator_matrix` passes nothing new (@/inv/ergonomic-defaults).
All three changes are safe Rust in gf2-coding; no kernel is involved, so the
scalar path is the only path and no fallback is needed.

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
- `parity_check_matrix` keeps its existing oracle comparison under the new
  plan argument.

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

## Open questions for the lead

1. **Trait shape.** Change 1 changes `MatrixFill::fill_generator`'s signature
   and change 2 adds a provided method to `GeneratorMatrixAccess`, a public
   trait. Recommendation: accept both; the alternative (a `BitMatrix`
   specialization inside `LayoutView`) is a bypass of the trait under
   @/inv/convention-convergence.
2. **Parity check in scope.** `LayoutView::parity_check_matrix_into` has the
   same per-cell permutation. REQ-01 names only the generator; recommendation:
   carry the parity-check `BitMatrix` arm in the same change, since the
   trait signature changes anyway, or file it as its own issue.
