# Handoff 33: daef2dc3 stopped after the owner's extra rework round

Main is at the commit carrying this file. The owner authorized one more rework round on `@/issue/daef2dc3`. `cargo-ci` passed at `ac89f1a07`; `code-review` failed there with three findings, so work stopped without a further round.

## Open findings

1. **High, REQ-02.** `dev/active/b8206228-permanent-statistics/progress.json` holds overnight execution records (`ran_at`, `wall_seconds`, `binary_sha256`, for example near line 1145). The budget admits every `progress.json` by name. Other `progress.json` files need the same audit.
2. **Medium, REQ-04.** `crates/gf2-coding/tests/data/bch_oracle/gap.json` lines 12 and 14 hold `gap-packages/...` paths, substituted by an earlier scrub, that resolve nowhere. The reference check reads only known root directories, so it accepts them.
3. **Medium.** `dev/active/ae03bcd0-general-bch/oracle/run.sh` writes `target/oracle/oracle-receipt.md` without creating `target/oracle`.

## Removed in this round

- The batch-decoder survey, whose Cargo paths no longer resolved, and its CI steps. The `55c3e433` handoff states where it survives.
- `fd9d5416`'s receipt renderer and its test, which needed a removed baseline.
- `dev/authoring-conventions.md`, which described archival.
- `dev/scripts`: `resume_identity`, `verify-campaign-log`, `verify-source-evidence`, `campaign_inputs`, `campaign_tables`, `crate_functions`, `check-addendum-schema-versions`, `check-family-producing-closures`, `pin-prior-receipt` and `validate_tuning_extent_campaign_test`. Some of these were in `506f76d4`'s scope, which shrinks accordingly.

## Traps

- `DVB_TEST_VECTORS_PATH` has no default now. Unset, the stream tests skip or report absent streams, so a slow-tier run must set it.
- The window tooling moved to `dev/scripts/bench-window`. No timer unit was armed when it moved; a new timer uses the new path.
- A broad `execution*.md` budget rule refused `execution-batching.md`, a plan. The rule excludes `.md`.
- The pattern-based reference check reads known roots. A bare relative path inside a data file passes it, as finding 2 shows.
- Only the release tests, formatting and Clippy of `permanent_wave_gpu` ran. The ROCm probe build of `permanent-wave-gpu-ci` did not.
