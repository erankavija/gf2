# Premeasurement implementation review

This review covers the preserved implementation at `acbccaf9`, against the
authoritative protocol linked to issue `eaae1b56` at `3514d75d`. It concerns
readiness for measurement; measured publication and issue gates remain pending.

## Findings and disposition

| Finding | Evidence at the reviewed commit | Required disposition |
|---|---|---|
| F1: The implementation changes the approved measurement architecture. | `run_child` rejects measured tasks in the controller, a separate `tuning_calibration_timed` binary omits effective observations, and the GEMM bridge gains production visibility. | Restore the approved single harness and test-support bridge. Preserve the separation of fixture setup and timed operations. Every child records its required preflight observations. |
| F2: Tests do not exercise the claimed fresh-process production behavior. | Harness tests construct synthetic reports; `every_field_runs_each_arm_in_a_fresh_child` checks only an enum. | Add bounded actual child-process witnesses using the shared production harness, with installed resolution, routes, effective observations, and paired semantic results. |
| F3: GEMM kernel decline lacks its reporting outcome. | The admitted accelerated GEMM arm returns a generic error on effective fallback; the omission vocabulary has no corresponding decline case. | Record the typed capability omission and effective fallback, retain the complete conservative result, and refuse authoritative publication for the incomplete comparison. |

The independent M4RM review finds no blocker in the effective observer and
deterministic no-SIMD witness. The implementation branch already includes these
changes; merging its separate observer branch again is unnecessary.

No prior configured code-review runs exist for this issue. The migration
design's historical open-question references resolve through its recorded
amendments; they do not authorize a new protocol or public API. The execution
lead selects restoration to the approved protocol. No measurement evidence
establishes a need for the proposed two-binary architecture.

## Verification

At `acbccaf9`, Rust 1.95 focused harness tests pass: 87 tests, no skips. The
deterministic no-SIMD M4RM and PLE observer checks pass: three tests, no skips.
Both executions use `scripts/cargo-budget.sh --test`, the `ci-test` Cargo
profile, and the `ci` nextest profile. These checks establish the existing
test results; F2 identifies behavior those tests do not establish.

The preserved branch incorporates main `734c2f9b` before correction, so the
next implementation review can assess integration with current repository
code. No calibration or publication occurs before the correction, its
independent review, and the required premeasurement checks.
