# Approved proposal: M4RI predeclared unavailable-row recording

> **Diátaxis Type:** Explanation
>
> **Status:** approved 2026-09-28; incorporated in the
> [versioned amendment](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-amendment-v2.md).

## Conflict and narrow replacement

The [v1 addendum](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum-v1.md#m4ri-comparator)
states:

> Three anchor strides have no M4RI arm. Their rows are retained in every
> receipt as `unavailable` with a reason and zero samples, and because they carry
> the `exploratory` role they spend no comparison:

The strict [protocol-v4 cell declaration and plan](../../tools/tuning-campaign-support/src/protocol.rs)
have no field for a predeclared unavailable shape. The
[runner](../../tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs)
records `unavailable` only for an inapplicable runtime core arm. The frozen
comparator uses the applicable single-core arm, while an unqualified shape is
rejected by the [case decoder](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness/src/wire.rs).
Declaring a false core arm or an unqualified timed case would change the
measurement contract.

Approved replacement for that paragraph:

> Three anchor strides have no qualified M4RI arm. Each M4RI receipt directory
> contains a generated `unavailable-rows.tsv` companion that records their
> cell identifiers, strides, `unavailable` status, reason, zero samples and
> zero comparisons. These rows are absent from the runner plan and
> `receipt.json`; they execute no arm and spend no comparison. The companion
> cites the frozen addendum's SHA-256 and is projected from the harness's
> frozen unavailable-row table:

This replacement changes only where the predeclared exclusions are recorded.
The [campaign JSON](../2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/dense-matvec-vs-m4ri.json)
keeps its qualified fresh and retained cells, seed/order, exploratory roles,
and exact operations. The [frozen sampling and effect rules](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md#cache-warmup-and-sampling)
still require 24 pilot pairs per measured exploratory cell, 24 fresh pairs per
confirmation, and the fixed M4RI resolution ceiling of 0.040. No unavailable
row is counted as a comparison or used for resolution.

## Prepared implementation

The [canonical dense campaign tool](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness/src/bin/dense-campaign.rs)
has an `unavailable` projection. It validates the family addendum
through the canonical protocol model, checks the M4RI family and frozen prose
digest, and rejects overlap with any measured cell. It then renders tab-separated rows with
`cell_id`, `stride_words`, `status`, `samples`, `comparisons`, and `reason`.
Its header names the family and addendum digest. The
[generated example](unavailable-rows.example.tsv) shows the exact output. The
[existing launcher](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-dense-harness.sh)
places the projection beside `receipt.json` after pilot finalization. The generator
derives rows from the existing `UNAVAILABLE_ROWS` constants; it creates no
new qualification, arm, timing cell, or statistical rule. The queue entry
remains under lead control until the committed preparation passes review.

A later confirmation uses the same command with its freezer-derived addendum
and output path in that confirmation's receipt directory:

```text
target/e1f9a78f-arms/release/dense-campaign unavailable \
  --family 2037941f-dense-matvec-vs-m4ri \
  --addendum <derived-confirmation-addendum.json> \
  --output <confirmation-receipt-dir>/unavailable-rows.tsv
```

The projection accepts the confirmation's changed roles, resolution evidence,
and family description after ordinary schema validation; it does not require
the pilot's exact transcription. The confirmation still follows the canonical
[freezer](../c7113c5a/survey/freeze-confirmation.py) and runner checks.

The approved amendment is in the comparator receipt's
[producing-input snapshot](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-producing-inputs.json).
The frozen v1 bytes and prior receipts remain unchanged.
