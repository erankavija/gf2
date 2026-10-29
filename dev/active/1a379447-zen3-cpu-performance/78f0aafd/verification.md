# Untimed verification

> **Diátaxis Type:** Reference

All commands run from the worktree root. Cargo uses `CARGO_NET_OFFLINE=true`,
`CARGO_CI_NO_SCCACHE=1`, and the worktree-local
`CARGO_HOME=$PWD/target/78f0aafd-cargo-home`. Cargo commands run through
`nice -n 19 ./scripts/cargo-budget.sh`; test execution adds `--test`.
Timings printed by builds, tests and smoke are diagnostic, not performance evidence.

## Commands

- `python3 -B dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/record-gf2-arms-build.py dev/active/1a379447-zen3-cpu-performance/78f0aafd/preparation --profile-only`
  builds only the profiler, through the existing recorder. `preparation/build-identity.json`
  records its command, flags, compiled production inputs and executable digest.
- `cargo +1.95 nextest run -p gf2-core --test gf256_gemm_allocation --test gf256_axpy_allocation --cargo-profile ci-test --profile ci`:
  `78f0aafd-alloc.log` and `allocation-witness.json`.
- `cargo +1.95 nextest run -p gf2-coding --test ldpc_decode_allocations --all-features --cargo-profile ci-test --profile ci`:
  `78f0aafd-ldpc-alloc.log` and `allocation-witness.json`.
- `cargo +1.95 nextest run --manifest-path dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/Cargo.toml -p ldpc-batch-profile -p ldpc-batch-tuned-arm --features ldpc-batch-profile/committed-envelope --cargo-profile ci-test --profile ci`,
  with `--config profile.ci-test.inherits='"release"'`, `--config profile.ci-test.lto=false`,
  `--config profile.ci-test.codegen-units=16`, `--config profile.ci-test.debug-assertions=true`
  and a scratch `--config-file` projecting the root's base `profile.ci` settings.
  The standalone workspace has no `ci-test` or nextest configuration. The projection
  omits root overrides for targets the standalone workspace does not contain;
  it changes no timeout. `profile-tests-final.log` records the result.
- `python3 -B dev/active/1a379447-zen3-cpu-performance/78f0aafd/test_preparation.py`:
  `preparation-tests.log`.
- `python3 -B dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/test_record_gf2_arms_build.py`:
  `recorder-tests.log`; its build-record-dependent tests retain their own skip conditions.
- `python3 -B dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/summarize-profile.py --self-test`.
- `python3 -B dev/active/1a379447-zen3-cpu-performance/78f0aafd/restore-arms.py build <subject>`
  for `byte-field`, `polynomial` and `nr-encode`: `restoration.json`. These builds
  use the installed Cargo home and sccache, because the build roots reproduce the receipts' paths.
- `python3 -B dev/active/1a379447-zen3-cpu-performance/78f0aafd/restore-arms.py smoke <subject> target/release/benchmark-ab-runner`:
  `smoke/restored/`.
- The launcher `--smoke` command uses the new `--record`, the live e5f8b3ae native
  `--arms-record`, `decoder-cases.tsv` and `smoke/`. `smoke/identity.txt` pins the
  executable and expanded input identities. Each diagnostic JSON has
  `perf_controlled=false`; no `perf` command runs.

## Check limitations

`launcher-tests.log` records the quality-file resume failure. The original
launcher reproduces it in `baseline-launcher-test.log`. Other existing launcher
checks pass. No existing record or fixture is rewritten to hide this failure.

Initial offline preparation found no `serde_json` in the empty worktree-local
Cargo cache. Copying the installed registry into this worktree resolves it.
An intermediate profiler build reported `cannot find value profile_policy in module tuned`;
the shared library import resolves it. Standalone nextest invocations initially
reported `profile ci not found`, then root overrides matching absent targets,
then `profile ci-test is not defined`; the explicit canonical profile projection
and nextest's forwarded Cargo `--config` options resolve these configuration errors.

No CI contract or timed series runs in this dispatch. REQ-01 window profiles,
the restored series' REQ-02 profiles, and their resulting
REQ-03 mechanism observations remain unmeasured.

## Reproduction checks

Run `python3 make-accounting.py` twice and compare SHA-256 of both generated
outputs. `python3 make-accounting.py --check` also compares without writing.
The plain-Python check runs between matching
`git status --porcelain --ignored --untracked-files=all` snapshots, so bytecode
or other repository writes are visible. `reproducibility.txt` records these checks.

The first status-snapshot comparison detects the check driver creating its own scratch logs; precreating those files removes that observation and isolates writes by Python.
