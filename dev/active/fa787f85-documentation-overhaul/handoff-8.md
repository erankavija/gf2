# Handoff — Overhaul documentation for research adoption (fa787f85) — session 9

**Date:** 2026-10-05
**Session number:** 9
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4), `handoff-4.md` (session 5), `handoff-5.md` (session 6), `handoff-6.md` (session 7), `handoff-7.md` (session 8)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 11 of 16; wave 12 is partly unblocked.
- Children summary for the wave plan: 108 done, 21 backlog or ready, 2 rejected, 1 in_progress (`616e1d7c`).
- Active claims: `616e1d7c` (`agent:jit-execution-lead`, claimed 2026-10-05 for its gates).
- Open escalations: two (`198aafa3`, `616e1d7c`); below and in `progress.json` `escalations`.
- Worktrees of this epic: `agent-616e1d7c` (branch merged; kept for the rework). Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner rulings: decision ids match `D(EC)?-[0-9]+` (`.jit/config.toml`, `packages/jit-default` 1.0.2); `dev/docs` is a temporary managed path; criteria amended on `616e1d7c` REQ-01, `f79bcba3` REQ-01 and REQ-02, `103a792a` REQ-02; descriptions of terminal issues keep their text (DEC-01 on `eace5009`, `103a792a`); decoupling work is filed as tasks in this epic; the doctest time of `2596b143` is an informational wall-clock figure (DEC-01).
- `ffc35b8c` (sweep story) done: `holistic-review` passed after `repo-validate` ran on the widened pattern.
- Done this session: `4f161183`, `eace5009`, `103a792a` (two reworks), `83f9ce69`, `3759f995`, `f29a9225` (two reworks), `42037c91`, `1ca94ec2`. `f902240f` done after two reworks.
- Twelve container archives executed on main, each from a worker pre-flight: babcf05e, f9717e7e, 97bf0879, 026fc832, b7157be6, e095a100, 806eb14e, 2928ccce, d4851c3d, bb85c68a, 6efb756b, 6dc81018. `archive-rerun-results.md` records a final `--execute` of each that publishes, relinks and deletes nothing.
- Pins applied so plans retain consumer-read files: eleven references of `cef1ae5f` and `a82f2dd9` (OSD dataset and reference notes) at `744aa9b03`; seven references of `a83583e0` at `87b5b733c` and `eebbc778c`.
- `616e1d7c`: 6dc81018 re-archived (395 files moved, 6 copied); `code-review` and `doc-review` failed once (findings under What to do next).
- Regroup results: 7 zen3 entries under `dev/active/1a379447-zen3-cpu-performance/`; 7 entries under the b8206228, 86b9c719, b4b4b9ee and ae03bcd0 directories. Entries left in place under REQ-04, each with consumers at file:line: decoupling tables of `4f161183-regroup.md` and `eace5009-regroup.md`.
- Six worktrees of other epics removed with owner approval (133 GB to 138 GB free at that point); eight empty leftover directories removed.
- Session `gf2-53` leads epic 1a379447 and commits on main from the same checkout.

## What to do next

- [ ] `616e1d7c` rework 2 (worker `w11-616e1d7c`, branch `worktree-agent-616e1d7c`, merged): code-review F2, `dev/active/389aa4de/receipt-notes.md:21` cites the removed 6dc81018 handoff path; doc-review F2, `616e1d7c-verify.py:125` accepts three links that resolve only from the plan source path (`handoff-11.md:140`, `2026-09-01-eaae1b56.md:82,98` in the archive). F1 of both reviews is the open question on 389aa4de.
- [ ] File the decoupling tasks in this epic from the two decoupling tables (00dd43c3, 19513245, a203a23c, bc091474, bdc507a3 with f547c394, c04dd4ac, f8dd4dde, 4e732b56), one per consumer group, each decoupling then moving its entries; wire each under `fa787f85`.
- [ ] File a task: `96cea1b9-archive-rows.py --check` exits on "2 archive events for container 6dc81018"; it must accept several events.
- [ ] File a task: `docs/lean4-verification-pipeline.md` (moved by 130d5fd7) is cited at `dev/active/6dc81018-field-capability-dispatch/investigation.md:512,586` and `dev/active/ae03bcd0-general-bch/investigation.md:130,140`.
- [ ] Dispatch after the benchmark window, in cache-seeded worktrees (they build): `7039d680`, `602652bf`, `f79bcba3` (the 2037941f entry and `dev/bench_results/2037941f/` stay in place). Then `2596b143` once Rustdoc citation edits have stopped.
- [ ] Dispatch (no build): `5ec5dcc1`, `7dbcde0c`, `e1f262a7`, `0a0ece44`, `b2bb1065`, `ec655592`, `fdb998ec`. Brief `ec655592` and `b2bb1065` that `dev/plans/small_prime_kernel_strategy.md`, `dev/plans/sota_target_matrix.md`, `dev/plans/flint_promotion_evidence.md`, `dev/plans/ntl_promotion_evidence.md` and `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` are held in place by commit-pinned references: jit retains them, so they move by `git mv` under the relocation protocol and their manifest rows change from `retained-operational` to the archive destination.
- [ ] `0c5aa3b7` is ready (no build). `f6148d1e` and `f694f59a` wait on `616e1d7c`.
- [ ] `198aafa3`: wait for `gf2-53` to report `4c1e441f`, `ad2a6a58`, `f63a2464` done or rejected, or apply the owner's answer.
- [ ] Reclaim `agent-616e1d7c` with `reclaim-worker-worktree.sh` when the issue closes; nine worktrees of this session are reclaimed.
- [ ] Then waves 13 onward per `progress.json`.

## Traps — do not repeat these

Traps of `handoff.md` to `handoff-7.md` remain in force. New this session:

