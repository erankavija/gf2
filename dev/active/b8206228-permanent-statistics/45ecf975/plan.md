# Plan: 45ecf975, separate experiments from libraries and cut committed bloat

## Context

`@/inv/library-first-generality` makes permanent campaign code in `gf2-sim` a defect, along with other campaign code. The tree review also determined where experiment data should live.

Owner decisions from the interview:

| Topic | Decision |
|---|---|
| Campaign | Re-freeze on a much smaller tool. New campaign id, preregistration and validation. September receipt stays as history. |
| Rare event | Keep sampler and runner. Delete the artifact system. |
| Code home | `experiments/<name>/` at repo root. `dev/` is for developer documentation. |
| Shared mechanics | `experiments/common` |
| Other experiment code | All in this effort |
| `gf2-stats` | Keep `binomial`, `intervals`, `weighted`. Delete the `accumulator` module and the zero-floor wrapper. Move `experiments/permanent-statistics/src/sampler.rs`. |
| Tracking | Tasks under epic `b8206228` |
| Snapshots | Stop and purge right away |
| Purge depth | Inputs plus raw logs and perf data |
| History | Rewrite it, and make recurrence impossible |

Default is delete. Code moves only if an epic criterion or a live consumer needs it.

## Phase A: stop the bloat (first)

**A1. Tree purge, one commit on main.** Remove under `bench_results` and `dev/active`:
- `inputs/`, `source-inputs/`
- `*.log`, `*.err`, perf `*.data`, `*.zst`, raw `test-logs/`

Receipts, summaries, CSV and TSV result tables, execution logs and execution journals leave the tree too: under `@/inv/committed-evidence-budget` the repository keeps the harness and its configuration, and a run writes its records outside the tracked tree.

Exempt until landed or closed: the `55c3e433` AFF3CT LDPC gaps directory, `02b8137c`, `.agents/`, and `bench-window/` (the armed timer runs `dev/scripts/bench-window/run-window.sh`).

**A2. Remove the rule and its tooling.**
- Replace the "receipt carries every input" paragraph in `AGENTS.md`: receipts hold results, configuration and digests, and the source revision is the rebuild pointer.
- Delete the receipt-input snapshot check, its omission list and their `scripts/cargo-ci.sh` steps.
- Stop snapshot writing in `dev/tools/tuning-campaign-support/src/receipt.rs`, `dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs`, `dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs`. Update `dev/scripts/validate-tuning-extent-campaign.py`.
- Remove `inputs/` entries from `contrib/gates/docs-mechanical-baseline.md`. This resolves the open docs-mechanical question.
- Fix or delete every CI step and test that read purged files. A check that only re-verifies purged bytes is deleted.

**A3. Prevention, at every point available.**
- New enforced invariant `committed-evidence-budget`: single-host raw output, copied source and third-party archives are never committed.
- `scripts/check-tracked-budget.py`: per-file limit 1 MiB, per-commit added limit 5 MiB, tree budget set to the post-purge size and only lowered. Exceptions are a short reasoned list in the script's config.
- Run it from a versioned pre-commit and pre-push hook (`core.hooksPath`, set by `scripts/install-hooks.sh` and the worktree dispatch script) and as a `cargo-ci` step.
- GitHub push ruleset restricting file size, the only control `--no-verify` can't bypass. Confirm its availability on this repo and plan; if unavailable, say so.
- `.gitignore` for `**/inputs/producing/`, `**/source-inputs/`, `perf.data*`.

## Phase B: rewrite history

Starts only when the window has ended and 55c3e433's pilots are collected and committed. Runs with no other session active; announce it to live sessions first.

1. Off-repo backup: bare mirror and `git bundle` outside the repo. These become the only source of old bytes.
2. `git filter-repo --analyze`, then settle the path list: Phase A patterns plus large historical blobs no longer in the tree. Keep the same exemptions as A1.
3. Rehearse in a throwaway clone. Check `jit validate`, full `cargo-ci`, tree and pack sizes, and that all branches rewrote.
4. **Stop and ask the owner to confirm before the force-push, showing the rehearsal's numbers.**
5. Force-push all refs. Reseat the local checkout and the 17 worktrees by runbook, preserving untracked built arms in the pinned ones. Ask for GitHub cache cleanup if old objects stay reachable.
6. Archive the old-to-new commit map beside the backup bundle, outside both repositories, named by its digest (owner decision of 2026-11-16, `@/issue/2a4c2684`). After the reseat, the runbook's old-ID step rewrites recorded commit IDs in tracked text and tracker issue records; records a digest covers, such as the tuning profiles, keep their old IDs, which the archived map resolves.

`@/inv/campaign-evidence` already says recorded commit IDs never decide acceptance. The rehearsal must surface any tool that still depends on them.

## Phase C: separate experiments

