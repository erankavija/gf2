# Review: tuned-profile arm through the shared cell runner

> **Diátaxis Type:** Reference

Lead review of `@/issue/e5f8b3ae` at `05944a9e0`.

**Verdict:** PASS

## Gate status

cargo-ci, code-review and research-review pass with zero findings.

## Success criteria

- REQ-01: `run_with_policy` and `BatchWorker` in
  `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs` run every
  gf2 arm; the tuned arm supplies an `ArmPolicy` and keeps only envelope
  installation, route observation and its diagnostics.
- REQ-02: the canonical build, smoke and validation recorders, run in the story
  build tree, write [native](../../../bench_results/e5f8b3ae/native/) and
  [portable](../../../bench_results/e5f8b3ae/portable/) records, which supersede
  the story's preparation records and cover every gf2 arm on both codes at one,
  six, twelve and twenty-four workers.
- REQ-03: story receipts and `inputs/` snapshots keep their bytes; the receipt
  input checker passes.

## Holistic findings

The committed AFF3CT-intra quality record pins an executable that a rebuild of
the current tree no longer reproduces; its replay test skips when the built
executable is not the pinned one, as it skips when the arms are unbuilt.