- **Do NOT execute a container archive from its eligibility alone.** `dev/simulation_results`, `dev/reference_data` and `dev/research` are managed paths, so an eligible plan moved the OSD dataset, deleted a digest-pinned reference file and three sources of a live research crate. Dispatch a pre-flight first: every `pending_deletions` source and every move with `already_archived == false` is checked against its manifest row, its consumers and its sha256.
- **Do NOT delete a live source whose bytes differ from the archived copy.** The executor skips that deletion for a reason; `f29a9225` failed code-review after the lead had it `git rm`'d. Put the last live bytes at the archive path, rescan the reference with `jit doc add`, then rerun.
- **Do NOT read handoff-7's "tracker references that name absent paths".** The blockers were two references mapping to one destination (babcf05e, 806eb14e: remove the duplicate reference and `git rm` the byte-identical live copy) and `unmanaged-selected-root` (the planner maps `dev/archive/<epic>/docs/x` back to `dev/docs/x`).
- **Do NOT order shared-source archives by convenience.** babcf05e's only real move was a source that the 97bf0879 and 026fc832 plans delete; babcf05e executes first.
- **Do NOT expect a rerun to be a no-op right after Phase B.** Edits inside an archive (link targets, self-path literals) make the next `--execute` adopt the changed files; a relative link that leaves the repository from the derived source path makes it ineligible (`unpreservable-layout`, `repository-escape`: b7157be6 `plan.md`, e095a100 Lean guide). Run `--execute` twice and record both.
- **Do NOT chain `git add` and `git commit` with `;`.** A failed pathspec left commit `a1f2e4f45` holding one file under a message describing eighteen; it was amended. Chain with `&&`, and guard merge subjects with a length check before the merge runs (two 72-character subjects were amended).
- **Do NOT commit a verifier or evidence script with a `dev/` path literal or a file list.** `3759f995` and `f902240f` failed `no-dev-path-coupling`; derive roots from the tracker or the `.jit-container` marker, inputs from the committed preview extract and `git log --grep 'jit:<id>'`, the root from `git rev-parse --show-toplevel`.
- **Do NOT repoint a path inside a sentence without rereading the sentence.** `103a792a`, `3759f995` and `1ca94ec2` each left a sentence that described a move or a location that no longer holds; the test is "true as written at HEAD".
- **Do NOT leave a manifest row of the unit's epic `pending` with an explanation.** `83f9ce69` failed REQ-05; a file the plan retains takes `retained-operational`, empty destination, `complete`, with the retaining reference named.
- **Do NOT call a directory absent while ignored files remain in the main checkout.** `dev/research/blas_sgemm_gf251/Cargo.lock` failed `f29a9225` doc-review; remove ignored leftovers after merging a move.
- **Do NOT tell a worker to hold a path because a branch lists commits on it.** `git log main..<branch>` lists patch-equivalent commits; `git cherry` shows whether any change is unmerged (`4f161183`, 613574db).
- **Do NOT run `git merge` of two regroup or archive branches without the row-wise manifest merge.** `migration/manifest.toml` conflicts on adjacent rows every time; resolve by taking, per line, the side that differs from the merge base and stop if both differ.
- **Do NOT grep `migration/manifest.toml` raw.** One row printed 60 kB into the session. Parse it with Python and print selected fields.
- **Do NOT `cd` into a worktree to run a probe.** The lead's shell moved there once this session; use `git -C` and subshells.
- **Do NOT run a build, `cargo-ci` or a gate between 01:55 and the `window end` line of `.agents/bench-window/window.log` on a night the window timer is armed,** and announce every gate batch to `gf2-53` before it starts; a commit on main during the other lead's CI detaches its verdict.
- **Worker `jit archive container <id> --json` can be denied by the worker's permission layer** (`w11-616e1d7c`, once). Leave the lead's preview in the worker's `target/`.
- **`jit doc add` without `--commit` unpins a pinned reference.** Check the `commit` field of every reference a rescan script re-adds before running it.

## Open questions needing invoker input

- Question: where does `dev/active/389aa4de/receipt-notes.md` go?
  - Context: `616e1d7c` fails both reviews on it; the tracker resolves 389aa4de under the open epic 86b9c719, the plan's "Active-layout ties" row assigns it to the archived 6dc81018, and no archive plan includes it.
  - Options: move it under `dev/active/86b9c719-quality-documentation-tech-debt/389aa4de/` and amend the tie row; add a reference so the 6dc81018 archive collects it; amend REQ-01 to leave it flat.
  - Recommendation: the first option; the tracker's dependency graph is authoritative for containment.
- Question: does `198aafa3` wait for `4c1e441f`, `ad2a6a58` and `f63a2464`, or is its REQ-01 amended as for `f79bcba3`?
  - Context: `gf2-53` states all three are live; their confirmations ran in the benchmark window of 2026-10-05 and each needs a further round and gates, more than a week in its estimate.
  - Options: wait; amend REQ-01 so the five entries with done owners move and the three stay and are reported.
  - Recommendation: the amendment, so the epic does not wait on another epic's schedule.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, rework_counts)
- Worker brief used for waves 11 and 12: `wave11-worker-brief.md`
- Archive evidence: `archive-rerun-results.md`; unit records `83f9ce69-rearchive.md`, `f29a9225-rearchive.md`, `3759f995-rearchive.md`, `f902240f-osd-archive.md`, `1ca94ec2-rearchive.md`, `42037c91-rearchive.md`, `616e1d7c-regroup.md`
- Regroup records: `4f161183-regroup.md`, `eace5009-regroup.md`, `103a792a-legacy-move.md`
- Migration manifest and checker: `migration/manifest.toml`, `migration/check.py`
