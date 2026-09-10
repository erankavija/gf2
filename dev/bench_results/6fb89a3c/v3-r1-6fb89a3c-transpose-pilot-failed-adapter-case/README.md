# Failed protocol-v3 transpose pilot `6fb89a3c-v3-r1-transpose-pilot`

This directory is negative protocol evidence, not a benchmark receipt. The
campaign opened its frozen inputs, reserved ledger sequence 0 of
`v3-transpose-family-ledger.jsonl` (an exploratory reservation, zero
comparisons) and completed five cells over three checkpointed sessions. On
the sixth cell, `transpose-consumer-64-vs-bitshuffle-adapter`, the gf2
baseline child rejected the case it was handed: the runner forwards one case
object to both arms, the adapter cells carry `"adapter": "padded"` for the
Bitshuffle arm, and the gf2 arm's strict case decoder had no such field
(`data did not match any variant of untagged enum Case`, exit 2). The runner
appended a terminal `failed` record without starting a pair.

The fix makes the gf2 transpose arm accept and ignore the field. That changes
the executable digest bound into the campaign's resume identity, so this
identity cannot be resumed and the family's pilot ran again as
`6fb89a3c-v3-r2-transpose-pilot` with fresh samples. No sample, interval or
checkpoint from this directory is used as resolution evidence or in the
findings; the five completed cells remain here only so the ledger's first
reservation has its durable execution record.

Contents: the exact plan, the immutable input snapshots the runner captured,
the checkpoint manifest and the five accepted units, the append-only execution
log and the launcher log.
