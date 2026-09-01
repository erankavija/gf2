# CPU-vs-GPU BCH decode outcome identity — receipt

JIT issue: `c3cc5226` (Migrate HIP syndrome/decode support behaviorally
intact), success criterion REQ-02.

Every result below comes from the run recorded here, on the hardware described
here, at the pinned revision. Nothing is estimated or reproduced from an
earlier run.

## Pinned revision

| Item | Value |
|------|-------|
| Revision measured | `b3679e50dfc77093f6c3b26b0698cb8f69c8c0e5` |
| Branch | `worktree-agent-c3cc5226` |
| Working tree at each measurement | clean (`git status --short`), apart from this receipt and from the two deliberate mutants named below, each reverted after its run |

The revision contains the measured source: the implementation commit landed
before the suites ran, so the code under test is the code the revision names.

## Host

| Item | Value |
|------|-------|
| Hostname | `fraktaali` |
| GPU | AMD Radeon RX 6950 XT, `gfx1030`, `GPU-8cd14d6d8a3c8a73` (`rocminfo`) |
| CPU | AMD Ryzen 9 5900X (12C/24T) (`rocminfo`) |
| ROCm | 7.2.4 (`/opt/rocm/.info/version`) |
| Kernel | Linux 7.1.11-arch1-1 (`uname -r`) |
| Toolchain | rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0 |
| Build | `--release`, `--features hip` |

## What the suites establish

GPU-assisted decoding consumes the canonical construction model
(`BinaryBchCode`) and reports `BchDecodeOutcome`. The claim under test is
outcome identity: for every received word, `BinaryBchDecoder::correct_batch_gpu`
reports the outcome and leaves the corrected word that the per-word CPU
`BinaryBchDecoder::correct_in_place` produces. GF(2^m) arithmetic is exact
integer arithmetic over the uploaded CPU `exp`/`log` tables, so the comparison
is equality with zero tolerance. The identity covers the device-failure paths
as well: a failed syndrome pass restores the batch, and a recoverable failure
decodes it on the CPU rather than erroring.

| Suite | Point | What it pins |
|-------|-------|--------------|
| gf2-coding, in-crate | six primitive narrow-sense codes, radii 1-3, lengths 7-63 | device syndromes equal the CPU evaluator value for value; GPU-assisted outcomes and corrected words equal the CPU path's |
| gf2-coding, in-crate | BCH(15, 7), all 455 weight-three words | the verification arm: each word one past the radius resolves to the same `Uncorrectable` or verified miscorrection on both paths, and a rejected candidate is rolled back |
| gf2-coding, in-crate | the same 455 words with every candidate applied, against a synthesized `HipError` | the device-failure arms: a failure that stays explicit restores the batch word for word, and a recoverable one returns the CPU path's outcomes and words over both verification arms |
| gf2-sim, rungs 4-5 | DVB-T2 Short GF(2^14) and Normal GF(2^16), 200 mixed frames each | all `2t` u16 syndromes byte-identical to the CPU `BchDecoder::compute_syndromes` |
| gf2-sim, rung 5 | the primitive narrow-sense mother codes of those two configurations — BCH(16383, 16215) and BCH(65535, 65343), t = 12 — 200 mixed frames each | outcome identity across all three outcomes |

The mixed populations hold one third valid codewords, one third with `1..=t`
errors, and one third with `t+1..=2t+1` errors, at fixed seeds.

The failure-path tests synthesize the `HipError` rather than provoking a device
fault, so they need no device and run wherever the `hip` feature builds; the
`gf2-sim` boundary test `test_stage_error_split_matches_the_kernel_predicate`
holds the pipeline's `StageError` mapping and `HipError::is_recoverable` to one
split of `@/inv/accelerator-safe-fallback`.

## Exact commands

```bash
# In-crate device tests over the canonical model (gf2-coding).
./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding \
    --features hip --release -E 'test(canonical_decoder_tests::gpu)' --no-capture

# Correctness ladder rungs 4-5 (gf2-sim).
./scripts/cargo-budget.sh --test cargo test -p gf2-sim --features hip --release \
    --test gpu_bch_syndrome_byte_identity -- --ignored --nocapture

# Feature gating: the same crates without `hip`.
./scripts/cargo-budget.sh cargo clippy -p gf2-coding -p gf2-sim --all-targets -- -D warnings
./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding -p gf2-sim \
    --cargo-profile ci-test --profile ci

# Repository CI contract, and the ROCm-only crate's own gate.
./scripts/cargo-ci.sh
./scripts/cargo-budget.sh --test cargo test --manifest-path crates/gf2-kernels-hip/Cargo.toml \
    --release --features hip --offline
./scripts/cargo-budget.sh cargo fmt --manifest-path crates/gf2-kernels-hip/Cargo.toml --all -- --check
./scripts/cargo-budget.sh cargo clippy --manifest-path crates/gf2-kernels-hip/Cargo.toml \
    --release --all-targets --features hip --offline -- -D warnings
```

