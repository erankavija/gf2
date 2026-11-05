# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 4

**Date:** 2026-11-06
**Session number:** 4
**Prior handoffs:** [handoff.md](handoff.md), [handoff-2.md](handoff-2.md), [handoff-3.md](handoff-3.md), [handoff-4.md](handoff-4.md), [handoff-5.md](handoff-5.md)

## Current state

- Epic: `55c3e433`, state `backlog`.
- Wave in progress: wave 6 (`5f401907`, `in_progress`, assignee `agent:worker`). No worker is running.
- Children summary: 7 done, 1 in_progress, 0 rejected.
- Open escalations: none.
- Timer armed: `gf2-bench-window-20261106`, 2026-11-06 04:00 Helsinki. The queue runs only `5f401907` from `prepared-2/window.sh`.

## What just happened

- The 2026-11-05 window ran the comparator pilot `55c3e433-adopted-vs-aff3ct-pilot`. Its receipt `1065c2cc…6ccdb` is accepted, qualifies false, with no findings.
- The freeze refused: equivalence 1.1 does not exceed 1 + 0.48. Cell `nr-bg1-z384-p12-b8` has pair ratios 0.106–0.322. Load at window start was 4.15 7.06 7.37; earlier windows started at 0.25, and their pilots had half-widths of 0.012 or less. The job exited rc=1 before the confirmation.
- Evidence preserved in `.agents/campaign-evidence/55c3e433/adopted-vs-aff3ct/run-1/`: the family target directory and the job output.
- The owner chose a re-pilot. Codex `gpt-6.1-sol` amended the declaration: `max_pilot_trials_per_cell` 2, campaign `55c3e433-adopted-vs-aff3ct-pilot-2` with seed 3264351777, and pilot 1 with its refusal stated. The projections were regenerated, and the lead merged the change at `a7f51916f`.
- `prepared-2/` holds the job, plan, smoke and inputs. The job refuses to start a timed stage when the 1-minute load exceeds 1.0, with exit 4. The pinned executable digests are unchanged.
- Pilot 1 ratios (gf2 time / AFF3CT time): DVB w1-b1 1.94, DVB p12-b8 1.79, NR w1-b1 0.19, NR p12-b8 0.29, DVB w1-b8 0.39, NR w1-b8 0.32.

## What to do next

- [ ] Read `.agents/bench-window/window.log` and the job's `.out` file.
- [ ] Remove the spent `5f401907` queue line in `dev/scripts/bench-window/queue.tsv` and commit.
- [ ] On rc=0: generate the REQ-02 table from both receipts outside the tree. Write the closing record (not named `*outcome*` or `*receipt*`) covering REQ-02 to REQ-05. Name pilot 1 and its refusal, and state that the quality files came from `545a3c814`. Then run `cargo-ci`, `code-review` and `research-review` on `5f401907`.
- [ ] On exit 4 (busy host): the job did not measure. Rearming needs the owner's approval.
- [ ] Then complete the epic per Section 10.

## Traps — do not repeat these

The traps in [handoff-5.md](handoff-5.md) and earlier stay in force.

- **Do NOT point the job at `$family/pilot`.** Pilot 1's receipt pins `target/aff3ct-ldpc-gaps/55c3e433-adopted-vs-aff3ct/pilot/addendum.json`. Pilot 2 uses `pilot-2/`.
- **Do NOT treat the projection's `pilot/addendum.json` path as a defect.** `make-family.py --configuration` emits that fixed relative path.
- **Do NOT read exit 4 as a result.** It is the load guard refusing to start; nothing was timed in that stage.
- **Do NOT arm a window while a session is loading the host.** The 2026-11-05 interference came from load at 04:00.

## Reference artefacts

- Declaration `experiments/aff3ct-ldpc-gaps/families/55c3e433-adopted-vs-aff3ct.json` at `b586b34eb`
- `.agents/campaign-evidence/55c3e433/adopted-vs-aff3ct/run-1/`, `prepared-2/`
