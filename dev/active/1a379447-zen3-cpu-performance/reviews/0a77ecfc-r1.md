## Review: Declare the decoder calibration campaign (0a77ecfc)

**Verdict:** PASS

### Gate status

cargo-ci, code-review and research-review passed on the merged tree `1a4182001`.

### Prior-findings regression table (Tier 1.5)

| Round | Finding (1-line) | Status at HEAD | file:line |
|-------|------------------|----------------|-----------|
| code-review R1 | procedural narration in four script headers | closed | `calibration/rehearse-campaign.sh:2`, `make-calibration-producing-inputs.py:2`, `make-protocol-tables.py:2`, `record-producer-reports.py:2` |

### Success criteria

- [x] REQ-01 — `calibration/campaign-declaration.json` names issue `0a77ecfc`, the coding owner as measured with its cell count, and the core and algebra owners by envelope path and digest; the validator's declaration check accepts it.
- [x] REQ-02 — `calibration/premeasurement-protocol.md` amends the seam protocol by reference and states fields, grid, extent decision, seed derivation and declared counts; its tables are generated.
- [x] REQ-03 — `calibration/producing-build-inputs.json` with `producing-build-inputs.sha256`.
- [x] REQ-04 — `calibration/producer/` holds the capability report, grid listing, self-check and the producer identity.
- [x] REQ-05 — one line for `0a77ecfc` in `bench-window/queue.tsv` with its estimated minutes.

### Stale-narrative sweep (Tier 2.5)

Zero matches.

### Deferred-items audit (Tier 2.75)

The protocol document carries no deferral. Three comment trims wait for the campaign's evidence because the campaign or the rehearsal record pins the files by digest: `calibration/run-campaign.sh`, `calibration/rehearsal/private-lock.sh`, `calibration/rehearsal/untimed-windows.patch`. No criterion names them; `@/issue/20af7e6e` collects the campaign.

### Holistic findings

The driver has no untimed mode; the end-to-end record `calibration/rehearsal/record.json` comes from a throwaway clone with two committed stand-ins and states that limit. The campaign runs the tree of `.agents/worktrees/agent-0a77ecfc`.
