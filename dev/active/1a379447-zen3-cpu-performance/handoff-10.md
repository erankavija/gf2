# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 13

**Date:** 2026-09-13T12:58Z (draft written mid-wait; the closing section states what changed after it)
**Session number:** 13
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md), [session 4](handoff-4.md), [session 6 halt](handoff-5.md), [session 7](handoff-6.md), [session 9](handoff-7.md), [session 10](handoff-8.md), [session 11](handoff-9.md). Session 12 wrote no handoff (usage limit at 09:22Z); its state is the session-12 notes in [progress.json](progress.json) and its commits 2dca1c7a..b441ee71.

## Current state

- Epic: `1a379447`, backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 3 of 7 (07ca8585 from wave 4 runs alongside because 3be770d5 is done). `jit issue status` is authoritative.
- Closed this session: `1c602857` (round 2, four gates pass, `reviews/1c602857-r3.md`). Closed in session 12: `428f2f6b`, `6c6b09b1`, `04b85d10`, `12fdeb5b`, `a203a23c`.
- Running at draft time, all native subagents named `w13-<id>` (a new session starts them fresh): `5cbb6545`, `1d4fd63d`, `53c5a8c0` (opus, worktrees `.agents/worktrees/agent-<id>`, base 2d8b6c0b with main merged), `07ca8585` (opus, in the reused `.agents/worktrees/agent-1c602857` directory on branch `worktree-agent-07ca8585`, base e479a99c). Each has eight or more commits on its branch (`git log main..<branch>`).
- Ready, not started: `19513245`; planning nodes `8d8be934` (for story 2037941f) and `8ed3ac58` (for story c04dd4ac); bugs `2c487595`, `706a8f93`; new task `94bbe5d7` (freezer stage prose). The invoker ended new dispatch for this session at 12:56Z.
- Active claims: assignee `agent:worker` on the four running issues (leases lapse; re-claim before gating). No leases held by the lead.
- Open escalations: none.
- Progress file: [progress.json](progress.json).

### Per-issue state

