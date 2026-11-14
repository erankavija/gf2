# Handoff: gf2 1.0 release (2caf738d), steward session 3

**Date:** 2026-11-14
**Lead handoffs:** one current handoff per epic directory, written at this
wind-down: `dev/active/b4b4b9ee-tech-debt-2026-06-30/` (lead `gf2-5b`),
`dev/active/d77176e5-competitive-benchmarking/` (lead `gf2-d7`, also epic
`ae03bcd0`) and `dev/active/b8206228-permanent-statistics/` (lead `gf2-e9`,
the `45ecf975` stream). They hold the issue-level state.

## Current state

- Steward `gf2-42` ran three peer leads from 12:30 local. The owner ordered a wind-down of all work at about 14:50; this file was written before the leads' final reports arrived. Read each lead handoff for what landed after this point.
- Nothing is pushed. Every push waits for the owner.
- Closed on main today: `5a3abee1`, `1b7228b9`, `d5cd53e0`, `259acf37`, `a6039c72`, `fd9d5416`, `bb28cf4f`, `32a8bc0e`, `5c1c87b7`, `51f299d2`.
- Benchmark window armed on the owner's word: transient timer `gf2-bench-window-20261115` at 2026-11-15 02:00 local, `GF2_WINDOW_MAX_EST_MIN=0`. Its command creates or detaches `.agents/worktrees/jit-3461a52a-run` at `main`, then runs `dev/scripts/bench-window/run-window.sh`. Two `3461a52a` lines run, about 155 minutes (estimate); the other lines skip on done markers.
- The tracked tree was at its byte ceiling at midday. History retirement freed about 307 kB; `d1c0e062` (337 kB) and `506f76d4` (804 kB) were in gating at the wind-down.
- The disk filled at about 14:35: `.agents/` held 319 GB of worktree build output. The steward removed `target/` in 23 worktrees of closed issues; 167 GB was free afterwards. Results from roughly 14:15 to 14:40 are suspect and are rerun.
- Tracker split: a planning lead (a subagent of the steward) left a draft on branch `worktree-agent-ae312a8f6f84377dd`, head `e432a6af7`, unmerged: a plan with decisions D-01 to D-10, a seven-task breakdown and an investigation record for container `6ba9be17`. Two adversarial reviews failed it (7, then 4 blocking findings); all are addressed at the head, the last four without a re-review. The plan-review gate has not run and the manifest is not applied.

## Owner decisions of this session

- `fd9d5416`: fix both doc-review findings as stated, without unnecessary verbosity. Done.
- `259acf37` F2: the LLR-symmetry basis of the guard multiple is accepted as structural; `47ec8c01` tightens the rustdoc.
- `d5cd53e0`: the Background states the measured 1.0 to 1.2 dB transition at NMS 0.875.
- `5f33fb28`: the `dbd8787d` declaration is retired with the nine tests that read through it. The `aff3ct-ldpc-gaps` tuned arm keeps its coding profile, as experiment code and gf2's counterpart to a reference's fastest configuration.
- History rewrite: the Codex review's reword proposals replace the drafts; one exceeded 71 characters and takes the 70-character form the lead recorded.
- `b8157e14` and `3461a52a` stay under `d77176e5`. Epic `ae03bcd0` goes to the `d77176e5` lead; its completion gates run after the BCH half of `9a1b24fc` lands.
- `9a1b24fc`: one batch-encoding trait is the only form. `BchCode` loses its layout-taking batch methods with no alias, and the LDPC and Richardson-Urbanke batch encoders converge on the trait in the same issue. REQ-06 has no exemption: the pre-cutover arm of `experiments/encode-compare` is removed.
- `3461a52a` REQ-02 stands. Literature first, then measurement; a table of the density-evolution optimum per code, rate and iteration cap, produced by an in-tree offline tool and caller-overridable, satisfies "a structural rule" if window curves confirm it. The same order applies to `b8157e14` REQ-03.
- Superseded history files are retired, not edited: each `dev/active/` directory keeps one current handoff with its live traps.
- Tracker: `.jit/` leaves the tree budget as an interim step (`fe2221d5`), and the tracker moves to a separate repository mounted as an independent checkout at `.jit/`, ignored by the code repository. A submodule, an orphan branch, a subtree and staying in-tree were rejected.
- `574ed420` (rustc under cargo-ci joins the host cargo pool) started at once.
- `41517289`, filing the amdgpu report upstream, stays an owner action, deferred.

## Steward rulings that bind the leads

