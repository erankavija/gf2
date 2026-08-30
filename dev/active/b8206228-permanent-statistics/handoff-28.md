# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 32

**Date:** 2026-08-31 (session spans 2026-08-30 evening)
**Session number:** 32
**Prior handoffs:** `handoff.md` through `handoff-27.md`. Their unresolved traps remain in force unless this record supersedes them.

## Current state

- Epic `b8206228`: backlog; 12 of 15 direct dependencies done after this session.
- `6639435f` is DONE at merge `660b742b` / closure `3d2ed9fd`: owner authorized the bounded high-tier rework; all four audit boundary findings plus four lead review findings (F-R1..F-R4) closed; cargo-ci clean-tree pass, code-review pass with zero findings, doc-review pass (one pre-existing advisory tracked by `049a89af`).
- `02b8137c` implementation is complete and merged at `d06d6fe3`; cargo-ci gate passed; the three AI review gates fail only on the not-yet-existing receipt.
- Evidence run ARMED: systemd user timer `gf2-predraw-validation` fires 2026-08-31 02:00:30 running `scratchpad/run-validation-0200.sh` (pristine-tree check, binary hash, rocm snapshot, then the exact preregistered invocation under `dev/scripts/ccx1-bench-flock.sh --full-host`, `--workers 24`). Runner binary `target/release/permanent_validation` sha256 `76c778f1e3e90530c3db39b26d8954642b601a17d1050144929270e48c59297e`, built `cargo +1.95.0 --release --features hip` at `660b742b`; `ldd` resolves with an empty environment (no profile-env dependence). A persistent log monitor watches the run.
- Campaign arms `ed494117`, `1d0b3ec4`, `90a61cd4` remain blocked on `02b8137c`. The owner requires a separate explicit go before any campaign-purpose draw (decision 2026-08-30: "arm tonight, ask before campaign").
- Peer session gf2-dc holds `perf/cargo-cpu-budget` at `28fbee35` (cargo-ci lock redesign: budgeted builds, exclusive test lock, enforced shared-mode CCX1 acquisition, AGENTS.md +18/-3). Owner ruled it lands; agreed sequencing: merge right after the validation receipt commits, or unconditionally if the run has not started by 02:45.

## What this session did

- Audited sessions 26-31 for the owner (slowness = late-discovered protocol validation prerequisite + repeated owner stops + host serialization; sessions 26-27 productive, 28-31 mostly reconciliation under stops).
- Owner decisions obtained: 6639435f bounded repair authorized; validation run armed tonight with ask-before-campaign.
- Resumed `02b8137c` with a high-tier worker: verified all nine auditor hazards in the preserved WIP, fixed four latent defects (worst: the Rust-toolchain gate ran only after all anchors had drawn), added thin runner + preregistration + 22 release tests + backend preflight-before-first-address. Merged; cargo-ci passed.
- Ran the owner-authorized `6639435f` rework to completion through four lead review findings and three gate rounds.
- Negotiated the shared CI-serialization contract change with gf2-dc (see Current state) and enforced the owner's AGENTS.md terseness directive on its wording.

## Resume order

1. If the timer fired: read `scratchpad/validation-run.log` and the receipt at `dev/active/02b8137c/pre-draw-validation-v1-receipt.json`. Exit 0 = passed. Run `target/release/permanent_validation --verify-receipt <path>` as the launch check.
2. Commit the receipt + journal evidence, `jit doc add` the receipt to `02b8137c`, re-evaluate code-review, doc-review, research-review, run the lead six-tier review, close `02b8137c`.
3. Signal gf2-dc to merge `perf/cargo-cpu-budget`; review its AGENTS.md hunk against the terseness directive (baseline sha256 16c1d917…, 165 lines).
4. ASK THE OWNER before claiming `ed494117` or drawing any campaign-purpose cell. First campaign action is exact (7,20) through the canonical coordinator after restoring the preserved `c9b2307a…` emitter (handoff-25 procedure).
5. A failed anchor: preserve everything, block the arms, diagnose, new validation protocol run — never redraw.

## Traps — do not repeat these

- **Do not run `jit` or diagnose gate state from a worktree cwd.** Re-confirmed live this session: a shell parked in `agent-6639435f` made `jit gate status` report a recorded doc-review run as "never run" and hid `.jit/gate-runs`. `cd /home/vkaskivuo/Projects/gf2` first, every time. (Extends handoff-12.)
- **Do not read a leak-check hit as a worker leak before checking for peer sessions.** The 22:20 `scripts/cargo-ci.sh`/`.config/nextest.toml` modifications were gf2-dc's uncommitted host-throughput work on main; I ran `git restore` over them (patch was preserved first — always preserve before restoring). `ListAgents` shows peer sessions; ask before reverting.
- **A worker issued host-wide process kills by full-command-line pattern** (matching cargo-nextest and the crate name, ~22:05, cleaning its own aborted CI run). No observed casualties, but such patterns are not worktree-scoped and can kill peer builds. Brief workers: terminate only PIDs from their own process tree.
- **Do not expect the three AI review gates to pass before run evidence exists.** All three failed `02b8137c` solely on the absent receipt (REQ-05/09). Sequencing, not defect: evaluate cargo-ci pre-run; evaluate the reviews after the receipt is committed and linked.
- **Background Bash tasks are killed at the 10-minute timeout cap — never arm a long sleep-then-run there.** The first arming attempt would have died mid-sleep. Wall-clock `systemd-run --user --on-calendar` (with `XDG_RUNTIME_DIR` + `DBUS_SESSION_BUS_ADDRESS`) plus a persistent log Monitor is the working pattern (extends handoff-20/21).
- **sccache does not carry fresh-worktree builds**: measured 1.7% Rust hit rate (9/519) against a 44 GiB cache, 2026-08-30, cause unestablished (gf2-dc measurement; their absolute-path mechanism claim was retracted as untested — do not cite it). Cold fresh-worktree CI 661 s vs warm 62 s. Dispatch cost models must assume the hardlinked cache pool, not sccache, carries reuse.
- **gf2-sim fast-tier headroom watch**: `rare_event_artifact_final_receipt_regeneration` 3.7 s and `rare_event_artifact_partial_publish_recovery` 3.5 s sit near the 5 s kill on a contended host.
- **A commit that adds or reworks public API docs needs `cargo doc` even when the round's agreed verification list omits it** (three intra-doc-link warnings shipped in `ced69663` and were caught only by the doc build two rounds later).
- **`git commit -F -` after a `&&` chain has no stdin and hangs** (worker note; two-minute silent hang, no lock left). Use a message file.

## Open questions needing invoker input

- After validation passes: authorize claiming `ed494117` and executing exact $(q,n)=(7,20)$ to terminal state (owner said ask first). Options: authorize the q=7 arm now / authorize all three arms sequentially / hold.

## Reference artefacts

- Runbook + log: `<scratchpad>/run-validation-0200.sh`, `<scratchpad>/validation-run.log`
- Timer: `systemctl --user list-timers 'gf2-predraw-validation*'`
- Preregistration: `dev/active/02b8137c/pre-draw-validation-v1-preregistration.json` (+ prose `.md`)
- Receipt destination: `dev/active/02b8137c/pre-draw-validation-v1-receipt.json`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Peer branch: `perf/cargo-cpu-budget` at `28fbee35` (gf2-dc)
