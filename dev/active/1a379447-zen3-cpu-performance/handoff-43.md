# Handoff — Zen 3 CPU performance (1a379447) — session 50

**Date:** 2026-11-01
**Session number:** 50
**Prior handoffs:** [handoff-42.md](handoff-42.md) and its ordered predecessor chain. Session 49 wrote none; `session_scope.session_50` in [progress.json](progress.json) records what was reconstructed from its gate runs and the window log.

## Current state

- Epic `1a379447`: wave 7 of 7. 78f0aafd is done. 31727c57 and 1362381c stay open.
- 31727c57: code-review F1 remains open until the pilot-cell batch-fill record lands. cargo-ci passed; research-review has not run.
- Window: `gf2-bench-window-20261101.timer`, 2026-11-01 04:00 Europe/Helsinki, invoker approved. The one new job is `decoder/batch-fill/pilot-cells/window.sh`, committed in worktree `agent-31727c57`.
- 1362381c: not started. Its dependency edges omit WP-3..WP-5, so `jit` shows it `ready`; it still waits on 31727c57.
- Open escalations: none.

## What just happened

- Invoker ruling: 31727c57 REQ-02 amended. The pilot covers the ten-cell matrix and confirmation covers the selected and holdout cells within the REQ-03 bound, with a confirmed cell at one core, six and twelve physical cores and 24 logical CPUs.
- Collected the 2026-10-30 window: `da4e84963` (batch fill on the confirmation cells), `b179da882` (FI-3 exact-arm profiles).
- Fixed on main: `cda95fd73` (shared outcome reader), `a10c139a9` (scaling baselines matched by batch), `655649403` (ratios state the route change they include).

## What to do next

- [ ] After the window, read `.agents/bench-window/window.log`, the job's `.out` and `decoder/batch-fill/pilot-cells/31727c57-decoder-batch-fill-pilot-cells/execution.log`. Verify a terminal `complete` and nonzero batch fill.
- [ ] Commit the stage on `worktree-agent-31727c57` with its nested `Cargo.lock` force-added, regenerate `31727c57/outcome.md`, merge, link, then run cargo-ci, code-review and research-review.
- [ ] Start 1362381c with a Claude worker on the strongest allowed tier below Fable. The last two dispatches were Codex (31727c57) and Claude (78f0aafd).
- [ ] In the 1362381c gap register, give the FI-3 twelve-core losses a tracked follow-up whose next experiment varies the lane workspace at a fixed worker count: the bandwidth explanation in `6230e0b6/findings.md` R-01 is an interpretation. The one-frame losses cite the preserved 07ca8585 and f63a2464 experiments.

## Traps — do not repeat these

- **Do NOT conclude a merge is clean from a check run before it.** The `78f0aafd` merge broke every outcome projection on main because a shared reader scans all committed `profile-summary.json` files. Rerun `make_outcome.py --check` after each merge.
- **Do NOT read the decoder profile routes from `wave_routes`.** The sessions record them in `selected_path`. A grep for the wrong key produced a false "no route recorded" statement that research-review caught.
- **Do NOT call the per-worker ratios thread scaling.** Eight-frame one-worker cases route `Lane(Avx2)` and multicore cases route `SingleFrame`, so each ratio includes that route change.
- **Do NOT treat `31727c57/test_preparation.py` failures on the main checkout as defects without checking.** `test_core_build_bytes` and the batch-fill plan test need built arms that only `agent-31727c57` holds. They are not CI steps.
- **`systemctl --user` and `systemd-run --user` need `XDG_RUNTIME_DIR=/run/user/$(id -u)` from this shell.** Without it they fail or print nothing, so an empty timer list proves nothing.
- **zsh aborts a command line on an unmatched glob and treats a word starting with `=` as a command lookup.** Check that a verification really ran before relying on it.
- **Run no gate during a benchmark window, and change no tracked file during a gate.** Either one disturbs the other: a measurement, or the checker's optimistic capture.
- Earlier unresolved traps in handoff-42 and its predecessors remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- [31727c57 outcome](31727c57/outcome.md), [78f0aafd report](78f0aafd/report.md), [1358d6ab outcome](1358d6ab/outcome.md)
- [Scorecard design](1362381c/design.md), [queue](bench-window/queue.tsv)