- cargo-ci is capped at two concurrent runs host-wide until `574ed420` passes its three-concurrent load measurement. A run that hits the 900 s gate timeout under load is rerun and is no failure against the issue; a warm pre-run is the retry, never the first attempt.
- When an issue closes, its lead removes that worktree's `target/` in the same wave and reports free space. Below 40 GB free, builds stop.
- Before each merge the lead computes its net byte change; a merge the budget refuses is aborted and reported.
- A docs-mechanical failure caused only by retired invariant addresses in old history files is no failure against an issue until `d1c0e062` lands.
- `574ed420`: the wrapper passes the pool's fifo through a generated per-toolchain `rustc` shim; it changes no CI step or default.
- `da6200b8` rework 2 is the last the steward approves: the written-name rule is a positive list. A third code-review failure goes to the owner, with ruling contrived inputs out of scope as an option.
- `9a1b24fc` order: `32a8bc0e` and `6379e04e` as they are, then the BCH half and the trait on its own cargo-ci and code-review, then `243d1160` and `7fbd1042` on top; the LDPC half in parallel; the issue closes on the LDPC half's gates. Its design is in rework 2 after two review failures; a third comes to the steward before the last attempt.
- `697b9178` lands before `243d1160` is rebased. `a33fda32` depends on `9a1b24fc` and is not folded into it.
- For a Codex-implemented change the lead adds a blocking Claude review beside the Codex code-review gate.
- Leads set each subagent's model by task difficulty, never Fable, and alternate Claude and Codex per dispatch.

## What to do next

1. Read `.agents/bench-window/window.log` from its last `window start` line. The `d77176e5` lead records both `3461a52a` curves with digest, host and start loads; a curve that contradicts the 0.90 estimate is recorded as a falsification.
2. Collect the three leads' wind-down states from their handoffs. Then take the tracker-split draft: re-review its last four fixes with another vendor, put the six questions below to the owner, run plan-review, land the branch, apply the manifest, and log the tracker decision in the charter as the next `D-N`.
3. Finish `574ed420` first: its timing measurements, gate, the run without sccache, and the three-concurrent run. Then restore or lift the cargo-ci cap.
4. Land `506f76d4` and `d1c0e062` when their gates pass, then the tech-debt branches awaiting gates and `6379e04e`.
5. Ask the owner whether `fe2221d5` is still wanted once the tracker split has a date, and when the split runs relative to the history rewrite.

## Traps

- sccache gives every `rustc` a private jobserver, so the host cargo pool bounds processes and not codegen threads: three cargo-ci runs reached a load of 69. `574ed420` fixes it.
- Each worktree keeps its own `target/`, 5 to 60 GB. Nothing removes it but the rule above.
- The pre-commit budget check counts the whole tree, on branches too, and every tracker operation grows `.jit/events.jsonl`.
- `systemd-run --user` needs `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` set, and `--setenv=PATH="$PATH"`: the user manager's `PATH` has no cargo.
- The arming command, not the runner, creates or detaches run worktrees at `main`. Confirm a line's code is on `main` before the timer fires.
- A relayed owner decision that changes a success criterion can be refused by a lead session's permission classifier; the edit is then made in the session where the owner answered. A lead's refused destructive command goes to the owner, never to another session.
- A `cd` into a worktree inside a Bash call moves the session's working directory.
- A `jit` write from the main checkout leaves `.jit/events.jsonl` modified beside other sessions' lines. Commit only your own files.
- Sweeps by one agent miss wording forms. State a sweep as a rule plus a search-generated inventory, and have a second vendor rerun the search.
- Check commit subjects before each merge until `c34e8203` lands: closed type list, `jit:<id>` scope, at most 71 characters, counted.

## Open questions for the owner

- The force-push of the rewritten history and every ordinary push.
- Whether a successor takes the `45ecf975` stream when work resumes.
- Tracker split, from the draft plan:
  - Timing. The draft recommends cutting over inside the history rewrite's quiet window, before its extraction step, so the rewrite drops the tracker paths from history.
  - New wording of `@/inv/resolvable-references`: a fresh clone resolves tracked text "alone or with the tracker checkout mounted at `.jit/`".
  - Landing check: a gate verdict counts only for the commit it ran on (the epic's REQ-04 as written), or also when the gate's declared inputs are identical. The draft recommends the amendment.
  - Document links from unmerged branches fail `jit validate` in every other checkout once the store is shared. The rehearsal tests commit-pinned links; failing that, link after merge or request a jit change. The draft calls this the largest operational risk.
  - The epic's REQ-06 covers "every gate checker and script"; vendored skill scripts read `.jit` under their checkout. The draft recommends narrowing the criterion to repository-owned scripts.
  - The tracker repository's remote URL, and who adds the tracker paths to the rewrite's path list in `45ecf975`.

## References

- Charter: `dev/research/2caf738d-charter.md`.
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`.
- Rewrite runbook: `dev/active/b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md`.
