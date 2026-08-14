# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 6

**Date:** 2026-08-14
**Session number:** 6
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (session 4), `handoff-3.md` (session 5). Their traps remain in force except where superseded below.

## Current state

- Epic: `b8206228` — state: backlog (children executing)
- Wave in progress: wave 1 of 13 (plan in `progress.json`)
- Children summary: 45 done (incl. `e31b1918`, closed this session), 26 backlog/ready, 0 rejected
- Active claims: none for the lead's account; the three campaigns (`047b62ed`, `91605d4d`, `6c7fcb38`) are available and waiting on tonight's runner output
- Open escalations: none
- Overnight timer: armed for 2026-08-15 02:00 (see below); manifest pinned at the session's final revision
- Progress file: `progress.json` here (reflects the above)

## What just happened

- Closed `e31b1918` (prototype batch evaluators and full-order isolates). Sequence:
  1. Fixed the equivalence overnight-budget defect (`de5f7414`, implemented by codex luna to the lead's spec): each (q, n) equivalence cell now halves from its per-order ceiling, flooring at 2, until the summed committed `probe_matrix_s` costs of its measured CPU backends fit a 240 s per-cell budget at grid-receipt speeds. Reproduces every previously validated count; only q=5, n=28 halves (4 → 2; its three CPU backends sum to 68.354 s/matrix). `usage.txt` and the emitted CSV notes rewritten in the same commit.
  2. Full-order host equivalence validation at the final revision (`0c0f525f`): 234 rows, q ∈ {3,5,7} × n ∈ {8..28}, **0 mismatches**, prototype candidates dispatching their own kernels, wall clock **5690 s** at the powersave governor. Linked on the issue with the pre-fix partial CSV as the defect record.
  3. Runner `prepare` + `smoke` (`7174718d`): run 20260814T171042Z-1587314 completes every step for all three fields; superseded the 2ddf2f4b-era smoke evidence (`git rm` + link re-point in the same commit, per the standing rule).
  4. Gates: `permanent-sampling-feas-ci`, `permanent-wave-gpu-ci`, `code-review` all passed with persisted run records (confirmed per the false-pass trap). Lead review PASS across all tiers; runner stub tests 10/10 at the final revision.
- Budget sanity for tonight: last night's whole campaign took ~51 min (02:00→02:51 EEST); the extended equivalence adds ~85 min, so the night projects to ~2.3 h. Fits.

## What to do next

- [ ] Morning checklist per `handoff-2.md`: `systemctl --user status gf2-permanent-campaign.service`, read `target/permanent-campaign/systemd-measure.log` and the three `*-run-summary.txt`, then commit study outputs per issue.
- [ ] In the same commits that land the new evidence: supersede run `20260813T230032Z-1321576` raw evidence AND `dev/studies/047b62ed/receipts.md` v1 + `analysis.py` (`6eb364e9`), re-pointing links (standing rule; the v1 receipts structure and `analysis.py` are a sound template for the rerun).
- [ ] Drive the campaigns serially F_3 → F_5 → F_7 against the amended criteria (REQ-01 corpus clause: identical sampler on preregistered disjoint streams for timing; literally identical corpus for equivalence).
- [ ] Wave 1 completes with the three campaigns; `41c3d91d` (wave 2) stays undelivered until then per wave discipline.

## Traps — do not repeat these

- **Superseded (session-5 trap 1):** the equivalence budget defect is fixed at `de5f7414` and validated end-to-end; the general lesson stands — never derive a per-cell budget from one field's fastest backend; sum the participating backends' measured committed costs.
- **Do NOT expect `/usr/bin/time` on this host** — GNU time is not installed; wrap timing with shell `date` arithmetic (a background run silently produced nothing but the error once).
- **The full equivalence step now costs ~95 min at the powersave governor.** Do not run cargo builds, tests, or gates while it runs (contention trap), and do not "optimize" it back down without a per-backend cost derivation; its cost is the price of full-order coverage.
- **`verify_manifest` pins binary hashes only; `source_revision` is informational.** Docs-only commits after `prepare` do not break `measure`, but re-run `prepare` after the session's last commit anyway so the manifest's recorded revision matches HEAD (this session did).
- All unresolved session-3/4/5 traps remain in force: breakdown.json never edited/resynced; gate-evaluate-then-status confirmation; cargo-ci lock/timeout; worker `git add -A`; superseded-evidence rule; gray-update rows of run 20260813T230032Z-1321576 never cited as prototype evidence; `tracked_worktree_dirty: true` in overnight provenance is mid-run state, not a failed pristine check; no two writers on one checkout; codex converge-tree → host checks → lead commits; main stays pristine overnight.

## Open questions needing invoker input

None. (Session-6 decisions were routine implementation choices within the amended criteria: the 240 s per-cell budget rule and the q=5,n=28 count of 2, both recorded in the committed code and evidence.)

## Reference artefacts

- e31b1918 closure: commits `de5f7414`, `0c0f525f`, `089d268e`, `7174718d`; validation CSV `e31b1918-equiv-host-full.csv` here
- Campaign runner: `dev/scripts/permanent-campaign-runner.sh` (+ `.test.sh`, 10 tests); smoke run 20260814T171042Z-1587314 under each study's `smoke/`
- Overnight evidence to supersede tomorrow: `dev/studies/{047b62ed,91605d4d,6c7fcb38}/permanent-campaign-20260813T230032Z-1321576*` and `dev/studies/047b62ed/receipts.md` + `analysis.py`
