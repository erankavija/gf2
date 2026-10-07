# Outcome: the matvec lane-threshold confirmation

> **Diátaxis Type:** Explanation

Issue `@/issue/4337c02e` contrasts the scalar lane of the allocated
`BitMatrix::matvec` with the SIMD lane the selector takes, at the strides the
[frozen addendum](matvec-lane-threshold-addendum.md) declares. This page states
what the confirmation records and what follows from it. It carries no measured
value: every estimate, interval, pair count and window count is in
[the generated tables](tables.md), and the rule's result and the ledger
arithmetic are in [the generated outcome record](outcome-record.md)
([`outcome.json`](outcome.json)).

## The confirmation is rejected

The confirmation's
[acceptance summary](../../../bench_results/4337c02e/4337c02e-matvec-simd-threshold/v4-r1-confirmation/acceptance-summary.md)
records the verdict `rejected` and does not qualify the receipt for production
selection; its finding is quoted in the outcome record, and the summary's row in
the tables' Source section holds the finding count. The campaign itself is
complete: its execution log holds every declared cell at its declared pairs and
a terminal `complete` record (`dev/scripts/verify-campaign-log.py`).

The finding is P-03, on the declared measurement resolution. The addendum's
*Margins and resolution* section sizes the resolution over the six confirmatory
cells, and the [confirmation addendum](campaigns/matvec-simd-threshold-confirmation.json)
declares the value its
[derivation record](campaigns/matvec-simd-threshold-confirmation-derivation.txt)
states under `floor scope`. Acceptance verifies a declared resolution against
the widest half-width of the whole pinned pilot, exploratory cells included,
and the pilot's cold cells are wider than the declared value. The freeze is
therefore falsified, and three committed statements are contradicted by the
acceptance summary and stay as recorded:

- the addendum's rule that the exploratory cells take no part in the
  resolution;
- the `eligible` outcome of [`pilot-resolution.md`](pilot-resolution.md);
- the derivation record's `frozen measurement resolution` line.

The canonical freezer floors every supplied resolution at the whole pilot's
half-width, and its test requires it to refuse this record
(`freeze-confirmation.test.sh`, case 4).

## Recorded cell outcomes

Every cell stays committed with the outcome the evaluator records: one row in
the tables' Confirmation section and one row in the outcome record's *Recorded
cell outcomes* table. Under a rejected verdict no row is confirmatory; each is
a descriptive estimate with its pair count and interval, as the pilot rows
are. The tables' direction-agreement section compares each row's direction
with the pilot's.

The recorded outcomes do not form one interval above the present threshold:
the rows at the 9-word and 63-word strides favour the scalar lane, the
32-word row between them records the scalar lane as a regression, and the
rows at 8, 64 and 65 words record no material difference. A single boundary
expresses neither a scalar lead at 9 and 63 words nor the SIMD lead at 32.
The 9-word and 63-word rows are a material remaining gap of the SIMD lane of
the allocated `matvec`; they are exploratory evidence of it.

## Decision

The frozen *Resolution rule* yields *no change* at its validity step; the
outcome record states the deciding step. This issue changes no threshold and
no selector.

No further confirmation is run. The outcome record's *A further confirmation
under P-20* table computes the next attempt's allocation from the committed
ledger: P-20's tail rule admits no further confirmatory cell for this family.

`@/issue/63bad95d` owns the value of `bit_matrix.matvec_simd_min_words`. Its
[`selector-outcomes.json`](../63bad95d/selector-outcomes.json) cites
`outcome.json` by path, SHA-256 and outcome, and the per-stride rows of both
stages reach it as descriptive input through `outcome.json` and the tables.

## Boundary suites and toolchain

No threshold changes, so no changed selection needs new boundary cells. The
shared suite `crates/gf2-core/tests/simd_equiv_matvec.rs` proves exact outputs
on both lanes:

- `matvec_boundary_shapes_index_canonically_and_keep_zero_tails` covers row
  counts and column counts 0, 1, 63, 64 and 65, and
  `matvec_word_boundary_lengths_match_scalar_reference` the square shapes of
  those sizes;
- `matvec_is_exact_on_both_sides_of_each_threshold_stride` covers column
  counts 0, 1, 63, 64 and 65 bits past the last full word on both sides of
  each stride a threshold value can take;
- `each_route_matches_the_reference_at_stride_and_word_boundaries` pins each
  lane through `matvec_with_route` at strides of 0, 1, 63, 64 and 65 words.

[`survey/boundary-suites.txt`](survey/boundary-suites.txt), written by
[`survey/run-boundary-suites.sh`](survey/run-boundary-suites.sh), records the
suite under Rust 1.95 in the build with the `simd` feature and in the build
without it.

## Source of the threshold

The selector reads one definition: the conservative constant of
`crates/gf2-core/src/matrix.rs`, which the conservative tuning profile names as
its default, and the baked counterpart of `crates/gf2-core/src/tuning/baked.rs`
that a `gf2_tuning_baked` build substitutes (claims `matvec-threshold-read`,
`matvec-threshold-conservative` and `matvec-threshold-baked-alias` of the
[source ledger of `63bad95d`](../63bad95d/survey/source-evidence.json)). This
issue adds no constant and no second source.

The outcome is an input of the offline tuning system through the citation
above. The `tuning-campaign-records` step of `scripts/cargo-ci.sh`
(`dev/scripts/validate_tuning_extent_campaign_test.py`, `SelectorOutcomes`)
fails when the citation's digest or outcome differs from `outcome.json`, when
the conservative profile differs from the constant the ledger's claim quotes,
or when a committed profile that states the selector holds another value
after a `no-change` outcome.
