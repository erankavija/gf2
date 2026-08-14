# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 5

**Date:** 2026-08-14
**Session number:** 5
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (session 4). Their traps remain in force except where superseded below.

## Current state

- Epic: `b8206228` — state: backlog (children executing)
- Wave in progress: wave 1 of 13 (plan in `progress.json`), plus inserted prerequisite `e31b1918` (wave 0.9)
- Children summary: 44 done, 1 in_progress (`e31b1918`, claimed `agent:worker`), 26 backlog/ready, 0 rejected
- Active claims: `e31b1918` → `agent:worker` (implementation committed at `3933338a`; review and gates pending)
- Open escalations: none — the 2026-08-14 escalation was answered (wire-and-rerun; REQ-01 corpus-clause amendment; 047b62ed Background ordering fix) and executed
- Progress file: `progress.json` here (reflects the above)

## What just happened

- Morning checklist on the 2026-08-14 02:00 overnight run: exit 0, all steps completed for q=3,5,7. Raw evidence committed per issue (`e0954ea6`, `533b2532`, `5e46f4d1`), including the final kernel resource receipt + logs preserved from transient `target/` into `dev/studies/6c7fcb38/hip-resource-usage-20260813T193800Z-793245/`.
- Dispatched `047b62ed` receipts worker (opus): receipts v1 + `analysis.py` committed (`6eb364e9`). Its conformance table exposed REQ-01/04/14 as structurally unsatisfiable (prototype registry batch `dispatch()` still the skeleton stub; no issue owned the wiring), REQ-03/10 single-order isolates, REQ-11 equivalence stopping at n=20, and REQ-01's identical-corpus clause contradicting the harness's deliberate disjoint per-cell streams.
- Escalated; owner chose **wire-and-rerun**, approved the REQ-01 corpus-clause rewording on all three campaigns, and the 047b62ed Background zero-fast-path ordering correction. Amendments applied (`66f56d93`); `047b62ed` claim released; campaigns re-blocked.
- Created `e31b1918` "Prototype batch evaluators and full-order isolates in the campaign harness" (gates `code-review`, `permanent-sampling-feas-ci`, `permanent-wave-gpu-ci`; deps wired from all three campaigns; `025a1a17`).
- Dispatched opus worker on `e31b1918`; three API-529 failures (server overload); per owner directive switched to codex luna, which converged the worker's ~850-line WIP to completion host-side (all four command groups PASS, runner tests 10/10, tree left uncommitted).
- Lead device validation on the host GPU: all 4 device-gated prototype tests pass; harness `equivalence` run live on device — 74/74 completed comparisons `identical`, 0 mismatches, covering q=3 at every order incl. n=24/28 and q=5 through n=24 with the new prototype paths dispatching their own kernels. Killed after 36:56 elapsed stuck >12 min inside the q=5, n=28 cell (see Traps). Partial CSV committed as `e31b1918-equiv-host-check-partial.csv` here.
- Committed the implementation (`3933338a`) after review of the diff (scope-clean; runner `CAMPAIGN_REPO_ROOT` override follows the existing `CAMPAIGN_*` test-hook convention; trimmed one process-narration comment in `main.rs`).

## What to do next

