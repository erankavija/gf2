# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 3

**Date:** 2026-09-07T20:05Z (final for session 3; some workers were still committing when written)
**Session number:** 3
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md)

## Current state

- Epic: `1a379447` — state: backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 1 of 7 (`f547c394`) nearly closed; wave 2 of 7 dispatched early on the invoker's instruction.
- Children summary: 0 done, 1 in_progress (`f547c394`), 8 assigned and backlog (wave 2), 12 backlog, 0 rejected.
- Active claims: lead lease on `f547c394` (`agent:jit-execution-lead`, TTL 3600 s, acquired ~19:10Z); wave-2 issues assigned to `agent:worker` with `--assign-only`.
- Open escalations: none formally raised. The invoker instructed at ~19:50Z: rate limits are close; let the running workers finish, write this handoff, run no more gates.
- Progress file: `progress.json` in this directory.

### Wave 1 (`f547c394`) exact state

- Rework 2 was resumed by a codex `gpt-6-astra` worker in `.agents/worktrees/agent-f547c394`; the worker validated but could not commit (sandbox mounted `.git` read-only). The lead committed the validated tree in final form: `4b2c2eb2` (code), `bf7801a8` (docs/evidence), merged into main at `873f7ec6`; report consistency fix `833cc452`; tracker/progress `037885c3`, `49fa0ee2`.
- Gates on the merged tree: `code-review` PASS (zero findings, closes R2 F1), `doc-review` PASS after the report fix, `cargo-ci` FAIL twice on host-contention flakes (run 1: GPU tests `test_gpu_box_muller_within_1_ulp_over_1024_frames` SIGABRT and `test_gpu_chacha_raw_words_full_range_byte_identical` timeout; run 2: `tuning_calibration::campaign_owner::tests::accepted_complete_experiment_measures_defaults_and_reopens_only_its_27_leaves` 8.0 s timeout; every one passes in isolation, the calibration test at 5.7 s under load 12), `research-review` FAIL twice at the checker level: the `gpt-5.6-sol` reviewer at xhigh effort filled its 258 400-token context (last turn 220 436 tokens) reading the linked artifacts and ended without any output ("Agent produced no output"); no content finding was ever produced.
- A third cargo-ci run queued under `dev/scripts/ccx1-bench-flock.sh --full-host env CARGO_CI_NO_LOCK=1 jit gate evaluate ...` was cancelled on the invoker's stop instruction before it acquired the mutex.
- Lead review tiers completed: Tier 1.5 resolution table (worker report `dev/active/f547c394/rework-validation.md`, 28 rows), Tier 2 (diff read: `CampaignFacts::resume_equivalent` shared comparator, `CheckpointStore::inspect` read-only entry point, P-23 in `protocol.md`), Tier 2.5 sweep clean (only frozen receipt-local snapshots of the older protocol text remain, by design), Tier 2.75 clean. Tier 1 is blocked only by the two gate runs above.

### Wave 2 dispatch state (native Claude subagents, worktrees `.agents/worktrees/agent-<id>`, branches `worktree-agent-<id>`)

At ~19:45Z the invoker ordered all workers to commit what they have and report (rate limits). State at 20:05Z; workers marked "still committing" may add commits after this handoff, so re-read each branch before acting.

| Issue | Model | Branch head / commits | Has f547c394 merge | Reported state |
|---|---|---|---|---|
| c077a88b LDPC arms | Opus | `296362a3`, 13 commits, clean | yes | No final report yet. findings.md and matched-algorithm + quality-compatible pilot addenda committed; AFF3CT v4.7.0, srsRAN, xdsopl, OAI staged under `.agents/ext/c077a88b`; BFER quality runs were starving the exclusive mutex (instructed to stop). No receipts seen on the branch. |
| c7113c5a polynomial | Opus | `6abb3e13`, 17 commits, clean | yes | No final report yet. gf2x 1.3.0 built conservative/tuned/native; both arms validated (8922 checks per variant, zero failures); pilot receipt `dev/bench_results/c7113c5a/2026-09-07-c7113c5a-polynomial-pilot` committed and accepted; confirmatory addendum frozen; confirmatory run cancelled while queued. |
| 6fb89a3c transpose/logical | Sonnet | `74f1b296`, 2 commits, 1 dirty file | **no** | No final report yet. findings.md scaffold and three pilot addenda drafts; C harnesses (M4RI/Bitshuffle/ISA-L) and gf2-side harness written but not committed at handoff time. Must merge main before any receipt. |
| 1d0da41f YMM clmul repair | Opus | `ef31a936`, 10 commits, clean | yes | Reported. REQ-07/08/10 MET (predicate + safety contract `crates/gf2-kernels-simd/src/x86/clmul.rs`, lane suites, consumer test, `asm/clmul.asm.txt` at rustc 1.95 with `vpclmulqdq ymm`); pilot receipt accepted and FALSIFIES the premise (YMM lane 1.24–1.72x slower than sequential PCLMULQDQ, 1.29x through the consumer); REQ-01/09 PARTIAL: confirmatory receipt never acquired the mutex. **Not mergeable as is**: commit `e78dd4ac` makes the YMM lane the default; the adoption default must follow the confirmatory receipt via detect-time lane selection in `gf2m::detect_x86`. |
| 6c6b09b1 byte-field | Opus | `da729f8a`, 7 commits, clean | yes | No final report yet. Pins and pilot addendum committed. |
| 04b85d10 profiling | Opus | `e2ff5405`, 9 commits, 1 dirty file | yes | No final report yet. Harness, pilot addendum and classified generated-code (spill vs scratch) evidence committed. |
| 26465e6c popcount | Sonnet | `0152643d`, 3 commits, 1 dirty file | yes | No final report yet. findings.md plus popcount and AND-popcount pilot/confirmatory addenda committed. |
| eda07788 shifts/permutations | Sonnet | `6d5f09ea`, 2 commits, clean | yes | Reported. REQ-02 MET (xdsopl `PCTITL` DVB-T2 interleaver arm validated bit-exact over 64 800 positions for two MODCODs; AFF3CT has no DVB-T2/NR; arbitrary shifts, NR circulant rotation and NR rate matching recorded as unmatched with code-level evidence); REQ-03 PARTIAL; REQ-01/04 UNMET: no frozen addendum, no receipt; `survey/fetch-build.sh` never executed (validation used an ad-hoc clone at the same pin); drafts in `dev/active/eda07788/survey/drafts/`. |

