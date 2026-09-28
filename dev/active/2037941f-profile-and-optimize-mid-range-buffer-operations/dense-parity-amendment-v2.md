# Dense-parity addendum amendment v2

> **Diátaxis Type:** Reference
>
> **Status:** approved 2026-09-28
>
> **Addendum identity:** `2037941f-dense-parity-v2`

The [v1 addendum](dense-parity-addendum-v1.md) is preserved with its original
bytes for existing receipts. This amendment changes only the representation of
the three predeclared unqualified M4RI shapes in the
[canonical addendum](dense-parity-addendum.md). The approved replacement for
the v1 comparator paragraph is:

> Three anchor strides have no qualified M4RI arm. Each M4RI receipt directory
> contains a generated `unavailable-rows.tsv` companion that records their
> cell identifiers, strides, `unavailable` status, reason, zero samples and
> zero comparisons. These rows are absent from the runner plan and
> `receipt.json`; they execute no arm and spend no comparison. The companion
> cites the frozen addendum's SHA-256 and is projected from the harness's
> frozen unavailable-row table:

The [canonical projection](survey/dense-harness/src/bin/dense-campaign.rs)
validates the family addendum and reads the
[frozen unavailable-row table](survey/dense-harness/src/cells.rs). The pilot
launcher writes the projection beside its receipt; a freezer-derived
confirmation invokes the same projection with its derived addendum and its
receipt directory. Neither path declares a timing cell for these rows.

The eight qualified M4RI pilot cells, exact 24 paired executions per measured
exploratory cell, 24 fresh pairs per confirmation, fixed 0.040 resolution
ceiling, separate ledger, and P-20 comparison budget remain as frozen in the
canonical addendum. No protocol or receipt schema changes.
