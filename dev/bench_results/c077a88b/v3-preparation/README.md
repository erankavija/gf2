# Protocol-v3 LDPC preparation

> **Diátaxis Type:** Reference

[build-identity.json](build-identity.json) pins the Rust 1.95 release arm binaries,
external AFF3CT build and exact build command. [validation-plan.json](validation-plan.json)
records the same-input arm commands; [validation.json](validation.json) records
per-frame error vectors, decoder settings, output agreement, iteration summaries,
selected backends and process diagnostics. [quality-final-execution.log](quality-final-execution.log)
is the durable quality-replay log. Files under `quality/` are projections of the
corresponding arm reports, reused by timing children without resampling.

The producing manifest is generated from Cargo's resolved local dependency graph,
source archives, recorded input archives and prepared quality. Its exact path is
selected by each saved v3 plan. The runner snapshots every selected file before
measurement. The recorded AList operation includes matrix/decoder construction
and destruction in each timed call and initializes no encoder.

`shared-contract-red.log` demonstrates rejection of a family-selected manifest
and of decoder reports differing only in process diagnostics before the fix.
`shared-contract-green.log` and `shared-contract-suite.log` record passing focused
and shared behavioral suites. The latter also tests omitted/default manifest
binding and the actual runner's custom-manifest lifecycle. V1/v2 compatibility
fixtures remain in that suite.

The standalone harness's ordinary tests pass under Rust 1.95 release mode in
`harness-tests-projected.log`. Its CI profile is projected from the repository's
`[profile.ci]` block; package-specific workspace overrides cannot match a standalone
workspace. The two earlier profile-selection failures remain in their logs.
`clippy-before.log` records the preexisting Rust 1.95 guard-collapse lint surfaced
by CI; `clippy-after.log` records its semantics-preserving correction.
The complete initial CI verdict is `cargo-ci.log`; the repeat verdict is
`cargo-ci-final.log`.

The frozen corpus and quality tolerance remain unchanged. The v3 paired FER
bound cannot establish quality non-inferiority on these frames, including for
arms with identical observed frame-error indicators. This does not establish
that every candidate has worse quality. The accepted v3 campaign summaries are
the authority for candidate admission and qualifying outcomes.

The final rebuild changed the AFF3CT executable identity. Its fresh quality replay
retains identical scientific vectors, settings, counts, intervals, iterations and
output agreements. The earlier untimed preparation remains in
`superseded-before-final-rebuild/`. The initial snapshot failure in
`snapshot-inputs-final.log` records the unavailable compiler cache; the final
refresh uses the repository no-sccache override and existing offline dependencies.

`pilot-claim-red.log` preserves the P-03 no-claim failure (and fixture preparation
errors). `pilot-claim-green-suite.log` retains the test fixture's initially
underdeclared resolution and the final complete passing shared suite. The
canonical raw-pair/seed/ledger recomputation remains strict for underdeclared
resolution and altered claims. `pilot-claim-clippy.log` and
`pilot-claim-evaluator-build.log` identify the passing lint and release evaluator
build. The full final CI rerun is `cargo-ci-after-acceptance-fix.log`.

The quality family stops at its accepted negative pilot. No quality confirmation
is run. The matched confirmation's original rejected summary is preserved in
`../v3-r1-matched-confirmation-rejection/`; its independent reevaluation audit
records unchanged receipt bytes in `../v3-r1-matched-confirmation-reevaluation.log`.
