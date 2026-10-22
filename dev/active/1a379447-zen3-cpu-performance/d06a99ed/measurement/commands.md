# Measurement commands

> **Diátaxis Type:** Reference

Run from the main repository root. The source checkouts are already detached
at the pinned revisions. Build and smoke commands open no timing windows.

```sh
repo=$PWD
m=dev/active/1a379447-zen3-cpu-performance/d06a99ed/measurement
survey=dev/active/ed3d490e-zen3-ldpc-frame-simd/survey
before="$repo/target/d06a99ed-builds/before"
after="$repo/target/d06a99ed-builds/after"
aff3ct="$repo/.agents/ext/c077a88b/aff3ct"
pinned="$repo/.agents/worktrees/agent-ed3d490e-run/target/ldpc-throughput/release/aff3ct-throughput-arm"
mkdir -p "$before/target/pinned/release" "$after/target/pinned/release"
# These links already exist. The unchanged recorder checks the catalogue digest.
[ -e "$before/target/pinned/release/aff3ct-throughput-arm" ] || ln -s "$pinned" "$before/target/pinned/release/aff3ct-throughput-arm"
[ -e "$after/target/pinned/release/aff3ct-throughput-arm" ] || ln -s "$pinned" "$after/target/pinned/release/aff3ct-throughput-arm"
(cd "$before" && python3 "$survey/record-gf2-arms-build.py" "$repo/target/d06a99ed-builds/before-record" "$aff3ct")
cp "$repo/target/d06a99ed-builds/before-record/build-identity.json" "$m/before-build.json"
(cd "$after" && python3 "$survey/record-gf2-arms-build.py" "$repo/target/d06a99ed-builds/after-record" "$aff3ct")
cp "$repo/target/d06a99ed-builds/after-record/build-identity.json" "$m/after-build.json"
python3 "$m/prepare.py" "$survey/make-family.py" "$m/before-build.json" "$m/after-build.json" "$m/pilot"
python3 "$survey/run-family-campaign.py" "$m/pilot/plan.json" "$m/before-build.json" "$m/after-build.json" --untimed
```

The lead commits the declaration, build records and generated pilot addendum
and plan, then inserts [window-queue.tsv](window-queue.tsv) into the shared
queue. The existing window runner supplies `GF2_BENCH_WINDOW=1`. The campaign
launcher acquires the full-host lock and sets `CARGO_CI_NO_LOCK=1` for its own
Cargo work. Never set the window flag to launch measurements during a working
session.

The sequential [window.sh](window.sh) runs the pilot, forwards the declaration's
fixed margins and rationales to the canonical freezer, composes confirmation,
and runs confirmation. The freezer stops if pilot resolution reaches a margin;
it does not widen the margins from results. The confirmation description removes
the exploratory-pilot sentence. Existing frozen addendum and plan files are
reused on resume.

The lead-reviewed checkpoint stages and commits only the generated confirmation
addendum, plan and derivation. It skips a commit when those three paths already
match HEAD. It honors the existing launcher's committed-addendum requirement
without including receipts, ledgers or unrelated staged work.

```sh
m=dev/active/1a379447-zen3-cpu-performance/d06a99ed/measurement
bash "$m/window.sh" dev/active/ed3d490e-zen3-ldpc-frame-simd/survey dev/active/1a379447-zen3-cpu-performance/c7113c5a/survey/freeze-confirmation.py
```

This sequential measurement command belongs only in the benchmark window.
The queue duration is an estimate, not a measurement. Rerunning resumes each
campaign without repeating completed cells. The existing launcher verifies the
final execution log, finalizes the receipt and invokes `benchmark-acceptance`.

Receipt destinations are
`dev/bench_results/d06a99ed/worker-routing/d06a99ed-worker-routing-{pilot,confirmation}/`.
Commit their existing receipt, plan, execution log, checkpoints, input snapshots
and acceptance summary files. Report small whole-call intervals and the
correctly labelled empty executor-dispatch diagnostics. The generated outcome
table uses the existing projection:

```sh
python3 dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/make-collection-outcome.py d06a99ed-worker-routing-confirmation dev/active/1a379447-zen3-cpu-performance/d06a99ed/measurement/outcome.md
```


The local ignore file excludes extracted pilot/confirmation bundle directories.
`prepare.py` recreates them from the existing committed archive. The receipt
runner captures its required immutable input snapshots when measurement runs;
these extracted preparation copies are not duplicate published evidence.