Create as tasks under `b8206228`, labelled `epic:permanent-statistics`, each with `cargo-ci` and `code-review`. Amend 45ecf975 to match, replacing REQ-04's "committed receipts keep verifying" with receipts written outside the tracked tree verifying from the location and digest the issue records, and make it the umbrella.

| Task | Content | Worker |
|---|---|---|
| C1 Home | `experiments/` as workspace members. `experiments/common` with identity types, provenance observation, atomic publish, each with two real consumers. File a tracked convergence issue with `tuning-campaign-support`'s copies (`@/inv/convention-convergence`). Update `AGENTS.md`, `dev/index.md`. | Claude Opus |
| C2 `gf2-stats` | As decided. Local draw helper for `gf2-algebra` tests. | Luna |
| C3 Rare event | Move sampler and `experiments/permanent-statistics/src/rare_event/runner.rs` to `experiments/permanent-statistics` on `gf2-sim::checkpoint` and one log. Delete the `permanent_rare_event::artifact` module. Amend `3f664839`-derived issue text where it names the artifact system. | Sol |
| C4 Campaign | Rebuild small in the same crate. Keep exact-anchor checks, sampling, scheduling with HIP, statistics, resume, dataset schema, closed-set identity. Delete continuation and frozen decoders, git-dependent acceptance, emission approval, `root_fs`, hard-coded paths, `FROZEN_*`, experiment paper types as compiled enums (move to data). `experiments/common/build.rs` version stamp moves with it. | Claude Opus designs, Sol implements |
| C5 Re-freeze | New protocol, campaign id and preregistration under `experiments/permanent-statistics/`. Drop "identical source revision" and "clean closure" clauses that conflict with the invariant. Retire the unused draft dataset. Reject `02b8137c` and `02ed3c51` as superseded. New validation task. Rewire arms onto it and `scripts/permanent_zero_fraction_analysis.py`, `.config/nextest.toml`. | Claude Opus |
| C6 Validation | Queue line for the overnight window. Campaign draws still need the owner's separate go. | Lead |
| C7 Delete one-shots | Six throughput and receipt binaries in `gf2-sim/src/bin`; four `gf2-algebra` examples. Trace remaining references first. Reasons in the commit. | Luna |
| C8 Move OSD, DVB-T2 | `experiments/osd-ebch/src/osd_campaign.rs`, `ebch_osd_awgn_campaign`, `dvb_t2_awgn_campaign` and their tests. Drop `rand08` and other experiment-only dependencies from `gf2-sim`. | Sol |
| C9 Aff3ct comparison | `ldpc_bler_sweep`, `export_alist`, `experiments/aff3ct-ldpc-gaps/src/bler_sweep.rs`, `ComparisonCode`. **Last, after coordinating with 55c3e433's lead**, because its harness uses them. | Sol |
| C10 Mixed items | Remove baseline-specific parts from `crates/gf2-coding/src/dvb_t2_bicm_harness.rs`; move `RicianConfig::fig*` to config; resolve `crates/gf2-sim/src/executor/failure.rs` default `dev/` path. Audit report for benches and remaining judgement calls; move or file each. | Luna |

Order: A1, A2, A3, B, C1, then C2/C3/C7 in parallel, C4, C5, C6, C8, C10, C9.

Target for C3 plus C4: at most 6,000 code lines against 17,631 today. This is my estimate, so it's `[aspirational]`.

Review: alternate Claude and Codex, with `gpt-6-astra` read-only as independent reviewer on C1, C4, C5 and the B runbook.

## Constraints

- No builds, tests, CI or gates from 03:50 Helsinki on 2026-11-02 until `.agents/bench-window/window.log` shows a `window end` dated 2026-11-02. A1 is deletion only and may land before 03:50. If A2 and A3 can't finish CI by then, they wait.
- 55c3e433's pilots will write `inputs/` in their worktrees. Tell its next lead, through its handoff, to drop snapshots when landing them.
- Leave `worktree-agent-1d787ed3`, `worktree-agent-a72f2b3f` alone until the B runbook reseats them.

## Verification

- A: `git ls-files | wc -l` and tracked size against recorded numbers. Fixture commits show each hook and CI check refusing oversized files, `inputs/producing/`, and a tree-budget overrun. Full `cargo-ci`, `jit validate`.
- B: rehearsal checks repeated after the push. Fresh clone size reported. Backup bundle passes `git bundle verify`.
- C: `git grep -n permanent_campaign -- crates` empty. No `experiments/` dependency in any `crates/*/Cargo.toml`. Library code count compared to the earlier 106,454. Full `cargo-ci`. New validation receipt verified by the new tool on a real run, with a test that verifies a validation record generated at test time, the run record written outside the tracked tree and its location and digest recorded in the issue, a gap in today's suite.
