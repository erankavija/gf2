# Rework round 4 raw outputs

This unlinked companion preserves command transcripts for the bounded rework.

## Focused red test run before implementation

Command:

```text
./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release --test protocol_contracts declared_margins_must_clear_resolution_at_all_three_boundaries bootstrap_uses_declared_corrected_alpha_for_all_family_sizes flagged_windows_include_the_exact_boundary_without_pooling_arms protocol_document_guard_includes_justifications_and_every_rule_it_declares v1_prior_trial_receipts_must_be_repository_relative
```

The sandbox invocation could not start the configured compiler cache. The
repository-permitted rerun reached the intended red state and failed to compile
the new semantic-table guard because `SharedSettings::table` still returned
only values:

```text
error[E0308]: mismatched types
    --> dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1951:9
     |
1951 |     for (name, value, justification) in rows {
     |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^    ---- this is an iterator with items of type `(&str, std::string::String)`
     |         |
     |         expected a tuple with 2 elements, found one with 3 elements
```

## Focused green contract suite

Command:

```text
./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release --test protocol_contracts
```

Output:

```text
Summary [0.704s] 40 tests run: 40 passed, 0 skipped
```

## Fresh bounded v3 evidence

Commands:

```text
dev/bench_results/f547c394/run-smoke-v3.sh prepare pilot
dev/bench_results/f547c394/run-smoke-v3.sh session pilot
dev/bench_results/f547c394/run-smoke-v3.sh session pilot
dev/bench_results/f547c394/run-smoke-v3.sh finalize pilot
dev/bench_results/f547c394/run-smoke-v3.sh prepare confirmation
dev/bench_results/f547c394/run-smoke-v3.sh session confirmation
dev/bench_results/f547c394/run-smoke-v3.sh session confirmation
dev/bench_results/f547c394/run-smoke-v3.sh finalize confirmation
```

Finalize output:

```text
GF2_BENCHMARK_RECEIPT=dev/bench_results/f547c394/v3-pilot/receipt.json
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
GF2_BENCHMARK_RECEIPT=dev/bench_results/f547c394/v3-confirmation/receipt.json
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
```

## Focused support release suite and compatibility

Commands:

```text
./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release
RUSTUP_TOOLCHAIN=1.95.0 ./scripts/cargo-budget.sh cargo check --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --all-targets --all-features
```

Output:

```text
Summary [0.829s] 166 tests run: 166 passed, 0 skipped
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
```

## Formatting and strict lint

Commands:

```text
./scripts/cargo-budget.sh cargo fmt --all -- --check
./scripts/cargo-budget.sh cargo clippy --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --all-targets --all-features -- -D warnings
```

The initial strict lint found and the change removed the version-match
range-pattern and test-helper argument-count warnings. The final lint output is
empty apart from Cargo progress and exits successfully.
