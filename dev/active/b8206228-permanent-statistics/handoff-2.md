# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 4

**Date:** 2026-08-13
**Session number:** 4
**Prior handoffs:** `handoff.md` (session 3, 2026-08-09). Its traps remain in force except where superseded below.

## Current state

- Epic: `b8206228` — state: backlog (children executing)
- Wave plan: re-derived 2026-08-13 from the live graph (13 waves over what were 28 open issues; see `progress.json`, `rederivation_note`). Wave numbers restart at 1 relative to session 3's plan.
- Children summary: 44 done, 0 in_progress, 26 backlog/ready, 0 rejected. Closed this session: `44534b2f` (exact anchors), `22c7a3cd` (shard accumulator), `e0251af3` (rank-deficiency rates), `2ddf2f4b` (campaign runner, created this session).
- Available now: `047b62ed`, `91605d4d`, `6c7fcb38` (the three receipt campaigns — measurement scheduled overnight, see below) and `41c3d91d` (campaign driver scheduling and shard emission, unblocked by the accumulator).
- Active claims: none. Open escalations: none (two were raised and resolved by the owner this session: e0251af3 citation amendment; 2ddf2f4b lead-takeover after the rework cap).
- `jit validate` and `jit profile validate` pass (profile drift resolved this session, see below).

## The overnight campaign run (2026-08-14 02:00 EEST)

