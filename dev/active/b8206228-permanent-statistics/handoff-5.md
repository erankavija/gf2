# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 7

**Date:** 2026-08-15
**Session number:** 7
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6). Their traps remain in force except where superseded below.

## Current state

- Epic: `b8206228` — state: backlog (children executing); wave 1 of 13 nearly complete
- Closed this session: `047b62ed` (F_3 receipts, 2 rework rounds, both gates passed), `91605d4d` (F_5 receipts, zero rework, both gates first-round), `a2c0db52` (two mis-rounded feasibility-table cells + recorded check + F_3 receipts §11.1 correction)
- Open, near close: `6c7fcb38` (F_7) — receipts committed at `093b846e` (16/19 MET; §12 REQ-16 partial, §13 REQ-17 unmet, both written to current evidence and designed to be replaced); claimed `agent:worker`. **Blocked on the lead's paired profiled evidence run** (see below). ALSO: the worker left an unexplained uncommitted 94+/52− delta on `dev/studies/6c7fcb38/{receipts.md,analysis.py}` after its commit — ask `worker-6c7fcb38` (or diff it) before doing anything; do not commit it blind and do not discard it blind.
- Open: `4fdd781a` (stopping-rule daggers in the prior study) — round-1 fix committed (`7a0af9ab`, codex luna), but doc-review FAILED with 3 findings (run record at `294e1fc9`): F1 the envelope table (feasibility-study.md:775) republishes the q=5,n=28 rayon rate 0.648 unmarked; F2 the q=3,n=28 GPU M=1024 rate 19.27 feeds a projection (line 1573) without the qualification; F3 the check scans §4.4 only and misses downstream renderings. Rework (Luna): dagger EVERY rendering of the two nonconforming rates across the study, and widen `verify-rendered-stopping-rule.py` to scan every rendering (or the whole document for the two rates), red-demonstrated. Then re-gate, confirm status, review, close. Rework count set to 1.
- Overnight evidence: run `20260814T230032Z-2085453` landed per issue (`6f5a23ac`, `93e5760a`, `55eabd95`), superseding the 20260813 run + v1 receipts per the standing rule. Shared equivalence 102/102 identical.
- Criterion amendments this session (all owner-approved, logged in `progress.json` escalations): REQ-04, REQ-03, REQ-07 on all three campaigns; REQ-16/REQ-17 on `6c7fcb38` (paired profiled run design).
- New invariants registered (`b94b1e5d`): campaign-resumability, behavioral-evidence-validity, runtime-observed-provenance; retrofit tasks `201caa23`, `0ba493e1` under tech-debt umbrella `86b9c719`, plus harness bugs `3ea21d74` (CSV quoting), `79c5ace7` (host_submission_s dual semantics).
- Owner directives this session: use codex Luna for trivial fixes; handoff once wave 1 completes.

## The paired profiled evidence run (the one blocker for wave 1)

Owner-approved design (amended REQ-16/17 on `6c7fcb38`, commit `afa665d9`): per-kernel durations and achieved-occupancy counters from rocprofv3 over the SAME hash-pinned binaries and cells as the timing run, committed as its own artifact under `dev/studies/6c7fcb38/`, kept separate from the timing run (counter collection perturbs timing — state this in the provenance). Smoke-validated recipe (lead-run, device work) — EXECUTABLE at `dev/active/b8206228-permanent-statistics/profiled-run.sh` (verifies the manifest hash, runs all six passes under the bench lock, writes `dev/studies/6c7fcb38/profiled-<UTC>/` with run.log; on success: write the provenance file per step 5, commit, `jit doc add`, message worker-6c7fcb38 per step 6). Prose recipe:

1. Verify the binary: `sha256sum target/permanent-campaign/permanent-sampling-feas-hip/release/permanent_sampling_feas` must equal the manifest value in `target/permanent-campaign/manifest-v1.txt` (edb03650…; matched this session).
2. Kernel-trace pass (per cell set):
   `dev/scripts/ccx1-bench-flock.sh --full-host /opt/rocm/bin/rocprofv3 --kernel-trace --stats -f csv -d <outdir> -o <name> -- target/permanent-campaign/permanent-sampling-feas-hip/release/permanent_sampling_feas grid --out <outdir>/grid-<cell>.csv --only q=7,n=20 --execution-id 7002 --skip-machine-warmup`
   — smoke at `q=7,n=12,backend=f7-three-plane-permanent` produced clean per-kernel stats separating `prepare_three_plane_columns` from `wave_gf7_three_plane_kernel` (23.1 µs avg walk vs its own prep line), 61 666 dispatches.
