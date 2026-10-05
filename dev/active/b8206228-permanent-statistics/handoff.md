# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 3

**Date:** 2026-08-09
**Session number:** 3
**Prior handoffs:** none written; session 2 ended with a bare `progress.json` bump (commit `2647a29a`) and no handoff document.

## Current state

- Epic: `b8206228` — state: backlog (children executing; epic itself not yet transitioned)
- Wave in progress: wave 3 of 20 (wave 2 closed this session)
- Children summary: 15 done, 0 in_progress, 28 backlog/ready, 0 rejected
- Active claims: none. All four issues worked this session were released to `done`.
- Open escalations: none. Two were raised and both resolved by the owner (see below).
- Progress file: `progress.json` in the epic's artifact directory, `jit doc dir b8206228 dev/active`
- `jit validate` passes.

## What just happened

- Pruned two stale merged worktrees (`agent-240b7618`, `agent-6ae9f4ac`); left `.claude/worktrees/agent-a7c37e2288e3dc230` (unmerged, different epic).
- Found session 2 had merged two workers' rework onto `main` without re-gating or closing them. Re-gated both.
- `6ae9f4ac` (F_q sampler) — doc-review re-run PASS; closed. All 8 criteria verified against tests, incl. 300k draws/field.
- `175972df` (permanental rank predicate) — dispatched, merged, failed clippy on the merged tree, worker fixed; then doc-review FAIL on a **mathematical error**: rustdoc claimed `rect-per(A)=0` "neither implies nor is implied by" deficiency. Deficiency zeroes every summand, so it *does* imply it. Worker corrected to a one-way implication; closed.
- `d36810e7` (dataset source identity + integrity) — dispatched; two code-review rework rounds, both real defects:
  1. `generate_integrity_file` exempted absent shards by the path prefix `shards/`, so a **completed** cell's lost shard was silently uncovered and `verify_dataset` returned `Verified`. Fixed to derive the exemption from `CellTerminalState::Halted`. Worker also found the mirror defect in `verify_dataset` (sidecar-only walk).
  2. `campaign_prefix` accepted any in-repository path as the campaign root; the repository root yields an empty exemption prefix, exempting **every** tracked source file from the source-identity check. Fixed with a production `DATASET_HOME` constant and a `CampaignPathFault` vocabulary.
  Closed with 18 tests.
- `240b7618` (narrative sweep) — five review rounds on REQ-05, escalated twice. Resolved below.
- Build infrastructure (owner-directed, outside the epic): `sccache` installed, cache at `/data/sccache` (150 GiB cap); wired into `scripts/cargo-ci.sh` behind an availability guard; `cargo-ci` failure summariser fixed (it anchored `^FAIL` while nextest indents, so failing gates recorded no diagnostics); host build lock changed to `${XDG_RUNTIME_DIR:-/tmp}/cargo-ci.lock`, shared with the jit repo by agreement with its lead; `cargo-ci` gate `timeout_seconds` raised 300 → 900 (owner-approved) after a cold build measured 304 s. `cargo clean` reclaimed 51.7 GiB; `/` went 92% → 87%.

## What to do next

- [ ] Wave 3 remaining: `7a816262` (Frozen root manifest for the campaign) is the only wave-3 item left; `531695da` and `240b7618` are done.
- [ ] `jit query available` shows 6 available in this epic, incl. `49cbb131` (Wilson score interval), `ddd0c6ee` (batched F_3 benchmark receipt), `ddac2d5d` (purpose-domain stream address allocator).
- [ ] Re-read the wave plan in `progress.json`: waves 4+ were computed before this session and should be re-derived against `jit graph deps` before dispatch, since several issues completed out of their planned wave.
- [ ] Before any dispatch, read the Traps section below in full.

## Traps — do not repeat these