- systemd user timer `gf2-permanent-campaign.timer` fires 02:00 and runs `dev/scripts/permanent-campaign-runner.sh measure >> target/permanent-campaign/systemd-measure.log`, a deterministic AI-free runner (owner directive). It refuses a non-pristine tree (tracked AND untracked) both before and after acquiring the full-host bench mutex — **main must stay pristine overnight; do not leave worktrees or untracked files.**
- It runs, serially per field q=3,5,7: one global backend-equivalence run (gating each field's timing on its per-q verdict), the timing grid, gray-update and horizontal-product isolation — outputs and provenance land directly in `dev/studies/047b62ed`, `dev/studies/91605d4d`, `dev/studies/6c7fcb38`. Exit 0 = all steps completed; exit 7 = censored (per-step evidence in the run-summary files); exit 2 = refused/infrastructure.
- Morning checklist: `systemctl --user status gf2-permanent-campaign.service`; read `target/permanent-campaign/systemd-measure.log` and the three `*-run-summary.txt`; commit the study outputs per issue; then drive each campaign issue's receipts/report against its REQ-01..15 (the runner produces raw evidence, the issues own the committed receipts/analysis), gates `code-review` + `doc-review`, serially F_3 → F_5 → F_7.
- Kernel resource receipts (REQ-07 inputs) are in `target/permanent-campaign/hip-resource-usage-*/receipt.txt` from the final `prepare` — copy/commit the relevant receipt contents with the campaign receipts; target/ is transient.

## What else happened this session

- **Profile drift resolved (owner pre-flight ask):** jit-default's `namespaces:type`/`component` adoption drift made `jit validate` fail and would have blocked the epic's `repo-validate` gate. jit 1.0.0 has no bless path (confirmed with the jit lead over the forum; fixed on jit main, not yet released). Restored the package-published values; re-localization tracked as `ec3971f3` under the tech-debt umbrella (`86b9c719`).
- **jit adopter feedback loop:** consolidated field report sent to the jit lead (agent:codex via ~/Projects/forum-poc, FORUM_DIR=/tmp/jit-forum, identity agent:gf2-lead); their owner opened customer epic `4a559332` in the jit repo (worktree-fork prevention, durable gate evidence, machine-readable profile remedies, selector discovery). They may ping the agent:gf2-lead mailbox for adopter testing.
- **Flaky test filed:** `398f0691` (gf2-sim kill_mid_write_randomized_defense_in_depth fails under load, passes solo) under the tech-debt umbrella. It cost one cargo-ci gate round.
- e0251af3's description was amended (owner-approved) to cite [GGK2025]/[Scheinerman2024] inline — the research-review tier-1 checker requires every `cites:` key in the gate context to appear as a `[Key]` token in the issue text.

## Traps — do not repeat these

- **Do NOT run jit or git state commands with the shell parked in a worker worktree.** jit materializes state into whichever checkout runs it, and `git add .jit && git commit` then lands on the worker's branch. This session created issue 2ddf2f4b that way; the state was briefly destroyed by a cleanup and recovered from the dangling commit `6ae316c4` by hand-merging event logs (recovery commit `09aa8b66`). The harness shell's cwd persists across calls unpredictably — start every state-touching command with `cd /home/vkaskivuo/Projects/gf2 &&` or explicit `git -C`/absolute paths.
- **Do NOT run merge/cleanup chains cd'd into the worktree being merged or removed.** `git merge <branch>` while ON that branch is a silent no-op, and `git worktree remove` of your own cwd strands the shell ("Unable to read current working directory"). Merge from the repo root; verify with `git log --oneline` on main afterwards.
- **Do NOT assume `jit doc add` with a new path replaces an old link.** It is idempotent per path — re-pointing evidence requires `jit doc remove <old>` first. A stale link to a deleted file is a doc-review blocker (burned one round).
- **Do NOT leave superseded run evidence committed once the producing tool changes.** Reviewers require the linked smoke/run evidence to come from the final tool revision; `git rm` the superseded run directories and re-point the link in the same commit (burned another round).
- **codex-sandbox workers cannot write worktree git metadata (`index.lock: read-only`) and cannot reach the GPU (HIP error 100 from hipGetDevice is the sandbox, not the host).** The lead commits their reviewed trees on the worker branch; device-touching validation must be re-run by the lead on the host.
- **GPU-adjacent gf2-sim tests fail under GPU/host contention** (dispatcher tests, the kill_mid_write flake `398f0691`). Re-run the single test solo on a quiet host before reading a cargo-ci failure as a regression; never run cargo-ci or benchmarks while any worker compiles or uses the device.
- **The harness `equivalence` subcommand is global (no q filter), and only `grid` parses `--execution-id`/`--skip-machine-warmup`.** The runner accounts for this (shared equivalence run, per-q verdict gating, `execution_id=fixed-streams` markers); do not "fix" summaries to show per-field equivalence invocations or synthetic ids — that is what review rounds 3–4 removed.
- **Infra scripts converge one review finding per round** (this session: equivalence gating → receipts TU coverage → id truthfulness → stale evidence → TOCTOU revalidation — all legitimate). Budget for it; apply the pre-dispatch audits from `lead-review-protocol.md` and sweep for adjacent instances before re-requesting a gate.
- Session-3 traps about `breakdown.json` (never edit/resync), gate-evaluate-then-status confirmation, cargo-ci lock/timeout behavior, and worker `git add -A` remain in force.

## Open questions needing invoker input

None pending. (Owner directives this session: campaigns run overnight via the deterministic runner; Opus for rework dispatches; e0251af3 citation amendment approved; 2ddf2f4b lead-takeover approved.)

## Reference artefacts

- Progress: `progress.json` here (wave plan re-derived 2026-08-13; `created_during_execution` carries 2ddf2f4b; `surfaced_pitfalls` has 15 entries incl. the preregistration-timing advisory that binds the aspirational estimator issues)
- Runner: `dev/scripts/permanent-campaign-runner.sh` (+ `.test.sh`, 8 stub tests), smoke evidence linked on `2ddf2f4b`
- Rank-deficiency receipt: `dev/bench_results/e0251af3-rank-event-rates.md`
- Accumulator: `crates/gf2-stats/src/accumulator.rs` + `tests/accumulator_contract.rs`
- Exact anchors: `crates/gf2-algebra/src/permanent/exact.rs`, `dev/benchmarks/permanent_campaign/exact-anchors.csv`
