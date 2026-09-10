# LDPC preparation evidence

> **Diátaxis Type:** Reference

`validation.json` records a deterministic replay of both 128-frame bundles.
Every arm receives all-zero and random codewords; repeats of this report do
not increase the sample count. BER Wilson intervals follow the shared protocol;
errors within a failed LDPC frame are correlated, so the bit-level interval
is not an independent-frame uncertainty estimate.

Command (from the worktree root):

```sh
GF2_CCX1_LOCK=/tmp/c077a88b-quality-budget.lock CARGO_CI_NO_SCCACHE=1 RAYON_NUM_THREADS=1 ./scripts/cargo-budget.sh taskset -c 0 /tmp/c077a88b-build/release/ldpc-validate --plan dev/bench_results/c077a88b/2026-09-08-c077a88b-preparation/validation-plan.json --output dev/bench_results/c077a88b/2026-09-08-c077a88b-preparation/validation.json
```

The dedicated quality budget lock avoids taking either side of the timing
mutex while retaining the CPU budget's presence-slot accounting. These runs
are not timing evidence. Their process RSS and one-pass latency are diagnostics;
paired campaign windows determine performance. No BFER simulation runs inside
a timed campaign. `quality/` contains exact projections of the validation
report's per-arm quality objects, loaded by the timed arms.

`capabilities-nr.json` preserves every command and raw output of a bounded
16-frame, five-second maximum AFF3CT CLI screen. It includes SPA, so INTRA
support is actually tested. Its all-zero-only simulated throughput is a
shortlist screen, not a performance claim or random-codeword quality proof.
The earlier frame-error-target screen is preserved in the adapter-validation
receipt dated 2026-09-07.

`build-identity.json` records upstream pins, submodules, actual compile flags,
the binary's own AVX2 version output, static library digest, arm executable
digests and Rust toolchain. `source-inputs/` preserves producing source and
recorded input bytes. The canonical family producing manifest includes these
archives and the quality records. Runtime command paths under `/tmp` are
locators; replay stages the archives there or regenerates plans with new
locators. No Git revision or whole-tree clean/dirty condition determines
provenance acceptance.

OpenAirInterface configuration is attempted with the system CMake and
`AUTO_DOWNLOAD_ASN1C=ON`; it fails on missing SCTP development dependencies.
The initial PATH CMake failure and initial missing-asn1c configuration are also
preserved. Configuration detects AVX2 and disables AVX512; no OAI decoder
binary is claimed built or measured. `upstream-status.json` preserves current
upstream observations, including OCUDU as srsRAN Project's successor and the
local DNS access failure.

The first `cargo-ci.log` preserves a failed full CI run. Worktree prefixes in
logs are rendered as `.` for portable paths; verdict lines are unchanged.
`msrv-check.log` records a successful Rust 1.95 release check, and
`provenance-test.log` records the passing family-source snapshot regression.
The missing `dev/tools/tuning-profile-compose/Cargo.lock` CI prerequisite is
regenerated locally. `gpu-test-before.log` preserves the dispatcher failure
on a host without a visible HIP device. The test uses the existing GPU
integration-test availability check; its focused rerun passes.
`harness-tests-final.log` records both passing warm/streaming and input-bank
regressions; `msrv-final.log` records the final Rust 1.95 check.

`quality-harness-source.tar.gz` captures the harness source at the quality
replay. The subsequent warm-cache fixture policy changes only timing.
`build-identity.json` records separate quality and final timing executable
identities. The final quality report was emitted as `validation-final.json`
and moved to canonical `validation.json`; `validation-final.log` retains the
original invocation. `validation-before-bank-fix.json` is an earlier replay,
not additional samples.

The completed `cargo-ci-final.log` run passes every step except the GPU test
executed before its repair. `cargo-ci-verdict.log` is the subsequent full rerun: every step passes and the
command exits 0. Its step durations include host-lock waits.
`quality-capture-before.log` and `quality-capture-after.log` preserve the failing
and passing runner regression: child quality survives checkpoint/finalization
without counting timing repetitions as independent BER/FER samples. The initial
matched pilot is retained as a rejected receipt; replacement pilots use distinct
campaign identities with the `2026-09-08-r2` prefix.
