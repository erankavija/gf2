# Handoff — Overhaul documentation for research adoption (fa787f85) — session 5

**Date:** 2026-10-04
**Session number:** 5
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3), `handoff-3.md` (session 4)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 8 of 16 (the source-comment sweep under story `ffc35b8c`). Every sweep unit is written; six are merged with open gates and two are unmerged.
- Children summary for the wave plan: 82 done, 9 in_progress, 1 ready, 33 backlog, 2 rejected.
- Active claims (`agent:worker`, no worker running): `8f451167`, `f2064e90`, `aa0558c1`, `72768e37`, `c6aa0e84`, `13dfe1a5` (merged, gates open), `eba21481`, `50201002` (unmerged), `554c2935` (waits on the sweep).
- Open escalations: none. Every owner ruling of this session is in `progress.json` `escalations` and as a DEC on its issue.
- Worktrees of this epic: `agent-eba21481`, `agent-50201002`. Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Closed with all gates passed: `96cea1b9`, `37099e5b`, `f0dd63f1` (manifest lane); `290b8716`, `3aef9e33`, `67b5d1d8`, `3da13f39` (earlier sweep units); `4997a3ed`, `9dda3958`, `cafb463c`, `8a9db61a`, `f22ec590`, `93aed46a` (wave C).
- Owner rulings, each a DEC on its issue:
  - `290b8716` DEC-01: the first-parent diff of a merge is its attributable diff.
  - `3aef9e33` DEC-01: one more rework; the rustfmt layout after a comment deletion is accepted.
  - `93aed46a` DEC-01: one string-literal edit in `gf2-kernels-hip/build.rs`.
  - `ffc35b8c` DEC-01: behavior-preserving hunks that rustfmt or clippy force after a comment deletion are inside the sweep when the commit body names them.
  - Workers run clippy (`-D warnings`) and `cargo fmt --check` before the last commit; the step is in `ffc35b8c-sweep-brief-template.md`.
- Wave D (`8f451167`, `f2064e90`, `aa0558c1`, `72768e37`, `c6aa0e84`, `13dfe1a5`) merged. `cargo-ci` failed at `caa4b7929` on `clippy::collapsible_if` in `crates/gf2-coding/tests/ldpc_validation.rs`; the fix is commit `445b6169c`, after which workspace clippy and fmt pass. `cargo-ci` has not been re-run.
- Wave D review batch stopped. Recorded doc-review findings besides the `cargo-ci` evidence:
  - `13dfe1a5`: `tests/nr5g_regression.rs:96` restates the adjacent helper.
  - `c6aa0e84`: `bench_seed.rs:53` lacks invalid-modulus panic contracts; `bench_seed.rs:167` misstates the density threshold at the boundary and for invalid input.
  - `f2064e90`: `field/matrix.rs:19` Rustdoc restates identifiers or signatures.
- Wave E written and unmerged: `eba21481` (branch tip `c48da77e6`) and `50201002` (`6769bb2d0`). Both pass clippy on their branches.
- Registered under `554c2935`: `Lentmaier2010a`, `Hinnant2021`, `SageMath2025`.
- Filed under tech-debt epic `b4b4b9ee`: tracking issues for removed planned-work statements and the defects that sweep workers reported; `progress.json` `created_during_execution` lists them with their sources. The defect reports are unverified; each issue's first criterion is to reproduce or refute.
- `8966d8da`: Lean proof comments cite `axiom_tests.rs` line numbers that were stale before the sweep; `proofs/` was left untouched.

## What to do next

