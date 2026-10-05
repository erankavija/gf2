# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 24

**Date:** 2026-08-30
**Session number:** 24
**Prior handoffs:** `handoff.md` (session 3) through `handoff-19.md` (session 23). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: in_progress (lead-claimed) — wave 8 of 13; freeze issue `7a816262` is the wave's last open item.
- Children summary at wave 8: `1947125e`, `ec22205e` done; `7a816262` in_progress (rework counter 2 = MAX per its note — any gate/lead-review failure escalates, never rework).
- `6dfb1b5e` (README receipt-identity) **done** this session: cargo-ci + code-review (0 findings) passed at `b3c06932`/`4e847a3f`; lead six-tier review PASS.
- `86185f88` (cell-exhaustive equivalence receipt) **in_progress**: harness extension merged at `4e847a3f`, reviewed by the lead but NOT yet gated. The measurement run is armed as a transient systemd user timer `gf2-equivalence-s24.timer` firing **2026-08-30 02:00:30 EEST**, executing `~/.local/state/gf2-session24/run-equivalence.sh` (session-independent). Reservation ≈ 8312 s, so expected finish ≈ 04:20.
- Active claims: `7a816262` → agent:jit-execution-lead; `86185f88` → agent:codex-sol (worker done; claim held for the run/gates).
- Open escalations: none.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics/` (see `session_24_state`).

## What just happened

- Owner confirmed the RAM+disk hardware upgrade complete; compute unblocked.
- Dispatched `6dfb1b5e` (Luna xhigh) and `86185f88` (Sol xhigh) concurrently on the main checkout with disjoint write sets and `< /dev/null` on both `codex exec` invocations; both converged cleanly.
- `6dfb1b5e`: README-side fix (driver untouched — the freeze pins emitter digest `2d6edcd9`). New test `frozen_manifest_checkpoint_identity_matches_integrity_sidecar` walks the two-identity audit path against the committed frozen campaign; lead verified `sha256sum -c` reproduces. Committed `b3c06932`, gates passed, closed at `28b8da3b`.
- `86185f88`: Sol delivered `campaign_selection.rs` (embedded SHA-256-pinned `backend-selection-v1.md` as the parsing authority; 63 cells / 124 nominations; per-cell budget extending `de5f7414` with conservative same-or-higher-order probe substitution and a recorded 2-matrix floor), `check_selected` equivalence rows with RNG/seed/stream provenance, `--q`/`--n` filters, and the addendum draft with `TO FILL AFTER RUN:` placeholders. Lead verified: authority SHA-256, three prior-evidence SHA-256s, probe constants against the `de5f7414`-tree CSVs, q3 low-order nomination coverage, fmt/clippy/tests, release+hip build (binary `9f7905ad…`). Committed `4e847a3f`.
- Lead corrected Sol's runbook before commit: `/usr/bin/time -p` → shell `date` wall-time wrapper (GNU time absent on host).
- Owner caught two defects in the lead's first armed run task: (1) whole-tree pristine gate is wrong on a shared repo — replaced by a built-closure-scoped gate; (2) the system clock was ~36 min 40 s AHEAD with NTP inactive — a fixed-duration sleep to "02:00" would have started GPU rows ~01:23 real time, violating `86185f88` REQ-04. Owner synced the clock (verified offset −8 ms); the run was re-armed as a wall-clock systemd timer.

## What to do next

- [ ] **Morning: read `~/.local/state/gf2-session24/RESULT.txt`** (runner transcript: `runner.log` beside it; run log + CSV under `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.{csv,log}`). Expected: `rows=124 q3=50 q5=41 q7=33 mismatches=0 unexecuted=0`. Clean up the transient unit if still listed (`systemctl --user reset-failed gf2-equivalence-s24.service` if needed; env: `XDG_RUNTIME_DIR=/run/user/1000`, `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`).
- [ ] If the receipt is clean: commit CSV+log, fill every `TO FILL AFTER RUN:` in `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence-addendum.md`, and update its runbook to the executed closure-scoped procedure (the committed draft still shows the over-broad whole-tree `test -z "$(git status --porcelain)"` line). A mismatch or unexecuted nomination is falsifying evidence: retain it, do not re-run, escalate.
- [ ] `jit doc add 86185f88` for the receipt CSV, log, and addendum; evaluate `code-review` + `doc-review` on `86185f88`; lead six-tier review; close.
- [ ] Then evaluate `research-review` AND `doc-review` on `7a816262` (both must re-run at final content; research-review F3 closes via the receipt). Lead six-tier review; close the freeze. Rework is at MAX — any failure escalates.
- [ ] Then wave 9: dispatch `3f664839` (design; native Opus per owner directive) and `73317b2e` (implementation; Sol xhigh; coordinator lives outside the frozen emitter process — see handoff-19).

## Traps — do not repeat these

- **Do NOT gate a shared-repo run on a whole-tree pristine check.** `git status --porcelain` conflates "receipt provenance is reproducible" with "nobody in gf2 has uncommitted work"; another user's unrelated edits would abort the run while a running build (the actual hazard) passes it. Scope the gate to the built source closure — `dev/research/permanent-sampling-feas`, `dev/research/permanent_wave_gpu`, `crates/` — and leave exclusivity to `ccx1-bench-flock.sh --full-host` plus the owner's window. The harness itself records `git_dirty` as informational and `harness_dirty`/`deps_dirty` as the provenance flags (`env.rs:49-68`).
- **Do NOT schedule a window-gated run without verifying clock sync.** This host's clock was 36 min 40 s ahead with `NTP service: inactive` (sntp offset −2200.48 s); a fixed `sleep $((target-now))` anchored to it would have launched GPU work ~37 min before the real 02:00 window (REQ-04 violation). Verify with `sntp <pool-host>` (read-only) first; schedule with wall-clock `OnCalendar` systemd timers, which also survive session end and later clock adjustments.
- **`env.rs` provenance flags do not cover the sibling path-dep.** `harness_dirty` pins the crate dir and `deps_dirty` pins `crates/`, but `dev/research/permanent_wave_gpu` is covered by neither (`env.rs:98-102`). Include it explicitly in any closure gate; a fix belongs in a follow-up, not a silent edit.
- **Workers repeat the GNU-time trap.** Sol's runbook used `/usr/bin/time -p`; GNU time is not installed (handoff-4). Sweep worker runbooks for it before commit.
- The handoff-19 `< /dev/null` codex-stdin trap held and is confirmed effective — both dispatches started cleanly with it.
- All unresolved traps in `handoff-19.md` and earlier remain in force — especially: recorded gate run is the authority; stale `claims.lock` wedges `jit gate evaluate` (diagnose via `.git/jit/locks/`, `jit recover`); no cargo-ci while anything else builds; no `git add`/`git commit` on main while a main-checkout worker runs; lead owns all `jit doc add`; never re-derive the 63-cell selection.

## Open questions needing invoker input

None. The owner's standing 02:00 window admission rule is satisfied by the timer; hardware is confirmed done.

## Reference artefacts

- Epic: `jit issue show b8206228`; freeze: `jit issue show 7a816262`; receipt task: `jit issue show 86185f88`
- Armed run: `systemctl --user list-timers gf2-equivalence-s24.timer`; runner + results: `~/.local/state/gf2-session24/{run-equivalence.sh,runner.log,RESULT.txt}`
- Harness extension: `dev/research/permanent-sampling-feas/src/campaign_selection.rs` (authority parsing + budgets), commit `4e847a3f`
- Addendum draft: `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence-addendum.md`
- 6dfb1b5e evidence: `crates/gf2-sim/src/permanent_campaign/driver.rs:738` test; `dev/simulation_results/permanent-zero-fraction/README.md` §manifest identities
- Progress: `dev/active/b8206228-permanent-statistics/progress.json` (`session_24_state`)
