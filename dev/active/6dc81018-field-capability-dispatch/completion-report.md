## Epic Complete: Reconcile field capability traits and profile-driven kernel dispatch (6dc81018)

**Started:** 2026-08-18
**Completed:** 2026-10-02
**Assignee:** agent:jit-execution-lead

### Summary

Field capabilities are classified with one canonical abstraction each, the
packed lane abstraction has a single owner, and kernel and backend selection
reads a versioned, committed tuning profile produced by an explicit,
lock-serialized calibration action. A pinned non-regression receipt gates the
cutover, and loop-free generic operations extract cleanly through
Charon/Aeneas on Rust 1.95.

### Metrics

| Metric | Value |
|---|---|
| Direct children completed | 13 / 13 |
| Waves executed | 28 (plus fractional sub-waves), 17 sessions |
| Escalations | Owner decisions DEC-A (REQ-05 scope), host admission option C, dbd8787d REQ-02 amendment, session 17 protocol lightening and busy-host override |
| Issues created during execution | Selector-migration, calibration and regression-resolution tasks recorded in `progress.json` |

### Success Criteria

- [x] REQ-01 inventory and cutovers — `classification.md` (265997f9); cutovers 7f818151, a6636671, the selector migration under 7d824b2f, 19424a0f, ef18c60b.
- [x] REQ-02 packed-capability placement — 7f818151 retires the parallel declarations; the gf2-algebra boundary is recorded in `classification.md`.
- [x] REQ-03 profile-driven selection — format 220cab0b, loader f35daec0, calibration action 5ecc9bf8, selector cutovers (0d819b62 onward, 19424a0f, ef18c60b), extent sweep a83583e0, seam calibration dbd8787d with declaration locator 8bd873f7.
- [x] REQ-04 non-regression gate — e8fe47f5 pins the set and tolerance; 278acf3a baseline; 50b47eae post-cutover receipt.
- [x] REQ-05 loop-free extraction — 34d85cb9 spike (F4 falsification preserved, DEC-A), 1ac74567 proof-obligation factoring, e6ea0dde clean-exit extraction; seam 7d7c647c and 06ba0418.

### Session 17 log

- Campaign `gf2-dbd8787d-20261001t230000z-2601601` measured 756 cells
  (4,536 accepted results); the finalize-time validator rejected it on a
  redundant key list lacking `imported_owners`.
- Fix `cb5708396`: the validator reads identity sources at the producing
  revision, records its own identity informationally, and accepts a terminal
  failed only by its verdict (owner-approved protocol lightening).
- Published `f6788d9cb`; cutover `8cc7f42ad`: `triangular.base_case_max_dim`
  12, `ple.scalar_base_max_cols` 24, `gemm.winograd_min_dim` 128 retained;
  baked `SIMD_MIN_WORDS` 4 → 8 on a non-monotone curve, with the contradiction
  recorded in 7d824b2f A12 and 7d7c647c A3.
- 8bd873f7 (fa787f85) taken in at owner direction to clear the
  `no-dev-path-coupling` code-review finding; validator verdicts identical
  before and after (`dev/active/fa787f85-documentation-overhaul/tuning-extent-verdict-baseline.md`).
- `BUSY_HOST_OVERRIDE` (`3e3b91acd`) selects the `ci-busy` nextest profile on
  contended hosts (owner decision).
- ef18c60b wired into the container to resolve holistic-review divergence.

### Escalations

Owner decisions are recorded in the handoff chain (`handoff.md` through
`handoff-17.md`). Session 17: publication permission, protocol lightening,
cross-epic 8bd873f7, busy-host override, and the out-of-scope repo-validate
failure (seven isolated roadmap issues), resolved by waiting for their owner.

### Holistic Quality Notes

- Provenance pinning of the checker inside the measurement identity caused a
  complete campaign to fail; checker identity is now informational, per
  `behavioral-evidence-validity`.
- The auto-mode classifier blocks agents from publishing campaign evidence;
  publication needs the invoker or an allow rule.
