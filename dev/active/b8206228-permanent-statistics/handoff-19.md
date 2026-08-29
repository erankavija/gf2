# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 23

**Date:** 2026-08-29
**Session number:** 23
**Prior handoffs:** `handoff.md` (session 3) through `handoff-18.md` (session 22). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — wave 8 of 13; freeze issue `7a816262` is the wave's last open item, in_progress, unassigned, rework counter 0 (owner guidance reset).
- Session 22's two open escalations are **resolved**: the owner approved the gate-script fix and the audit fast path. Every further owner decision this session was also answered — there are **no open escalations**.
- The owner is performing hardware upgrades next (RAM and disk only — CPU/GPU unchanged, so the frozen backend selection, the pinned emitter digest, and every measurement receipt remain valid). Do not start the equivalence run or other long compute until the owner confirms the machine is ready.
- Closed this session: `ec22205e` was already done; newly done are `69e7d174` (gate-scoping fix), `e1d45c20` (emission-check fix), `bc6155fa` (pre-draw receipt).
- Ready, unclaimed, never started: `86185f88` (equivalence receipt) and `6dfb1b5e` (README receipt-identity fix) — their dispatches died before doing anything (see traps).

## What happened this session

1. **research-review Tier 1 scoping fixed** (`69e7d174`, owner-approved, closed). `contrib/gates/research-review.sh` now derives owned `cites:` keys from `.issue.labels` and matches citations against issue title+description only, via jq; a 6-case fixture suite exercises the real checker (`contrib/gates/tests/research-review-tier1-test.sh`). This also fixed a latent spurious-PASS: prior failure stdout embedded in the context's `run_history` contained the bracketed keys the old whole-file grep looked for. code-review passed, 0 findings. Commit `0fed351c`.
2. **Freeze audit findings repaired** (owner fast path). `freeze.md`'s falsified pre-commit narration rewritten as revision-pinned transcripts with real output at `67f5108f` (emission-check advancing to the binary-digest refusal; provenance diff isolating `git_revision`); git history recorded as freeze.md's tamper evidence; campaign README layout row added for `freeze.md`; supersession banners on the three phase-1 drafts. doc-review re-passed. Commit `ed2015ef`.
3. **Citation label/text drift batch-fixed** (owner-approved): bracketed citekeys woven into 9 issue descriptions (3f664839, 8ec10dde, 1d0b3ec4, 90a61cd4, ed494117, 53d8e438, b05c908b, f74e327d, b5d45bcd) per the frozen protocol's own attributions (floor proved by [HKS2026], q=3 curve [Scheinerman2024], asymptotics [GGK2025]+[HKS2026]).
4. **research-review ran its first-ever tier-2 methodology review of the freeze — VERDICT: FAIL, five blocking findings** (recorded run at `f853fe6`, 537 s). Owner dispositions, all applied:
   - F1 (REQ-04 requires the *manifest* to carry the error budget; the strict schema cannot): **REQ-04 amended** to name the freeze record as the carrier.
   - F2 (provenance `invocation` is the inspection argv): **reading recorded** in freeze.md — the field holds the producer-of-the-observation's argv per README:101; run shape is determined by protocol+manifest; each arm's checkpoint records its actual argv.
   - F3 (selection receipt claims cell-applicable equivalence for all 124 configs; evidence covers orders {8,12,16,20,24,28}+(3,4) only): **owner chose to produce the evidence** — task `86185f88` (cell-exhaustive equivalence receipt), wired before all three campaign arms. Not started.
   - F4 (no committed RNG identity for the selection cohorts): **closed** by `dev/benchmarks/permanent_campaign/backend-selection-v1-rng-addendum.md` + committed harness lock `premeasure-v1-harness.Cargo.lock`. The addendum proves rand_chacha 0.9.0 by byte-identical RNG-chain rlibs between the retained measurement build and a pinned-revision rebuild, binary symbol forensics (the digest-matched executable links only the 0.9.0 stack), and derives every process's seed address from committed ledger columns (verified over all 1,439 data rows plus the censored position 539). The whole-executable digest is NOT reproducible off-path (embedded build-location text; explained mechanistically in the addendum).
   - F5 (draft labels projections as measurements): relabeled inline in `freeze-feasibility-draft.md`.
5. **Pre-draw obligations closed before any draw**: `e1d45c20` (emission-check now verifies the manifest's pinned emitter — no-arg inspection mode plus optional emitter-path full check; writer guard untouched; cargo-ci + code-review passed) and `bc6155fa` (freeze.md §"Pre-draw execution receipt" claims the protocol's receipt role with the three bindings; doc-review passed).
6. **Follow-up issues filed**: `6dfb1b5e` (README says execution receipts quote the sidecar SHA-256; driver emits blake3 over a re-serialization — doc-side fix, prompt drafted at scratchpad `prompt-6dfb1b5e.md`, not executed) and `86185f88` (above). Both wired before the three campaign arms alongside `e1d45c20`.
7. **Frozen emitter insurance**: byte copy of the pinned emitter at `~/.local/state/gf2-frozen/permanent_campaign-2d6edcd9` (sha256 matches the manifest pin). Cargo builds overwrite `target/release/permanent_campaign`; the manifest pins the closure (`deps_source_revision` 6348b0c9) for rebuilds, but the copy is cheap insurance.

