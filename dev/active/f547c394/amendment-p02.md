# Protocol P-02 amendment record

[protocol.md](protocol.md) is the normative Zen 3 benchmark protocol. This
record supersedes the Check text of acceptance rule P-02 in every edition of
the protocol document: [protocol.md](protocol.md),
[protocol-v1.md](protocol-v1.md) and each receipt-local protocol snapshot.
Receipts pin those documents by digest, so the documents keep their bytes and
this record carries the rule. The record defines no protocol version: a
receipt is evaluated under the version it pins.

## Rule

| Rule | Check |
|---|---|
| P-02 | The protocol, contract and addendum-schema pins each carry a literal relative source path, a literal relative receipt-local snapshot path and a 64-hex digest. The snapshot bytes hash to the pinned digest, and the bytes themselves declare an edition of the pinned document kind: a protocol version, the measurement contract, or an addendum schema of the family. A missing snapshot, a digest mismatch and bytes that declare no such edition each reject. The source path is informational provenance: acceptance compares it with no expected path. |

## Enforcement

- `evaluate_version` in
  [receipt.rs](../../tools/tuning-campaign-support/src/receipt.rs) applies
  P-02 to the three shared pins of every receipt, whichever protocol version
  the receipt pins.
- `ArtifactPin::validate_shape` and `ArtifactPin::verify_content` in
  [protocol.rs](../../tools/tuning-campaign-support/src/protocol.rs) check the
  path syntax, the digest syntax and the snapshot digest.
- `SharedInput::identity` in the same file reads the edition a document's
  bytes declare.
- `acceptance_identifies_shared_pins_by_digest_not_by_source_path` in
  [protocol_contracts.rs](../../tools/tuning-campaign-support/tests/protocol_contracts.rs)
  accepts pins whose source path differs from the live document's path and
  rejects a pin whose digest identifies another document kind; the
  neighbouring P-02 tests reject a missing snapshot and a digest mismatch.

## Verdicts

This record is text: the evaluator reads no amendment record.
[The verdict record](../fa787f85-documentation-overhaul/5a25717c-verdict-baseline.md)
lists the verdict and findings of every committed
`zen3-benchmark-receipt-v1` receipt and the command that reproduces the
listing.
