# LDPC adapter correctness and capability evidence

> **Diátaxis Type:** Reference

Evidence produced before any timed cell of JIT issue `c077a88b`, so that the
throughput arms measure an operation both sides agree on. Nothing here is a
performance receipt.

| File | Producer | Content |
|---|---|---|
| `validation-plan.json` | hand-written, host paths | The arms, the bundles and the frame budget the validator ran. |
| `validation.json` | `survey/harness` `ldpc-validate` | Per-arm BER/FER with Wilson intervals at 95% [Wilson1927], and the per-frame decision agreement of every pair of arms on the information window. |
| `aff3ct-capabilities-nr.json` | `survey/probe-aff3ct-capabilities.py` | For each of 108 AFF3CT decoder combinations, whether the pinned build constructs it, and if so its screened frame error rate and simulated throughput. |
| `xdsopl-code-identity.json` | `survey/verify-xdsopl-code.py` | Whether the xdsopl DVB-T2 accumulator table rebuilds the parity-check matrix the gf2 exporter writes. |
| `source-evidence.json` | `survey/inspect-comparators.py` | The pinned file, line and literal text behind every comparator capability claim in `findings.md`. |
| `host.txt` | `run-campaign.sh` conventions | The host, compilers and pinned external commits at the time these artifacts were produced. |

The headline results are read in
[`dev/active/c077a88b/findings.md`](../../../active/c077a88b/findings.md).
Two of them are worth naming here: the gf2 and AFF3CT matched arms agree on
128 of 128 DVB-T2 frames with zero differing bits, and the xdsopl DVB-T2 table
rebuilds the identical 226799-non-zero parity-check matrix column for column.
