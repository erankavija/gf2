# docs-mechanical baseline findings

The `baseline` key of `[docs-mechanical]` in `.jit/config.toml` names this
record, and the check suppresses exactly its rows. Each row of the first
table quotes an unresolvable address on purpose (8f61d6de DEC-01).

| Location | Class | Target | Reason |
|---|---|---|---|
| `dev/archive/6dc81018-field-capability-dispatch/active/6dc81018-field-capability-dispatch/handoff-4.md:31` | `unresolved-item` | `@/inv/canonical-abstraction` | Records the dangling citation a code review rejected. |
| `dev/archive/6dc81018-field-capability-dispatch/active/6dc81018-field-capability-dispatch/handoff-4.md:54` | `unresolved-item` | `@/inv/canonical-abstraction` | Names the dangling citation as a trap to avoid. |
| `dev/active/1a379447-zen3-cpu-performance/9f0e385e/coverage-addressability.md:19` | `unresolved-item` | `@/issue/c04dd4ac/requirement/REQ-06` | Example of an address that does not resolve. |

Each row of the second table is a relative link in a frozen addendum or
comparator specification of the mid-range entry. A harness constant, a
receipt or a record of the receipt directory holds the SHA-256 of the file,
and the link text counts directory levels from the flat entry location
(`f79bcba3-zen3-move.md`, section "Frozen documents").

| Location | Class | Target |
|---|---|---|
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:13` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:112` | `broken-link` | `../../../crates/gf2-core/src/matrix.asm.txt` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:536` | `broken-link` | `../5cbb6545/findings.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:539` | `broken-link` | `../../bench_results/5cbb6545/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md:557` | `broken-link` | `../../bench_results/04b85d10/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:17` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:116` | `broken-link` | `../../../crates/gf2-core/src/matrix.asm.txt` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:546` | `broken-link` | `../5cbb6545/findings.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:549` | `broken-link` | `../../bench_results/5cbb6545/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md:567` | `broken-link` | `../../bench_results/04b85d10/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md:13` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md:341` | `broken-link` | `../../bench_results/04b85d10/tables.md` |
| `dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-operation-match.md:59` | `broken-link` | `../../../crates/gf2-core/src/matrix.rs` |

Each row of the third table is a relative link in a protocol document of the
`f547c394` entry that leaves the entry. A committed receipt holds the SHA-256
of the file, and the link text counts directory levels from the flat entry
location (`602652bf-protocol-inputs-move.md`, section "Frozen documents").

| Location | Class | Target |
|---|---|---|
| `dev/active/1a379447-zen3-cpu-performance/f547c394/amendment-v4.md:31` | `broken-link` | `../../bench_results/26465e6c/v3-and-popcnt-pilot/acceptance-summary.json` |
| `dev/active/1a379447-zen3-cpu-performance/f547c394/protocol-v1.md:5` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
| `dev/active/1a379447-zen3-cpu-performance/f547c394/protocol.md:5` | `broken-link` | `../1a379447-zen3-cpu-performance/measurement-contract.md` |
