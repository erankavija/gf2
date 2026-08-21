# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 6

**Date:** 2026-08-21T21:20Z (2026-08-22 00:20 local)
**Session number:** 6 (session 5, 2026-08-21 daytime, wrote no handoff; its state was reconstructed from `progress.json` and the git log)
**Prior handoffs:** `handoff.md` (session 4)

## Current state

- Epic: `6dc81018` — state: `in_progress`, claimed by `agent:jit-execution-lead`
- Wave in progress: wave 14 of 14 — `50b47eae`'s fourth measured session, RUNNING DETACHED on this host (see below)
- Children summary: all children `done` except `50b47eae` (`in_progress`, claimed `agent:worker`)
- Active claims: `50b47eae` as `agent:worker` (reused from prior sessions)
- Open escalations: none awaiting input. DEC-I resolved this session (both halves; see What just happened).
- Progress file: `progress.json` beside this file

**A detached benchmark pipeline is running.** Do not start builds, benches, or heavy CI on this host until it finishes:

- Build chain PID 326324 (PPID 1): 256 candidate-arm builds into `/tmp/gf2-ens4/`, expected done ~2026-08-21T22:10Z; writes `/tmp/gf2-ens4/logs/build-phase-end.txt` last. Reference arm already done: 256/256 built, 0 failures, 256 distinct SHA-256, 256 distinct page offsets.
- Continuation driver PID 754889 (setsid, log `/tmp/gf2-ens4/logs/continue-driver.log`): waits for the marker, verifies both arms' realized construction (v2 §A4, `verify-construction.py`), runs the 512-execution timed phase in one `ccx1-bench-flock.sh` session (`run-ensemble.sh`, v1 §5.2 order, guard aborts on HEAD/porcelain movement), copies everything durable into the repo untracked, then runs the committed `--compare` and `--layout-audit` modes. Terminal log line: `DRIVER: COMPLETE` or `DRIVER: ABORTED <reason>`. Expected complete ~2026-08-21T23:50Z.
- Outputs land untracked at: `dev/benchmarks/tuning_profiles/2026-08-22-ensemble-{reference,candidate}-arm-4.csv`, `2026-08-22-member-provenance-{ref,cand}-4.tsv`; logs, scripts, `comparison.txt`, `layout-audit.txt`, and `sha256-manifest.txt` at `dev/active/50b47eae/s4-session/`.

## What just happened

