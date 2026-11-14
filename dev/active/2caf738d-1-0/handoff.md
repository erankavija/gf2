# Handoff — gf2 1.0 release (2caf738d) — steward session 1

**Date:** 2026-11-12
**Prior handoffs:** none for the steward. Lead handoffs: the d77176e5 lead
handoffs are retired; its latest is `dev/active/d77176e5-competitive-benchmarking/handoff-6.md`. Also
`dev/active/b8206228-permanent-statistics/handoff-35.md` (lead `gf2-79`; retired by @/issue/d1c0e062).

## Current state

- The steward role follows the `jit-project-lead` skill. The charter is `dev/research/2caf738d-charter.md`, linked to `2caf738d`, with decisions D-1 to D-6 ratified by the owner on 2026-11-12.
- Milestone `2caf738d` is `backlog`. Its open dependencies are `0fc3c9d0`, `7f809931`, `86b9c719`, `9d8cb409`, `b117c95e`, `b81c239c`, `d77176e5` and `f0448e55`; D-3 keeps all of them in 1.0.
- Two peer sessions led work today. Their handoffs hold the issue-level state.
  - `gf2-7b`: the `d77176e5` review findings and the close of epic `ae03bcd0`.
  - `gf2-79`: `45ecf975`, `daef2dc3` and `4f21da96`.
- Both leads reported idle with their handoffs committed; nothing is running on the host.
- Nothing is pushed. `main` is far ahead of `origin/main`; the history rewrite of `45ecf975` REQ-04 precedes any push.
- The steward session committed only the charter (`6e03fe1ef`, `dc04db327`) and this file.

## What just happened

- The night window of 2026-11-11 ran all five jobs with exit 0. `gf2-7b` recorded its tables in `de311215`, `040bc2b8`, `befccab3`, `c9cb0dd2` and `be113cbf`.
- Closed today: `07a1813a`, `069e3c33`, `de311215`, `040bc2b8`, `8a8a3bc9`, `befccab3`, `c9cb0dd2`, `1ce7e8a5`, `cbd1eb28`, `4f21da96`, `1aea3958`.
- Every GF(2^8) cell is at 1.0 or above after an alignment fix in the AVX2 axpy kernel (`c9cb0dd2`).
- `fd9d5416` falsified its aspirational REQ-03 on an indication. The gaps are milestone gaps under D-2 and are filed as `60c82c4c` (bchlib, B2), `5e71e993` (AFF3CT, B1) and `92ec2e2b` (M4RI and AFF3CT, W2).
- `4f21da96` removed the cargo budget wrapper script; `AGENTS.md` states the current commands.
- Owner decisions of the day outside the charter:
  - Nothing reaches the remote without the owner's confirmation.
  - A lead may arm a short benchmark window once its queue is merged, the host is quiet and every lead's workers are idle. Long lines run at night.
  - Subagents get a model matched to task difficulty, never Fable.
  - `07a1813a` REQ-01 and `259acf37` REQ-05 were reworded; the reasons are in the issues.
  - `fd9d5416` REQ-02 is read literally: the long pre-cutover cells are measured, not projected.
  - `daef2dc3` got one class-wide rework round. A further code-review failure stops the work and returns to the owner.
  - Untracked and ignored measurement leftovers in the main checkout were deleted: the old benchmark, simulation-result and study directories under `dev/`, and the directory of the done epic `1a379447`.

## What to do next

1. Confirm the owner armed the timer `gf2-bench-window-20261113a` for 2026-11-13 02:00 local. If it did not fire, the queue in `dev/scripts/bench-window/queue.tsv` is intact and needs a new timer from the owner.
2. After the window, read `.agents/bench-window/window.log` from its last `window start` line. Expected jobs in order: `67194d70` validation, the `de311215` decode table and route profiles, the `fd9d5416` cutover timing, the `259acf37` DVB-T2 FER curve.
3. Have the owning lead record each table in its issue with host and executable digest. `gf2-79` records the validation record's location and digest in `67194d70`; the record is under `.agents/campaign-evidence/67194d70/validation` in the main checkout.
4. Accept `fd9d5416` and `259acf37` when their gates pass on the window tables, then take epic `ae03bcd0` through its gates and criteria mapping.
5. `daef2dc3`: the class-wide round is on branch `worktree-agent-daef2dc3-c`, twelve commits ahead of `main`, unmerged and ungated; an independent audit ran on it and its findings are fixed there. About 33 tracker-text faults remain, and the `b8206228` lead handoff describes regenerating them. Accept it when its code-review passes; that releases `3478cb98`, `506f76d4` and `595e2fe5`, then `45ecf975` up to the rehearsed history rewrite.
6. `ebfaff31` is merged and its doc-review failed with five findings on `docs/reference/performance-evidence.md`; the `d77176e5` lead handoff lists them. After the rework, check coherence: permanent pages name harnesses and methods and hold no measured ratio (D-6).
7. Next efforts in order: the three BCH gap issues and `a6039c72`; epic `b8206228` once `45ecf975` is done; epic `b4b4b9ee`, starting with `32a8bc0e` and `e29fd776`.

## Traps — do not repeat these

- A lead session's permission classifier refuses `systemd-run --user`. The owner arms timers; a peer session must not do it on the lead's behalf.
- A window timer that checks a run worktree out at `main` runs the merged code only. Confirm a line's branch is merged before the timer fires.
- Each `.out` file under `.agents/bench-window/` is appended across windows; the last table is the latest run.
- The runner waits for a quiet host only at window start. Jobs after a many-worker job start under that job's decaying load average.
- `echo ====` fails in zsh; quote separator strings.
- A `jit` write from the main checkout leaves `.jit/events.jsonl` modified beside other sessions' tracker changes. Commit only your own issue file and tell the lead whose tracker commit will carry the event.
- A lead running a `cargo-ci` gate on the main checkout needs a commit freeze on `main`; ask before committing there.

## Open questions needing invoker input

- The force-push of the rewritten history (`45ecf975` REQ-04, `daef2dc3`) and every ordinary push wait for the owner.
- `67194d70` still states "receipt committed"; `3478cb98` amends it after `daef2dc3`. Confirm the amended wording when the lead proposes it.
- `gf2-79`'s handoff raises two more: the measured tuning profiles kept because tests read them, and 46 plan passages that still tell workers to commit receipts.
- `a6039c72` records two GF(2^8) cells that fell between windows while staying above their references. Priority is unset by the owner.

## Reference artefacts

- Charter: `dev/research/2caf738d-charter.md`.
- Dashboard of ratios, private: `https://claude.ai/artifact/4ZoceRZh2aYohYxaumogP7`.
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`.
- Harnesses: `experiments/encode-compare/cutover.sh`, `experiments/permanent-statistics/validation-window.sh`.
