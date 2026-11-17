# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 11

**Date:** 2026-10-01T11:40+00:00
**Session number:** 11
**Prior handoffs:** none in the tree; the live traps of the retired handoffs are condensed under Traps.

## Current state

- Epic `ae03bcd0` — state: backlog (container); claimed by agent:jit-execution-lead.
- Wave in progress: wave 14 (`current_wave` = 14). Every child except `fd9d5416` is done (direct deps 5/6).
- Open: `fd9d5416` (Committed performance receipts), claimed by agent:worker. Preparation merged: `receipt-protocol.md` (linked), `render_receipt.py`, `test_render_receipt.py` (6/6 on a synthetic fixture). Gates doc-review, research-review pending.
- Open escalations: none.
- Progress file: `progress.json` here (R-55..R-59, session_notes_s11, surfaced_pitfalls).
- **Queued measurement:** the bench window (timer `gf2-bench-window-20261002`, 2026-10-01T23:00Z) runs `dbd8787d` (epic 6dc81018, ~3 h) and then `d1b4f85e`'s runner from `.agents/worktrees/agent-d1b4f85e-run` (detached at `4dff32e2a`), writing `2026-10-02-d1b4f85e-w1w2` in that worktree (~105 min). Host idle until ~04:30Z.

## What just happened

- Gate re-runs closed b1bd75ca, 3f7edef1. Code-review reworks closed 5ee83cd3, 97410c80 (generic `LayoutView`; 113ae672 delivered and closed), e1e0e7ff (production DVB-T2 and `ExtendedBchComponent` in the roster).
- Wave 12 (591a1c5e, ef8ff9c4, 0c21cb1e, 227ac5c8) and 94597a51 (Lean O-5) closed; 591a1c5e renamed non-comparable bench IDs (mapping in `smoke-run.md`).
- 4a2baa12 deleted the superseded surface (`BchCode::new/dvb_t2`, legacy encoder/decoder, `ExtendedBchCode`).
- f759d724 closed under invoker DEC-01 (archives out of scope). d1b4f85e closed (W1/W2 cells, dispatch records with digests, window runner). 4ad869d6 closed (bch module guide, two runnable examples, `tests/bch_examples.rs`).
- Invoker deleted-as-obsolete: all ROADMAP files, Copilot instructions and agent config (R-59). Filed cbd1eb28 (user-layout decode) outside the epic.

## What to do next

- [ ] After the window: confirm `window.log` shows the d1b4f85e job exit 0 and the runner's post-check passed; copy `2026-10-02-d1b4f85e-w1w2` from the run worktree onto main.
- [ ] Dispatch fd9d5416 (Opus) to render and commit the receipt with `render_receipt.py` under R-57/R-58, link it, then run doc-review and research-review. If Tier A/B shows a DVB-T2 W1 regression, a33fda32 becomes a prerequisite (REQ-13). If research-review rejects the cross-harness Tier B basis, take option B from the protocol note (legacy encoder re-measured at da47ec76 in its own window).
- [ ] Epic close: map success criteria, reconcile `surfaced_pitfalls` against criteria, `jit gate evaluate-all ae03bcd0`, completion report, transition, archive.
- [ ] Reclaim `agent-d1b4f85e-run` after copying its output.

## Traps — do not repeat these

