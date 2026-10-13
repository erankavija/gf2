# Matvec threshold calibration: corrections to the premeasurement protocol

> **Diátaxis Type:** Reference

The receipt of campaign `gf2-63bad95d-20261006t230547z-2073085` pins
[`premeasurement-protocol.md`](premeasurement-protocol.md) by SHA-256
(`dev/benchmarks/tuning_profiles/gf2-63bad95d-20261006t230547z-2073085-evidence.md`,
row *Protocol*), so that document keeps its bytes. This note states where the
current state differs from its text.

| Protocol text | Current state |
|---|---|
| Status line: "declared; no campaign of this declaration has run" | The campaign named above executed the declaration; its receipt and validator record are listed in [`campaign-findings.md`](campaign-findings.md). |
| §8: the baked constant changes "after the protocol confirmation of `4337c02e`" | The confirmation receipt of `4337c02e` is rejected by acceptance and installs no value. The published owner states the conservative value, and the baked constant equals it ([`matvec-sweep-design.md`](matvec-sweep-design.md#4-relation-to-the-protocol-confirmation)). |
