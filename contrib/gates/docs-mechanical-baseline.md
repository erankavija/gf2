# docs-mechanical baseline findings

The `baseline` key of `contrib/gates/docs-mechanical.toml` names this
record, and the check suppresses exactly its rows. Each row of the first
table quotes an unresolvable address on purpose (8f61d6de DEC-01).

| Location | Class | Target | Reason |
|---|---|---|---|
| `dev/active/1a379447-zen3-cpu-performance/9f0e385e/coverage-addressability.md:19` | `unresolved-item` | `@/issue/c04dd4ac/requirement/REQ-06` | Example of an address that does not resolve. |

Each row of the second table is a relative link in a frozen addendum or
comparator specification of the mid-range entry. A harness constant, a
validation record or a receipt holds the SHA-256 of the file,
and the link text counts directory levels from the flat entry location
(record of @/issue/f79bcba3, section "Frozen documents").

| Location | Class | Target |
|---|---|---|
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:13` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:112` | `broken-link` | `../../../crates/gf2-core/src/matrix.asm.txt` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:539` | `broken-link` | `../../bench_results/5cbb6545/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:557` | `broken-link` | `../../bench_results/04b85d10/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:17` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:116` | `broken-link` | `../../../crates/gf2-core/src/matrix.asm.txt` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:549` | `broken-link` | `../../bench_results/5cbb6545/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:567` | `broken-link` | `../../bench_results/04b85d10/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-operation-match.md:59` | `broken-link` | `../../../crates/gf2-core/src/matrix.rs` |

Each row of the third table is a relative link in a protocol document of the
`f547c394` entry that leaves the entry. A committed receipt holds the SHA-256
of the file, and the link text counts directory levels from the flat entry
location (record of @/issue/602652bf, section "Frozen documents").

| Location | Class | Target |
|---|---|---|
| `dev/active/1a379447-zen3-cpu-performance/f547c394/amendment-v4.md:31` | `broken-link` | `../../bench_results/26465e6c/v3-and-popcnt-pilot/acceptance-summary.json` |
| `dev/active/1a379447-zen3-cpu-performance/f547c394/protocol.md:5` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |

Each row of the fourth table is a relative link in the frozen lane-threshold
addendum of the `4337c02e` entry. A harness constant and the family's receipts
hold the SHA-256 of the file, and the link text counts directory levels to the
flat location of the entries it names.

| Location | Class | Target |
|---|---|---|
| `dev/active/1a379447-zen3-cpu-performance/4337c02e/matvec-lane-threshold-addendum.md:16` | `broken-link` | `../../f547c394/protocol.md` |
| `dev/active/1a379447-zen3-cpu-performance/4337c02e/matvec-lane-threshold-addendum.md:17` | `broken-link` | `../../f547c394/amendment-v4.md` |
| `dev/active/1a379447-zen3-cpu-performance/4337c02e/matvec-lane-threshold-addendum.md:19` | `broken-link` | `../../f547c394/addendum.schema.json` |
| `dev/active/1a379447-zen3-cpu-performance/4337c02e/matvec-lane-threshold-addendum.md:25` | `broken-link` | `../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md` |
| `dev/active/1a379447-zen3-cpu-performance/4337c02e/matvec-lane-threshold-addendum.md:28` | `broken-link` | `../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-harness.md` |
| `dev/active/1a379447-zen3-cpu-performance/4337c02e/matvec-lane-threshold-addendum.md:40` | `broken-link` | `../../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-tables.md` |

Each row of the fifth table is a relative link in a file that keeps its bytes:
the pre-execution record of @/issue/616e1d7c holds the SHA-256 of each of the three files and the
archive holds an equal copy of each. The link text counts directory levels from
the flat location of the a83583e0 entry (record of @/issue/f694f59a).

| Location | Class | Target |
|---|---|---|
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:5` | `broken-link` | `../../benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:6` | `broken-link` | `../../benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298-evidence.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:10` | `broken-link` | `../7d824b2f/design.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:12` | `broken-link` | `../3fa7c9d0/design.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:14` | `broken-link` | `../eaae1b56/premeasurement-protocol.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:15` | `broken-link` | `../../benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` |
| `dev/active/7d824b2f/design.md:933` | `broken-link` | `../a83583e0/premeasurement-protocol.md` |
| `dev/active/7d824b2f/design.md:990` | `broken-link` | `../a83583e0/premeasurement-protocol.md` |
| `dev/active/7d824b2f/design.md:1044` | `broken-link` | `../a83583e0/premeasurement-protocol.md` |
| `dev/active/dbd8787d/premeasurement-protocol.md:10` | `broken-link` | `../a83583e0/premeasurement-protocol.md` |

Each row of the sixth table is a link or an address in a protocol snapshot
under `inputs/producing/`: the receipt of the campaign states the SHA-256 of
the snapshot, and `dev/scripts/validate-tuning-extent-campaign.py` reads the
protocol under that digest. The link text counts directory levels from the
protocol's own location.

| Location | Class | Target |
|---|---|---|
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/inputs/producing/dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:9` | `broken-link` | `../7d824b2f/design.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/inputs/producing/dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:11` | `broken-link` | `../3fa7c9d0/design.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/inputs/producing/dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:13` | `broken-link` | `../eaae1b56/premeasurement-protocol.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/inputs/producing/dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:14` | `broken-link` | `../../benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` |
| `dev/active/6dc81018-field-capability-dispatch/a83583e0/inputs/producing/dev/active/6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md:584` | `unresolved-item` | `@/inv/campaign-execution-logging` |
| `dev/active/dbd8787d/inputs/producing/dev/active/dbd8787d/premeasurement-protocol.md:7` | `broken-link` | `../a83583e0/premeasurement-protocol.md` |
| `dev/active/dbd8787d/inputs/producing/dev/active/dbd8787d/premeasurement-protocol.md:10` | `broken-link` | `../7d7c647c/design.md` |
| `dev/active/dbd8787d/inputs/producing/dev/active/dbd8787d/premeasurement-protocol.md:11` | `broken-link` | `../7d824b2f/design.md` |
| `dev/active/dbd8787d/inputs/producing/dev/active/dbd8787d/premeasurement-protocol.md:25` | `broken-link` | `producing-build-inputs.json` |
| `dev/active/dbd8787d/inputs/producing/dev/active/dbd8787d/premeasurement-protocol.md:168` | `broken-link` | `campaign-declaration.json` |