3. Counter pass: same invocation shape with `--pmc SQ_WAVES OccupancyPercent MeanOccupancyPerCU` instead of `--kernel-trace --stats` (all three confirmed available on gfx1030 via `rocprofv3-avail list`; if a single pass fails, drop to `OccupancyPercent` alone).
4. Cell set: `q=7,n=20` all backends (declared operating point; covers every executing F_7 device kernel incl. gpu_hip), plus `q=7,n=16` and `q=7,n=24` with `backend=f7-three-plane-permanent` (REQ-18 orders, prep/walk scaling). Reusing `--execution-id 7002` reproduces the committed run's stream addresses (same cells literally).
5. Commit under `dev/studies/6c7fcb38/profiled-<UTC>/` with a provenance file: commands verbatim, binary hash + manifest cite, rocprofv3 1.1.0, ROCm/driver, host, git revision, bench-lock note, and the timing/profiling separation rationale. `jit doc add` it.
6. Message `worker-6c7fcb38` (SendMessage; it resumes with full context) with the paths; it rewrites §12/§13 against the artifact, then run both gates, six-tier review, close. Wave 1 complete → write the completion note in progress.json and THEN the next handoff per the owner's standing request.

## What to do next (ordered)

- [ ] Resolve the uncommitted `6c7fcb38` study delta with the worker (ask what it is; likely post-commit §12/§13 or citation edits — if legitimate, have the worker commit it itself with staged-diff discipline).
- [ ] Re-run + confirm `4fdd781a` doc-review; review; close.
- [ ] Execute the profiled run per the recipe above; commit; hand paths to `worker-6c7fcb38`; worker finishes §12/§13; gates; review; close `6c7fcb38`.
- [ ] Post-wave: `jit graph downstream` on the three campaigns; `jit validate`; advance `current_wave` to 2 (`41c3d91d`, `a9284086`); commit progress.
- [ ] Wave 2 note: `a9284086` (runtime qualification) is stated to consume the F_7 occupancy/bit-plane figures — link the profiled artifact when dispatching it.

## Traps — do not repeat these

- **The lead's `git add <paths> && git commit` commits whatever a same-checkout worker has STAGED.** One worker's staged receipts got swept into an unrelated lead commit (repaired by splitting unpushed HEAD: `2a5cda47`+`62c061c7`). Run `git diff --cached --stat` before every lead commit while a worker is active; never rely on the worker's own staging discipline to protect the lead's commits.
- **Do not label a criterion "satisfied in part" in a conformance table.** Reviewers read it as UNMET. The correct move when evidence structurally cannot satisfy a criterion is a criterion-collision escalation BEFORE gating (this session: REQ-04, REQ-03, REQ-07, REQ-16, REQ-17 — all resolved by owner-approved amendment; the pattern is now stable wording on all three campaigns).
- **An aspirational/hard criterion naming evidence "during the measured run" cannot be satisfied post-hoc by writing, and its escape clause must be checked against facts** — REQ-17's "profiler unavailable" was factually false (rocprof installed). Writing the permitted-but-false reason would have been a falsification. Audit evidence availability BEFORE the campaign's receipts are authored (the F_7 worker's evidence-audit-first instruction caught this early; keep that instruction for future evidence-bound issues).
- **Trailing zeros are significant in this project's published integer tables.** The F_3 worker's checker excused `30 210` (raw 30 209.4610) by reading 4 sig figs; the table's own convention (other 5-digit cells reproduce exactly) makes it a second wrong cell. When checking published-vs-artifact rounding, derive the convention from the table's own conforming cells, never per-cell.
- **The Ryser work-model projection lands HIGH on probe-calibrated prototype chains** (+474 % to +1685 % observed across F_5/F_7) and LOW only on fixed-batch GPU chains. F_5 has a censored cell OWNED by such a chain (`f5-byte-control` n=28): its projection is not conservative and must never be cited as a bound. F_7's censored cells are all fixed-batch (safe).
- **The stopping-rule cap bounds when a further repetition may START, not the cell's total** (`protocol.rs:178-191`): q=3 n=28 gpu_hip M=256 at 5 reps/150 s CONFORMS. A total-seconds check wrongly flags it; the two genuine violations are the daggered cells. `verify-rendered-stopping-rule.py` encodes the correct rule.
- **codex Luna cannot commit or touch `.jit`** — dispatch as converge-tree → lead reviews diff → lead commits (worked cleanly on `4fdd781a`). GNU time is still absent; harness CSVs still need the seed_root-anchored split (bug `3ea21d74`).
- All unresolved session-3..6 traps remain in force: breakdown.json never edited/resynced; gate-evaluate-then-status confirmation (a background `jit gate evaluate` that never persisted a run record = no evidence — `4fdd781a` this session); cargo-ci lock/timeout; worker `git add -A`; superseded-evidence rule; equivalence-is-global truthfulness; `tracked_worktree_dirty: true` mid-run; no two writers on one checkout; main stays pristine if the overnight timer is re-armed (it was NOT re-armed this session — no campaign runs tonight).

## Open questions needing invoker input

None. All five criterion collisions were escalated and resolved this session (decisions in `progress.json` escalations).

## Reference artefacts

- F_3 receipts: `dev/studies/047b62ed/receipts.md` + `analysis.py` (gates passed at `5222f61c`); F_5: `dev/studies/91605d4d/` (passed at `1de2b950`); F_7: `dev/studies/6c7fcb38/` at `093b846e` + uncommitted delta (see above)
- Published-table checks: `dev/studies/b488f02c/verify-published-table.py`, `verify-rendered-stopping-rule.py`
- Profiling smoke output (scratch, session-local): kernel-stats format validated; recipe above is self-contained
- Escalation log, pitfalls, rework counts: `progress.json` here
