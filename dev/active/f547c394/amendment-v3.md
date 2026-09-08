# Protocol v3 amendment record

[protocol.md](protocol.md) is the normative Zen 3 benchmark protocol. Version
3 defines the acceptance semantics used by the v3 smoke receipts.

- Every declared worthwhile, equivalence, and material-gap margin strictly
  exceeds one plus the declared measurement resolution. Equality rejects.
- The runner and evaluator pass the declared corrected alpha directly to
  bootstrap rank selection. Receipt intervals record that alpha.
- A window at or above the execution-local flagged-window factor is flagged.
  The maximum flagged fraction remains a strict rule.
- The protocol guard compares shared-setting values and their semantic
  justifications, and derives the complete P-rule set from the evaluator.
- Every v1 prior-trial receipt path uses the repository-relative path rule.
- P-20 independently compares the stored v3 alpha with the corrected alpha
  used to recompute every interval. Resolution evidence binds a pilot from the
  same family, derives its corrected alpha from frozen addendum and ledger
  snapshots, and recomputes the widest relative bootstrap half-width from its
  verified raw pairs.
- Lock acceptance records the inherited held descriptor and an independent
  conflicting-lock attempt with the lock path and holder PID.

The v1 and v2 evidence collections are immutable superseded evidence. The v3
r1 collection is immutable falsified evidence: its producing closure names
`run-smoke-v2.sh` while the timed runs use `run-smoke-v3.sh`, so it has
incomplete launcher provenance. Fresh v3 r2 pilot and confirmation samples are
published in `v3-r2-pilot/` and `v3-r2-confirmation/`; their acceptance
summaries report zero findings. The v1/v2 preservation record checks baseline
`c01be44e`, and the r1 preservation record checks all 90 r1 evidence paths
against `7756e1fd`.
