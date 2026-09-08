# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 4

**Date:** 2026-09-08
**Session number:** 4
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md)

Written on the invoker's instruction while eight workers are still running. Every
branch state below is a snapshot; re-read each branch before acting.

## Current state

- Epic: `1a379447` — state: backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 1 of 7 (`f547c394`, rework round 3) with wave 2 running in parallel on the invoker's session-3 authorization.
- Children summary: 0 done, 1 in_progress (`f547c394`), 8 wave-2 issues assigned and backlog, 12 backlog, 0 rejected. `1d0da41f` is **merged into main** but its issue stays open pending the v2 re-run.
- Active claims: lead lease on `f547c394` (`agent:jit-execution-lead`, acquired ~11:16Z, renewed by use).
- Open escalations: none. Three were raised and resolved this session (below).
- Progress file: `progress.json` in this directory.

### Workers still running at handoff time

| Issue | Model | Branch head | Commits vs main | State |
|---|---|---|---|---|
| `f547c394` | codex gpt-6-astra | `f4b17148` | 6 | Rework 3. v2 markers verified in its own validation: `protocol_version: 2`, `host_observations: 2`, `cold_calls_verified: 10000`, `decoder_quality_in_every_measured_cell: true`, `original_receipt_unchanged: true`, all 81 published v1 files byte-identical. Running v2 pilot sessions. |
| `c077a88b` | codex gpt-6-astra | `98d61d46` | 21 | Past the F3 wall — producing `2026-09-08-r2-c077a88b-ldpc-matched-algorithm-pilot`. **Carries its own edits to `benchmark-ab-runner.rs` and `protocol_contracts.rs`** (the P-18 decoder-quality repair) which will conflict with v2. |
| `6fb89a3c` | codex gpt-5.6-sol | `16885638` | 11 | Preserved a failed launch as explicitly-not-a-receipt evidence; re-running under a fresh campaign identity and executable digest after a formatter fix. |
| `04b85d10` | codex gpt-5.6-sol | `f6715ca6` | 15 | Holding position in the CI queue rather than abandoning an in-progress contract run. |
| `c7113c5a` | claude-opus | `8c518e26` | 53 | REQ-01..04 met, REQ-05 partial. |
| `26465e6c` | claude-opus | `3d6345bb` | 21 | Premise falsified; harness defects fixed. |
| `6c6b09b1` | claude-opus | `9b2ef8e2` | 35 | Pilot accepted; F8 predicate derived. |
| `eda07788` | claude-opus | `c7fbf618` | 17 | Pilot accepted; AFF3CT claim falsified and corrected. |

`main` is at `b3de0686`, clean, no leaks.

## What just happened

- **Unblocked `research-review`.** Two session-3 runs failed at the checker level with no verdict: the reviewer filled its 258 400-token context on the issue's linked documents. Split 128 KB of raw command transcripts byte-for-byte out of the linked `rework-validation.md` into an unlinked companion (`259c1b9a`). Next run produced a full 12-finding verdict in 1083 s. No gate config, criterion or evidence changed.
- **`f547c394` gates:** `cargo-ci` PASS on a quiet host (5952 tests), `code-review` PASS, `doc-review` PASS, `research-review` FAIL with 7 blocking + 5 advisory.
- **Escalated and resolved (invoker):** rework round 3 authorized with the counter reset; wave 2 kept running with affected timed campaigns re-run under v2; the fast-tier test cap raised 8 s → 15 s (`23aa0824`) instead of filing a bug.
- **Rework 3 dispatched** on codex astra with lead direction that the protocol becomes **version 2** (the amendment materially changes P-17/18/19/22), v1 receipts staying valid as v1 evidence and acceptance refusing to evaluate a receipt under a version it was not pinned to.
- **Fixed CCX1 mutex writer starvation** (`d088c284`, invoker-authorized). A pending `flock -x` does not block a new `flock -s` on Linux, so a stream of sibling builds left no zero-reader instant and four runners queued 10–29 minutes for work needing under five. Both sides now pass a turnstile; the wrapper test gained an ordering case that fails against the old behaviour. The fix binds only checkouts carrying it, so main was merged into all nine live worktrees and the lead held the turnstile to drain the six runners launched under the old wrapper.
- **Merged `1d0da41f`** (`4d630aec`); merged tree passes 25/25 CI steps, 5959 tests. Its confirmatory receipt is accepted and **rejects** YMM adoption; production keeps sequential PCLMULQDQ via canonical detect-time lane selection. Eight evidence artifacts linked.
- **Commissioned the two stood-down duplicate workers as read-only auditors** rather than idling them. One verified 106 citations across all five survey findings documents and nine external checkouts; the other audited the shared crate for the defect class the first finding revealed. Between them: F13–F19, two false claims caught before they reached receipts, and a three-rule closing set. Neither wrote a byte in any worktree.
- **Findings grew 7 → 19.** All recorded in [`reviews/f547c394-r3.md`](reviews/f547c394-r3.md), which the rework worker reads as part of its mandatory audit step.

