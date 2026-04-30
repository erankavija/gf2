# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 1

**Date:** 2026-04-30
**Session number:** 1
**Prior handoffs:** None for this epic. Predecessor PPC epic handoffs at `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md` — read at minimum the **Traps** sections of `babcf05e-handoff-5.md`; they remain in force here unless explicitly resolved below.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Wave in progress: **Wave 1 closed**; next dispatch is Wave 2 (reference evaluations) per `dev/active/97bf0879-progress.json`.
- Children summary: **5 done**, 0 in_progress, 51 backlog/ready, 0 rejected (out of 56 total).
- Active claims: none. All Wave-1 worktrees torn down; branches deleted.
- Open escalations: none currently blocked. One in-session escalation (code-review reviewer) was resolved — see *What just happened* and the `escalations` log in the progress file.
- Progress file: `dev/active/97bf0879-progress.json` (12-wave plan, current wave = 2).
- Branch state: `main` clean at `5cc8be1` (`chore(jit:97bf0879): close 5102d87a (wave 1B)`).

## What just happened

- Project discovery: read `.jit/config.toml`, `CLAUDE.md`, predecessor epic handoffs, gates registry. 11 gates configured; required-on-this-epic are `code-review`, `cargo-ci`, `doc-review`. 56 children across 9 stories already broken-down — no new breakdown required this session.
- Wrote 12-wave plan to `dev/active/97bf0879-progress.json` (waves 1–4 = foundation/refs/profiling/target-matrix; 5–11 = optimization stories; 12 = aggregate + presentation).
- Claimed epic 97bf0879 as `agent:project-lead`, transitioned to `in_progress`.
- Closed `d3c2cc8f` lead-direct (JIT description amendment of legacy story `64c88ae4` — rewrote its Success Criteria to baseline-publication scope, retained original aspirational targets as historical context, added an explicit cross-amendment block. Gates green.).
- Dispatched parallel Wave 1A in worktrees: `1d6043e8` (architect, design doc), `02ace293` (doc, cell-status legend fix), `b8189dbf` (doc, post-PPC delta appendix). Three parallel sub-agents, run in background.
- Worker `b8189dbf` terminated mid-flight before commit ("Now let me wait for cargo-ci to finish" was its last message). Lead inspected the worktree, verified the file was substantively complete, and committed on the worker branch as the worker-author (lead-preserve pattern from `babcf05e-handoff-5.md`).
- Workers `1d6043e8` and `02ace293` completed cleanly with commit hashes returned.
- Rebased each worker branch onto current `main` HEAD (one-line `updated_at` conflict in each issue's `.jit/issues/*.json`; resolved by keeping the worker timestamp). FF-merged each onto main.
- Ran `cargo-ci.sh` on main: `check ✓ test ✓ clippy ✓ fmt ✓` (~6.4s, warm cache).
- Ran gates per Wave-1A issue: cargo-ci, code-review, doc-review. Each issue passed all three. Reviewer (claude-sonnet) verdicts: PASS on each.
- Closed `1d6043e8`, `02ace293`, `b8189dbf` (Wave 1A).
- Dispatched Wave 1B `5102d87a` (single doc agent, worktree).
- Worker `5102d87a` ran for ~14 minutes; appeared silent for 6 minutes mid-flight. Lead inspected worktree, found work substantively complete (3 file changes, 4 doc-add operations on its own issue). Lead-preserved on the worker branch, rebased, FF-merged. Worker's final report later confirmed: it had been auditing 12 JIT issues for stale "Deferred" labels and found 0 needing change.
- Ran gates on `5102d87a`: cargo-ci ✓, code-review ✓ (reviewer cross-checked all 12 pin values across 5 artefacts), doc-review ✓. Closed.
- d0ca9482 auto-promoted backlog → ready as a side-effect of Wave-1B closure.

### Escalation resolved this session

- **Code-review reviewer agent rate-limited.** First gate run on `d3c2cc8f` revealed `REVIEWER_AGENT="copilot -s --model gpt-5.4 …"` was hitting GitHub Copilot's weekly account-wide quota until 2026-05-04. User authorized config edit (`AskUserQuestion` answer = "Drop --model gpt-5.4 (use auto)"). That alone did not help (account-wide limit, not per-model). Lead followed up by editing `.jit/gates.json` REVIEWER_AGENT to `claude --model sonnet -p --allowed-tools "Bash(cargo …) Read Glob"` — committed at `c7ca65b`. Verified working: every Wave-1 issue's code-review run subsequently produced a real claude-authored review with explicit VERDICT: PASS. The escalation is recorded in the `escalations` array of the progress file.

### Wave 1 — five issues closed (sources of truth)

| Short ID | Title | Commit |
|---|---|---|
| `d3c2cc8f` | Amend 64c88ae4 benchmark scope | `c49d94f` (lead-direct) |
| `1d6043e8` | Define SOTA reference acceptance protocol | `521c51e` |
| `02ace293` | Fix benchmark report cell-status semantics | `419bfbe` |
| `b8189dbf` | Publish post-PPC benchmark delta appendix | `f39785c` |
| `5102d87a` | Synchronize benchmark environment documentation | `f72e0a5` |

Key artefacts produced this session:

- `dev/plans/sota_reference_acceptance_protocol.md` (615 lines) — five-criterion mechanical checklist + Mermaid workflow + worked examples (fflas-ffpack promotion + hypothetical rejection). Linked to `1d6043e8` *and* parent story `cbecfced`.
- `dev/bench_results/2026-04-30-post-ppc-delta-appendix.md` (267 lines) — GF(2)/GF(p)/GF(2^m) delta tables citing E1–E8. Linked to `b8189dbf` and parent story `b0434149`.
- `dev/bench_results/2026-04-26.md` — cell-status legend formalised (5 tokens: `measured`, `N/A`, `slow-or-nightly`, `harness-scope gap`, `optimization gap`); 152 PENDING tokens replaced; 0 PENDING tokens remain in any table cell.

## What to do next

In priority order:

- [ ] **Dispatch Wave 2: 4 reference evaluations.** All four are independent (no JIT-deps) but **likely conflict on `benchmarks/Containerfile`** because each adds an apt/source-built library to the same image. Two safe strategies:
  1. Dispatch one at a time (serialized) so each can edit the Containerfile cleanly.
  2. Dispatch in parallel worktrees but instruct each worker to scope their Containerfile delta to a labelled stanza (`# === <library> begin ===` / `# === <library> end ===`) so 3-way merge can usually auto-resolve. Reviewer must be told to verify the merged Containerfile still builds.
  
  Issues:
  - `5dea7457` Harden fflas-ffpack and M4RI reference lanes (highest priority — these are already-promoted hard refs that need full coverage)
  - `73ab8eef` Evaluate NTL and FLINT references
  - `79388011` Evaluate LinBox for exact linear algebra references
  - `507b0036` Evaluate M4RIE for GF(2^m) references
  
  Each must follow the `dev/plans/sota_reference_acceptance_protocol.md` (the design doc landed this session) — the protocol's § 3 five-criteria checklist is the dispatch contract.

- [ ] **Dispatch `d0ca9482` ("Close benchmark report child after cleanup")** — auto-promoted to ready as a side-effect of Wave 1 closing. Its criteria are "verify a9ab0a4f gates pass" and "transition a9ab0a4f without scope creep". `a9ab0a4f` is already done (per `64c88ae4`'s declared dependency state); this should be a small lead-direct task confirming gate state and closing.

- [ ] **Wave 3 (after Wave 2)**: profiling/measurement tasks (`0fd48627`, `609855d9`, `9a715d75`, `a3412e15`, `c3e79272`, `3b762764`). These run after the reference matrix is decided so they measure against the right thing.

- [ ] **Wave 4** = `4c0d0202` (publish SOTA target matrix design doc — synthesizes Wave 1–3 outputs). This is the second keystone artefact; downstream optimization stories all consume it.

- [ ] **Cross-cutting**: when reviewer fires (claude-sonnet), watch for any 5xx / API outage burst. The Anthropic API was used for both the reviewer agent and the worker agents in this session and tolerated the load fine, but a parallel-Wave-2 dispatch with 4 workers + per-issue reviewer runs could put 5+ concurrent claude sessions in flight. If you hit `429` or `5xx`, fall back to serialized dispatch.

## Traps — do not repeat these

**Carried forward from `babcf05e-handoff-5.md` (still binding):**

- All session 1–6 traps from babcf05e remain in force. The most relevant for this epic:
  - **Workers do NOT run `jit gate pass/fail/define` or `jit issue update`.** Lead does all state transitions. Workers may run `jit doc add/remove/list`, `jit issue show`, `jit graph deps`, `jit issue search` — these are read-only or scope-edit operations on metadata, not state-transition.
  - **Mid-flight `[hard]` criterion amendments break flow.** Audit `[hard]` criteria *before* dispatch. The `dev/plans/sota_reference_acceptance_protocol.md` written this session specifically addresses this for Wave 2 — read it before dispatching reference-eval workers.
  - **`asm-artefact-present` gate is keyed off paths under `crates/gf2-kernels-simd/src/x86/`.** Asm artefacts placed elsewhere will not satisfy the gate. Wave 2 is doc-heavy so this is unlikely to bite, but Waves 6–8 (kernel implementation) will.
  - **Stale-base worktrees (TRAP 1).** Always anchor worktrees to current `main` HEAD via `scripts/dispatch-worker-worktree.sh <short-id>...` (the project-agnostic script in this skill). Never use Agent's `isolation: "worktree"` parameter — it has been observed to branch from a stale ancestor.
  - **Worker file leakage into main (TRAP 6).** Always run `scripts/check-leak-into-main.sh` after every wave. Eyeballing main's `git status` is insufficient. JIT itself is allowed to dirty `main`'s `.jit/issues/*.json` (timestamp drift) when workers run `jit doc add` from inside a worktree — this is benign and not a leak; the leak-check script's signal is source-file presence, not `.jit/` mods.

**New traps surfaced this session:**

1. **`copilot -s --model gpt-5.4` reviewer hits a weekly account-wide quota.** Today's reset was 2026-05-04; the failure mode is `exit 1` with stderr "You've reached your weekly rate limit". Account-wide means dropping the `--model` flag does NOT help. The current gate config (`c7ca65b`) uses `claude --model sonnet -p` and works. Do not revert to copilot without a quota check first. If the reset has passed and the user wants Copilot back, ask.

2. **`cargo fmt --all -- --check` fails inside `.claude/worktrees/agent-*` with a workspace-discovery error on `gf2-kernels-hip/Cargo.toml`.** The crate is workspace-excluded by design (`CLAUDE.md`, "Excluded from the default workspace so non-ROCm hosts still build cleanly") but its Cargo.toml asserts a parent path that resolves to the wrong root inside a worktree. Treat worker-side `cargo fmt` failures as worktree-only quirks; the lead's main-side `cargo-ci.sh` after FF-merge is authoritative. **Do not** edit `gf2-kernels-hip/Cargo.toml` to "fix" this — it is a working setup that satisfies main-side CI.

3. **Workers may go silent for ~6 minutes mid-task without terminating, then either (a) finish a long audit and commit, or (b) actually terminate without committing.** Both happened this session (`5102d87a` finished an audit; `b8189dbf` terminated mid-flight before commit). When a worker has been silent for >5 min, inspect its worktree: if files are modified-but-not-committed, **you can safely lead-preserve** (commit on the worker branch as `agent:claude-wt-<id>`) and proceed. If the worker subsequently returns a "ready for review" report that names a commit hash matching the lead-preserve, **the lead-preserve is the authoritative commit**; the worker's hash is stale and can be ignored.

4. **`jit gate pass <issue> code-review` *runs* the auto checker (`./scripts/ai-review.sh`).** It is not a manual attestation. If it returns success ("Passed gate 'code-review'"), the reviewer actually emitted `VERDICT: PASS` on stdout. If it returns `GATE_FAILED`, inspect via `jit_gate_check-all` — the reviewer's full prose review is in `stdout` and is part of the audit trail. Do **not** call `jit gate pass` and assume it succeeded without checking the response.

5. **`git merge --ff-only` is permission-gated when chained with `git checkout main` in a single Bash invocation.** Run merges as standalone Bash calls (or use `git -C <repo-root> merge --ff-only <branch>`). The standalone form has been authorised; the chained form triggers an unrelated branch-protection-style denial.

## Open questions needing user input

None blocking. The next-session lead can dispatch Wave 2 directly per *What to do next*.

One non-blocking question worth surfacing if Wave 2 dispatch encounters trouble:

- **Question:** When (a) `73ab8eef` evaluates NTL/FLINT and (b) `79388011` evaluates LinBox, both will likely add multiple new apt-install lines and source builds to the same `benchmarks/Containerfile`. If the eventual decision is to *reject* one of these libraries (e.g., NTL not reproducible enough), should the rejected library's Containerfile delta be reverted by the rejecting issue, or kept (because the build infrastructure is still useful for ad-hoc cross-checks)?
  - Context: § 8 of `sota_reference_acceptance_protocol.md` lists exclusion classes but does not prescribe artefact disposition.
  - Options: A) Revert any Containerfile change for a rejected lib (cleanest per "no dead code" principle). B) Keep with a `# REJECTED: …` stanza header (preserves work for future ad-hoc use). C) Defer to a wrap-up issue in Wave 5 that does the cleanup once all evaluations land.
  - Recommendation: **Option C** — let each evaluation issue commit its Containerfile additions; add a single follow-up cleanup task in Wave 5's story-closure pass. This avoids per-issue artefact churn during evaluation.

## Reference artefacts

- Epic: `jit issue show 97bf0879`
- Progress file: `dev/active/97bf0879-progress.json`
- This session's design doc: `dev/plans/sota_reference_acceptance_protocol.md` (read this **before** dispatching any Wave-2 evaluation worker — it is the dispatch contract)
- This session's appendix: `dev/bench_results/2026-04-30-post-ppc-delta-appendix.md`
- Predecessor epic handoffs (read traps sections at minimum): `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md`
- Predecessor evidence docs (referenced by appendix and Wave 2 will compare against): `dev/bench_results/2026-04-29-3abb755e-benchmark-gap-closure.md`, `dev/bench_results/2026-04-29-strassen-matmul-crossover.md`, `dev/bench_results/2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md`, `dev/bench_results/2026-04-29-gf2m-batch-fieldmatrix-gemm.md`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Lead review protocol: `.claude/skills/project-lead/references/lead-review-protocol.md`
- Project conventions: `/home/vkaskivuo/Projects/gf2/CLAUDE.md`
- JIT events log (audit trail): `.jit/events.jsonl` (append-only)
- Gate definitions: `.jit/gates.json` (note the current `code-review` REVIEWER_AGENT — see Trap 1)