- **Do NOT defend an audit exclusion with a claim about the excluded file.** REQ-05 on `240b7618` burned five review rounds. Every justification written for excluding `breakdown.json` — "frozen", "historical", "faithful to the store", "snapshotted at commit 89531d65" — was a checkable claim, and each round the reviewer checked it and found it false or self-contradictory. Fixing one created the next: resynchronising the manifest to satisfy a fidelity check made it "not frozen"; reverting made the cited revision wrong. Settled only when the owner recorded the exclusion as a **decision** in the issue description, explicitly not contingent on any property of the file. If an exclusion is needed, state it as a decision; do not argue the excluded artefact deserves it.
- **Do NOT amend a JIT issue description without resynchronising or re-examining `breakdown.json`'s copy of it.** Amending REQ-05 changed `240b7618`'s stored description, which broke the manifest's copy of that entry and produced two immediate review failures. Current standing decision: the manifest is never edited and never resynchronised. See `dev/active/b8206228-permanent-statistics/README.md`.
- **Do NOT treat a `jit gate evaluate` success line as evidence without its run record.** One `cargo-ci` run printed `Passed gate 'cargo-ci' for issue 175972df` and persisted nothing under `.jit/gate-runs/`; the next identical invocation on the same commit failed clippy, and a direct `cargo clippy` reproduced the lint. Always confirm with `jit gate status <id> <gate>` after evaluating.
- **Do NOT read a gate `error` at ~300000 ms as a code verdict.** `scripts/cargo-ci.sh` re-execs under a blocking host-wide `flock`, and queued time counted against the old 300 s wrapper budget. The budget is now 900 s, but the lock is now *shared with the jit repository*, so queueing is more likely, not less. Check lock ownership (`ps` for `flock /run/user/1000/cargo-ci.lock`) before diagnosing.
- **Do NOT let a worker commit with `git add -A`.** `jit` materialises the shared `.git/jit/` store into whichever checkout runs a `jit` command, so a worker's `.jit/*.json` and `.jit/events.jsonl` acquire *other issues'* gate transitions. One worker swept the lead's `175972df` gate state into its own branch and caught it only on self-review. Brief workers to stage explicit paths, and check every worker commit for `.jit/` paths before merging.
- **Do NOT accept a falsification that does not reproduce the defect byte-for-byte.** A worker reinstated a bug to prove its test caught it; the test went red, but for the *opposite* reason — its rewrite produced an exemption prefix of `"/"` (matches nothing, everything refused) where the real defect produced `""` (matches everything, everything exempted). Require the reinstated body to be diffed against the real pre-fix code, and ask which red was observed.
- **Do NOT trust a test that asserts an exemption without checking the fixture reaches the exempted path.** A `derived/` exemption test wrote its file untracked; the untracked rule excused it regardless, so the test would have passed with the exemption completely broken.
- **Do NOT edit `scripts/cargo-ci.sh` while an instance is running.** Bash reads scripts lazily by byte offset; inserting lines can make a running instance resume mid-token. Done once this session (it survived, but do not rely on that).
- **Do NOT hold merges out of fear the tree moves under a running review.** Owner feedback, and correct: it costs bandwidth. Only `cargo-ci` is genuinely exclusive (host lock). Review gates are API-bound; merge as soon as a branch is ready, gate concurrently. Hold only when two changesets touch the same files, which is visible from the diff.
- **Do NOT run a `cargo-ci` gate while workers compile.** Seven `gf2-coding::ldpc_systematic_encoding` tests timed out at 5.0–5.3 s against the 5 s fast-tier kill; the same twelve pass in ~1.0 s each uncontended. The gate runs at `nice -n 19`, so it yields CPU and starves marginal tests.
- Traps from prior sessions: none recorded (session 2 wrote no handoff). The `surfaced_pitfalls` array in `progress.json` carries 14 entries and remains in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Artefact directory guide: `dev/active/b8206228-permanent-statistics/README.md`
- Plan: `dev/active/b8206228-permanent-statistics/plan.md`
- Analysis narrative: `dev/active/b8206228-permanent-statistics/investigation.md`
- Feasibility study (gap table G1–G8; G1/G4/G5/G6 now landed): `dev/studies/b488f02c/feasibility-study.md`
- Dataset home and verification procedure: `dev/simulation_results/permanent-zero-fraction/README.md`
- Preregistration: `dev/simulation_results/permanent-zero-fraction/protocol.md`
- Execution batching guidance: `dev/active/b8206228-permanent-statistics/execution-batching.md`
