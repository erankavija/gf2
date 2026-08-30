# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 25

**Date:** 2026-08-30
**Session number:** 25
**Prior handoffs:** `handoff.md` (session 3) through `handoff-20.md` (session 24). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog, lead-claimed (`agent:jit-execution-lead`); the epic has never been transitioned out of backlog despite 25 sessions of work — wave 8 of 13.
- `86185f88` (cell-exhaustive equivalence receipt) **done** this session: `code-review` and `doc-review` both passed, lead six-tier review PASS.
- `7a816262` (campaign freeze) is now the only open item in wave 8. `doc-review` passed 2026-08-29; `research-review` still shows the 5-finding FAIL from 2026-08-29T16:57. F3 of that run is closed by this session's receipt; F1, F2, F4, F5 have not been re-verified.
- Children summary at wave 8: `1947125e`, `ec22205e`, `6dfb1b5e`, `86185f88` done; `7a816262` in_progress (rework counter 2 = MAX — any gate or lead-review failure escalates, never reworks).
- Active claims: `7a816262` → agent:jit-execution-lead.
- Open escalations: none.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics/` (see `session_25_state`).

## What just happened

- Session-24's armed 02:00 run had fired on time and died 30 s in, at the `cargo build --features hip` step. Cause: `hipcc` adds `-I$ROCM_PATH/include` only when `ROCM_PATH` is set, and a systemd user unit does not source `/etc/profile.d/rocm.sh`. Isolated in both directions before fixing.
- Receipt delivered: 124 nominated configurations across all 63 exact campaign cells agree with the reference per matrix. `rows=124 q3=50 q5=41 q7=33 mismatches=0 unexecuted=0`, wall time 230 s, run started 03:22:05+03:00 inside the owner's window.
- `code-review` first failed `86185f88` with two findings, both now closed:
  - **F1**: `campaign_selection.rs` carried the prior-run probe costs as a hand-written constant table, which `@/inv/runtime-observed-provenance` names a staleness defect. The 28 observations now live in `dev/benchmarks/permanent_campaign/probe-costs-de5f7414.csv`, parsed under a pinned SHA-256. Every row was verified to appear verbatim in the `de5f7414` grid receipts, and every per-cell budget is unchanged.
  - **F2**: the addendum mis-attributed the preamble's `git_worktree_dirty` flag to the CSV. `HostInfo::probe()` captures the flag before `open_csv` creates the CSV; it is the `.log`, which `tee` creates first.
- **Provenance design change, owner-directed.** `env.rs` recorded the measured source as three git *commit* hashes (`git_sha`, `harness_sha`, `deps_sha`). A commit hash names a position in a mutable DAG and is invalidated by ordinary history operations while the measured bytes are unchanged; `harness_sha` was `git log -1 -- <dir>`, the last commit that happened to touch a directory, which is not a content identifier at all. Binding provenance is now content-addressed: `harness_tree`, `deps_tree`, `wave_gpu_tree` (each `git rev-parse HEAD:<path>`), each qualified by its own dirty flag, alongside `binary_sha256`. `git_sha` and `git_worktree_dirty` are retained and explicitly labelled informational, and the emitted note now states which fields bind.
- `wave_gpu_tree` closes the standing gap recorded in handoff-20: `dev/research/permanent_wave_gpu` holds measured kernels and was covered by neither previous source field.
- `@/inv/claims-trace-to-artifacts` said "git revision", which mandated the design just removed. Amended to "source identity" (owner-directed), re-projected into `AGENTS.md`, `jit invariant check` reports no enforcement drift. The wording is satisfied by both content-addressed trees and a commit hash, so historical receipts stay conformant.

## What to do next

- [ ] Evaluate `research-review` **and** `doc-review` on `7a816262`; both must re-run at final content. F3 closes via this session's receipt. Verify F1, F2, F4 and F5 of the 2026-08-29T16:57 run are closed at HEAD in the Tier-1.5 cumulative table before accepting. Rework is at MAX — any failure escalates rather than reworking.
- [ ] Then wave 9: dispatch `3f664839` (design; native Opus per owner directive) and `73317b2e` (implementation; Sol xhigh; the coordinator lives outside the frozen emitter process — see handoff-19).
- [ ] **Follow-up, needs owner decision:** `crates/gf2-sim/src/permanent_campaign/provenance.rs` carries a second, independent provenance implementation with the same content-versus-history defect — `RuntimeSourceIdentity.deps_source_revision` (lines 415–423) is captured by `git log -1 --format=%H -- crates/ Cargo.lock` (lines 579–586). It was deliberately left unchanged this session as out of `86185f88`'s scope. AGENTS.md's "preserve one canonical abstraction" makes the duplication itself worth resolving.
- [ ] **Follow-up, needs owner decision:** nine JIT issue descriptions quote the invariant's superseded field list ("seeds, git revision, hardware, toolchain"). Four are still `backlog` and will be worked in waves 10–13 with stale criteria text: `1d0b3ec4`, `90a61cd4`, `ed494117` (the three campaign arms) and `b05c908b`, plus `b5d45bcd`. Editing an issue description is a scope change, so it was not done unilaterally.

## Traps — do not repeat these

- **Do NOT schedule a HIP build from a systemd user unit without exporting `ROCM_PATH`.** User units do not source `/etc/profile.d/rocm.sh`, and `hipcc` silently omits `-I/opt/rocm/include` without it; the build dies at `#include <hip/hip_runtime.h>` after the window has opened, wasting the night. A scheduled unit inherits none of the login-shell profile, so every toolchain variable the interactive build depends on must be exported in the runner itself.
- **Do NOT treat "the run is scheduled" as "the run will produce a receipt."** Session 24 armed a timer and handed off; the failure was silent until morning. Verify a scheduled long run past its first fragile stage — here the build — before the session ends.
- **The shell is zsh: `status` is a read-only variable.** `status=$?` fails with "read-only variable: status", and a `... | tee LOG; status=$?; printf ... >> LOG` wrapper then loses the wall-time line the receipt depends on. Use any other name.
- **Do NOT anchor a receipt to a commit hash.** This is now enforced by the harness, but the reasoning must survive: a receipt answers a content question, and a commit hash answers a history question. Read `harness_tree` / `deps_tree` / `wave_gpu_tree` and `binary_sha256` as the binding fields; `git_sha` is a human pointer only.
- **A tree hash names committed content, not the working tree.** It is only a valid identifier of the measured bytes when the matching dirty flag is `false`. Never publish a receipt whose closure flags are true.
- Carried forward, unresolved: the handoff-20 traps (a whole-tree pristine gate is wrong on a shared repo — scope it to the built closure; workers repeat the GNU-`time` trap, `/usr/bin/time` is not installed on this host).
- All unresolved traps in `handoff-19.md` and earlier remain in force — especially: the recorded gate run is the authority; a stale `claims.lock` wedges `jit gate evaluate` (diagnose via `.git/jit/locks/`, `jit recover`); no cargo-ci while anything else builds; the lead owns all `jit doc add`; never re-derive the 63-cell selection; `< /dev/null` on every `codex exec` dispatch.

## Open questions needing invoker input

- Question: resolve the duplicated provenance implementation in `crates/gf2-sim`?
  - Context: it carries the same commit-hash-as-provenance defect this session removed from the feasibility harness.
  - Options: fold into a shared module; fix in place as its own issue; leave and record a tracked exception.
  - Recommendation: its own issue — it touches the campaign emitter, which the freeze pins.
- Question: amend the four backlog issue descriptions that quote the superseded invariant wording?
  - Context: they will be worked in waves 10–13 and their criteria text enumerates "git revision".
  - Recommendation: amend them, since the criteria are read literally by both workers and the review gates.

## Reference artefacts

- Epic: `jit issue show b8206228`; freeze: `jit issue show 7a816262`; closed receipt task: `jit issue show 86185f88`
- Receipt: `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.{csv,log}`, addendum alongside
- Derivation record: `dev/benchmarks/permanent_campaign/probe-costs-de5f7414.csv`
- Provenance implementation: `dev/research/permanent-sampling-feas/src/env.rs`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json` (`session_25_state`)