- **`evaluate-all` stops at the first failing gate**, so a missing lake-build run surfaced as a code-review finding (94597a51). Run the executable gate first on Lean issues.
- **cargo-ci builds all features**, so a default-feature-only warning (a hip-gated test import) passes CI; check `cargo check -p <crate> --tests` without features when a change gates code by feature.
- **Commit with explicit paths.** Another session shares main; `git add .jit`/`commit -a` would sweep its files. Gate evaluations write `.jit` outside lock windows: check `git status --porcelain .jit` first, then stage issue files, `events.jsonl` and `index.json` by name. Never `git add .jit`; `.jit/gate-runs` is ignored and the add aborts a `&&` chain.
- **Merge into main only between gate runs**, so a running cargo-ci or AI review does not see a moving tree. Do not run cargo-ci on main or a heavy probe while workers build or measure; check `uptime` first. A transient GPU smoke-test SIGABRT (`gf2-sim::dvb_t2_regression`) passes isolated; rerun.
- **Kept bench IDs must measure the same workload** (591a1c5e F1); renamed cells need a committed ID mapping and smoke evidence.
- **Codex `exec` can hang silently at startup** (futex wait, no child, no session rollout). Probe for a fresh rollout file (`ls -t` plus mtime) within ~3 min of each dispatch; on a hang, kill and retry or use a native agent. `pgrep -f` matches its own wrapper shell, so do not use it for liveness.
- **Codex sandboxes** have no network, block the GPU device (hipGetDeviceCount error 100, a sandbox artifact) and may need `CARGO_CI_NO_LOCK=1 CARGO_CI_NO_SCCACHE=1`. Route network or device work to native agents; the lead's gate outside the sandbox is the evidence.
- **Codex workers cannot mutate jit state from worktrees**: linked-worktree git metadata sits under `.git/worktrees/<name>/`, outside the sandbox. Pass `-c sandbox_workspace_write.writable_roots=[...]` or commit for the worker; the lead links docs after merge.
- **`jit gate evaluate` retries its checker when the repo changes** (`with_mutation_attempts`) and silently discards a completed verdict. Freeze repo writes during long gates; salvage a discarded verdict from the codex rollout. Fix demanded upstream; not confirmed fixed.
- **`jit gate evaluate` exit codes and verdicts**: exit 1 with "Failed to restore recovery serialization" means the checker never ran; run `jit recover` and evaluate again. "ERROR: Agent produced no output" is a checker failure, not a verdict. Read `jit gate status`, not the exit code. A detached gate chain fails every link once the first does.
- **`jit issue claim` refuses** an existing assignee (run `jit issue release` first) and a dep-blocked issue (use `jit issue assign` and verify the assignee). `cmd | tail` hides the refusal. `jit issue create --priority medium` is rejected; omit the flag.
- **Done-transitions respect the DAG**: an issue refuses `done` while a dependency is in progress. Order transitions by dependency.
- **`jit project render`** regenerates AGENTS.md's invariants after editing `.jit/invariants.toml`; `jit validate` flags the stale projection until run.
- **`repo-validate` blocks on any isolated issue repo-wide.** Wire creations at creation time: follow-ups depend on the epic, standalone bugs depend on the issue that made them relevant, and live issues depend on archived anchors, never on a child of a closed container. `jit dep add --reduce` may drop other redundant edges; verify the tree afterward. Story-to-epic edges are waits-for edges, not containment inversions.
- **`jit doc` links pin content hashes.** Any change to a linked doc, including another issue's done doc, needs `jit doc remove` then `jit doc add` as the last step; check `jit doc list`, since remove can fail silently when piped. Link receipts, proof sketches and design docs right after merge, before gates. Refresh a done issue's design doc when its anchor test lands.
- **`make-receipt.py` prints to stdout.** Redirect it to the receipt file, then verify the rendered hash table against the files.
- **Measure only from committed revisions.** Commit harness edits first and record `git rev-parse HEAD`; research review rejects receipts whose revision lacks the measured source. Benchmark cells must match the contract's declared cache state (fresh allocation for W2).
- **Keep the persistent forum recv loop** (`while true` around `forum.sh recv --as agent:claude` in a Monitor) up for the whole session; a lapsed loop blocks peers. Relaunch on death, check the mailbox after any gap, and never route its output to null.
- **Never `pkill -f scripts/cargo-ci.sh`**: it kills every worker's CI. State "never pkill by pattern" in every dispatch.
- **Shell chains through wrappers do not propagate failure.** Guard every step with `|| { echo FAIL-<step>; exit 1; }` and check artifacts are non-empty. `cmd | tail -N && next` takes tail's status. A zsh `IFS='|' read` loop inside one Bash call broke PATH; use separate commands.
- **Another lead's protocol may have defects** (cargo bench rejects `--release`; `jq -rs` writes raw strings). Report defects and get approval before deviating; never deviate silently.
- **Pooled or seeded worktree targets carry stale artifacts**: `gf2pow32_*` in the pool, and `gf2-kernels-simd` (`could not find shift_funnel`, `unresolved import gf2_kernels_simd::bch_encode`). Purge `gf2pow32_*` and run `cargo clean -p gf2-kernels-simd --profile <p>` after each harvest; tell every worker up front.
- **New algorithm families must satisfy every completed contract on their path**; name them in the dispatch prompt (a lazily built table family failed a done issue's allocation-free REQ-01).
- **Review rounds**: a round-N pass does not bound round N+1; reviewers probe adjacent contract surfaces. When a review direction implies hardening a non-adversarial API, escalate the frame to the owner instead of fixing level N+1; codify the `_unchecked` vocabulary and cite the invariant. Sweep every instance of a doc claim the reviewer cites.
- **Do not ship an untestable fallback** to satisfy a review pattern; revert it and record why.
- **Do not state a measured number in prose without a committed run artifact.** Commit the verbatim script that ran, never a reconstruction; publish probe figures only from the scope's own accounting (`scope.txt`), not a sampler without a PID.
- **Representation-specialization traits stay public**, with provided bodies and in-tree overrides; a newly public caller-trusted method needs a `# Panics` section.
- **A plan amendment does not bind a reviewer enforcing the issue's text.** Binding places are the issue description, criteria or a `## Decisions` item.
- **Trait-widening and a suite consuming it in parallel need a merge plan**: route the consumer through `git merge main` after the first lands.
- **Waiting on teammates**: give an idle one a single SendMessage nudge, then stop and re-dispatch fresh. A Sonnet worker that backgrounds cargo-ci waits until notified; nudge it.
- **Worker reports truncate ~3 KB in SendMessage.** Tell workers to lead with essentials; budget a round-trip for the tail.
- **Environment**: `free -g` fails under the user's shell alias (use `command free -m`); `/usr/bin/time` is not installed. `cargo` spawns sccache under the CCX1 flock and the server inherits the lock fd; check `lslocks | grep ccx1` after locked runs, then `sccache --stop-server` or set `CARGO_CI_NO_SCCACHE=1`. `/tmp` is tmpfs: check `du -sh /tmp/gf2-*` before a memory-heavy run, since `used` memory misleads.

## Open questions needing invoker input

None.

## Reference artefacts

- Receipt protocol: `receipt-protocol.md`; renderer `render_receipt.py`.
- Runner: `run.sh`; smoke evidence `smoke-run.md`; contract amendments in `workload-selection.md`.
- Baselines: `88ca7d2f/`, survey `findings.md`.
- Queue: `dev/scripts/bench-window/queue.tsv`.
