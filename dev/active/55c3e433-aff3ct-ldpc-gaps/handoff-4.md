# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 3, second stop

**Date:** 2026-11-04
**Session number:** 3
**Prior handoffs:** [handoff.md](handoff.md), [handoff-2.md](handoff-2.md), [handoff-3.md](handoff-3.md)

## Current state

- Epic: `55c3e433`, state `backlog`.
- Wave in progress: wave 5 (`3ee00fcf`). The adoption is implemented on branch `worktree-agent-3ee00fcf` and not merged; one escalation blocks the merge.
- Children summary: 6 done, 1 in_progress (`3ee00fcf`), 1 backlog (`5f401907`), 0 rejected.
- Active claims: `3ee00fcf`, assignee `agent:worker`. No worker is running.
- Open escalations: one, below.
- Progress file: `progress.json` in this directory.
- No timer is armed. The owner approved one window for `comparator-family`, to run after adoption lands.

## What just happened

- The owner approved option A, the composer's fixed-selector input, and a second window.
- Claude Opus worked on branch `worktree-agent-3ee00fcf` in two commits:
    - `6946a4057` adds `coding-owner --fixed-selector decode.qc_route_cases`, restricted to that path and validated through `CodingTuningCodec`, with tests.
    - `235238c95` composes the production profile `gf2-3ee00fcf-20261104t064432z-4012544` from a clean tree. It keeps the calibrated measurement and has `source_dirty` false.
    - The tuned arm attaches QC through the shared `prepare_code`.
    - A defect in `verify_executed_routes` is fixed: it now names NMS and the NR structure.
    - Tests and docs are swept.
    - `nextest` passes on `gf2-coding`, `gf2-kernels-simd` and `aff3ct-ldpc-gaps`, and the profile `cargo-ci` steps pass.
- The lead added `b71907313`. The superseded owner envelope moves byte-identical, under its own id, into the experiment's `tuning-profiles/` directory on that branch, and the pinned arm `gf2-batch-pre-change` points to it. Plan reproduction still passes; the 16 Python tests pass.
- `check-references` then reports three faults, all naming the old path:
    - the `20af7e6e` document link, a done issue, which the lead may relink with `jit doc`;
    - twice in the description of `d06a99ed`, a done issue of another epic.

## What to do next

- [ ] Get the owner's answer below.
- [ ] On yes, in main:
    - Relink `20af7e6e`: run `jit doc remove` on the old path, then `jit doc add` on the new one with the same label.
    - Update the path in `d06a99ed`'s description.
    - Merge `worktree-agent-3ee00fcf`.
    - Run `check-references`, then `cargo-ci`, `code-review` and `research-review` on `3ee00fcf`.
- [ ] Before review, write the adoption record. Name the receipts by digest (pilot `ea8df84f…f447`, confirmation `87cfeaa2…a18a`) and the cells with their host label. State that the line budget binds 898 production lines (1,882 including tests), and that the quality files came from `545a3c814`.
- [ ] `5f401907`: write `55c3e433-adopted-vs-aff3ct` under `comparator-family`. Build a fresh `gf2-batch-tuned` record, because its digest changed, then smoke it and arm the approved window.

## Traps — do not repeat these

The traps in [handoff-3.md](handoff-3.md) and earlier stay in force. Its trap on the tuned arm's `attach_qc` is resolved by `235238c95`.

- **Do NOT move the old owner envelope into `dev/reference_data/tuning-profiles/`.** That directory holds complete envelopes (`dev/index.md`), and `crates/gf2-algebra/tests/tuning_repository_envelopes.rs` reads it.
- **Do NOT keep the superseded profile under `crates/gf2-coding/data/tuning-profiles/`.** It is a second production representation under `@/inv/canonical-cutover`.
- **Do NOT edit `d06a99ed`'s description without the owner's approval.** It is a done issue of another epic, and descriptions are scope-defining.

## Open questions needing invoker input

- Question: May the lead replace the old profile path in the description of done issue `d06a99ed` with the experiment path?
  - Context: the adoption removes the superseded production profile from `crates/gf2-coding/data/tuning-profiles/`. `d06a99ed` (another epic, done) names that path twice, so `check-references` fails and the merge is blocked.
  - Options: A, replace the path with the moved file's path in the experiment's `tuning-profiles/` directory, whose bytes are identical; B, keep the old file in place, which breaks the canonical cutover; C, replace it with the profile id alone, without a path.
  - Recommendation: A. The path stays resolvable, the bytes are the same, and nothing else in the issue changes.

## Reference artefacts

- Branch: `worktree-agent-3ee00fcf` (`6946a4057`, `235238c95`, `b71907313`)
- New profile: `gf2-3ee00fcf-20261104t064432z-4012544` in the coding crate's committed tuning data on that branch
- Evidence: `campaign-evidence/55c3e433/integrated-adoption/` in the agent workspace