## What to do next

- [ ] **Wait for the owner to confirm the hardware upgrade is done** before any compute. RAM/disk only — no re-measurement is needed.
- [ ] Re-dispatch `6dfb1b5e` (Luna xhigh; prompt at scratchpad, or rewrite from the issue — it is self-contained) and `86185f88` (Sol xhigh; dispatch prompt inline in session 23's transcript, self-contained in the issue description). **Add `< /dev/null` to every `codex exec` invocation** (see traps).
- [ ] After `86185f88`'s harness extension lands and reviews, run the full cell-exhaustive equivalence in an **exclusive window** (no cargo builds/tests/gates concurrently; GPU rows in the owner's safe window from 02:00): the receipt closes research-review F3.
- [ ] Then evaluate `research-review` AND `doc-review` on `7a816262` (both must re-run at the final content), perform the lead six-tier review, close the freeze.
- [ ] Then dispatch wave 9: `3f664839` (design; native Opus per owner directive) and `73317b2e` (implementation; Sol xhigh). **Constraint for 73317b2e:** the acceptance/coordinator layer must live outside the frozen emitter process — the arms execute emitter digest `2d6edcd9` built from closure `6348b0c9`; a coordinator process orchestrates and halts. The plan's "Source identity at emission" decision and the manifest's `deps_source_revision` are the authority.
- [ ] `jit query divergence` shows one pre-existing label divergence on `ef18c60b` (different epic) — out of scope, left alone.

## Traps — do not repeat these

- **Do NOT dispatch `codex exec` in a background Bash without `< /dev/null` on the codex command.** Both session-23 re-dispatches (86185f88, 6dfb1b5e) hung forever at "Reading additional input from stdin..." because the background task's stdin was a never-closing socket; codex waits for stdin EOF before starting when stdin is a non-tty. The earlier single-command dispatches worked only because their wrappers happened to get `/dev/null`. Kill signature: log file containing only that one line, no rollout file for the run.
- **Do NOT trust the first reviewer rollout you find; the recorded gate run is the authority.** research-review R2's first reviewer pass returned 2 findings; ai-review re-ran after a stall and the recorded verdict has 5 findings naming different defects. Read findings via `jit gate status <id> research-review --findings`, never from `~/.codex/sessions` rollouts.
- **A stale `claims.lock` silently wedges `jit gate evaluate` between checker completion and verdict recording.** The evaluate process sleeps in a retry loop (nanosleep, 0 CPU, checker children gone). A timed-out/killed jit invocation leaves the lock behind. Diagnose with `ls .git/jit/locks/`; `jit recover` clears it and the wedged evaluate then proceeds. Two separate wedges this session, both caused by earlier killed jit processes.
- **Do NOT run a codex worker's `cargo-ci` while another agent builds anything on the host.** The e1d45c20 worker burned its run: cargo-ci contended with the addendum agent's rebuild, the worker diagnosed lock/CPU contention, retried, and codex died (exit 144) before reporting. Its code was complete; the lead ran cargo-ci after the other build finished and it passed unchanged.
- **`jit issue create` content-standards: success criteria must match `[hard]/[aspirational] REQ-nn:`** or every item emits a warning; write them in that shape the first time.
- **`jit dep add` refuses edges that break transitive reduction** — when wiring a new pre-draw task under both the epic and the arms, add the arm edges with `--reduce` so the direct epic edge is dropped in the same operation.
- **The `dispatch-worker-worktree.sh` script requires a clean main including `.jit/`** — claim/release bookkeeping dirties `.jit` instantly. Commit state before running it, or create the worktree manually anchored to HEAD and snapshot `git status -uall` yourself.
- All unresolved traps in `handoff-18.md` and earlier remain in force — especially: never `git add`/`git commit` on main while a main-checkout worker runs; lead owns all `jit doc add`; do not re-derive the 63-cell selection; `ps -o etime` lies about codex health (rollout growth is the signal — but see above: its absence can also mean thinking; check `task_complete` in the rollout tail and the recorded gate run before concluding).

## Open questions needing invoker input

None. All five research-review findings and both session-22 escalations carry owner decisions, applied and committed. The only gate on further progress is the owner's hardware-upgrade window.

## Reference artefacts

- Epic: `jit issue show b8206228`; freeze: `jit issue show 7a816262` (REQ-04 as amended)
- Recorded freeze review: `jit gate status 7a816262 research-review --findings` (R2, five findings, dispositions above)
- RNG addendum: `dev/benchmarks/permanent_campaign/backend-selection-v1-rng-addendum.md`; lock: `premeasure-v1-harness.Cargo.lock`
- Pre-draw receipt: freeze.md §"Pre-draw execution receipt" (bc6155fa)
- Frozen emitter backup: `~/.local/state/gf2-frozen/permanent_campaign-2d6edcd9`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json` (`session_23_state` key)
- Worker branches kept: `worktree-agent-e1d45c20`, `worktree-agent-bc6155fa` (merged); none pending
