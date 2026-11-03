# Handoff — Close the integrated LDPC gaps against AFF3CT (55c3e433) — session 2

**Date:** 2026-11-03T03:10+02:00
**Session number:** 2
**Prior handoffs:** [handoff.md](handoff.md)

## Current state

- Epic: `55c3e433`, state `backlog` (blocked by its sink; assigned to `agent:jit-execution-lead`).
- Wave in progress: wave 2b is held at the rework limit; wave 5 waits on it. Waves 1 to 4 are done.
- Children summary: 5 done (`10590c11`, `1d787ed3`, `a72f2b3f`, `4555dfb3`, `0cc6c195`), 1 in_progress (`2088e0ec`), 2 backlog (`3ee00fcf`, `5f401907`), 0 rejected.
- Active claims: `2088e0ec`, assignee `agent:worker`. No worker is running.
- Open escalations: one, below. It blocks everything left in the epic.
- Progress file: `progress.json` in this directory.
- No timer is armed. The owner pre-approved one window for this epic.

## What just happened

- Wave 2 pilots ran on 2026-11-02. Receipts stayed untracked, as `@/inv/committed-evidence-budget` requires. They are preserved in the ignored `campaign-evidence/55c3e433` directory of the agent workspace, with the prepared build records and inputs.
- `1d787ed3` closed with a QC go ([record](qc-route-decision.md)). All gates passed after three review rounds.
- `a72f2b3f` closed with no nomination ([record](check-variant-nomination.md)). The only candidate for the adoption family is therefore QC, with worthwhile speedup 1.25.
- `2088e0ec` created and held. Codex `gpt-6.1-sol` ported the harness; Claude Sonnet did two rework rounds. All of it is on `main`, and `cargo-ci` passes.
- `4555dfb3` closed: Codex `gpt-6-astra`, first review pass, 517 production lines and one unsafe kernel.
- `0cc6c195` closed: Claude Opus, first review pass, 381 library lines. The default envelope leaves QC unselected; `experiments/aff3ct-ldpc-gaps/tuning-profiles/qc-route-candidate.json` selects it.
- Owner decisions this session: receipts stay outside the tree, cited by digest, and five issues' criteria changed accordingly; the loaded check-variants start is accepted; `1d787ed3` REQ-02 accepts the post-timing bit comparison with its order stated; `committed-evidence-budget` now lets a decision record state the values its rule decides on.
- Disk: the owner ordered an emergency prune. Cargo output in idle worktrees is deleted. Main's `target/debug`, `ci-test` and `release` are symlinks to `/mnt/nvme/gf2/target/`, requested through a peer session on the owner's behalf. Worktrees for `55c3e433r`, `4555dfb3` and `0cc6c195` are reclaimed.

## What to do next

- [ ] Get the owner's answer below. On yes: dispatch one more rework of `2088e0ec` for the finding, with Codex next in the alternation.
- [ ] Rerun `code-review` on `2088e0ec`, then complete it.
- [ ] Run the `gf2-batch-qc-candidate` arm end to end through `smoke-family.py` on a real adoption-family plan. It has only unit tests so far. Lay out `GF2_LDPC_INPUT_ROOT` from the preserved inputs, and prepare the `quality/` files the harness expects.
- [ ] `3ee00fcf`: write the `55c3e433-integrated-adoption` declaration, freeze through `experiments/aff3ct-ldpc-gaps/freeze-confirmation.py`, and arm the pre-approved window. Queue only this epic's job.
- [ ] `5f401907`: on adoption, run `comparator-family`, which needs fresh window approval. Otherwise write the closing record with no campaign.
- [ ] Reclaim `agent-2088e0ec`, `agent-1d787ed3` and `agent-a72f2b3f` when finished. The last two hold unmerged branches: the prototype, and the variant trees.

## Traps — do not repeat these

Traps in [handoff.md](handoff.md) stay in force, except that its instruction to commit receipts in each worktree is superseded. The wave 2 worktrees need no further merge.

- **Do NOT commit a receipt, execution log, plan JSON or patch under `dev/active`.** `scripts/check-tracked-budget.py` refuses them. Configuration goes under `experiments/aff3ct-ldpc-gaps/`, and a file named `*outcome*` or `*receipt*` is refused anywhere, so the closing record needs another name.
- **Do NOT call a pre-timing check "posterior bits" without comparing `f32` bit patterns.** `qc-bits` compared hard decisions, the worker's record called that posterior equality, and review caught it. The corrected comparison was preserved with the evidence.
- **Do NOT trust the default leak-check snapshot.** `check-leak-into-main.sh` read an old `/tmp` snapshot and reported false leaks. Diff `git status -uall --porcelain` against the snapshot the dispatch printed, when `LEAD_SNAPSHOT_DIR` is set.
- **Do NOT symlink `target` itself.** `.gitignore`'s `target/` does not match a symlink, and the harness's `output_path` compares resolved paths against `target/`. Link only profile directories.
- **Do NOT delete a whole `target/`.** Campaign output and pinned builds live there. Delete only `debug`, `ci-test`, `release` and `doc`, and leave `target/d06a99ed-builds`, which holds registered worktrees.
- **Do NOT read `cargo-ci` exit 10 as a test failure.** It was the gate's 900-second limit after the QC merge forced a rebuild under load. A retry passed in 437 seconds.
- **`jit claim acquire` needs an agent argument.** Gates ran without leases this session.
- **A reclaimed branch blocks re-dispatch under its name.** Dispatch a rework as `2088e0ec-r3`, or reuse the kept `agent-2088e0ec` worktree after fast-forwarding it to `main`.
- **Do NOT write `====` as a bare shell word.** zsh aborts the whole command on it.
- **The envelope's `assembly.tool` names `tuned.rs`, but a one-off script assembled it.** `source_dirty` is true. `tuning-profile-compose` needs calibrated owners, so it did not fit. `code-review` passed it; correct it if the confirmation's build record reads it.

## Open questions needing invoker input

- Question: May `2088e0ec` have one more rework round?
  - Context: it reached MAX_REWORK_ATTEMPTS, and `code-review` raises a new finding: `experiments/aff3ct-ldpc-gaps/src/arm.rs:25` keeps its own `Request` and result wire types with string roles and cache states, instead of `tuning_campaign_support::arm::{ArmRequest, ArmResult}`. Earlier findings are closed.
  - Options: A, one more round, replacing the local types. B, take over manually. C, reject the issue, which leaves the confirmation without a harness.
  - Recommendation: A. The canonical types exist, `crates/gf2-core/benches/shifts.rs` already uses `ArmRequest`, and the change stays inside the experiment.

## Reference artefacts

- Epic: `jit issue show 55c3e433`
- Planning docs: [plan.md](plan.md), [investigation.md](investigation.md)
- Decision records: [qc-route-decision.md](qc-route-decision.md), [check-variant-nomination.md](check-variant-nomination.md)
- Harness: `experiments/aff3ct-ldpc-gaps/README.md`
- Receipts: `55c3e433-nr-route-pilot-pilot`, SHA-256 `0a98003957cce247411aa123685f5b8385bf0d2604716f490d324563d9a0891e`; `55c3e433-check-variants-pilot`, SHA-256 `64dc191d5d728b430a9c624ee2afcf5139f81175b1232d543f0efc73c79366aa`
