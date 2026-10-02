# Frozen validation evidence verdicts (29331bd3)

The committed frozen-evidence bundle is `dev/active/02b8137c/`: the v1
preregistration, the v2 continuation authorization, and the producer-0
validation journal. No validation receipt and no snapshot copy of the bundle
is committed (`git ls-files | grep pre-draw-validation` lists only the bundle).
The continuation authorization and the digests in
`crates/gf2-sim/src/permanent_campaign/validation.rs` pin the bundle bytes.

The verdict run is the gf2-sim validation suite, whose tests listed below read
the committed bundle:

```console
$ ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --all-features \
    --cargo-profile ci-test --profile ci --test permanent_validation
```

## Verdicts

Both runs use rustc 1.97.0 (2d8144b78 2026-07-07). Commit `ff34a202b`
compares recorded paths; commit `2c0d225eb` identifies the evidence by digest
and adds `relocated_frozen_evidence_is_identified_by_content`, which also
passes. Every other test passes at both commits.

| Test | Evidence it validates | `ff34a202b` | `2c0d225eb` |
| --- | --- | --- | --- |
| `frozen_preregistration_binds_the_committed_protocol_and_manifest` | preregistration accepted as the frozen plan | PASS | PASS |
| `frozen_preregistration_anchor_counts_match_the_committed_exact_evidence` | preregistration anchors match the exact-anchor authority | PASS | PASS |
| `the_current_journal_admits_an_ordered_continuation_before_q5_n2_starts` | continuation authorization admits the journal prefix | PASS | PASS |
| `default_resume_refuses_the_original_producer_mismatch_without_opening_q5_n2` | journal refuses an unauthorized producer | PASS | PASS |
| `continuation_rejects_changed_prefix_bytes_gaps_and_preopened_suffixes` | altered journal copies are refused | PASS | PASS |
| `an_immutable_second_segment_refuses_a_third_runtime` | continued journal refuses a third producer | PASS | PASS |
| `the_committed_q5_terminal_is_adopted_without_float_drift_or_redraw` | committed q=5 n=1 terminal adopted | PASS | PASS |
| `a_genuinely_altered_committed_terminal_is_rejected_without_overwrite` | altered committed terminal refused | PASS | PASS |
| `the_runner_refuses_an_incomplete_invocation_and_a_missing_receipt` | preregistration refused as a receipt | PASS | PASS |