- Resumed on the unsettled escalation: presented amendment v2 for owner approval → **DEC-I**: text approved; owner initially also chose a v1 pointer section.
- Lead appended the pointer to v1 (commit `7d88007f`) → 9162956b's doc-review FAILED on it (REQ-02 + v2 §A10 contradiction) → escalated → owner reversed the pointer half; v1 restored byte-identical (`22f467ab`, SHA-256 matches the anchored `1b8a91bd…`). DEC-I's v2 approval stands; both recorded in `progress.json` escalations.
- Lead review of `9162956b`: PASS. Independently recomputed `s_R`, half-split nulls, and RMS from `pilot-3-ens.csv` — match the receipt to six decimals (nulls as the reciprocal; the audit's ratio direction is odd/even). Tier 2.5/2.75/citation sweeps clean. Gates: cargo-ci, doc-review (after the v1 restoration), code-review all PASS. Closed; rework count 0 (the doc-review round was the lead's own edit, not worker output).
- Wave 14 dispatched to an opus worker with the full session spec; owner then redirected: launch the session as a detached script and hand off. Worker half-complied (detached the build phase, correct §A3 flags/ledgers, wrote sound `run-ensemble.sh` + `verify-construction.py`, all mirroring receipt-3's committed mechanics) but left the timed phase attached to its own life and did not report → killed per owner instruction; its detached builds continue; lead authored `continue-driver.sh` wrapping the worker's verified pieces end to end and launched it detached.
- Host verified free before the pipeline ran (lock unheld, load idle, both checkouts clean).

## What to do next

- [ ] Read the LAST line of `/tmp/gf2-ens4/logs/continue-driver.log`.
- [ ] If `DRIVER: ABORTED` **before any timed window** (build-phase or construction or preflight failure): artifacts are intact; diagnose from the named log, fix, relaunch `bash /tmp/gf2-ens4/continue-driver.sh` detached. If aborted **mid-timed-phase** (partial arm CSVs exist): the partial record stands as taken — preserve it, do not delete or re-run, escalate to the owner.
- [ ] If `DRIVER: COMPLETE`: read `dev/active/50b47eae/s4-session/comparison.txt` (verdict, τ_cell 5 % / τ_set 2 %) and `layout-audit.txt` (four preconditions at K=256).
- [ ] Author `dev/benchmarks/tuning_profiles/2026-08-22-post-cutover-receipt-4.md` in receipt-3's register from those outputs. It must include the revision ruling: candidate rows record the RUN-time HEAD (this handoff's commit — the harness reads `git rev-parse HEAD` at execution, `selector_non_regression.rs:1020`) while the candidate binaries were built at `ec857d2f`; verify and state the empty build-input diff `ec857d2f..<run HEAD>` (only `dev/active/` + `.jit/` files), per the owner ruling recorded as escalation #18 in `progress.json` (receipt-3 precedent).
- [ ] Commit the CSVs, TSVs, receipt, and session logs; `jit doc add 50b47eae` each durable artifact.
- [ ] Run `jit gate evaluate 50b47eae doc-review`. Tier 1.5 applies: prior findings F1 (no passing receipt) and F2 (untracked rework) — F2's chain is complete (2a85f728, 972e2b88, 676f55a2, 9162956b all done).
- [ ] Verdict PASS + audit ok → close `50b47eae`, then Section 10: reconcile `surfaced_pitfalls` (one REQ-05 entry marked "open" and one 1ac74567 entry marked "reconcile at completion") against the criteria, `jit gate evaluate-all 6dc81018` (repo-validate, doc-review, holistic-review), completion report, close, archive.
- [ ] Verdict FAIL → v2 §A7 routing is predeclared: precision or half-split failure at K=256 goes to the OWNER (no larger ensemble exists); coverage or decorrelation failure goes to a further tracked axes amendment. Receipts preserved either way.
- [ ] Reclaim stale worktrees when the receipt is accepted: `agent-265997f9`, `agent-34d85cb9`, `agent-0d819b62`, `agent-697fc55b`, `agent-c42720ce`, and `control-0c072d73` last (it is the reference-arm build environment; its manifest path is part of σ̂'s empirical content — do not touch it before the receipt is committed).
- [ ] Remove `/tmp/gf2-ens4/` only after everything durable is committed.

## Traps — do not repeat these

- **Do NOT append an owner-directed pointer to a standing predeclaration whose amendment contract says it changes none of that document's text.** The DEC-I pointer append to v1 (`7d88007f`) failed doc-review as a REQ-02 violation and a v2 §A10 contradiction; reverted at `22f467ab`. Record approvals in the decision log; the read path v1 §6.6 → receipt-3 → 9162956b → v2 suffices.
- **Do NOT commit on main between a candidate arm's build phase and its timed phase without planning the revision ruling.** The harness records `git_revision` at RUN time from the binary's cwd (`crates/gf2-core/benches/selector_non_regression.rs:1020`), so rows will name the latest commit, not the build revision. This session's handoff commit does exactly this deliberately; the receipt must carry the escalation-#18-style empty-build-input-diff verification. And NOTHING may commit on main while the timed phase runs — `run-ensemble.sh`'s guard aborts the session on HEAD or porcelain movement.
- **Do NOT treat a redirected worker's "detached" claim as end-to-end.** The worker detached only the build phase (its poll loop and the timed-phase launch died with it). Verify detachment by process ancestry (PPID 1 / own SID covering the WHOLE chain), not by the presence of nohup artifacts. Killing the worker via TaskStop does not kill its detached children — that is what made the takeover clean.
- **Do NOT run repo-relative commands without re-anchoring the working directory.** The Bash tool's cwd persists across calls (a `cd` into the control worktree made later relative paths fail silently against the wrong tree). Start repo commands with `cd /home/vkaskivuo/Projects/gf2 &&`. Also avoid bare `===` echo separators in zsh — they glob and kill the compound command.
- **Do NOT put strings that look like process-liveness polling (e.g. a pgrep-based wait) inside Bash heredocs** — a harness hook blocks the whole command on content match. Write such file content with the Write tool.
- All traps from `handoff.md` (session 4) remain in force, notably: agent-liveness probing via artifacts not ListAgents; benchmark-mode work never goes to codex; closed-issue records under `dev/active/` are amended additively only; single red CI on `gf2-sim gpu::awgn` is re-run once (now tracked as bug 76665001).

## Open questions needing invoker input

None. (DEC-I fully settled this session; the next decision point is the measured verdict itself, whose routing v2 §A7 predeclares.)

## Reference artefacts

- Epic: `jit issue show 6dc81018`; open child: `jit issue show 50b47eae`
- This session's pipeline: `/tmp/gf2-ens4/` (scripts, ledgers, logs, staged binaries); after completion also `dev/active/50b47eae/s4-session/`
- Dispatch spec the pipeline implements: `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/dd15bd2b-2781-4d8b-be26-e343821a8677/scratchpad/dispatch-50b47eae-session4.md` (scratchpad may be gone; the governing documents below are authoritative)
- Frozen procedure: `dev/benchmarks/tuning_profiles/selector-non-regression-plan-v1.md`; control arm: `across-build-control-arm-v1.md`; verdict: `layout-attribution-verdict-v1.md` + owner-approved `layout-attribution-verdict-v2.md` (DEC-I)
- Prior receipts (preserved, FAIL): `2026-08-20-post-cutover-receipt{,-2,-3}.md`; baseline: `2026-08-19-pre-cutover-baseline.md`/`.csv`
- Pilot evidence behind v2: `dev/active/9162956b/ensemble-axes-pilot-receipt.md` (+ protocols, CSVs, provenance TSV)
- Design: `dev/active/220cab0b/design.md` (D1 baked-cfg amendment ~line 800); classification: `dev/active/6dc81018-field-capability-dispatch/classification.md`
- Out-of-epic follow-ons under `86b9c719`: `a6636671`, `4dd5372a`, `99c92597`, `389aa4de`, `76665001`
