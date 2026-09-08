# Rework round 5 raw outputs

This unlinked companion retains command transcripts for the bounded rework.

## Focused red test

Command:

```text
./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release --test protocol_contracts v3_binds_claimed_alpha_in_receipts_and_pilot_resolution
```

Before the production change, the checkpoint-consistent altered v3
`claim.interval.alpha` remained accepted. The focused test failed at the
missing P-20 assertion:

```text
assertion failed: summary.findings.iter().any(|finding| finding.rule == "P-20")
Summary: 1 test run: 0 passed, 1 failed, 40 skipped
```

## Focused green contracts

```text
./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release --test protocol_contracts
Summary: 41 tests run: 41 passed, 0 skipped
```

## Fresh r2 bounded evidence

```text
dev/bench_results/f547c394/run-smoke-v3.sh prepare pilot
dev/bench_results/f547c394/run-smoke-v3.sh session pilot
dev/bench_results/f547c394/run-smoke-v3.sh session pilot
dev/bench_results/f547c394/run-smoke-v3.sh finalize pilot
dev/bench_results/f547c394/run-smoke-v3.sh prepare confirmation
dev/bench_results/f547c394/run-smoke-v3.sh session confirmation
dev/bench_results/f547c394/run-smoke-v3.sh session confirmation
dev/bench_results/f547c394/run-smoke-v3.sh finalize confirmation
target/release/benchmark-acceptance dev/bench_results/f547c394/v3-r2-pilot
target/release/benchmark-acceptance dev/bench_results/f547c394/v3-r2-confirmation
```

Each finalization and each independent evaluation reports:

```text
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
```

## Final focused validation

```text
./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release
Summary: 167 tests run: 167 passed, 0 skipped

RUSTUP_TOOLCHAIN=1.95.0 ./scripts/cargo-budget.sh cargo check --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --all-targets --all-features
Finished `dev` profile [unoptimized + debuginfo]

./scripts/cargo-budget.sh cargo fmt --all -- --check
./scripts/cargo-budget.sh cargo clippy --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --all-targets --all-features -- -D warnings
```

The format and strict clippy commands exit successfully. The r1 preservation
command recorded in `research-r5-v3-r1-preservation.json` exits successfully.