## Results

### gf2-coding — canonical-model device tests

```text
        PASS [   0.069s] (1/6) gf2-coding bch::core::canonical_decoder_tests::gpu::a_candidate_failing_verification_is_uncorrectable_on_both_paths
        PASS [   0.017s] (2/6) gf2-coding bch::core::canonical_decoder_tests::gpu::a_fatal_verification_failure_restores_the_batch
        PASS [   0.017s] (3/6) gf2-coding bch::core::canonical_decoder_tests::gpu::a_first_pass_failure_never_alters_the_batch
        PASS [   0.018s] (4/6) gf2-coding bch::core::canonical_decoder_tests::gpu::a_recoverable_verification_failure_falls_back_to_the_cpu
        PASS [   0.065s] (5/6) gf2-coding bch::core::canonical_decoder_tests::gpu::device_syndromes_equal_the_cpu_evaluator
        PASS [   0.066s] (6/6) gf2-coding bch::core::canonical_decoder_tests::gpu::gpu_assisted_correction_reports_the_cpu_outcomes
────────────
     Summary [   0.254s] 6 tests run: 6 passed, 1611 skipped
```

The three device tests self-gate on `device_mem_info()` and skip without a
usable device; on this host they ran, as the timings above show. The three
failure-path tests need no device. None carries an ignore tier, so the
repository CI contract runs all six.

Each new failure-path test was run against two mutants of the pinned source —
the revision above with one named change, reverted after each run — so none of
them is a test that cannot fail. Both runs used the `gf2-coding` command above
without `--no-capture`:

| Mutant | Result |
|--------|--------|
| `recover_from_device_error` drops its rollback loop | `6 tests run: 4 passed, 2 failed` — `a_fatal_verification_failure_restores_the_batch` and `a_recoverable_verification_failure_falls_back_to_the_cpu` fail; the applied candidates survive, and the CPU pass then decodes the altered words |
| `HipError::is_recoverable` answers `false` for `OutOfMemory` and `UnsupportedArch` | `6 tests run: 4 passed, 2 failed` — `a_recoverable_verification_failure_falls_back_to_the_cpu` and `a_first_pass_failure_never_alters_the_batch` fail; the fallback never runs and the error propagates instead |

The three device tests pass under both mutants, which is why the failure paths
needed coverage of their own.

### gf2-sim — correctness ladder rungs 4-5

```text
running 4 tests
mother-gf14-t12: 200 frames over BCH(16383, 16215), t = 12, outcome-identical (CPU==GPU): 67 clean, 67 corrected, 66 uncorrectable
test test_gpu_assisted_decode_outcome_identical_to_cpu_gf14 ... ok
dvb-t2-short-r1/2-gf14: 200 frames, 24 syndromes/frame, byte-identical (CPU==GPU)
test test_gpu_bch_syndrome_short_byte_identical_to_cpu ... ok
mother-gf16-t12: 200 frames over BCH(65535, 65343), t = 12, outcome-identical (CPU==GPU): 67 clean, 67 corrected, 66 uncorrectable
test test_gpu_assisted_decode_outcome_identical_to_cpu_gf16 ... ok
dvb-t2-normal-r1/2-gf16: 200 frames, 24 syndromes/frame, byte-identical (CPU==GPU)
test test_gpu_bch_syndrome_normal_byte_identical_to_cpu ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.42s
```

Each of the two outcome-identity legs reaches all three outcomes — 67 words
decoded clean, 67 corrected and verified, 66 with no verified correction — so
the identity is pinned on every arm rather than on the error-free one.

### Feature gating, without `hip`

Clippy over both crates: no warnings. Tests: `2034 tests run: 2034 passed, 132
skipped`.

### Repository gates

`./scripts/cargo-ci.sh`: every step passed, `test: 5540 passed, 0 failed, 241
skipped`. The `hip` feature is in that run's feature set, because `hipcc`
resolves on this host.

The ROCm-only `gf2-kernels-hip` gate passed all three of its legs: its release
test run totalled 125 passed, 0 failed, 29 ignored across its binaries and
doctests, and its formatting and Clippy checks were clean.