Leak check on main: clean at 20:05Z. Disk: 80 GB free. No wave-2 branch is merged; no wave-2 gate has run.

## What just happened

- Resumed the interrupted rework of `f547c394` on codex astra, committed its validated tree, merged, linked `rework-validation.md` and `revalidation.json`, ran gates individually (see above).
- Applied a lead-only consistency fix to the report (`833cc452`) after doc-review F1 (stale "commits blocked" claims).
- Dispatched wave 2 in two groups (A before the merge, B after) as native subagents on the invoker's instruction; instructed all eight to stop delegating to codex after the invoker objected to wrapper agents.
- Gave `1d0da41f` guidance: keep the predicate repair, preserve the falsification with intervals, let the frozen non-regression rule decide adoption through the canonical detect-time lane selection; no shared-infrastructure change.

## What to do next

- [ ] First: collect the six missing final reports. Each worker is a named subagent of session `cbccd0cf`; if that session is gone, read each branch (`git log main..worktree-agent-<id>`, `findings.md`) and treat the branch as the report. Lead-preserve any dirty worktree (`git add -A && git commit` with a `wip(jit:<id>)` subject) before anything else.
- [ ] `f547c394`: re-run `cargo-ci` on a quiet host (nothing else building) through the gate, ideally `dev/scripts/ccx1-bench-flock.sh --full-host env CARGO_CI_NO_LOCK=1 JIT_AGENT_ID=agent:jit-execution-lead jit gate evaluate f547c394 cargo-ci`; the checker's 900 s budget starts after the mutex is acquired.
- [ ] `f547c394`: resolve the research-review checker failure (see Open questions), then re-run `research-review`. Then `jit issue update f547c394 --state done`, commit, `jit graph downstream`, `jit validate`, release the lease, reclaim `agent-f547c394` and `agent-f547c394-identity-tests` worktrees.
- [ ] Wave 2 review order suggestion: c7113c5a and 1d0da41f (closest to complete; both need a confirmatory receipt on a quiet host), then eda07788 (run `fetch-build.sh`, re-validate, freeze the drafts, pilot + confirmation), then the four with no report. Wave 2 is unlikely to close without one more dispatch round per issue; budget it.
- [ ] Wave 2: for each finished worker, run `check-leak-into-main.sh`, lead-preserve any uncommitted worktree changes, merge `--no-ff` one branch at a time, gate the merged tree (cargo-ci) after each merge, then code/doc/research reviews individually, six-tier lead review, rework up to 2 attempts. Group-A branches that did not merge main must be merged with main first (conflict-free by construction: they touch none of the protocol files).
- [ ] `1d0da41f` closure needs both the negative receipt and the adoption default that follows it; check the asm artefact is in the same commit as the last SIMD source change (`scripts/asm-artefact-present.sh` inspects HEAD~1..HEAD of the merge).
- [ ] After wave 2 closes: advance `current_wave` to 3, write the wave-3 dispatch (3be770d5, 53c5a8c0, 19513245, 1d4fd63d, 5cbb6545, 2037941f, c04dd4ac).

## Traps — do not repeat these

