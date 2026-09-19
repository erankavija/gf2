# Protocol v4 amendment record

[protocol.md](protocol.md) is the normative Zen 3 benchmark protocol. Version
4 changes the acceptance rule P-11 so that a campaign whose session stops
inside a cell can still yield an acceptable receipt. Version 3 rejects every
such campaign: its runner measures the unfinished cell again and its P-11
allows one `cell-start` per cell.

- A resumed session journals one `cell-abandoned` record for a cell attempt
  that an interrupted session left without a checkpoint, then measures the
  cell again from its first pair. The abandoned attempt's executions stay in
  the log and never enter a checkpoint or receipt.
- P-11 accepts several `cell-start` records for a cell when every attempt
  before the last was abandoned exactly once, by a later session, after an
  `interrupted` record closed its own session. It rejects a cell that
  completes more than once, a restart without its abandonment record, and
  accepted samples other than the executions the last attempt journaled; a
  sample drawn from an abandoned attempt is named as such.
- The protocol text states that a `failed` session ends its campaign, as the
  journal already enforced, and that `interrupted` sessions resume.
- A version-4 receipt may cite a version-3 pilot as resolution evidence; both
  versions derive the pilot resolution identically.
- The addendum schema changes only its identity and protocol version constant.

Receipts that pin versions 1 to 3 are evaluated under the rules they pin, and
the evaluator refuses to evaluate them under version 4.
[verdict-preservation.json](../bdc507a3/verdict-preservation.json) compares
the verdict of every committed receipt under the evaluator before and after
this change. The version-3 AND-popcount pilot of `26465e6c`, cut by an
operator stop, keeps the rejection in its
[acceptance summary](../../bench_results/26465e6c/v3-and-popcnt-pilot/acceptance-summary.json).
The contract tests in `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs`
kill a campaign's session inside a cell and carry the resumed campaign through
finalization and acceptance.

Confirmation under v4 uses fresh samples. The per-candidate confirmatory
attempt cap counts attempts per protocol version (shared settings table), and
the family ledger's sequential attempt budget counts every earlier
reservation, so a version-4 confirmation after a version-3 confirmation
spends the next attempt's alpha.

The ledger section gains the voided-attempt rule, which the NR rate-matched
encoder family of `12fdeb5b` is the first to exercise.

- An attempt the executor aborts for a procedural defect in its own freeze or
  launch, before reading any of its results, may be voided. The executor keeps
  the aborted stage available and commits an attempt record naming the
  campaign, the addendum digest, the defect, the cells measured and unmeasured,
  and the abort.
- A voided attempt's reservation does not enter the chain the replacement
  attempt reserves on. It is therefore never interior to that chain, and the
  P-22 sentence about a removed interior attempt breaking its successor's link
  stays true by construction.
- A voided attempt spends no comparison and no candidate attempt. An attempt
  whose results were read is never voided and spends its reservation, so the
  rule cannot retire a losing measurement.
- The executor, not the tool, applies the rule. Acceptance behaviour is
  unchanged: P-22 validates the ledger prefix a receipt itself pinned and reads
  no live ledger, so the evaluator's verdict on every committed receipt is the
  same before and after this entry. The protocol version does not change.
