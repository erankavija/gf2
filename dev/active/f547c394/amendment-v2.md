# Protocol v2 amendment record

Protocol v2 amends the v1 acceptance semantics following research review of
`259c1b9a`. [protocol.md](protocol.md) is the normative specification.

- P-23 binds all declared material host conditions across sessions and retains
  every session observation. Informational time, load and free memory can vary.
- P-22 reserves attempts in an independent append-only predecessor chain before
  measurements, retaining interrupted and failed attempts in correction.
- P-17 defines cold as first-use workload without calibration, with frozen
  positive calls; it claims no hardware cache or frequency reset.
- P-18 carries runner-produced quality, binds frame/bit denominators and BER
  points, and treats frames as independent for BER uncertainty.
- P-19 certifies FER non-inferiority using a bound on paired frame indicators.
- P-20 flags outliers within executions and checks numerical bootstrap endpoint
  resolution. Timing non-regression is described as one-sided non-inferiority.

V1 receipts remain v1 evidence, including their limitations. The v1 pilot's
addendum declares `2026-09-07T00:00:00Z`, after its launch at
`2026-09-06T23:07:25Z`. That human-readable timestamp is contradictory metadata;
the preserved opening record and exact snapshot establish the bytes used before
measurement under the v1 content-freeze rule. It cannot establish the declared
calendar freeze time. No published receipt, log, snapshot or summary is edited.
The v2 launcher writes actual UTC freeze times before starting either run, and
acceptance rejects a v2 timestamp later than its opening journal record.

The v1 receipt-local protocol snapshots and the pre-amendment active v1 document
have different digests despite sharing version 1; earlier acceptance changes
were not versioned correctly. The immutable v1 evidence retains exactly those
pinned bytes. `protocol-v1.md` and `addendum-v1.schema.json` archive the active
pre-amendment text for the v1 compatibility fixtures; they do not replace the
receipt snapshots. The v2 branch in the shared evaluator has explicit version
checks against pinned documents and addenda. A caller cannot evaluate a v1 pin
under v2 rules or the reverse. This named version-1 evaluation boundary remains
while committed v1 evidence must be reproducible; it is not a producer default.

The v2 smoke launcher's synthetic XOR-fold timing and deterministic decoder
quality slots are pipeline proofs. They support no performance or coding-quality
claim about gf2 or an external decoder.

V2 descriptive clarifications distinguish nominal timing targets from duration
caps, fixture-bank rotation from established cache eviction, and outlier flags
from a causal diagnosis. They change no numerical setting or acceptance rule.
The published pilot’s earlier v2 document bytes remain pinned and immutable;
[findings.md](findings.md) records the clarification’s scope.
The same clarification makes explicit that unstable/inconclusive outcomes do
not override the already-frozen one-attempt cap. No extra confirmation is
authorized by an outcome label. The cap is enforced by the same ledger code in
both v2 receipts. Frozen document snapshots retain their original wording.
