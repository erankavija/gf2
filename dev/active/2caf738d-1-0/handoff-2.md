# Handoff: gf2 1.0 release (2caf738d), steward session 2

**Date:** 2026-11-13
**Prior handoffs:** `dev/active/2caf738d-1-0/handoff.md`. Lead handoffs of this
session: `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-4.md` (lead `gf2-73`),
`dev/active/d77176e5-competitive-benchmarking/handoff-6.md` (lead `gf2-af`) and
`dev/active/b8206228-permanent-statistics/handoff-38.md` (lead `gf2-a6`).

## Current state

- Steward `gf2-f3` ran three peer leads from 19:50 to 23:15 local. All three stopped with clean worktrees and committed handoffs; the host is idle.
- Nothing is pushed.
- The steward armed the benchmark window on the owner's instruction: transient timer `gf2-bench-window-20261114` at 2026-11-14 02:00 local, `GF2_WINDOW_MAX_EST_MIN=0`. The queue is `dev/scripts/bench-window/queue.tsv` at `b09114607`. Two lines run: `a6039c72` in `jit-a6039c72-run` and `67194d70` in `jit-67194d70-run`. The other lines skip on done markers. The two `259acf37` rerun lines are commented out as `#PENDING-GATE`.
- Closed on main this session: `f21b25ac`, `0afbd34a`, `93df507a`, `595e2fe5`, `c6b20bec`.
- On main with a failed review: `66fefd9e` (two failures, rework 2 approved) and `da6200b8` (one failure, rework 1 approved; its two live defects are restored in `876c3c581`).
- Everything else is on branches; the lead handoffs list each branch and its state.

## Owner decisions of this session

- `9c0568ee`: `standard_u128_irreducibility_note` is renamed with a clean cutover.
- `243d1160`: design (b1), a safe inline enum in `BitVec`, admitted only when indications show no regression on matvec, transpose-matvec, LDPC batch decoding, OSD rows and long kernels. The choice followed a Codex `gpt-6-astra` analysis.
- `f21b25ac` REQ-04 is amended to accept a drafted upstream report; filing is `41517289`, an owner action.
- `2e6e3557`: option A. The workflow takes its feature set from `scripts/cargo-features.sh`, and clippy gains `tuning-profile`.
- `4443de83`: library first. REQ-01 stays; `9a1b24fc` adds the batch entry points to the DVB-T2 BCH code.
- `b8157e14`, the layered i8 floor on DVB-T2 R1/2, blocks 1.0.
- `92ec2e2b` REQ-02 stands: both W2 arms count, the faster per row sets the bar.
- Measured tuning profiles: "Remove such tests or tell me why they are not a defect." `5f33fb28` carries it.
- `66fefd9e`: the plan passages are rewritten in place, `b8206228`'s `breakdown.json` included, as a one-time exception. The owner's words: "Only this time to prevent harmful 'evidence' accumulation in the future. Otherwise we don't edit old plan files etc."
- `506f76d4` merges on passing gates; it removes the `tuning-campaign-validator` step from `scripts/cargo-ci.sh`.
- History rewrite: it runs after the live branches land, on the owner's go. All stale branches are deleted first, the twelve doubtful ones included; the verified bundle keeps them. Every non-conforming subject in `origin/main..main` is reworded.
- `c34e8203`: a versioned commit-msg hook is approved.
- Gating: "max throughput". No test lock; independent gates, reviews and workers run concurrently; overlapping test runs use `BUSY_HOST_OVERRIDE=1` and record it.
- Owner decisions reach leads through the steward. A lead asks the owner directly only when a session refuses the relayed action.
- The steward may arm the window timer when the owner says so.

## Steward rulings that bind the leads

- Every gate passes on the branch before the merge. Nothing lands against a failing or pending review.
- `259acf37`: the flooding reference follows each code's layered normalization (`layered.rule()`); the superseded NMS 0.75 curve stays recorded. Its rerun line runs from a worktree pinned to the branch head, after cargo-ci passes there.
- `d5cd53e0`'s conclusions are provisional until both its reviews pass.
- `da6200b8`: choices C1a to C4a, with a named writer-helper list in the checker configuration. The checkers catch mistakes; they carry no version, ledger or schema.
- `0fe85425`: no dependency from `crates/` to `dev/tools`, dev-dependencies included. The layout-asserting test in `grand_phase1_smoke.rs` is removed.
- `32a8bc0e` lands as one change after `gf2-af`'s review (given) and a BCH batch-encode indication before and after.
- The BCH chain stacks branches: `6379e04e`, `7fbd1042`, `9a1b24fc`, `92ec2e2b`, `2bca2b28`; `243d1160` stacks on `6379e04e`.

## What to do next

1. Read `.agents/bench-window/window.log` from its last `window start` line. Have the owning leads record the `a6039c72` table and the `67194d70` validation record's location and digest.
2. Put the open owner questions below to the owner, then brief the leads the owner assigns.
3. Gates first: the tech-debt branches of handoff-4 are reworked or complete and need only gates.
4. `66fefd9e` rework 2 needs an independent inventory by the other vendor before its review. A third failure stops the work and goes to the owner.

## Traps

- A relayed owner decision that changes a success criterion is refused by a lead session's permission classifier, and the steward's own approval of a criterion change is refused too. Put the question to the owner and make the edit in the session where the owner answered.
- Every `cargo` on this host joins one jobserver pool of 24 tokens; `CARGO_BUILD_JOBS` does nothing under it. The pool did not bound the load: with gates running concurrently in several worktrees the one-minute load reached 88. The cause is unattributed. Capture `ps` before blaming compilation.
- A lead that merges before gating puts review findings on main. Two issues did so this session.
- Check commit subjects before each merge until `c34e8203` lands: `wip` types, default merge subjects and 72-character subjects entered main this session. The rewrite's reword list covers them.
- `cd` into a worktree in a Bash call moves the session's working directory.
- `systemd-run --user` from a session shell needs `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` set.
- Sweeps by one agent miss wording forms. Slice by directory and reconcile two independent inventories.

## Open questions for the owner

- The work split when sessions restart.
- `fd9d5416`: terms for rework after its second doc-review failure. The lead proposes fixing the two findings as stated.
- `259acf37` F2: whether the consistent-Gaussian LLR symmetry derivation of the guard multiple counts as structural. If it does not, F2 moves to `b8157e14`.
- `d5cd53e0`: its Background sentence "near 1 dB" has no source; drop it or replace it with the measured NMS 0.875 transition.
- `5f33fb28`: nine `tuning-extent-campaign-driver.rs` tests read the `a83583e0` owner through the `dbd8787d` declaration (synthetic owner, or retire the declaration); and whether the `aff3ct-ldpc-gaps` tuned arm keeps its included coding profile.
- The eleven reword proposals of the Codex review, and the rewrite go once its preconditions hold.
- Whether concurrent gating stays at "max throughput" given the load of 88.
- `41517289`: file the amdgpu report upstream.
- Container for `b8157e14` and `3461a52a`, now under `d77176e5`.

## References

- Charter: `dev/research/2caf738d-charter.md`.
- Rewrite runbook: `dev/active/b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md`.
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`.
