# Handoff — Technical debt assessment remediation (2026-06-30) (b4b4b9ee) — session 1

**Date:** 2026-10-09
**Session number:** 1
**Prior handoffs:** None.

## Current state

- Epic: `b4b4b9ee` — state: backlog (assigned to `agent:jit-execution-lead`, assign-only).
- Wave in progress: wave 1 of 3 (wave 1 holds every issue without an open sibling dependency).
- Children summary (issues labelled `epic:tech-debt-2026-06-30`): 23 done, 4 in_progress, 138 ready, 10 backlog, 1 rejected. This session closed 17 and rejected 1.
- Active claims (all `agent:worker`, no worker running):
  - `3464c241` — branch `worktree-agent-3464c241` at `35480688f`, worktree `.agents/worktrees/agent-3464c241`, **not merged**.
  - `dfb8820b` — merged, then reverted on `main`; branch `worktree-agent-dfb8820b` kept.
  - `dd493649` — merged; `cargo-ci` passed, `code-review` failed.
  - `fbcadf17` — merged; `cargo-ci` passed, `code-review` failed twice.
- Open escalations: four, listed under "Open questions needing invoker input".
- External blocker: `bfc2ceab` (zen3 epic, in progress) blocks `a57cb8d0` and `07fe99b8`; `1ffa72fc` and `ad978596` touch the same kernel entry points and are held back by the lead.
- Progress file: [progress.json](progress.json) (reflects the above; `created_during_execution`, `escalations`, `surfaced_pitfalls`, `batches`).

## What just happened

- Attached 15 unlabelled bugs and 10 orphan issues to the epic, amended its scope and REQ-03 (invoker-approved), commit `d5b6bd2f8`.
- Wrote the wave plan; ran four dispatch batches, at most eight worktree workers at once (invoker decision), Opus for hard issues and Sonnet for bounded ones.
- Gates: added the invoker-approved gate sets to 41 gateless issues. The invoker asked to be consulted before any further gate change.
- Reviewer models changed (invoker-approved): `code-review`, `doc-review`, `plan-review`, `breakdown-review` on `gpt-6.1-sol`; `research-review`, `holistic-review` on `gpt-6-astra`. `codex-cli` upgraded 0.157.0 → 0.162.0 (0.157.0 refuses `gpt-6.1-sol`).
- Closed, all gates passed: `6bb13c31`, `6b5f4f3c`, `df144ecf` (no-claim branch), `79a62102`, `9600eb56` (rework 1), `33057520` (lead fixed two doc findings), `0657c9fa` (rework 1), `8342487d`, `2a9cc72c` (lead fixed three rustdoc findings), `f556d663`, `c7c11e7e` and `f0228940` (shared rework 1), `b2520e0f` (lead-requested touch-up keeping an unrolled fold for N in 22..=31), `9d474049`, `5c62bb1c`, `1fe6e218`, `d1bfe986`.
- Rejected `45a962da` (invoker decision): the five tests run in about 6 ms; ignore tags reverted.
- `dfb8820b`: worker deleted two schema-v2/v3 profiles; both reviews failed because `tuning_profiles/*.sha256` and zen3 producing-input records pin them by path and digest. Merge reverted in `fix(jit:dfb8820b): restore the receipt-pinned core tuning profiles`.
- `dd493649`: merged with `5c62bb1c` (one worker, two commits). Review F1 (high): a multi-frame seeded call still crosses its reserved stride. F2: the regression runs with debug assertions in CI.
- `fbcadf17`: two reviews failed on REQ-01. The substituting arm is unreachable; the reviewer reads the criterion's other branch (reject with evidence) as the applicable one and notes `DvbT2Concat::new` never returns `Unsupported`.
- `3464c241`: REQ-02 delivered; REQ-01 unmet literally. No 5G NR code probed (BG1 and BG2, 42 lifting sizes up to 176) has non-identity `systematic_cols`; sizes 192..=384 unprobed.
- Filed 7 follow-ups (`e29fd776`, `8a8a3bc9`, `bbbaa9ee`, `9d474049`, `c318a817`, `a34ef1c2`, `f0228940`); two are closed. The invoker then set a convergence rule (see Traps).
- Disk filled to 590 MB free mid-session. Reclaimed 21 merged worktrees, deleted the cache pool (invoker instruction); 109 GB free at close.
- `/tmp` cleaned (invoker request): about 142,000 test scratch entries, stale gf2 Claude session dirs, four campaign run dirs.