| Issue | Branch / worktree | State and next step |
|---|---|---|
| `1c602857` | **done**; branch `worktree-agent-1c602857` merged at `c1cd9256`; directory reused by 07ca8585 | Nothing. Advisory A1 (confirmation addendum keeps the pilot's stage sentence) is a pitfall + task `94bbe5d7`. |
| `5cbb6545` | `worktree-agent-5cbb6545` in `.agents/worktrees/agent-5cbb6545` | Worker running: survey, shared count suite, asm artefacts, campaign tables, crossover sweeps, adopted a measured 256-word carry-save boundary. Next: worker report, then lead review (`lead-review-protocol.md`, all tiers), merge, four gates + asm-artefact-present, links from `/tmp/claude-1000/.../scratchpad/5cbb6545-links.tsv` (scratchpad of session 13; re-request from the branch if gone). |
| `1d4fd63d` | `worktree-agent-1d4fd63d` in `.agents/worktrees/agent-1d4fd63d` | Worker running: ranking pilot of three lanes, selected-lane stage, frozen confirmation of the avx2-ymm6 lane, comparator family against M4RI/Bitshuffle. Same next steps. |
| `53c5a8c0` | `worktree-agent-53c5a8c0` in `.agents/worktrees/agent-53c5a8c0` | Worker running: arm executables, frozen crossover/polynomial grid, holdout cells through the canonical freezer, wire smoke, two arm fixes. Campaigns may need the benchmark window (queue lines in its report). Same next steps. |
| `07ca8585` | `worktree-agent-07ca8585` in `.agents/worktrees/agent-1c602857` (reused directory) | Worker running: structural/allocation evidence, single-worker and comparator pilots and confirmations, campaigns exceeding a session queued (see its report for queue lines), tables projected. Lead decision recorded in progress notes: the disclosed numerical alignment (canonical reduction follows the scalar reference; AVX2 lane divergence at degree >= 9 on -0.0/NaN is bug 39cbde20) is accepted with conditions (findings section, rustdoc citing `@/issue/39cbde20`, divergence test, kernel untouched, pinned before arm). Verify the conditions in review. |
| `2037941f`, `c04dd4ac` | no worktree | Stories, breakable: brackets scaffolded at 8295b69f (planning `8d8be934`/`8ed3ac58` with plan-review gate; breakdown `30c3aef1`/`16560579` with coverage-preview + breakdown-review). Next: run the jit-planning-lead flow per story (investigator, manifest-first synthesizer, reviewer, plan-review gate, then jit-breakdown on the story, coverage-preview + breakdown-review), then wave-plan the children. c04dd4ac's implementation leaves need the eda07788 AFF3CT arms rebuilt first (session-11 trap). |

## What just happened

- Resumed after session 12's usage-limit death: recovered the four session-12 dispatch prompts from its transcript (`~/.claude/projects/-home-vkaskivuo-Projects-gf2/a6191159-*.jsonl`, Agent tool_use inputs) and re-dispatched them with resumption notes about the uncommitted WIP each worktree held.
- The `contrib/gates/{code,doc}-review-prompt.md` working-tree diff on startup was `jit recover`'s derived-state repair reverting 33f08f02 (clause added to the projections without the package source). Fixed at the source: package assets + manifest 1.2.1 + `jit profile upgrade --profile path:packages/sim-research` (3dad2478).
- 1c602857 round 2: worker closed both round-2 findings; lead commit 529662f0 extended the predicate citation to four test doc-comments that still said "PCLMULQDQ alone"; merged c1cd9256; code/doc/research pass on e479a99c, cargo-ci on b9989fc7 (5987 tests); closed at 42a976a5.
- Scaffolded plan brackets on the two stories (8295b69f). Filed `94bbe5d7` (freezer copies the pilot's stage sentence). Amended the worker brief (`--family-description`) and messaged the four workers.
- 07ca8585 interim: answered its "main is broken" report (partial-feature commands; cargo-ci passes; 94ab019f tracks the test gate) and accepted its disclosed numerical alignment (progress notes).

## What to do next

- [ ] Collect the four workers' branches: review each per `lead-review-protocol.md`, merge one at a time (`--no-ff`), run reviews then cargo-ci under the full-host lock, `asm-artefact-present` where defined, link artifacts, close. Reclaim `agent-5cbb6545`, `agent-1d4fd63d`, `agent-53c5a8c0` after; keep `agent-1c602857` (07ca8585) or reclaim after its close.
- [ ] Queue lines the workers report go into `bench-window/queue.tsv` with `env PATH=` prefixes where the launcher does not export cargo; arm the window; collect from each campaign's own log.
- [ ] Dispatch `19513245` (task, opus) and the two planning nodes (jit-planning-lead flow), then the small tasks `2c487595`, `706a8f93`, `94bbe5d7` (sonnet).
- [ ] Rebuild the eda07788 arms before c04dd4ac's leaves dispatch.
- [ ] `63bad95d` unblocks when the seven wave-3 items close; `f63a2464` when 07ca8585 closes.

## Traps — do not repeat these

- **Do NOT edit a jit profile projection without its package source.** `contrib/gates/*-prompt.md` are live targets of `packages/sim-research` (manifest `[[live]]` entries); `jit recover` and `jit validate --fix` re-materialize them from the package and silently revert a direct edit (happened to 33f08f02 on session-13 startup). Recipe: edit the asset under `packages/sim-research/assets/live/...` and the target to the same bytes, bump `[profile] version`, `jit profile upgrade --profile path:packages/sim-research`, commit assets, manifest, `.jit/profiles/sim-research.json`, `.jit/events.jsonl`.
- **Do NOT treat a worker's partial-feature `cargo clippy`/`cargo check` failure as a broken main.** The CI contract is `./scripts/cargo-ci.sh` (`--all-features`); `choose_k_block` dead-code and `tuning_conservative_cfg.rs` (94ab019f) fail only under partial features. 07ca8585 reported both as "pre-existing breakage blocking everyone".
- **Do NOT freeze a confirmation addendum without `--family-description`.** The canonical freezer copies the pilot's family prose, ending in "This addendum is the pilot: every cell is exploratory"; research-review flagged it (advisory) in two rounds on 1c602857. Task 94bbe5d7 changes the default.
- **Do NOT dispatch the stories 2037941f / c04dd4ac as if they were tasks.** `story` is in the plan template's `applies_to`; the bracket is scaffolded and the plan-review gate must pass on the planning node before breakdown. Session 12's wave note already said so; nothing had been scaffolded.
- **Do NOT lose a session's dispatch prompts with the session.** They live in the transcript jsonl as Agent tool_use inputs; a python pass over `type == assistant` messages recovers them verbatim (done this session for four workers). Consider committing prompts alongside `worker-brief.md` when they stabilize.
- **Do NOT `cd` into a worktree in the lead's shell.** The harness re-anchors the primary working directory to the worktree and refuses `cd` back; use `git -C <worktree>` and absolute paths.
- **Do NOT use `ls -t` in this zsh.** `ls` is aliased to a tool that rejects `-t`; use `/bin/ls`.
- Unresolved traps from [session 11](handoff-9.md#traps--do-not-repeat-these) and earlier remain in force (zsh `path` loop variable, `codex exec < /dev/null`, no full-command-line kills, comparator builds under `.agents/ext/<issue>` of the checkout that built them, window launchers must export `~/.cargo/bin`, no cargo-ci during three codex reviews, `cites:` labels must appear in text, `main...<branch>` for branch files, worker reports under 3500 characters).

## Open questions needing invoker input

- None.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [worker brief](worker-brief.md); [progress](progress.json); reviews under [reviews/](reviews/) (`1c602857-r3.md` this session).
- Protocol v4: `dev/active/f547c394/{protocol.md,amendment-v4.md,addendum.schema.json}`; canonical freezer `dev/active/c7113c5a/survey/freeze-confirmation.py`.
- Window: [run-window.sh](bench-window/run-window.sh), [queue.tsv](bench-window/queue.tsv), [follow-window.sh](bench-window/follow-window.sh).
- Story brackets: planning `8d8be934` (2037941f, plan at `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/plan.md`), `8ed3ac58` (c04dd4ac, plan at `dev/active/c04dd4ac-zen3-shifts-and-permutations/plan.md`); jit-planning-lead references under `.agents/skills/jit-planning-lead/references/`.
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`; leak baseline `/tmp/lead-pre-dispatch-latest.txt` (session 13: 20260913-113720).
- Gate records: `.jit/gate-runs/`.
