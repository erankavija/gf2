# Historical permanent-benchmark receipt provenance

This directory preserves the permanent-campaign CSVs, including the S3
cross-CPU and S5 GPU-crossover receipts and their conventional-name snapshots
under `csvs/`.

## S3 and S5 status

The historical receipts below are **corroboration-only, not authoritative
measurement evidence**:

- `s3_cross_cpu-2026-05-12.csv` and `csvs/s3_cross_cpu.csv`
- `s5_gpu_crossover-2026-05-15.csv` and `csvs/s5_gpu_crossover.csv`

Neither measurement has immutable measurement-source provenance. In
particular, the committed files do not identify the exact executable state by
a committed harness source SHA, so the measurement source cannot be
reconstructed from the repository. Their dates, host details, seeds, and CSV
rows remain useful for corroboration and for preserving the historical record,
but they must not be used alone to establish a performance claim, choose a
backend, or override a newer receipt with complete provenance.

This qualification records the empirical-receipt requirement from the
[2026-08-07 external research-methodology review](../../active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md):
research receipts must cite exact figures and configurations. The subsequent
[feasibility-study review RCA](../../sessions/2026-08-08-b488f02c-review-rca.md#recommendations)
also requires receipts to carry data and mechanical provenance rather than
interpretive authority. The current feasibility-study measurements are the
evidence for present comparative conclusions; see
[`dev/studies/b488f02c/feasibility-study.md`](../../studies/b488f02c/feasibility-study.md).

The `csvs/` copies are byte-preserving snapshots and inherit this status; they
are not independently refreshed or promoted to authoritative evidence.

The CSV contents are preserved as historical artifacts. This README is the
durable provenance record; do not rewrite the old headers to imply a source
identity that was not recorded at measurement time.