### Results worth carrying forward

- `26465e6c` **falsified its own issue's premise**: libpopcnt and Mula beat gf2's dispatcher materially at four of nine cells. Its measured crossover (Mula loses at 63 words, wins at 64) lands exactly on the 512-byte carry-save boundary it read off the disassembly *before* running.
- `1d0da41f` falsified its premise: YMM is 1.24–1.72x slower than sequential PCLMULQDQ. F14 biases *toward* declaring improvement and this receipt declined to, so the negative holds a fortiori.
- `eda07788` falsified a claim on its own branch (AFF3CT does implement 5G NR bit selection) and built an executable comparison showing identical selection in all six surveyed configurations.
- `c7113c5a` and `6c6b09b1` independently derived the F8 onset predicate: **flags iff `fast_max < slow_max - slow_min`**, under separation. Validated against four datasets spanning 1.09x to 857x.
- Every external pin in the wave — nine checkouts across five surveys — is exact, clean, and consistent wherever two surveys share a comparator.

## What to do next

- [ ] Collect each running worker's final report. Codex workers cannot commit: read their `COMMIT-PLAN.md` at the worktree root and replay it. `scratchpad/parse_commit_plan.py` + `apply_commit_plan.py` from session 4 do this; the pattern is `git reset --hard main`, `git checkout <final-tree-tag> -- .`, then stage each commit's exact path list. Verify the resulting tree is byte-identical to the validated one before merging.
- [ ] **Land v2 first.** `f547c394` is the critical path: every wave-2 confirmatory receipt is queued behind it, and four workers were told to hold their confirmations until the lead says v2 has landed. Re-run all four gates on the merged tree.
- [ ] Verify the rework closed **F13–F19**, not just F1–F7. It reads `reviews/f547c394-r3.md`, but check rather than assume — the audit covered `main` only and does not know the rework's branch state.
- [ ] Resolve the `c077a88b` shared-crate collision **in favour of the rework's implementation**, then re-run that survey. Its P-18 repair and v2's F3 fix are two implementations of the same thing.
- [ ] Re-run every wave-2 confirmatory campaign under v2, then close the wave. Also re-run `1d0da41f`'s confirmation: its receipt pins v1.
- [ ] Chase the outstanding corrections: `6c6b09b1`'s 0x11B framing, `eda07788`'s three AFF3CT citations plus the missing `Codec_LDPC.cpp:75-84` factory leg, `6fb89a3c`'s M4RI licence label (`GPL-2.0` → `GPL-2.0-or-later`, to match `6c6b09b1`), `c7113c5a`'s line-815 wording.
- [ ] Collect each survey's answer to the F18 question: declared `measurement_resolution` versus its pilot's widest observed relative CI half-width. A declared resolution below its pilot's observation is a falsified freeze.
- [ ] Put the `eda07788` 5G NR whole-consumer encoder cell follow-up to the invoker (see Open questions).
- [ ] After wave 2 closes: advance `current_wave` to 3 and dispatch `3be770d5`, `53c5a8c0`, `19513245`, `1d4fd63d`, `5cbb6545`, `2037941f`, `c04dd4ac`.

## Traps — do not repeat these

