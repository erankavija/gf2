# 11964cb8 rare-event artifact validation verdicts

Baseline commit: `ff34a202b`. Change commit: `d13d03f83`.

## Committed artifacts and snapshot copies

Every rare-event artifact carries an `ENVELOPE_SCHEMA_V1`-family schema string
(`gf2.rare-event-*`) and embeds a `design_identity`. The enumeration below
covers every tracked file except Rust and Markdown sources, snapshot copies
under `dev/bench_results/` included:

```sh
git grep -l -e 'gf2.rare-event-' -e '"design_identity"' -- ':!*.rs' ':!*.md'
```

It lists no file at either commit, so the set of committed rare-event
artifacts and snapshot copies is empty and has no per-artifact verdict. The
`dev/bench_results/07ca8585/*/inputs/producing/` copies of
`crates/gf2-sim/src/permanent_rare_event/` are digest-pinned producing-source
snapshots, not artifacts, and stay byte-identical.

## Validation suite

```sh
./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --all-features \
  --cargo-profile ci-test --profile ci \
  -E 'binary(permanent_rare_event_artifacts) | binary(permanent_rare_event) | test(permanent_rare_event)'
```

| Commit | Tests run | Passed | Failed |
| --- | --- | --- | --- |
| `ff34a202b` | 25 | 25 | 0 |
| `d13d03f83` | 26 | 26 | 0 |

At `d13d03f83` the 25 baseline tests pass unchanged; the added test is
`rare_event_design_identity_is_path_independent`.