- [ ] Restart sccache from a normal shell, then force one `cargo-ci` on `8f451167` and reuse it with `jit gate evaluate-many cargo-ci` for the other five wave D units.
- [ ] One fix lane (one worker, one worktree, one commit per issue, merge tagged `jit:fa787f85`) for the three content findings above, rework attempt 1 of 2 each. Have it read each failed run's full prose in `.jit/gate-runs/*/result.json` and sweep each defect class over the unit's scope.
- [ ] Merge `worktree-agent-eba21481` and `worktree-agent-50201002` (merge subjects tagged `jit:fa787f85`), then run workspace clippy, `cargo-ci` once and the review gates for all eight open units.
- [ ] After the sweep: regenerate `554c2935-citation-map.txt` with `554c2935-citation-map.py`, confirm its "citations without a registry entry" section is empty, gate `554c2935`. Open point for that check: `tests/channel_capacity.rs:11` names `scipy.integrate.quad` as the tool that computed a reference table and has no key.
- [ ] `030496bd`: the after census and justification list. The record lists every planned-work issue in `created_during_execution`, the tool-forced hunks covered by `ffc35b8c` DEC-01, and the narration left in string literals, test names and `#[ignore]` reasons that workers listed (file one tracking issue for it).
- [ ] Before the `ffc35b8c` story gate: reconcile `surfaced_pitfalls` against REQ-21 to REQ-28.
- [ ] Then wave 9 remainder and waves 11 onward per `progress.json`. `3fd3db5e` waits on `fd9d5416`.

## Traps — do not repeat these

Traps of `handoff.md`, `handoff-2.md` and `handoff-3.md` remain in force. New this session:

- **Do NOT merge a comment-only unit without running clippy.** A deleted comment between two nested `if`s exposes `clippy::collapsible_if`; one such hunk failed `cargo-ci` for six units. Workers run clippy and fmt; the lead runs workspace clippy after merging a wave and before the gate batch.
- **Do NOT start review gates after a failed `cargo-ci`.** Every doc-review then fails on the missing CI evidence. Stop the batch, fix, re-run `cargo-ci`, then review.
- **Do NOT tag a single-unit merge with the unit's id.** `290b8716` failed doc-review three times because the reviewer read merge `c8e6f7da6` by its second-parent diff. Tag every merge `jit:fa787f85`.
- **Do NOT gate a unit whose commit bodies leave removed planned work unmatched.** `290b8716` failed on one "is deferred" sentence. File the issue under `b4b4b9ee`, then add an empty commit `docs(jit:<id>): name the issue tracking ...` with a `planned work:` line on the branch before merging.
- **Do NOT leave lane artifacts unlinked or executed relink scripts in place.** `96cea1b9` failed doc-review for a script and its output without `jit doc add`; `f0dd63f1` failed for a relink script naming removed paths.
- **Do NOT let a worker keep a comment to hold a rustfmt layout.** Reviewers flag it as narration (`3aef9e33`, three rounds). Delete it and name the layout hunk in the commit body (`ffc35b8c` DEC-01).
- **Do NOT accept name-echo deletions that leave a public type undocumented.** REQ-04 needs a purpose line; `50201002` was sent back for eleven types.
- **Do NOT trust a citation key by surname and year.** `Lentmaier2010` was a different paper than the GLDPC construction, and `SageMath2026` names version 10.9 where vectors came from 10.8. Read the registry entry before keying.
- **Do NOT remap line-number references mechanically.** The Lean references into `axiom_tests.rs` already pointed at closing braces; a content match showed it. Convert to symbol names under `8966d8da`.
- **Do NOT write `$VAR:path` in zsh.** `$B:crates/...` applies the `:c` modifier; write `"${B}:crates/..."`.
- **Do NOT kill processes by a full-command-line pattern that also occurs in the calling shell's own command.** It killed the lead's command. Stop a gate batch through its task handle.
- **A line-anchor link into a swept file breaks `docs-mechanical` for every issue.** One such link in another epic's `investigation.md` failed seven gates; run `python3 contrib/gates/docs-mechanical.py` after merging a wave.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, traps, created_during_execution)
- Sweep worker brief: `ffc35b8c-sweep-brief-template.md`; per-unit notes and the binding review findings (a) to (g) are in `progress.json` notes of session 5
- Citation registry: `.jit/references.toml`; map: `554c2935-citation-map.md`
- Manifest reconciliation: `96cea1b9-archive-rows.py`, `96cea1b9-archive-rows.txt`
