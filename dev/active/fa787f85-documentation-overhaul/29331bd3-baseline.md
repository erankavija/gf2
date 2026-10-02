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

## Before the change

Commit `ff34a202b`, rustc 1.97.0 (2d8144b78 2026-07-07). All 33 tests pass.

| Test | Evidence it validates | Verdict |
| --- | --- | --- |
| `frozen_preregistration_binds_the_committed_protocol_and_manifest` | preregistration accepted as the frozen plan | PASS |
| `frozen_preregistration_anchor_counts_match_the_committed_exact_evidence` | preregistration anchors match the exact-anchor authority | PASS |
| `the_current_journal_admits_an_ordered_continuation_before_q5_n2_starts` | continuation authorization admits the journal prefix | PASS |
| `default_resume_refuses_the_original_producer_mismatch_without_opening_q5_n2` | journal refuses an unauthorized producer | PASS |
| `continuation_rejects_changed_prefix_bytes_gaps_and_preopened_suffixes` | altered journal copies are refused | PASS |
| `an_immutable_second_segment_refuses_a_third_runtime` | continued journal refuses a third producer | PASS |
| `the_committed_q5_terminal_is_adopted_without_float_drift_or_redraw` | committed q=5 n=1 terminal adopted | PASS |
| `a_genuinely_altered_committed_terminal_is_rejected_without_overwrite` | altered committed terminal refused | PASS |
| `the_runner_refuses_an_incomplete_invocation_and_a_missing_receipt` | preregistration refused as a receipt | PASS |
