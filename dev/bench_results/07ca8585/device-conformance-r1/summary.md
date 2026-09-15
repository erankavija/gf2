# LDPC gfx1030 device conformance (jit:07ca8585)

> **Diátaxis Type:** Reference

This correctness receipt validates the current HIP min-sum check update on the
AMD Radeon RX 6950 XT (`gfx1030`). It interprets no runtime duration as a performance
measurement. Source and executable content identities are in `receipt.json`.
The portable producing-input closure is under `inputs/producing/`.

## Verdict

- The device check-update kernel matches `min_sum_check_row` bit for bit for
  MinSum, NormalizedMinSum(0.75), and OffsetMinSum(0.5).
- The device matrix covers positive zero, negative zero, positive-sign NaN,
  negative-sign NaN, an all-NaN excluded set, and NaN with negative zero.
- The five ignored ordinary DVB-T2/5G NR correctness legs pass.
- The unignored 5G NR smoke correctness leg passes.

The raw transcript is `execution.log` (SHA-256 `1eaa040c296e080d9db52548cd2825ec3cb037cd5b86d804f7a8b456a106e09d`). Per-command
output, toolchain reports, runtime `rocminfo`, and nextest executable manifests
are under `raw/`; their digests are recorded in `receipt.json`.