- [ ] Resolve the equivalence overnight-budget defect on `e31b1918` before anything else: the `EQUIVALENCE_SIZES` counts (n=24: 32, n=28: 4) were derived from q=3 `cpu_scalar` probe costs, but slow backends (`cpu_ryser_generic`, F5/F7 scalar) dominate — the q=5,n=28 cell alone exceeded 12 minutes. Either bound slow backends' equivalence orders with honest unsupported/budget reasons, or shrink counts, then measure the true full-step duration on the host. This is REQ-04 feedback ("chosen against the overnight budget"), fix before gates.
- [ ] Run `dev/scripts/permanent-campaign-runner.sh prepare` then `smoke` on the committed revision (REQ-07). Smoke outputs land under each campaign study's `smoke/` dir and supersede the 2ddf2f4b-era smoke evidence linked on closed `2ddf2f4b` — `git rm` superseded evidence and re-point `jit doc` links in the same commit (standing rule).
- [ ] Evaluate gates on `e31b1918` (`code-review`, `permanent-sampling-feas-ci`, `permanent-wave-gpu-ci`), confirm each with `jit gate status` (false-pass trap), full lead review per `lead-review-protocol.md`, close.
- [ ] Re-arm the overnight runner (one-shot `systemd-run --user --on-calendar='<next day> 02:00'` per the runner header) on a pristine tree; verify timer state with `systemctl --user list-timers` from a login shell (no DBUS in the harness shell).
- [ ] Morning after: checklist per `handoff-2.md`; commit evidence per issue; supersede run `20260813T230032Z-1321576` raw evidence AND `dev/studies/047b62ed/receipts.md` v1 + `analysis.py` in the same commits that land the new evidence/receipts, re-pointing any links.
- [ ] Then drive the campaigns serially F_3 → F_5 → F_7 against the amended criteria; the F_3 receipts v1 structure and its `analysis.py` are a sound template.

## Traps — do not repeat these

- **Do NOT derive large-order equivalence budgets from q=3 `cpu_scalar` probe costs.** Measured: the q=5, n=28 equivalence cell ran >12 min without completing (36:56 total elapsed for what the derivation priced at ~seconds); `cpu_ryser_generic` and the F5/F7 scalar backends are orders slower than the q=3 packed scalar. Evidence: `e31b1918-equiv-host-check-partial.csv` ends at q=5,n=24; killed process had 100% CPU single-thread. Measure per-backend costs or bound slow backends before arming `measure`.
- **Do NOT read the campaign run `20260813T230032Z-1321576` gray-update `wave-gf3`/`fold-gf3` rows as prototype evidence.** They alias the shipped `gray_update_micro_kernel` (q-only kernel selection at that revision; spans agree to 0.02%). De-aliased at HEAD by `3933338a`; the committed CSVs still carry the rows.
- **Do NOT treat the overnight provenance's `tracked_worktree_dirty: true` as a failed pristine check.** It records mid-run state after the runner's own outputs exist; the gating is the runner's double refusal before and after the bench mutex. Documented in `receipts.md` §1.
- **Do NOT keep re-dispatching an opus worker through an API-529 storm, and never let two writers share the checkout.** Three consecutive 529s killed the same worker; `SendMessage` resume preserves context and the tree, but during a storm switch to codex luna — and `TaskStop` the Claude worker FIRST. Codex sandbox cannot execute on the GPU and must not commit: structure codex dispatches as converge-tree → host-side checks → leave uncommitted; the lead runs device validation and commits. This worked cleanly this session.
- **Do NOT leave the main tree dirty at end of session.** A recurring campaign timer may exist (unverifiable from the harness shell — no DBUS); `measure` refuses a non-pristine tree and the night is wasted.
- Session-3 and session-4 traps (breakdown.json never edited/resynced; gate-evaluate-then-status confirmation; cargo-ci lock/timeout; worker `git add -A`; superseded-evidence rule; runner one-finding-per-round history; equivalence-is-global truthfulness) remain in force.

## Open questions needing invoker input

None. (Owner directives this session: wire-and-rerun with criteria amendments as recorded in `progress.json` escalations; use suitable model tiers for subagents; use codex during the 529 storm.)

## Reference artefacts

- Epic: `jit issue show b8206228`; wave plan + escalation log: `progress.json` here
- e31b1918 implementation: commit `3933338a`; partial device validation: `e31b1918-equiv-host-check-partial.csv` here (74 identical, 0 mismatches)
- F_3 receipts v1 (template for the rerun): `dev/studies/047b62ed/receipts.md` + `analysis.py` (`6eb364e9`) — superseded once the rerun lands
- Campaign runner: `dev/scripts/permanent-campaign-runner.sh` (+ `.test.sh`, now 10 tests); harness: `dev/research/permanent-sampling-feas/`; prototypes: `dev/research/permanent_wave_gpu/`
- Overnight evidence (to be superseded by the rerun): `dev/studies/{047b62ed,91605d4d,6c7fcb38}/permanent-campaign-20260813T230032Z-1321576*`
