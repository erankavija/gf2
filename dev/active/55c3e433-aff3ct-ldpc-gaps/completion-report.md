## Epic Complete: Close the integrated LDPC gaps against AFF3CT (55c3e433)

**Started:** 2026-11-01
**Completed:** 2026-11-08
**Assignee:** agent:jit-execution-lead

### Summary

The QC-aware intra-frame route for NR BG1 Z=384 entered production through canonical tuning and dispatch after a qualifying adoption confirmation. Each of the four losing cells closes as a preserved negative or inconclusive experiment against AFF3CT, because no comparator confirmation ran. The [closing record](closing-record.md) holds the per-cell results.

### Metrics

| Metric | Value |
|---|---|
| Children completed | 9 / 9 |
| Waves executed | 7 (1, 2b, 2, 3, 4, 5, 6) |
| Rework cycles | 12 (across all issues, including one epic doc-review round) |
| Escalations | 14 |
| Sub-agent dispatches | 23 (including reworks) |
| Issues created during execution | 1 |

### Success Criteria

- [x] REQ-01 NR pilot with per-frame iteration counts. Delivered by 10590c11, 2088e0ec and 1d787ed3 ([qc-route-decision.md](qc-route-decision.md)).
- [x] REQ-02 isolated check-update variants on both codes. Delivered by a72f2b3f ([check-variant-nomination.md](check-variant-nomination.md)). No variant was nominated.
- [x] REQ-03 QC productization decision. The decision was go: 4555dfb3 delivered the decoder core with a kernel in `gf2-kernels-simd` and a scalar fallback, 0cc6c195 delivered canonical dispatch, and 3ee00fcf delivered the confirmation and adoption ([adoption-decision.md](adoption-decision.md); `qualifies=true`).
- [x] REQ-04 compact-lane and selective NR lane decision. Delivered by 1d787ed3: the candidates are held, with the reason recorded in qc-route-decision.md.
- [x] REQ-05 numerical contract kept. No quantized, layered, reassociated or AFF3CT-sign variant entered production (5f401907).
- [x] REQ-06 terminal result for every losing cell. Delivered by 5f401907: all four losing cells are preserved inconclusive experiments, and both `w1-b8` leads are reported.
- [ ] REQ-07 (aspirational) not met. In pilot 3 on fraktaali, the NR `w1-b1` and `p12-b8` ratios (gf2 time / AFF3CT time) are 0.19 and 0.18, but nothing confirmed them. DVB-T2 `w1-b1` and `p12-b8` remain slower, at 1.97 and 1.81. Both `w1-b8` leads hold in the pilot.

### Wave Execution Log

**Wave 1:** 1 issue. Comparison arms and the per-frame iteration record.
**Wave 2b:** 1 issue. The comparison harness under `experiments/aff3ct-ldpc-gaps/`.
**Wave 2:** 2 issues. The NR route pilot with its decisions, and the check-variant comparison.
**Wave 3:** 1 issue. The QC decoder core, its SIMD kernel and the scalar fallback.
**Wave 4:** 1 issue. QC routing through canonical tuning and dispatch.
**Wave 5:** 1 issue. Adoption confirmation, qualified, and the production envelope cutover.
**Wave 6:** 1 issue. The comparator family over three pilots, then the closing table and record.

### Key Decisions

- Each timed job resumes per stage. After pilot 3 the frozen confirmation plan is preserved, so a later window could run the confirmation without another pilot.
- The isolated task 096f2ba3, created during execution, was wired as an epic dependency so `repo-validate` passes.
- The lead closed research-review advisory F1 on 5f401907 by stating in the record that pilot 3 was selected adaptively.

### Escalations

All 14 are recorded with their resolutions in `progress.json`. The final three:

- Pilot 1's freeze was refused at resolution 0.48 under host load. The owner approved a second pilot.
- Pilot 2's freeze was refused at resolution 0.75. The owner approved a third pilot.
- Pilot 3's freeze passed, but the job's load guard refused the confirmation, because the pilot's own run had raised the load. The owner chose to stop and record, and amended 5f401907 REQ-01 to allow closing without a confirmation.

### Issues Discovered During Execution

- 096f2ba3: Exempt archive evidence records from the reference check (created during wave 5, because the format 4 tracker rewrite broke `check-references` repo-wide).

### Holistic Quality Notes

- Every timed job checks load at each timed stage. A confirmation stage that follows a pilot in the same window therefore sees the pilot's own load. A job that runs both stages in one window has to wait for the load to fall before the confirmation, or arm the confirmation in a separate window.
- The reference-check exception for this directory stays, because its records cite paths that were removed from the tree at the revisions they state.
