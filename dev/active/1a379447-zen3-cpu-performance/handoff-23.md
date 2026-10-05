# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 27

**Date:** 2026-10-05T03:20Z
**Session number:** 27
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-22.md` in this directory. Read their unresolved traps in order.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`; a container stays backlog until its subtree is terminal).
- Wave in progress: 4 of 7. Wave 3 is complete: stories `c04dd4ac` and `2037941f` are done.
- Done this session: `50f0bd42`, `ad2a6a58`, `4c1e441f`, `6e87c436`, `2ad3a3e0`, story `2037941f`.
- Open direct children of the epic: `1362381c` (backlog), and five issues filed this session, all ready: `4337c02e`, `6833c5b5`, `7d44b71f`, `b2e09d41`, `fa1d8733`. `63bad95d` is ready. Story `ed3d490e` waits on `f63a2464`.
- `f63a2464` is in progress, assigned `agent:worker`: cargo-ci and research-review pass, code-review fails on three findings that are invoker rulings. Rework count 1.
- Active claims: story and task leases taken for gate evaluation expire on their own; reacquire before any gate.
- Open escalations: one, on `f63a2464` (see Open questions).
- No worker, build, gate or timer is running. Main is clean.
- Worktrees: `agent-9fb40c83` (branch `worktree-agent-f63a2464`, merged through `99c81b745`) is kept for `f63a2464`'s next round. The five other worktrees of this session are reclaimed; their branches remain. `agent-02b8137c-run`, `agent-616e1d7c`, `agent-d1b4f85e-run` and `agent-fd9d5416` belong to other epics.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Collected the M4RI pilot of `50f0bd42` (window 2026-09-28): verified from its execution log, resolution within the frozen ceiling, confirmation frozen and smoked untimed.
- Invoker lifted the hold on the five out-of-wave confirmation rows. Window `gf2-bench-window-20261005` ran six jobs, 2026-10-04T23:00:04Z to 23:13:51Z, all with done markers; every campaign was then verified from its own execution log.
- `50f0bd42`: no adoption, both confirmatory M4RI cells `regressed`/`fail`. code-review F1 (path coupling in new tooling) closed by rework 2. Review `reviews/50f0bd42-r2.md`.
- `ad2a6a58`: lane retained; gates passed first round. Review `reviews/ad2a6a58-r1.md`.
- `4c1e441f`: path retained; gates passed first round. Review `reviews/4c1e441f-r1.md`.
- `6e87c436`: no-change verdict with a content-pinned drift record, a route comparison over the dense cell arms, new boundary tests that reach the SIMD strides, and the safety contract of the fused AND-popcount kernel. code-review F1 (reproduction anchor) closed by rework 1. Review `reviews/6e87c436-r1.md`.
- `2ad3a3e0`: findings and generated tables for the story. doc-review F1 (unqualified production-change statement) closed by rework 1. Review `reviews/2ad3a3e0-r1.md`.
- Story `2037941f`: seven gates pass with advisory findings only, each tracked. Review `reviews/2037941f-r1.md`.
- `f63a2464`: three QC confirmations verified; decisions recorded; smoke converged; untimed peak-memory record added; research-review F1 to F3 closed through `dev/active/f63a2464/corrections.md` with frozen bytes preserved.
- Shared tooling changed at its source: `dev/scripts/smoke-campaign-arms.sh` derives its label from cell roles and has a test; `dev/scripts/campaign_tables.py` prints acceptance finding counts; `dev/scripts/repository_files.py` gained `live_file`; the canonical freezer gained `--resolution-decimals`.
- Filed under the epic: `b2e09d41` (private LDPC plan checker), `6833c5b5` (dense-parity source-evidence ledger), `7d44b71f` (AVX2 safety contracts), `4337c02e` (matvec SIMD threshold against the scalar lane), `fa1d8733` (unselected XOR unroll bodies).
- Coordinated with session gf2-b2 (epic fa787f85) on the shared main checkout; that session has closed.

## What to do next

- [ ] Read the invoker's rulings on `f63a2464` (Open questions). Then dispatch its next round in `agent-9fb40c83` (rework 2 of 2), re-run code-review, and close or reject it. `ed3d490e` follows.
- [ ] Tell the lead of epic fa787f85 or the repository owner that `4c1e441f` and `ad2a6a58` are done and when `f63a2464` reaches done or rejected: its issue `198aafa3` REQ-01 waits on all three (that epic resumes from `dev/active/fa787f85-documentation-overhaul/handoff-8.md`).
- [ ] Wave 4: dispatch `63bad95d` starting with its untimed route inventory, and the untimed issues `7d44b71f`, `b2e09d41`, `6833c5b5` in parallel. Serialize `fa1d8733` after `7d44b71f`: both edit `crates/gf2-kernels-simd/src/x86/avx2.rs`.
- [ ] `4337c02e` and `63bad95d` both concern the `matvec` threshold and the offline tuning system; give each worker the other's issue text, and check `63bad95d` against the tuning epic's extent-sweep tooling (`dev/scripts/tuning-extent-campaign.sh`) for a cross-epic dependency before any calibration is frozen.
- [ ] Any timed work is a queue line for an overnight window; the invoker's standing authorization covered wave 3 only. Ask before arming a window for wave 4.
- [ ] Before the epic's own gates, reconcile `surfaced_pitfalls` in `progress.json` against the epic's criteria.

## Traps — do not repeat these

