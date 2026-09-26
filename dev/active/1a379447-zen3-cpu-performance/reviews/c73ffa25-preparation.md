# Review: dense baseline preparation (c73ffa25)

**Verdict:** FAIL; required measurement evidence is pending the scheduled window.

## Gate status

Merged-tree cargo-ci passes at `efaea4eaf`. Code-review
`76ce431c-bc47-4942-b897-0e0c7f9c5bad` at `a2d9884cd` reports three blocking
requirements: F1, absent baseline receipts; F2, absent completed profile; F3,
absent profile-supported portfolio or no-candidate decision. Research review
remains pending. None of these requirements is waived.

## Prior-findings regression table (Tier 1.5)

This is the first review. All three findings remain open pending window evidence.

## Success criteria

- [ ] REQ-01: Both baseline receipt collections are queued; raw samples, logs and acceptance interpretation require execution.
- [x] REQ-02: Release semantic oracles and shared staged smoke cover parity, lengths, tail padding and boundary cases; smoke records zero timing samples.
- [ ] REQ-03: Release assembly is committed. Repeated profile attribution requires the queued run.
- [ ] REQ-04: The portfolio requires the profile and baseline evidence.
- [ ] REQ-05: The no-candidate decision, if warranted, requires that same evidence.
- [ ] REQ-06: Canonical boundaries are preserved in preparation; the eventual bounded portfolio remains to be recorded.

## Stale-narrative sweep (Tier 2.5)

The preparation note identifies the receipts, profile and portfolio as pending.
These statements match the current state. The M4RI queue hold names its actual
owner, `50f0bd42`.

## Deferred-items audit (Tier 2.75)

The preparation note's pending profile and portfolio are in-scope unfinished
requirements. They prevent closure. The external M4RI comparison belongs to the
downstream measurement task and remains disabled here.

## Holistic findings

The profiler uses the existing arm and canonical timed-request builder. It
collects counters and sampled hot data over nine repetitions, records terminal
status, and renders intervals. Host perf help/list checks select supported
events. The two baselines and subsequent profile are queued from the clean
worker at `815010276`. No measurements or candidate selection were made during
preparation.

## Required changes

1. Collect and preserve both baseline receipts in the scheduled window.
2. Collect the repeated profile and interpret its attribution with the release assembly.
3. Freeze one bounded candidate portfolio or record the supported no-candidate outcome, then rerun the configured gates.