## What to do next

- [ ] Get the invoker's answers to the four open questions, then act on `dfb8820b`, `fbcadf17`, `3464c241`.
- [ ] Rework `dd493649` (attempt 1): make a multi-frame seeded call stay inside its stride or be refused in every build, and add a test that runs without debug assertions in the gate (a `--release` step or a `cfg(not(debug_assertions))` arm the CI contract executes). The worker's note on per-frame reseeking (it changes the noise of multi-frame CPU batches) is in the `5c62bb1c` return recorded under `surfaced_pitfalls`.
- [ ] Start a disk watchdog before dispatching, and reclaim each worktree when its branch merges (`rm -rf <worktree>/target` first).
- [ ] Continue wave 1 by priority. Remaining high priority: `1ffa72fc`, `ad978596` (after `bfc2ceab` lands). Then normal-priority bugs: `d5afe5aa`, `a44566c0`, `17559b23`, `01877f76`, `5a3abee1`, `d73ac633`, `22bb2dcd`, `3a5894ed`, `9ff05f12`, `8d5b7ad7`, `37235fd6`, `e29fd776`, `8a8a3bc9`, `c318a817`, `a34ef1c2`.
- [ ] Fold these recorded small items into the next issue that touches the same file instead of filing them: the `compute_lut_checksum_gpu` allocation-lifetime sentence (next HIP issue), `bbbaa9ee` (next `gf2-kernels-simd/src/gf2m.rs` issue), the `Scheduler` rustdoc attribution at `executor/scheduler.rs:287`.
- [ ] Queue a bench-window line for `crates/gf2-core/benches/fp_specialized.rs` to check the Mersenne-31 path after `b2520e0f`.
- [ ] Scan for open design choices before dispatching these: `252814d8`, `ca9a5194` (decide), `b9238d44`, `726bfeee`, `0ecf6fcd`, `774f2de4` (either/or), and the measurement issues `b9a352b4`, `e046803d`, `720f5f74`, `1e272575`, which need the overnight window.

## Traps — do not repeat these

- **Do NOT run `code-review` or `doc-review` before `cargo-ci` has passed.** Reviewers fail an issue whose `cargo-ci` is pending (`45a962da` F2, `6bb13c31` summary). Order: merge, `cargo-ci`, then reviews in parallel.
- **Do NOT run a `cargo-ci` per merge.** A run on a changed tree took 11 min 40 s, a second issue on the same tree 1 min 40 s. Merge the batch, then `jit gate evaluate-many cargo-ci <ids>`.
- **Do NOT merge, edit or commit source while a `cargo-ci` gate is running on `main`.** The gate judges the working tree. `.jit/`-only commits are safe.
- **Do NOT read the first `cargo-ci` failure after a cold rebuild as a defect.** `tuning-campaign-support::protocol_contracts acceptance_rejects_unrecorded_restarts_double_completions_and_abandoned_samples` (6 s deadline) failed twice for `d1bfe986` right after merges and passed on the warm re-run; `0657c9fa` lost its test step once under eight-worker load.
- **Do NOT trust "could not compile" across many dependency crates.** `sccache: error: Failed to create temp dir` broke every build on `main` until `sccache --stop-server`. Open issue `e250b5bb` describes the cause.
- **Do NOT let the disk fill.** Each worktree grows to 4–11 GB, `main`'s `target` is 36 GB. ENOSPC truncated `target/doc/trait.impl/core/marker/trait.UnsafeUnpin.js` in the pool and five workers hit "invalid template". Run a watchdog; reclaim on merge.
- **Do NOT recreate the cache pool from existing worktrees.** It carried a stale `gf2-kernels-simd` artifact ("cannot find m4rm") that every worker had to clean. Delete `<worktree>/target` before `reclaim-worker-worktree.sh` so nothing is harvested.
- **Do NOT let a worker delete a committed artifact without a pin search.** `git grep -n <filename> -- dev` first; receipts pin inputs by path and digest (`dfb8820b`).
- **Do NOT accept a "reproduce" criterion on an issue whose premise is unverified.** Time or run the reported behavior first: `45a962da`'s five "slow" tests ran in 6 ms, `fbcadf17`'s arm is unreachable, `3464c241`'s swapped column exists in no probed code. A report that does not hold is rejected with evidence, not worked around with an injected or synthetic case.
- **Do NOT tell a worker to make a test "independent" in a way that duplicates a canonical verifier.** `0657c9fa` failed review for a private primitivity helper beside `Gf2mField::verify_primitive`. The standing brief now says to reuse canonical mechanisms.
- **Do NOT leave a named behavior untested directly.** `9600eb56` failed review because no test called `decode_soft` itself.
- **Do NOT put a hand-copied inventory in docs.** `33057520` failed doc review for listing the hash inputs in the campaign guide; cite the API.
- **Do NOT let a hot-kernel fix change code shape without need.** `b2520e0f` first replaced the unrolled Mersenne-31 fold with a loop; timing cannot be measured in a working session.
- **Do NOT file more follow-ups than issues closed (invoker rule), and fix small things immediately.** Record a finding in `surfaced_pitfalls` and fold it into the next issue touching that file.
- **Do NOT add or remove a gate without asking the invoker.** The lead added `doc-review` to `f0228940` unasked and had to remove it.
- **Do NOT leave a finished worker agent running.** Agents with a background command still alive re-send their report on every wake; stop them with TaskStop once the commit is on the branch. Briefs now end with "leave no background command running".
- **Do NOT use a `pgrep -f` pattern in a compound command.** The harness blocks it; use `ps -eo comm,etimes,args`.
- **Do NOT let a merge subject exceed 71 characters.** Check `git log --format=%s | awk 'length>71'` before the next commit lands on top; one merge was redone.
- **Do NOT pass an optional `--gate` through `${6:+--gate $6}` in zsh.** It becomes one word and `jit issue create` rejects it.