- **Do NOT `cd` into a worktree in the lead's shell, even inside a compound command.** It re-anchored the session's working directory once this session; `cd` back to the primary checkout worked, but use `git -C` and absolute paths.
- **Do NOT read a gate list from `gates_required` in `jit issue show --json`.** The field is null; a loop over it ran zero gates and looked like success. Name the gates explicitly.
- **Do NOT send two tool calls in one block when the second depends on the first's commit.** A gate chain started beside the commit that preceded it.
- **Do NOT expect a worker's "newly added" tooling to pass code-review with `../..` root climbs or literal `dev/` locations.** `@/invariant/no-dev-path-coupling` failed `50f0bd42`; the reviewer attributes every `jit:<id>` commit of the issue, so sweep the issue's older scripts too. Compliant forms on main: `git rev-parse --show-toplevel`, `dev/scripts/repository_files.py` (`package_directory`, `live_file`), the story's `survey/repo_artifacts.py`, each byte-field family's `survey/locate.py`.
- **Do NOT document a reproduction command that depends on where HEAD is.** `git merge-base HEAD main` resolves to HEAD after the merge (`6e87c436` code-review F1). Pin baselines by content digests in a committed record.
- **Do NOT link a `.py` file or an assembly listing expecting a clean link check.** The scanner reads `](` in Python and `@plt` symbols as links; `repo_artifacts.py` had to be unlinked, and story `2037941f` shows scanner errors on seven assembly listings while repo-validate passes.
- **Do NOT treat an instruction to run queued confirmations as settling a frozen-record interpretation.** The lead recorded "enable the five held rows" as accepting the per-ledger reading of `f63a2464`'s confirmatory cap; code-review rejects a reading added after the receipts. Ask the interpretation question separately and before the window.
- **Do NOT merge a branch that deletes files without checking the documentation epic's manifest.** `dev/active/fa787f85-documentation-overhaul/migration/check.py` fails on main because `manifest.toml` and `67048b47-linked-pairs.txt` still list `dev/active/ad2a6a58/survey/axpy-arm/src/bin/gf256-axpy-smoke.rs`, `dev/active/ad2a6a58/survey/runner-smoke.txt`, `dev/active/4c1e441f/survey/gemm-arm/src/bin/gf256-gemm-smoke.rs`, `dev/active/4c1e441f/survey/runner-smoke.txt` and `dev/active/f63a2464/survey/arms/src/bin/qc-arm-smoke.rs`. That file belongs to epic fa787f85; the check is not part of `cargo-ci.sh`.
- **Do NOT run `./scripts/cargo-ci.sh` with a stale sccache server.** A server started under a gate kept a deleted temp directory and failed a worker's build; `CARGO_CI_NO_SCCACHE=1` avoids it.
- **Do NOT assume the tree a campaign measured equals main.** Twelve production files differ in code from the dense campaigns' snapshot through other issues' commits (`dev/active/1a379447-zen3-cpu-performance/6e87c436/survey/production-drift.json`).
- **Do NOT regenerate `dense-parity-source-evidence.json` in place.** It is receipt-pinned and cited by the frozen addendum; `6833c5b5` owns the repair.
- **Queue row `dbd8787d` has no done marker and no worktree.** The window runner logs it as missing and skips it; leave another epic's row alone.
- All traps of [handoff-22](handoff-22.md) and earlier remain in force.

## Open questions needing invoker input

- Question: `f63a2464` code-review F1 — three QC confirmations ran against a frozen cap of one confirmatory campaign per family, the per-ledger reading having been written after the receipts. Which way?
  - Context: the reviewer requires an explicit amended version and a fresh confirmation under the measurement contract, and treats the QC decision evidence as invalid.
  - Options: (A) record that QC has no valid confirmatory verdict under the frozen budget, so no family proceeds, with all receipts preserved; (B) a versioned amendment stating the per-ledger cap, then fresh confirmations in a new window after rechecking each ledger's attempt arithmetic.
  - Recommendation: A.
- Question: `f63a2464` code-review F2 — REQ-04 has no timed cells for Q and L (the frozen screen excludes them) and no fastest quality-compatible external comparison (`c077a88b` admits no external candidate). Is the criterion met by the recorded absences?
  - Options: (A) rule that a family the frozen screen excludes is outside REQ-04 and that an absent admitted comparator is a recorded limit, as an explicit amendment of the criterion's reading; (B) new measurement under an amended instrument.
  - Recommendation: A, as an explicit invoker amendment of the issue text.
- Question: `f63a2464` code-review F3 — the QC production design is unreviewed and no issue owns its scope. Review and file, or drop?
  - Recommendation: moot under option A of the first question; under B, review the design in `dev/active/f63a2464/decision-record.md` and let the lead file its three tasks under `ed3d490e`.
- Question: may wave 4 arm overnight benchmark windows at 02:00 Europe/Helsinki as wave 3 did?
  - Context: `63bad95d` and `4337c02e` need timed campaigns.
  - Recommendation: yes, with the same nightly rule.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`; worker brief `worker-brief.md`; measurement contract `measurement-contract.md`.
- Reviews of this session: `reviews/50f0bd42-r2.md`, `reviews/ad2a6a58-r1.md`, `reviews/4c1e441f-r1.md`, `reviews/6e87c436-r1.md`, `reviews/2ad3a3e0-r1.md`, `reviews/2037941f-r1.md`.
- Story synthesis: `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-findings.md`, `mid-range-tables.md`.
- `f63a2464`: `dev/active/f63a2464/decision-record.md`, `findings.md`, `corrections.md`; `dev/bench_results/f63a2464/timing-tables.md`, `memory/peak-rss.json`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.
