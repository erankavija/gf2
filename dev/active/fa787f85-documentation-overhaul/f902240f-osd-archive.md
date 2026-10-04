# Container archival of the OSD epic b7157be6 (f902240f)

`jit archive container b7157be6 --execute` ran on main in commit `7335791e4`;
its result is [`f902240f-exec.json`](f902240f-exec.json). Figures below come from
[`f902240f-evidence.py`](f902240f-evidence.py), output in
[`f902240f-evidence.txt`](f902240f-evidence.txt).

## Criteria

| Criterion | Evidence | Establishes | Does not establish |
| --- | --- | --- | --- |
| REQ-01 | The planner adopts a markerless `<short8>-<slug>` directory when each file in it is the mirror destination of a linked document. The 8 hand-archived files are linked by b7157be6 and 312200e4; the execution reports them `adopted`, and 8/8 are byte-identical across the execution commit. | No file lost; the execution ran, and `--execute` refuses an ineligible plan. | — |
| REQ-02 | `dev/archive/b7157be6-osd/.jit-container` holds `b7157be6-16d8-4050-834c-e996d4fa27c3`; the issue record state is `archived`. | Marker and state. | — |
| REQ-03 | SHA-256 `5e40581d…5f56` at the live path, at `dev/archive/b7157be6-osd/active/aed96ef9-finite-blocklength-bounds/`, and before execution. The live file keeps owners aed96ef9, 55087229, c7cfd37e, cce5da8c; the b7157be6 reference names the copy. | Live path kept, copy byte-identical. | — |
| REQ-04 | 14/14 dataset files present; `README.md` differs by one link target. Consumer: `schema_1_committed_receipt_remains_readable_with_historical_semantics` in `crates/gf2-sim/tests/osd_campaign_protocol.rs`, which reads `schema1/ebch_osd_awgn.json`. | Dataset in place. | The test verdict; `cargo-ci` runs it. |
| REQ-05 | Live preview: eligible, 0 blockers, 0 move, copy or block entries outside the archive, 0 deletions. [`archive-rerun-results.md`](archive-rerun-results.md), rows b7157be6: run 1 adopts the archived `plan.md`, run 2 publishes, relinks and deletes nothing. | A rerun reports no change. | — |
| REQ-06 | 0 unresolved local links in the Markdown files the `jit:f902240f` commits touch, this record, and the archive directory. | Inline links outside fenced code resolve. | Anchors; code-span paths. |

Test command:
`./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --all-features --cargo-profile ci-test --profile ci -E 'test(schema_1_committed_receipt_remains_readable_with_historical_semantics)'`.

## What the execution changed

| Source | Action | Reason |
| --- | --- | --- |
| 8 files under `dev/archive/b7157be6-osd/active/` | adopted in place | hand archive, identical content |
| `dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md` | copied | four non-terminal owners outside the epic |
| `dev/bench_results/2026-08-27-258be082-osd-campaign-worker-scaling.md` | moved to `dev/archive/b7157be6-osd/bench_results/` | sole owner 258be082, no code consumer |
| 8 references of cef1ae5f into `dev/simulation_results/osd-ebch-128-64/`, 3 references of a82f2dd9 into `dev/reference_data/osd_ebch_128_64_fossorier1994_{access_audit,digitization}/` | retained; references pinned to commit `744aa9b03` | code-consumed or linked from a digest-pinned file (below) |

The planner retains a pinned reference whatever the working-tree content of
its file: the preview reports all eleven as `retain` with evidence
`pinned-historical`. Ten of the eleven files carry in the working tree the blob
they have at `744aa9b03`. `dev/simulation_results/osd-ebch-128-64/README.md`
differs in one line, the target of the receipt link at line 84, changed by
`cae128c4e`; at `744aa9b03` that link resolves to the receipt's source path,
which exists in that tree. The reference keeps its pin.

## Review links in the archived plan

The planner evaluates each archived file at its mirrored source path. A
target `../../../active/aed96ef9-finite-blocklength-bounds/…` in `plan.md`
leaves the repository from `dev/active/plan.md` and blocks the plan with
`unpreservable-layout`. The three review links in `plan.md` take the target
`aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md`, which
resolves at the archive copy and at the mirrored source. No other archived
file links the review; `investigation.md` cites it as a code-span path.

## Citations of the moved receipt

| File:line | Form | Target |
| --- | --- | --- |
| `dev/simulation_results/osd-ebch-128-64/README.md:84` | link | `../../archive/b7157be6-osd/bench_results/…` |
| `dev/active/ae03bcd0-general-bch/investigation.md:276` | code span | archive path |
| `dev/active/fa787f85-documentation-overhaul/investigation.md:48` | code span | archive path |
| `dev/active/fa787f85-documentation-overhaul/perf-evidence-catalog.md:132` | code span | archive path |

Root `README.md`, `AGENTS.md`, `docs/` and `crates/` hold no citation of the
receipt. Two citations keep the source path: the manifest row key, and the
status string at `dev/archive/b7157be6-osd/active/progress.json:208`.

## Manifest rows

| Rows | Disposition | Status | Evidence |
| --- | --- | --- | --- |
| 8 files under `dev/archive/b7157be6-osd/active/` | jit-container-archive, destination equals path | complete | adopted, marker present |
| the worker-scaling receipt | jit-container-archive | complete | source absent, destination hash equals the source hash before execution |
| the external review | retained-operational | complete | REQ-03 |
| `dev/simulation_results/osd-ebch-128-64/README.md`, `schema1/README.md` | retained-operational, empty destination | complete | REQ-04; pinned references of cef1ae5f |
| `osd_ebch_128_64_fossorier1994_access_audit/README.md`, `…/researchgate_lead_identification.md`, `osd_ebch_128_64_fossorier1994_digitization/README.md` | retained-operational, empty destination | complete | root-relative link targets of digest-pinned `osd_ebch_128_64_fossorier1994.md` (lines 61, 69, 212); pinned references of a82f2dd9 |

## Verification

| Command | Result |
| --- | --- |
| `python3 dev/active/fa787f85-documentation-overhaul/f902240f-evidence.py` | exit 0 |
| `python3 contrib/gates/docs-mechanical.py` | PASS |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding on the 15 rows of this unit |
