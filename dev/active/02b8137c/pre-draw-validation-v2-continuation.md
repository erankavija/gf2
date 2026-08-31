# Pre-draw validation producer continuation v2

This addendum authorizes one provenance-preserving continuation of the
validation cohort fixed by
[`pre-draw-validation-v1-preregistration.json`](pre-draw-validation-v1-preregistration.json).
The machine-readable authority is
[`pre-draw-validation-v2-continuation.json`](pre-draw-validation-v2-continuation.json).
The v1 scientific plan, addresses, draw counts, decision rule, stopping rule,
and preregistration prose remain immutable.

The first execution produced the ordered terminal prefix `q3,n=1..4` and
`q5,n=1`, then stopped when publication re-read the `q5,n=1` terminal with two
finite JSON values one ULP away from the values computed in memory. The
terminal bytes are valid under bit-exact float parsing. This failed publication
is preserved as falsification evidence; no completed address is redrawn or
rewritten.

The continuation envelope replaces only the v1 requirement that one exact
producer identity cover the whole cohort. The active requirement is one exact
runtime-observed producer identity per immutable ordered segment, with exactly
two segments:

1. Producer segment 0 covers anchor indices `[0, 5)` and is bound to the
   original `run-state.json` and the complete 22-file durable journal prefix.
2. Producer segment 1 covers anchor indices `[5, 10)`, beginning at `q=5,n=2`.
   Its runtime identity is observed internally and durably published in
   `producer-segment-state-v2.json` before that address is opened.

The authorization content-binds the v1 preregistration, original run state,
frozen-artifact start snapshot, every phase marker and terminal in the prefix,
each completed address, the boundary, the next address, the correction reason,
and the two-producer limit. Admission rehashes those bytes and rejects a gap,
suffix artifact, incomplete started address, changed prefix, changed plan, or
an existing third runtime identity.

The thin runner makes the exception explicit:

```console
permanent_validation \
  --preregistration dev/active/02b8137c/pre-draw-validation-v1-preregistration.json \
  --state-dir dev/active/02b8137c/validation-journal \
  --receipt dev/active/02b8137c/pre-draw-validation-v1-receipt.json \
  --workers 24 \
  --continue-producer-segment dev/active/02b8137c/pre-draw-validation-v2-continuation.json
```

The final schema-v2 receipt records contiguous half-open producer ranges,
their immutable state identities and complete runtime provenance, and one
terminal content identity for every anchor. Verification rehashes both state
records and every terminal, revalidates address order and segment coverage,
and rechecks the frozen-artifact before/after guard. This evidence contains no
campaign-purpose draw or scientific campaign estimate.

The v1 run-state reader is a private migration boundary restricted to the
content identity and complete prefix named by this authorization. Fresh runs,
phase markers, receipts, publication, and verification use schema v2; no new
v1 state is created or generally admitted. This migration boundary is removed
after the `02b8137c` final receipt is published and independently verified.
