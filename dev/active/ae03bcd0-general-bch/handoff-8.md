# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 11

**Date:** 2026-10-01T11:40+00:00
**Session number:** 11
**Prior handoffs:** handoff.md through handoff-7.md — their Traps sections remain in force unless resolved below.

## Current state

- Epic `ae03bcd0` — state: backlog (container); claimed by agent:jit-execution-lead.
- Wave in progress: wave 14 (`current_wave` = 14). Every child except `fd9d5416` is done (direct deps 5/6).
- Open: `fd9d5416` (Committed performance receipts), claimed by agent:worker. Preparation merged: `dev/active/fd9d5416/receipt-protocol.md` (linked), `render_receipt.py`, `tests/test_render_receipt.py` (6/6 on a synthetic fixture). Gates doc-review, research-review pending.
- Open escalations: none.
- Progress file: `progress.json` here (R-55..R-59, session_notes_s11, surfaced_pitfalls).
- **Queued measurement:** the bench window (timer `gf2-bench-window-20261002`, 2026-10-01T23:00Z) runs `dbd8787d` (epic 6dc81018, ~3 h) and then `d1b4f85e`'s runner from `.agents/worktrees/agent-d1b4f85e-run` (detached at `eb5575495`), writing `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2` in that worktree (~105 min). Host idle until ~04:30Z.

## What just happened

- Gate re-runs closed b1bd75ca, 3f7edef1. Code-review reworks closed 5ee83cd3, 97410c80 (generic `LayoutView`; 113ae672 delivered and closed), e1e0e7ff (production DVB-T2 and `ExtendedBchComponent` in the roster).
- Wave 12 (591a1c5e, ef8ff9c4, 0c21cb1e, 227ac5c8) and 94597a51 (Lean O-5) closed; 591a1c5e renamed non-comparable bench IDs (mapping in `dev/active/591a1c5e/smoke-run.md`).
- 4a2baa12 deleted the superseded surface (`BchCode::new/dvb_t2`, legacy encoder/decoder, `ExtendedBchCode`).
- f759d724 closed under invoker DEC-01 (archives out of scope). d1b4f85e closed (W1/W2 cells, dispatch records with digests, window runner). 4ad869d6 closed (bch module guide, two runnable examples, `tests/bch_examples.rs`).
- Invoker deleted-as-obsolete: all ROADMAP files, Copilot instructions and agent config (R-59). Filed cbd1eb28 (user-layout decode) outside the epic.

## What to do next

- [ ] After the window: confirm `window.log` shows the d1b4f85e job exit 0 and the runner's post-check passed; copy `dev/bench_results/fd9d5416/2026-10-02-d1b4f85e-w1w2` from the run worktree onto main.
- [ ] Dispatch fd9d5416 (Opus) to render and commit the receipt with `render_receipt.py` under R-57/R-58, link it, then run doc-review and research-review. If Tier A/B shows a DVB-T2 W1 regression, a33fda32 becomes a prerequisite (REQ-13). If research-review rejects the cross-harness Tier B basis, take option B from the protocol note (legacy encoder re-measured at 74871c6b in its own window).
- [ ] Epic close: map success criteria, reconcile `surfaced_pitfalls` against criteria, `jit gate evaluate-all ae03bcd0`, completion report, transition, archive.
- [ ] Reclaim `agent-d1b4f85e-run` after copying its output.

## Traps — do not repeat these

- **Seeded worktree targets carry a stale `gf2-kernels-simd` artifact.** Workers hit "could not find shift_funnel in gf2_kernels_simd"; `cargo clean -p gf2-kernels-simd --profile <p>` fixes it. Tell every worker up front.
- **A worker's `pkill -f scripts/cargo-ci.sh` kills every worker's CI on the host.** State "never pkill by pattern" in every dispatch prompt.
- **`evaluate-all` stops at the first failing gate**, so a missing lake-build run surfaced as a code-review finding (94597a51). Run the executable gate first on Lean issues.
- **cargo-ci builds all features**, so a default-feature-only warning (a hip-gated test import) passes CI; check `cargo check -p <crate> --tests` without features when a change gates code by feature.
- **Commit with explicit paths.** Another session shares main; `git add .jit`/`commit -a` would sweep its files.
- **Merge into main only between gate runs**, so a running cargo-ci or AI review does not see a moving tree.
- **Kept bench IDs must measure the same workload** (591a1c5e F1); renamed cells need a committed ID mapping and smoke evidence.
- Prior traps remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Receipt protocol: `dev/active/fd9d5416/receipt-protocol.md`; renderer `dev/active/fd9d5416/render_receipt.py`.
- Runner: `dev/active/d1b4f85e/run.sh`; smoke evidence `dev/active/d1b4f85e/smoke-run.md`; contract amendments in `dev/active/4e732b56/workload-selection.md`.
- Baselines: `dev/bench_results/88ca7d2f/`, survey `dev/active/4e732b56/findings.md`.
- Queue: `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv`.
