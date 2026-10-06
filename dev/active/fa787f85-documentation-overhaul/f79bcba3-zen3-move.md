# f79bcba3 zen3 survey-harness entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.

## Baseline

Source: `b05286e53614b62324f985832af766423849465c`. Commands, run from the
repository root:

```
R/f79bcba3-state.sh <state-file>
R/f79bcba3-builds.sh <build-file>
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
R/5a25717c-verdicts.sh <verdict-file>
```

| Output | Holds |
| --- | --- |
| `f79bcba3-state-before.txt` | `--check` exit status of each live closure generator, each relative Cargo path dependency that names or leaves an entry, each line of entry code that counts directory levels |
| `f79bcba3-builds-before.txt` | per harness manifest: exit status of `cargo +1.95.0 metadata`, exit status of `cargo +1.95.0 build`, first error line |
| `f79bcba3-verdicts-before.txt` | benchmark-acceptance verdict line and findings of each committed `zen3-benchmark-receipt-v1` receipt |
