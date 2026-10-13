# Decode selector calibration: corrections to the premeasurement protocol

> **Diátaxis Type:** Reference

The receipt of campaign `gf2-0a77ecfc-20261013t011147z-815998` pins
[`premeasurement-protocol.md`](premeasurement-protocol.md) by SHA-256
(`dev/benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998-evidence.md`,
row *Protocol*), so that document keeps its bytes. This note states where the
current state differs from its text.

| Protocol text | Current state |
|---|---|
| Status line: "declared; no campaign of this declaration has run" | The campaign named above executed the declaration; its receipt, verification record and validator record are listed in [`campaign-findings.md`](campaign-findings.md). |
| §1: `producing-build-inputs.json` "names the campaign's behaviour, lifecycle and build inputs" | The campaign pins the manifest bytes committed at [`inputs/producing/…/producing-build-inputs.json`](inputs/producing/dev/active/ed3d490e-zen3-ldpc-frame-simd/calibration/producing-build-inputs.json), whose digest is the `producing_manifest` entry of [`campaign-verification.json`](campaign-verification.json). The live manifest and `producing-build-inputs.sha256` follow the current tree. |
