# Epic completion: Zen 3 CPU performance

> **Diátaxis Type:** Reference

Completion report of `@/issue/1a379447`. Figures live in the [scorecard](../../bench_results/1362381c/scorecard.md); [findings](1362381c/findings.md) states its conclusions. Execution history is in [progress.json](progress.json) and the handoff chain ending at [handoff-44.md](handoff-44.md).

## Outcome

Every hard criterion is delivered and the four epic gates pass. The aspirational fastest-open-source target is met in some primary cells and missed in others; gf2 stays behind on the cells in the [gap register](1362381c/gaps.json), each with a measured limit, a preserved experiment or a tracked follow-up.

## Success criteria

| Criterion | Delivered by |
|---|---|
| REQ-01 protocol, surveys, frozen settings | `@/issue/f547c394` and the family surveys of the [family registry](1362381c/survey/family-registry.json) |
| REQ-02 measured kernel and consumer outcomes | The registry's CLMUL, polynomial, $\mathrm{GF}(2^8)$, popcount, transpose, logical-buffer and shift families |
| REQ-03 steady-state and batch decoding, LDPC checkpoint | `@/issue/3be770d5`, `@/issue/ed3d490e`, `@/issue/f63a2464` |
| REQ-04 canonical tuning and dispatch | `@/issue/63bad95d`, `@/issue/d06a99ed`, confirmed by `@/issue/31727c57` |
| REQ-05 scorecard, reprofile, residual gaps | `@/issue/1362381c`, `@/issue/78f0aafd`, `@/issue/1358d6ab` |
| REQ-06 conformance, MSRV, isolation, determinism | Shared suites in `cargo-ci`; [determinism record](../../bench_results/1362381c/determinism/nextest.log) |
| REQ-07 aspirational target | Reported per primary cell in the scorecard's target column |

## Invoker rulings of the closing session

- `@/issue/31727c57` REQ-02: confirmation holds within the P-20 bound with a confirmed cell at each core configuration.
- `@/issue/101604ef`: a launcher's parsed program is the measuring tool; older-form logs do not resume, and the failed `2037941f/dense-baseline-profile` launch is an accepted exception.
- Host and tracker changes: the `criterion-1-5x` rename, `@/issue/54b34be1`, `@/issue/9ed4b23c` and the one-time record canonicalization on main.

## Remaining limits

- Follow-ups outside the epic: `@/issue/55c3e433` for the AFF3CT cells, `@/issue/040bc2b8`, `@/issue/c9cb0dd2`, `@/issue/be6d6f45`, `@/issue/befccab3`; tracker items `@/issue/bd7a8748`, `@/issue/43c542b4`.
- Ten low-severity holistic-review advisories stay recorded on the gate run, none blocking.
- The determinism suites and receipt re-evaluation ran on rustc 1.97, as their run identity records. Five receipts outside the workload set differ from their committed verdict, each with its recorded cause in the [re-evaluation record](../../bench_results/1362381c/receipt-reevaluation.json).
- docs-mechanical reports broken links in frozen `inputs/protocol.md` snapshots; it is not a gate of this epic and awaits an invoker ruling.
- Eight stale branches need special handling if ever merged after the record canonicalization.
- This directory stays under `dev/active/`: live follow-ups and `@/issue/55c3e433` cite its paths.
