# Preserved failed transpose pilot launch

This exploratory campaign opened its frozen protocol inputs and started the
first `transpose-64-canonical-vs-m4ri` pair. The gf2 baseline child completed,
then the M4RI C child emitted a truncated `GF2_TUNING_RESULT` object because
the shared C harness formatter omitted the final JSON brace. The runner
rejected the child output and appended a terminal `failed` event before any
cell completed.

The directory preserves the exact plan, input snapshots, checkpoint manifest,
launcher log, and append-only execution log. It is not a benchmark receipt and
supports no performance conclusion. The fresh replacement campaign uses a new
campaign identity and executable digest after the formatter fix; none of this
failed launch's timing is reused.