## Open questions needing invoker input

- Question: How should `dfb8820b` satisfy REQ-02 ("the directory holds one schema version; superseded representations are removed")?
  - Context: the two v2/v3 profiles are pinned by path and digest in checksum manifests and zen3 producing-input records.
  - Options: (A) move their exact bytes to an evidence location and repoint every pin; (B) amend REQ-02 so the parser test covers only the current schema and the old files stay; (C) reject the issue.
  - Recommendation: B. Repointing digest-pinned evidence across the zen3 epic is a cross-epic change with no behavioral gain.
- Question: Reject `fbcadf17` with evidence, or attempt a third rework?
  - Context: the reported substitution is unreachable; REQ-02 is accepted; the cleanup and a real non-convergence test are merged.
  - Options: (A) reject as not reproducible and keep the merged code; (B) rework: remove the dead `ConcatError::Unsupported` variant in `gf2-coding`, a public API change.
  - Recommendation: A, with the dead variant recorded for the next `gf2-coding` DVB-T2 issue.
- Question: Accept synthetic-code evidence for `3464c241` REQ-01, or reword it?
  - Context: REQ-01 asks for a base graph and lifting size whose `systematic_cols` differ from the identity prefix; none found up to Z = 176.
  - Options: (A) amend REQ-01 to accept the in-module (6, 3) code and merge; (B) probe Z = 192..=384 under the slow tier first; (C) reject the bug as not reachable and keep REQ-02 as a refactor.
  - Recommendation: B then A; the probe is one ignored test.
- Question: Do the `3be770d5` receipts need revalidation after `33057520`?
  - Context: `snr_checkpoint.rs` and `executor/drain.rs` are listed as producing inputs by source digest and both changed; snapshot checks pass and the worker reports timing behavior unchanged.
  - Options: (A) no revalidation; (B) queue a re-run in the bench window.
  - Recommendation: No strong preference.

## Reference artefacts

- Epic: `jit issue show b4b4b9ee`; assessment report [b4b4b9ee-assessment-report.md](b4b4b9ee-assessment-report.md).
- Progress: [progress.json](progress.json).
- Gate records: `.jit/gate-runs/`; findings per issue with `jit gate status <id> <gate> --findings`.
- Reviewer configuration: `.jit/gates.toml` (commit `2b4d4a4ba`).
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.
- Worker branches are kept as `worktree-agent-<short-id>`; rework branches carry `-r1`.
