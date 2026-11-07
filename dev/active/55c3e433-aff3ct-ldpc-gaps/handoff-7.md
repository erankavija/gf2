# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 5

**Date:** 2026-11-08
**Session number:** 5
**Prior handoffs:** [handoff.md](handoff.md), [handoff-2.md](handoff-2.md), [handoff-3.md](handoff-3.md), [handoff-4.md](handoff-4.md), [handoff-5.md](handoff-5.md), [handoff-6.md](handoff-6.md)

## Current state

- Epic: `55c3e433`, state `backlog`.
- Wave in progress: wave 6 (`5f401907`, `in_progress`, assignee `agent:worker`). No worker is running.
- Children summary: 7 done, 1 in_progress, 0 rejected.
- Open escalations: none.
- Timer armed: `gf2-bench-window-20261108`, 2026-11-08 04:00 Helsinki. The queue runs only `5f401907` from `prepared-3/window.sh`. The owner keeps the host idle from about 03:30.

## What just happened

- The 2026-11-06 window ran pilot 2. Its receipt `f47929f8…71da` is accepted, qualifies false, with no findings.
- The freeze refused at resolution 0.75. The 1-minute load guard passed at 0.75, but the 5- and 15-minute loads were 2.80 4.99. The widest interval was `dvb-t2-r12-p12-b8`, [1.33, 2.26].
- Evidence preserved in `.agents/campaign-evidence/55c3e433/adopted-vs-aff3ct/run-2/`.
- The owner approved a third pilot. Claude `sonnet` amended the declaration: `max_pilot_trials_per_cell` 3, campaign `55c3e433-adopted-vs-aff3ct-pilot-3` with seed 1589200133, and pilots 1 and 2 with their refusals stated. The lead merged the change at `0c3f43b45`.
- `prepared-3/` holds the job, which refuses a timed stage when the 1- or 5-minute load exceeds 1.0 (exit 4). The pinned builds and quality files are byte-identical to `prepared-2/`.
- Pilot 2 ratios (gf2 time / AFF3CT time): DVB w1-b1 1.97, DVB p12-b8 1.38, NR w1-b1 0.11, NR p12-b8 0.30, DVB w1-b8 0.38, NR w1-b8 0.33.

## What to do next

- [ ] Read `.agents/bench-window/window.log` and the job's `.out` file. Preserve the family target in `run-3/`.
- [ ] Remove the spent `5f401907` queue line and commit.
- [ ] On rc=0, follow handoff-6's rc=0 steps, naming pilots 1 and 2 and both refusals.
- [ ] On exit 4, nothing was measured and no pilot trial was spent. Rearming needs the owner's approval.
- [ ] On another refused freeze, the pilot budget is spent: escalate.

## Traps — do not repeat these

The traps in [handoff-6.md](handoff-6.md) and earlier stay in force.

- **Do NOT trust the 1-minute load alone.** On 2026-11-06 it read 0.75 while the 15-minute load was 4.99, and the freeze was refused.
- **Do NOT call `systemd-run --user` without the user bus.** From the agent shell, set `XDG_RUNTIME_DIR=/run/user/$(id -u)` and `DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus`.
- **Do NOT point the job at `pilot-2/`.** Pilot 2's receipt pins that path. Pilot 3 uses `pilot-3/`.

## Reference artefacts

- Declaration `experiments/aff3ct-ldpc-gaps/families/55c3e433-adopted-vs-aff3ct.json` at `3bd70555f`
- `.agents/campaign-evidence/55c3e433/adopted-vs-aff3ct/run-2/`, `prepared-3/`