- **Do NOT treat a provider-limit stop as terminal.** Four native workers stopped at a session limit; the lead read that as death, preserved their trees and spawned four continuations — but a `SendMessage` to a stopped agent revives it from its transcript, and the lead's own post-reset messages had already done so. Four worktrees were double-dispatched, and one accumulated two implementations of the same rule plus a committed `.pyc` before the owner consolidated it. Check `ListAgents` for running/idle before re-dispatching, and prefer reviving the original by message over spawning a replacement.
- **Do NOT assume a shared-tooling fix binds workers who have not merged it.** The turnstile landed on main and changed nothing for two hours: every worktree runs its own copy of `scripts/cargo-budget.sh`. Merge main into every live worktree, then verify the fix is present in each, before expecting it to take effect.
- **Do NOT assume the CCX1 turnstile binds every acquirer.** It binds only acquirers that pass through it. Two survey launchers take `flock -s` on the mutex directly to wrap a harness build, skipping it. Bounded in practice, but `cargo-budget.sh` is the only supported shared acquirer — see the comment in `dev/scripts/ccx1-bench-flock.sh`.
- **Do NOT reset a worker branch to `main` without checking what the branch already carried.** Replaying `1d0da41f`'s commit plan on a reset branch silently dropped its pre-continuation commits, whose files then showed as untracked; the plan's path lists assumed they were already committed. Tag the validated tree first (`git tag wip-final-<id>`), and diff the reconstructed tree against that tag before merging.
- **Do NOT let a survey put code citations in prose.** Two of the five surveys carried claims their citations did not support — a kernel described as unreachable that is load-bearing in `BitMatrix::matvec`, and a cell justified on a `0x11B` field gf2 does not ship. Neither would surface from re-reading the prose. `c077a88b`'s `source-evidence.json` ledger (project, commit, path, line, **the verbatim text of that line**, why) audited clean on 37 citations in seconds; adopt it.
- **Do NOT accept a frozen addendum an agent other than the owner derived.** A frozen addendum in `26465e6c` was produced by the stood-down duplicate and swept into the owner's commit by `git add -A`. Require the owner to re-derive it and confirm byte-identity.
- **Do NOT expect `research-review` to survive a large linked footprint.** The reviewer reads every linked document. Keep raw command transcripts in unlinked companion files; the report carries the argument, the companion carries the evidence.
- **Do NOT expect a codex worker to commit from a linked worktree.** Confirmed empirically this session: `--add-dir <repo>/.git` does not help; the sandbox mounts the git directory read-only regardless. Codex workers keep a `COMMIT-PLAN.md`; the lead commits and checkpoints their trees periodically.
- **Do NOT run `cargo-ci` while a wave is building.** Still true, but the 15 s cap (`23aa0824`) and the turnstile remove most of it. The four previously-flaky tests are named in `.config/nextest.toml`'s rationale comment.
- Unresolved traps from [session 3](handoff-3.md#traps--do-not-repeat-these), [session 2](handoff-2.md#traps--do-not-repeat-these) and [session 1](handoff.md#traps--do-not-repeat-these) remain in force, except the rate-limit ones, which the invoker voided and which have been erased from the chain.

## Open questions needing invoker input

- Question: Should the epic gain a 5G NR whole-consumer encoder cell as a new survey family?
  - Context: `eda07788` proved gf2's rate-matching column selection and AFF3CT's `Puncturer_5G` select identical bits in all six surveyed configurations, but neither exposes bit selection as an entry point, so no timed cell is possible at that level. `gf2`'s `encode_rate_matched` is private while `BlockEncoder::encode` is public — the asymmetry that makes a whole-consumer cell feasible where the bit-selection cell is not.
  - Options: A) file it as a new task inside this epic; B) record it as a tracked follow-up outside the epic; C) drop it.
  - Recommendation: B. It is a different comparison family needing its own addendum, and adding a survey family mid-flight is a scope decision. The write-up is in `eda07788`'s findings, ready to lift.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [plan review](plan-review.md); [progress](progress.json).
- **[Round-3 review record](reviews/f547c394-r3.md)** — F1–F19, the F8 onset predicate, the F14 amplification, and the three-rule set (R1 declared decimals stay decimal; R2 declared is checked against observed and a digest is not a value, and the checker may not share the deriver's implementation; R3 rules total over their class, guards total over their property). Earlier rounds: [r1](reviews/f547c394-r1.md), [r2](reviews/f547c394-r2.md).
- Wave-1 evidence: `dev/active/f547c394/{protocol.md,design.md,provenance-clarification.md,rework-validation.md,rework-validation-raw-outputs.md,addendum.schema.json}`, `dev/bench_results/f547c394/`.
- Merged wave-2 work: `1d0da41f` at `4d630aec`, evidence under `dev/active/1d0da41f/` and `dev/bench_results/1d0da41f/`.
- Infrastructure changed this session: `.config/nextest.toml` (`23aa0824`), `scripts/cargo-budget.sh` + `dev/scripts/ccx1-bench-flock.sh` + its test (`d088c284`, `b3de0686`), `AGENTS.md` mutex paragraph.
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.
