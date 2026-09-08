# Validation command record

All commands run from the assigned worktree root. Cargo commands use the
repository budget wrapper; test runs use its serialized `--test` path.
`CARGO_CI_NO_SCCACHE=1` disables the sandbox-incompatible compiler cache.
Raw outputs are companion files in this directory, not benchmark receipts.

| Check | Command | Outcome / output |
|---|---|---|
| Required repository CI | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh` | Exit 1; `cargo-ci.log`. Every step passes except the GPU dispatcher test (HIP device detection code 100). |
| Focused behavioral suite | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --all-features --cargo-profile ci-test --profile ci` | 158 passed; `suite-chain.log`. |
| MSRV | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95 check -p tuning-campaign-support --all-targets --all-features` | Passed; `msrv-ready.log`. |
| Focused lint | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo clippy -p tuning-campaign-support --all-targets --all-features -- -D warnings` | Passed; `clippy-ready.log`. |
| Release executables | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support --bin benchmark-ab-runner --bin benchmark-acceptance --bin ab-smoke-workload` | Passed; `release-ready.log`. |
| GPU diagnosis | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --all-features --cargo-profile ci-test --profile ci -E 'test(test_dispatcher_acquires_stream_and_allocates)'` | Reproduced HIP 100 failure; `gpu-diagnosis.log`. |
| Lead’s incoming mutex regression | `./scripts/cargo-budget.sh --test bash dev/scripts/ccx1-bench-flock.test.sh` | Passed; `lock-wrapper-budgeted.log`. |
| Final launcher syntax | `bash -n dev/bench_results/f547c394/run-smoke-v2.sh` | Passed after fixing the preserved preflight rejection. |

CI started before the last focused source changes and completed afterwards;
its support-suite and clippy stages ran with the final implementation. The
separate focused suite, lint, MSRV and release build above validate the final
source. The launcher’s decoder-family generator correction followed CI; its
syntax and actual release pilot/confirmation are validated separately.

Initial test-first commands could not compile through sccache (EPERM); `r3-red.log`
and `check.log` retain that result. Intermediate compilation/fixture/lint failures
are preserved in the other logs. They are development diagnostics, not additional
scientific trials or claimed original-code behavioral red evidence. No ignored
nightly suite, Lean regeneration, HIP build or real decoder campaign was run.

The post-measurement checks are `research-r3-portability.json` (copied receipts
re-evaluate with byte-identical summaries), `final-source-consistency.json`
(identical frozen numeric table columns, Rust producing sources and executable
bytes), and `launcher-syntax.json` (exit 0). `launcher-description.diff` contains
the complete two-line descriptive change after measurement. The complete CI
contract was not repeated for these documentation/rationale-only edits.
Standalone `cargo doc`, Lean/lake and HIP builds, ignored/nightly tests, actual
external decoder comparator studies and performance adoption campaigns were not
run. The code/doc/research review gates were not invoked by this read-only worker.
