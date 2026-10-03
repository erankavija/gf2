# Handoff — Overhaul documentation for research adoption (fa787f85) — session 3

**Date:** 2026-10-03
**Session number:** 3 (session 2 ended without a handoff; its state was reconstructed from `progress.json`, JIT and git)
**Prior handoffs:** `handoff.md` (session 1)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Waves 6 and 7 dispatched and reviewed. Everything still open in them is blocked on the owner (below).
- Children summary for the subtree: 65 backlog, 49 done, 8 in_progress, 3 ready, 1 rejected.
- Active claims (`agent:worker`, blocked on the owner): `12907582`, `5a25717c`, `ec7d2aec`, `fb209a81`, `b16d013a`, `7bac1303`, `01c20b61`, `ee50e270`.
- Open escalations: ten, listed under Open questions; each is also in `progress.json` `escalations`.
- Progress file: `progress.json` in this directory.

## What just happened

- Reconciled the stale `progress.json` against JIT: wave 6 was mostly done; 14 page and move tasks were pulled forward from wave 7 into wave 6.
- Owner rulings this session: committed the `.agents/skills` edits (9b219e095); released the tuning-tooling hold; approved a final full-sweep rework for 12907582; instructed the lead never to stop for questions.
- Wave 6 done: 1fca3739, 2ff9953a, 44f4586d, 4f752758, 5625a423, 698fa793, 6c314026, 8b42d23b, 9dcc0cae, da120b98, deda0962, 112f23c6, 65a5436a. Common rework causes: plain-text math (LaTeX required), README Rustdoc link and feature-table citation, uncited comparisons, universal statements that one case contradicts, hand-copied counts in records.
- Wave 7 done: 130d5fd7, 153297cf, 8a9156fc, cdba4e71. Blocked: 01c20b61 (final attempt used), ee50e270 (cargo-ci evidence).
- Merged on main with gates open (owner-blocked): 12907582, 5a25717c, fb209a81, b16d013a. Unmerged branch: `worktree-agent-ec7d2aec`. No work on 7bac1303.
- Filed in the epic: b0b1faf5 (survey scripts hard-code the f547c394 producing manifest; 602652bf waits on it) and 629e0ff0 (gf2-coding standards doc and test drift).
- Fixed in passing: `perf-evidence-catalog.md` gained §6 recording its inverted comparator readings; `.gitignore` ignores `__pycache__/` (the new `dev/scripts/repository_files.py` helper wrote one on every cargo-ci run).
- Host disk filled to 100% while 17 worktrees were created. All merged worktrees were reclaimed without harvesting caches (`LEAD_CACHE_DIRS=__none__`); the cache pool is 80 GB.

## What to do next

- [ ] Apply the owner's answers to the Open questions, then re-gate the affected issues on main.
- [ ] After ec7d2aec merges: run the `jit profile` re-mint, `jit validate`, then 67048b47.
- [ ] Wave 7 remainder: 62f0d0e6 (pre-sweep census), c84ac71a (after 7bac1303), 3fd3db5e (only once fd9d5416 is done or rejected — its REQ-01), 629e0ff0, b0b1faf5.
- [ ] Wave 8: the sweep units under ffc35b8c. Feed them the sweep pitfalls in `progress.json` `surfaced_pitfalls`.
- [ ] Reclaim the remaining worktrees once their issues close.

## Traps — do not repeat these

Session-1 traps (`handoff.md`) remain in force. New this session:

- **Do NOT create more than about six worktrees at once.** Each worktree is a full 55k-file checkout plus seeded caches. Seventeen filled the 912 GB disk, and one worktree (7bac1303) was created empty. Check `df -h /` before dispatch, and reclaim merged worktrees as each wave closes.
- **Do NOT write an unquoted heredoc that contains backticks.** zsh executes the backtick spans as commands. Use `<<'EOF'` and pass values through the environment.
- **Do NOT gate while workers build.** cargo-ci failed under load average ~30 on gf2-sim 15 s timeouts and on fixture-leak counts that unrelated workers moved; both passed on an idle host. Gate after a wave's workers finish.
- **Do NOT merge onto main while a gate batch is running.** The runs judge HEAD; a merge mid-run records `inputs_moved`. Batch the merges, then gate.
- **Do NOT reuse one README rule set without checking reviewer consistency.** doc-review accepted `[Rustdoc](../../target/doc/<crate>/index.html)` for gf2-core and gf2-coding, then rejected the same form three times for gf2-sim ("not durable"). This is the open question on b16d013a.
- **Do NOT let a worker edit a digest-pinned protocol to match new code.** 5a25717c edited `dev/active/f547c394/protocol.md`, whose SHA-256 is pinned by committed receipts; the edit was reverted. A protocol text change is an owner decision.
- **Do NOT let a doc fix touch a line carrying an old false claim.** The edited line becomes the issue's own, and the reviewer flags the claim as a regression (12907582: gfx1030 in gf2-algebra `gpu.rs`).
- **Do NOT transition issues from a script whose status check can fail silently.** A broken python one-liner marked five issues done unchecked. They were verified afterwards and all had passed; check gate JSON before every `--state done`.
- **Do NOT expect `jit dep add` to accept a redundant edge.** Adding the edge a containment already implies is rejected; use `--reduce`.

