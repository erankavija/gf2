# Reproduce or resume the protocol-v3 LDPC survey

> **Diátaxis Type:** How-to

Run commands from the assigned repository worktree root. Source pins and exact
Rust 1.95 release build commands are in the v3 preparation build identity. The
input archive preserves both bundles; extract it beneath `/tmp/c077a88b-ext/inputs`
on another host, and stage the pinned AFF3CT source/static build before rebuilding
arms. The producing-input manifest selects source, archives and prepared quality.
Refresh it before preparing a new campaign with
`CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh env CARGO_CI_NO_LOCK=1 python3 dev/active/c077a88b/survey/snapshot-inputs.py --external /tmp/c077a88b-ext --evidence dev/bench_results/c077a88b/v3-preparation`.
The outer budget wrapper owns the shared host locks while its nested Cargo
metadata query uses the inherited lock exemption. A measured preparation
requires a new evidence directory instead of overwriting its inputs.
Do not recreate an existing family ledger or overwrite a published receipt.

1. Build arms with the recorded command. Replay the saved validation plan under
   `cargo-budget.sh` on a single CPU, using a fresh durable quality log and report
   directory. The validator emits start/completion lines for every arm. Different
   binaries, corpus bytes or quality evidence require a separately named campaign.
2. Prepare one pilot plan with `run-campaign.sh FAMILY pilot RUN_ID prepare`.
   The `prepare-v3.py` genesis action is only for an initial ledger on a fresh
   reconstruction; it refuses an existing ledger. The committed ledger is the
   authority for continuation.
3. Run one bounded session at a time with:

```sh
dev/bench_results/c077a88b/run-campaign.sh matched-algorithm pilot v3-r1 run
dev/bench_results/c077a88b/run-campaign.sh quality-compatible pilot v3-r1 run
```

Repeat the identical command after exit 3, which means completed cells are
checkpointed. Exit 0 completes that family. Other exits require inspecting the
announced authoritative execution log and preserving the failed attempt. Never
launch both commands concurrently. Each invocation holds the repository's
`--full-host` CCX1 mutex for one bounded session and prints the durable log before
measurement. Quality simulation uses the shared CPU-budget lock; timed windows require the
exclusive full-host lock.

4. Finalize each completed pilot by replacing `run` with `finalize`; the launcher
   runs the independent acceptance binary. Freeze the matched confirmation addendum with
   `survey/freeze-addendum.py`, using its exact accepted pilot receipt and canonical
   family ID. The derivation file records the pilot alpha and snapshots. Publish
   the pilot, ledger and frozen addendum before confirmation. The quality family
   stops at its accepted six-candidate pilot: all P-19 admission checks fail, so
   no candidate is eligible for quality confirmation or fastest-arm selection.
5. Prepare, run, resume and finalize the matched confirmation with the same
   launcher and `confirmation` in place of `pilot`. Its fixed sample budget
   measures both matched cells. The launcher checks publication of the exact
   confirmation addendum. Do not run a quality confirmation from this pilot.
6. Regenerate the projection with `python3 dev/active/c077a88b/survey/summarize.py`.
   Report receipt acceptance separately from a cell's qualifying outcome.

The standalone harness tests use release mode. Its nextest CI settings are a
projection of the repository's `[profile.ci]` block; workspace-specific package
filters do not apply to the standalone workspace. Preparation logs preserve the
initial unsupported-profile invocations and the passing projected-profile run.
