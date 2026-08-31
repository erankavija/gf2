# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 34

**Date:** 2026-08-31
**Session number:** 34
**Prior handoffs:** `dev/active/b8206228-permanent-statistics/handoff.md` through `dev/active/b8206228-permanent-statistics/handoff-29.md`

## Current state

- Epic: `b8206228` — state: backlog
- Wave in progress: wave 10 of 13
- Children summary: 11 of 15 direct epic dependencies are done; `0665e7be`, `53d8e438`, `b05c908b`, and `b5d45bcd` remain backlog. The operational frontier is non-direct prerequisite `02b8137c`, which is in progress; all three campaign arms remain blocked on it.
- Active claims: `b8206228` and `02b8137c` remain assigned to `agent:jit-execution-lead` because a session-independent validation timer is armed against their protected evidence.
- Open escalations: none for the validation continuation. A separate owner go remains mandatory before the first campaign-purpose draw after validation closes.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics/` reflects the above and the armed timer.
- Scheduled work: transient user timer `gf2-predraw-validation3.timer` is active/waiting for 2026-09-01 02:00:30 EEST with `AccuracySec=1s`; service is inactive/dead until then. It does not survive reboot.
- Frozen execution checkout: `.agents/worktrees/agent-02b8137c-run`, branch `worktree-agent-02b8137c-run`, clean at `fa8a9d9cd89dcc2fccb3dde8d67366e42730f3ed`. Preserve it until the service is terminal and its evidence has landed.

## What just happened

- Interpreted the owner's “One exact producer can be weakened” decision as exactly one runtime-observed producer per immutable ordered segment, with no redraw, overwrite, protocol change, or third producer.
- Amended `02b8137c` REQ-01/05/07 and its Decision section at `2791d8b1`; resolved the epic escalation record at `ce0c820d`.
- Dispatched a Sol/xhigh implementation and independent Sol/xhigh review. Initial implementation `493ebebf` failed review on four concrete findings: stale v1 receipt alias, an unsafe cutover promise, a schema-mismatch false-positive test, and missing adversarial on-disk rehash coverage.
- Rework `fb822683` closed all four findings. Canonical output is `pre-draw-validation-v2-receipt.json`; the v1 document points forward; fresh schema-v2 runtime drift proves byte-for-byte refusal; both producer states and one terminal per segment are mutation-tested through the production journal verifier.
- Enforced canonical-cutover: after the complete receipt is committed and independently verified, remove the CLI continuation flag, public continuation admission choice, and runnable v1 branch. Permanently retain only the private exact-SHA `FrozenProducer0RunStateV1Evidence` decoder needed to verify immutable producer-0 evidence, plus the cited v2 evidence decoders.
- Independent rereview passed `fb822683` with no blockers. Merged at `e0c3959a`; integrated main CI passed 5,208 tests with zero failures and every auxiliary stage green.
- Linked the v1 preregistration and v2 continuation prose/JSON to `02b8137c` at `8ac6a9d3`; recorded continuation readiness at `2367066f`.
- Created the clean execution worktree from main `4eed177e`, built the Rust 1.95 release+HIP validator, and committed its checked one-shot runbook at `fa8a9d9c`.
- Qualified that exact execution tree with full CI: 5,210 passed, 0 failed, 239 skipped; every auxiliary stage passed. Validator SHA-256 is `d0d674800f0d32778b3d50fd176d3e810daebc986620a537b437633a66ae788e`; dependency-source revision is `c0db11e25936ceb8323d864b88de54923281c0a5`.
- Armed and verified `gf2-predraw-validation3.timer`. The service environment pins exact HEAD, dependency revision, binary SHA-256, ROCm path, and external log. The runner additionally verifies the authority SHA, producer-0 run-state SHA, all 22 authorized prefix files, clean tree, absent v2 receipt, and post-02:00 window before an address can open.
- Coordinated with Claude's concurrent execution lead through `/home/vkaskivuo/Projects/forum-poc`. Claude stopped his gate, left main clean, confirmed no host work remains, and committed to no CCX1 release after 01:30 or before this validation service is terminal.
- Recorded the armed timer and identities at `47cadfdb`. No validation continuation draw or campaign-purpose draw occurred during this session.

## What to do next

- [ ] At or after 2026-09-01 02:00:30 EEST, inspect `systemctl --user status gf2-predraw-validation3.service` and `/home/vkaskivuo/.local/state/gf2-validation/validation-v2-continuation-20260901.log`. Do not infer timer failure from a sandboxed user-bus denial; retry inspection with the explicit host permission.
- [ ] Verify the execution worktree remains at `fa8a9d9c`, the binary remains `d0d67480...`, and the service either refused before opening an address or produced exactly the five missing terminals (`q5,n=2..3`; `q7,n=1..3`) plus `producer-segment-state-v2.json` and the canonical v2 receipt. Any partial start is terminal evidence under no-redraw.
- [ ] Independently run the frozen receipt verifier against `dev/active/02b8137c/pre-draw-validation-v2-receipt.json`; verify two contiguous, disjoint five-anchor producer segments and rehash all cited journal bytes.
- [ ] Commit the complete journal, receipt, external run evidence as appropriate, and the runbook from `worktree-agent-02b8137c-run`; merge under `/tmp/gf2-main-leads.lock`, then run integrated CI. Do not reclaim the worktree before this lands.
- [ ] Perform the promised canonical cutover in focused rework: remove temporary continuation execution surfaces while retaining only private exact-SHA historical receipt verification. Rebuild and reverify the committed v2 receipt with the post-cutover binary.
- [ ] Link the canonical v2 receipt to `02b8137c`; re-evaluate `cargo-ci`, `code-review`, `doc-review`, and `research-review` on current evidence; perform the six-tier lead review and close only if every hard criterion passes.
- [ ] Ask the owner for the already-required separate campaign go. If granted, execute exact `q=7,n=20` to terminal state before any other campaign-purpose cell, then serialize the remainder of q=7, q=5, and q=3 under the frozen protocol.
- [ ] Continue through `f27150a5`, wave 12, wave 13, and the epic gates. Direct epic backlog remains `53d8e438`, `b05c908b`, `0665e7be`, and `b5d45bcd`.

## Traps — do not repeat these

- **Do not reuse `/home/vkaskivuo/.local/state/gf2-validation/run-validation-0200.sh`.** It runs from mutable main, names the superseded v1 receipt, and omits the explicit continuation authority. The committed runbook is `dev/active/02b8137c/run-validation-v2-continuation.sh` on `worktree-agent-02b8137c-run` at `fa8a9d9c`.
- **Do not run the evidence binary from mutable main.** `observe_provenance` resolves runtime HEAD and the latest `crates/`/`Cargo.lock` revision. Main advanced during coordination, so a main-based run could claim source identities different from the built binary. The pinned clean worktree closes that provenance gap.
- **Do not rebuild, test, merge into, reset, or reclaim `.agents/worktrees/agent-02b8137c-run` before the timer is terminal.** The service pins HEAD `fa8a9d9c` and binary `d0d67480...`; any change must cause pre-draw refusal, not be worked around.
- **Do not treat a transient timer as reboot-safe.** `gf2-predraw-validation3.timer` lives under `/run/user/1000/systemd/transient` and disappears on reboot. Verify it is active/waiting before relying on the schedule; if it vanished, inspect the journal and log before considering a new timer.
- **Do not diagnose a user-bus sandbox denial as a timer or permission failure.** The earlier timer fired and ran for 17m36s. `systemctl --user` needs host access in this environment; retry the inspection with approval.
- **Do not alter, delete, or redraw the original five terminals or any later partial start.** The continuation authority binds all 22 producer-0 prefix files byte-for-byte, and the protocol's no-redraw rule makes every durable start final evidence.
- **Do not publish the final schema-v2 receipt at the old v1 path.** `pre-draw-validation-v1-receipt.json` is superseded historical prose only; the canonical path is `pre-draw-validation-v2-receipt.json`.
- **Do not leave the temporary v1 execution admission surface after the receipt lands.** Canonical-cutover requires deleting runnable compatibility. Retain only the exact-SHA private evidence decoder required for permanent receipt verification.
- **Do not start a campaign-purpose draw merely because validation passes.** A separate owner go is still required, and exact q=7,n=20 must reach terminal state before every other campaign cell.
- **Carry forward every unresolved trap in `handoff-29.md` and earlier handoffs.** In particular, the float-roundtrip repair, no-third-producer rule, exact post-02:00 window, and serialized field ordering remain binding.

## Open questions needing invoker input

- Question: After `02b8137c` closes with a verified passing receipt, may the campaign-purpose phase begin?
  - Context: Existing authority covers validation only; no campaign-purpose matrix has been drawn.
  - Options: (A) authorize the frozen campaign, starting with exact q=7,n=20 alone; (B) keep all campaign arms blocked; (C) revise the frozen campaign protocol through a separately tracked decision.
  - Recommendation: A only after the v2 receipt, canonical cutover, all four `02b8137c` gates, and lead review pass; this preserves the preregistered ordering and evidence boundary.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Validation issue: `jit issue show 02b8137c`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-29.md`
- Preregistration: `dev/active/02b8137c/pre-draw-validation-v1.md` and `pre-draw-validation-v1-preregistration.json`
- Continuation authority: `dev/active/02b8137c/pre-draw-validation-v2-continuation.md` and `pre-draw-validation-v2-continuation.json`
- Preserved journal: `dev/active/02b8137c/validation-journal/`
- Pinned runbook: `.agents/worktrees/agent-02b8137c-run/dev/active/02b8137c/run-validation-v2-continuation.sh`
- Timer: `gf2-predraw-validation3.timer`; service: `gf2-predraw-validation3.service`
- External log: `/home/vkaskivuo/.local/state/gf2-validation/validation-v2-continuation-20260901.log`
- Key commits: `3fd2893b` journal preservation; `2e760af7` float repair; `e0c3959a` continuation merge; `8ac6a9d3` document links; `47cadfdb` timer record; `fa8a9d9c` pinned execution runbook
