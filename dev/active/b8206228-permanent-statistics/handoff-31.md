# Handoff — Experiment separation and committed evidence (45ecf975, epic b8206228) — session 35

**Date:** 2026-11-02
**Session number:** 35
**Prior handoffs:** [handoff-30.md](handoff-30.md) and its ordered predecessor chain. This session ran under the lead of epic 1a379447 by owner direction. Plan: [45ecf975/plan.md](45ecf975/plan.md).

## Current state

- Epic `b8206228`: backlog. `@/issue/45ecf975` is in progress, waiting on `@/issue/67194d70` and `@/issue/1aea3958`.
- Done with gates passed: `146910f2`, `e3370140`, `fd1ef344`, `c251bf43`, `313ba890`, `779d1926`, `1634c185`, `39e5f643`, `5355dd77`, `2edb4aa5`, `a492bef0`, `f58b67d8`, `7916a7c5`, `e6b1ba65`, `68277bed`.
- Rejected as superseded: `02b8137c`, `02ed3c51`. The campaign arms now depend on `67194d70`.
- Main is about 1,850 commits ahead of `origin/main`, unpushed. Nothing was pushed this session.
- No worker or timer is active. The 2026-11-02 window ended at 04:11; its pilots for `55c3e433` are uncollected in their worktrees.

## What just happened

- Purge: 52,154 files and 3.44 GB left the tree at `e3700fccc`. The tree is 485 MB and 26,909 files.
- Prevention: `@/inv/committed-evidence-budget`, `scripts/check-tracked-budget.py`, versioned hooks in `.githooks`, `cargo-ci` and GitHub CI steps.
- Separation: library production code fell from 106,454 to 84,451 code lines by the same count as the session's first measurement; `gf2-sim` from 29,013 to 7,997. Experiments live in `experiments/`.
- `permanent-statistics` is 6,177 Rust code lines including tests and the moved sampler, 177 over the aspirational 6,000 in `fd1ef344`.
- Rewrite rehearsal: pack 221 MiB to 63 MiB, 334 pinned commits mapped, `jit validate` and full `cargo-ci` pass on the rewritten checkout. See [45ecf975/history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md). The rehearsal mirror is at `local-path

## What to do next

- [ ] Settle the questions below.
- [ ] `55c3e433`'s next lead collects pilots, dropping `inputs/` and raw logs, then lands its branches.
- [ ] `1aea3958` (aff3ct comparison move) lands after that; its helpers are used by `55c3e433`'s harness.
- [ ] On the owner's confirmation, run the runbook with no other session active. Rerun its rehearsal first: main will have moved.
- [ ] After the rewrite, create worktree `agent-67194d70` at main, and arm a window with the owner's approval: `queue.tsv` has its line. The binary digest then pins the validation, so settle code first.
- [ ] Close `45ecf975` through its gates when `67194d70` and `1aea3958` are done.

## Traps — do not repeat these

- **Do NOT push `main` with the hook before the rewrite.** The range holds purged history, and the pre-push hook refuses it. The rewrite's push uses `--no-verify` by design.
- **Do NOT rewrite history without `map_pins.py`.** 334 tracker documents pin commit IDs, and `jit validate` fails on the rewritten repository without it.
- **Do NOT merge a purge or tool deletion without unlinking first.** `jit validate` fails on issue documents whose file is gone, twice this session. Check after each deletion.
- **Do NOT commit in the same command as a test whose result you only print.** `a23c2eafa` and `42dde1744` landed broken this way. Gate the commit on the result.
- **Do NOT `cd` into a worker's worktree from the lead.** The harness moved a teammate's directory with it.
- **Do NOT file an issue with `--orphan`.** It fails whole-repository validation until wired; wire it to its epic in the same step.
- **Do NOT run gates while workers build.** `cargo-ci` hit its 900-second limit, and whole test binaries timed out; `49b040e6` tracks why. A failed verdict is cached by input digest until an input changes.
- **Do NOT treat a passing gate round as final after changing shared code.** `experiments/common` changed after `e3370140` and `779d1926` first passed.
- Traps in handoff-30 about the September validation no longer apply: that path is rejected.

## Open questions needing invoker input

1. **Force-push confirmation.** Rewrites all history, changes every commit ID, breaks old clones and needs the `Main protected` bypass. Recorded IDs resolve through a committed map, and the bytes stay in an off-repo backup. Recommendation: confirm once `55c3e433`'s pilots land.
2. **Validation window and campaign go.** Arm a window for `67194d70`; the first campaign draw needs a separate go.
3. **Protocol differences made on defaults in `1634c185`, all stated in `experiments/permanent-statistics/protocol.md`:** Wilson intervals only with one z constant; shard size 200,000 in precision tiers; a killed process no longer uses a retry; the 12-hour ceiling; `--on-unavailable` modes. Recommendation: accept.
4. **Remaining 485 MB.** `bench_results` still holds about 340 MB of JSON, TSV and summaries, and 227 older files sit in the budget baseline, including `archive`, `studies` and `dev/research` logs. Recommendation: move to a summary-only layout in a separate issue.
5. **Server-side enforcement is unavailable.** GitHub refuses push rulesets on a public, user-owned repository, so `--no-verify` can bypass the hooks and CI reports only afterwards. Recommendation: accept, or move the repository to an organization.
6. **`build_profile.rs` is more machinery than I'd like.** Code review drove it through five rounds, past the two-retry policy, and the flags rest on a documented caller precondition. Recommendation: revisit it when `74f8cf16` converges the shared code.

## Limits

- Old receipts: 192 protocol receipts can no longer be re-evaluated; `benchmark-acceptance` reports `ConfigurationUnavailable`. Old input bytes come from git history until the rewrite and from the backup after it. External archives are in `local-path
- The September validation's independent verifier never ran, and that receipt is now history only.
- GPU runtime paths in `permanent-statistics` are unverified: no usable device was exposed, though the `hip` feature builds. The `crates/gf2-kernels-hip` standalone build was blocked offline.
- `rank_event_rates` was moved without rerunning its ignored receipt test; stream equality rests on code reading.
- Issues closed earlier were not re-reviewed after the last moves; the final state passed `cargo-ci`.

## Reference artefacts

- [Plan](45ecf975/plan.md), [C4 design](45ecf975/c4-design.md), [C4 report](45ecf975/c4-report.md), [library audit](45ecf975/library-audit.md), [runbook](45ecf975/history-rewrite-runbook.md)
- Protocol: `experiments/permanent-statistics/protocol.md`
- Budget: `scripts/tracked-budget.toml`
