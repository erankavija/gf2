# Analysis fixtures

Synthetic schema-3 datasets for `tests/test_permanent_zero_fraction_analysis.py`
and `committed_analysis_fixtures_verify`. Run identity is placeholder: zero
executable digest and source revision, host and toolchain `synthetic`, argv
without output paths. Shard counts are the sampler's seeded draws from the
tracked configurations.

- `small-campaign`, `small-campaign-halted`: complete and halted datasets.
- `checksum-mismatch`: `small-campaign` with a stale `summary.csv` digest.
- `invalid-q`: `small-campaign` with an unsupported field order in the manifest.
- `golden/small-campaign`: the analysis script's output for `small-campaign`.

Regenerate with `python3 regenerate.py`, which writes to
`target/permanent-statistics-analysis-fixtures`. Replace the tracked files with
`python3 regenerate.py --output .` from this directory.