## Open questions needing invoker input

- Question: approve one more 12907582 rework for two factual sentences, or split the remaining drift into a new task and close 12907582?
  - Context: the rework limit is exceeded (4 rounds, 7+ gate failures). code-review F1: `crates/gf2-core/src/lib.rs:340` misstates the Candidate C dispatch rule. F2: `crates/gf2-algebra/src/permanent/mod.rs:13` says $n \le 64$ for $\mathbb{F}_5$ where the code requires $n \le 63$.
  - Recommendation: approve a two-sentence rework; the findings are concrete.
- Question: how should rule P-02 in the digest-pinned `dev/active/f547c394/protocol.md` be reconciled with content-identity acceptance (5a25717c)?
  - Context: the protocol's SHA-256 `e0f67e4e…` is pinned by committed receipts. Editing the file changes the identity that future runs pin. Both reviewers block on the mismatch.
  - Options: issue a protocol amendment or new version; or accept the recorded discrepancy.
  - Recommendation: a protocol amendment.
- Question: may ec7d2aec REQ-01 cover only container-archive rows under `development_root`, and may the `packages/jit-default` profile be re-minted?
  - Context: the 24 `docs/` rows (decks, Lean guide) cannot sit under a managed path. They move by `git mv`. Removing absent policy entries requires a profile re-mint (precedent 08da466f9). The worker also added `dev/archive` to `managed_paths`.
  - Recommendation: amend REQ-01, approve the re-mint, and confirm `dev/archive` as managed.
- Question: should fb209a81 absorb 1adfe2e9 (the figure example's output directory)?
  - Context: doc-review blocks fb209a81 because `gen_presentation_figures.rs` still writes `docs/presentations/figures`. 1adfe2e9 owns that change but depends on fb209a81 and on the sweep story.
  - Options: absorb; or drop the edge from 1adfe2e9 to ffc35b8c and run 1adfe2e9 first.
  - Recommendation: absorb.
- Question: what satisfies b16d013a REQ-03 ("links to the crate's Rustdoc") while the crates are unpublished, and how much capability statement does REQ-01 allow?
  - Context: doc-review rejected the `target/doc` link three times for gf2-sim, but accepted the same link for gf2-core and gf2-coding. It also rejected the capability list twice.
  - Options: hosted Rustdoc; accept the generated link; the command only.
  - Recommendation: accept the generated link and amend REQ-03 to say so.
- Question: 7bac1303 — narrow REQ-02 or widen the scope, and approve `[patch.crates-io]` in the repository `.cargo/config.toml`?
  - Context: the harness crates reach other paths by directory-counting, so a deeper copy fails to build under REQ-03. The only depth-independent form found needs shared config and `Cargo.lock` changes.
  - Recommendation: widen the scope to every path dependency, and approve the patch form.
- Question: re-add `cargo-ci` to ee50e270, and to the other README issues whose REQ cites "Rust CI gate passes"?
  - Context: doc-review requires cargo-ci evidence. The owner removed cargo-ci from these issues on 2026-10-02.
  - Recommendation: re-add.
- Question: 01c20b61 — fix the bench-window runner's hard-coded `repo=/home/vkaskivuo/Projects/gf2` (epic 1a379447 tooling) first, or accept the page?
  - Context: the final rework is used. doc-review blocks because the queued reproduction cannot run from another clone path.
  - Recommendation: file and fix the runner, then re-gate.
- Question: push `main` to origin (currently at f9224650, 2026-08-14)?
  - Context: commit-pinned evidence links on permanent pages resolve on GitHub only after a push.
  - Recommendation: push once the owner has reviewed the session's commits.
- Question: which containers should hold the out-of-scope defects?
  - Context: `progress.json` `surfaced_pitfalls` entries marked "file issue: owner picks container": the GPU BCJR panic, sim_runner SIGINT and resume cache, the checkpoint config hash (correctness), `trinomials(8)` (correctness), the tuning-profile schema, and the window-runner path.
  - Recommendation: the correctness defects go to their crate epics first.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes)
- Worker-prompt rules: the "Worker-prompt lessons" and "README rules" entries in `progress.json` notes
- Gate scripts: `contrib/gates/docs-mechanical.py`, `contrib/gates/doc-review-prompt.md`