- **Do NOT expect a codex `workspace-write` worker to commit from a linked worktree, even with `--add-dir <repo>/.git`.** The sandbox remounts `.git` read-only (`index.lock: Read-only file system`, `rework-validation.md` Delivery status). Either the lead commits the validated tree on the worker branch (done this session) or dispatch native subagents.
- **Do NOT rely on `.jit/gate-runs` from a worker worktree.** It is untracked, so the rework audit step 1 prints "No matching gate runs"; paste prior findings into the rework prompt (done) and expect the worker to cite the committed review verdicts.
- **Do NOT let native workers inherit the global CLAUDE.md delegation rule.** Four of eight spawned codex luna jobs ("lazy wrapper agents"); the invoker objected. The dispatch preamble now says "do the work yourself"; keep that line in every future dispatch.
- **Do NOT run the cargo-ci gate while eight workers build.** Two runs failed on unrelated 8-second-budget tests (GPU and calibration) that pass in isolation; the wrapper's test lock serializes test runs but not builds. Use the full-host mutex wrapper around the gate, or run it when the host is quiet.
- **Do NOT expect `research-review` to survive the current f547c394 footprint.** The sol reviewer at xhigh effort reads every linked document; `rework-validation.md` (154 KB, mostly raw nextest output) plus receipts and diffs fill its context and it exits with no output twice. Two prior runs: gate runs `ba756d8f` and `b2af1fd3`; transcripts `~/.codex/sessions/2026/09/07/rollout-*-01a07d4b-*.jsonl`, `...-01a07d57-*.jsonl`.
- **Do NOT queue long evidence runs behind AFF3CT BFER simulations held under the shared side of the CCX1 mutex.** A shared holder blocks every exclusive timed run; the LDPC worker's BFER runs are quality evidence and do not need exclusivity, but they should not hold the shared side for long either. Consider a separate lock domain for quality sims in the next dispatch.
- **`du -sh` over hardlink-seeded worktrees miscounts.** The first worktree listed absorbs the whole 135 GB pool; use per-worktree `du` invocations or `df` to reason about disk. Disk was 84 GB free at 19:30Z with eight worktrees.
- **Background `./scripts/cargo-ci.sh > log` was blocked by the auto-mode classifier;** run CI through `jit gate evaluate` (background) instead.
- **Group-A wave-2 branches predate the f547c394 merge.** They must merge `main` before their receipt runs (instructed in the prompt) and the lead must verify the receipts' producing-input identities match post-merge tooling before accepting them.
- **Do NOT dispatch eight measurement workers on one host at once.** The exclusive CCX1 mutex became the bottleneck: confirmatory campaigns queued 40+ minutes and were cancelled; `flock` is not FIFO, so short shared holders (aff3ct BFER runs) starve exclusive waiters indefinitely. Next time: at most two or three timed campaigns in flight, quality sims pinned off CCX1 without the mutex, builds batched under one shared hold.
- **Do NOT plan a session around eight native Opus/Sonnet workers plus AI review gates under a shared rate limit.** The invoker hit the limit within ~2.5 hours and stopped the session before any wave-2 issue closed.
- Unresolved traps from [session 2](handoff-2.md#traps--do-not-repeat-these) and [session 1](handoff.md#traps--do-not-repeat-these) remain in force: `jit claim acquire --agent-id agent:jit-execution-lead` and `JIT_AGENT_ID` for gate evaluation; evaluate gates individually, never `evaluate-all` after HEAD moved; commit early; no `nextest` without `cargo` through the budget wrapper; nested `Cargo.lock` snapshots must be force-added; `jit doc check-links` warnings are nonzero.

## Open questions needing invoker input

- Question: How should the research-review checker failure on `f547c394` be resolved?
  - Context: the reviewer exhausts its context reading the 154 KB `rework-validation.md` and the receipts; it never reaches a verdict. Any of the fixes touches either a linked artifact or the gate configuration.
  - Options: A) split the raw command output of `rework-validation.md` into an unlinked companion log file, keeping audits, resolution table and results in the linked report (artifact edit, lead-authored, no evidence change); B) change the gate's `REVIEWER_AGENT` model/effort or add a footprint size limit (shared infrastructure, escalation policy item 8); C) unlink the report from the issue (weakens invariant 8 discoverability).
  - Recommendation: A, then re-run; escalate B only if A still fails.
- Question: Should the contention-sensitive tests (`test_gpu_box_muller_within_1_ulp_over_1024_frames`, `test_gpu_chacha_raw_words_full_range_byte_identical`, `accepted_complete_experiment_measures_defaults_and_reopens_only_its_27_leaves`, `rare_event_artifact_partial_publish_recovery`) get a tracked bug for their 8-second-budget margin under load?
  - Recommendation: yes, one bug task outside this epic (owner: test-tier-budgets), so wave gates stop flaking; not a criterion of `1a379447`.

## Reference artefacts

- Epic: `jit issue show 1a379447`; contract `measurement-contract.md`; plan review `plan-review.md`; progress `progress.json` (this directory).
- Wave-1 evidence: `dev/active/f547c394/{protocol.md,design.md,provenance-clarification.md,rework-validation.md,addendum.schema.json}`, `dev/bench_results/f547c394/{revalidation.json,2026-09-07-f547c394-protocol-pilot,2026-09-07-f547c394-protocol-confirmation}`; reviews `reviews/f547c394-r1.md`, `reviews/f547c394-r2.md`; gate runs: code-review pass on `037885c3`, doc-review pass on `49fa0ee2`.
- Salvage refs: branch `salvage/f547c394-rework2-cf2e7270`; worker branches `worktree-agent-<id>` for all wave-2 issues.
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.
