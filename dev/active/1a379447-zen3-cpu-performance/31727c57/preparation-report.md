# Integration route preparation and evidence

> **Diátaxis Type:** Reference

The core and decoder integration campaigns of `@/issue/31727c57` have complete
pilot, canonical freeze and confirmation evidence under the
[scorecard design](../1362381c/design.md). The [outcome record](outcome.md)
projects the acceptance verdicts, per-cell estimates and intervals, pair counts,
frozen margins, cell allocations and separate decoder batch-fill evidence.
Its margin-ruling sections preserve the pre-pilot margin refusals and the
invoker's authorized replacements before confirmation sampling.

## Prepared inputs and operation

The [core build record](core/arm-builds.json) pins the release arms.
The executed decoder plans in the outcome record identify the envelope and
the conservative and tuned executables. Both families' receipts carry the
input snapshots they pin. The [core](core/holdout-cells.json) and
[decoder](decoder/holdout-cells.json) holdout declarations and family ledgers
are the inputs to the canonical freezes linked in the outcome record.

The [core](core/confirmation-untimed-smoke.json) and
[decoder](decoder/confirmation-untimed-smoke.json) confirmation smoke records
exercise the arm wire contract with zero timing windows. The
[preparation diagnostics](decoder/untimed-diagnostics.json) check the presence
of `batch_fill_ns`; that field alone does not establish a stage measurement.
The outcome record identifies the tuned arm's unmeasured batch-fill latency
in the timed receipts. This REQ-04 evidence gap requires lead disposition.

The [core](core/window.sh) and [decoder](decoder/window.sh) launchers require
the overnight benchmark window. The [queue file](window-queue.tsv) records
their invocation paths. Timed execution logs, receipts and freeze outputs are
committed evidence; the outcome generator reads them without running arms.

## Verification

From the worktree root, the following checks pass with Python bytecode disabled:

```sh
python3 -B dev/scripts/check-receipt-input-snapshots.py
python3 -B dev/scripts/check-addendum-schema-versions.py
python3 -B dev/active/1a379447-zen3-cpu-performance/31727c57/test_preparation.py
python3 -B dev/active/1a379447-zen3-cpu-performance/31727c57/test_outcome.py
python3 -B dev/active/1a379447-zen3-cpu-performance/31727c57/make_outcome.py --check
```

The outcome test checks reproducibility against the saved report, preservation
of per-cell intervals and adverse verdicts, allocation counts, and the distinction
between a measured batch-fill value and an unprobed zero. The generator verifies
the execution logs against their receipts and plans before writing the report.
