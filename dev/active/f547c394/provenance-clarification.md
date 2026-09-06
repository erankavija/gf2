# Provenance review clarification for f547c394

## Authority

The invoker states on 2026-09-07: “git commit hash shall never be critical
provenance.” Parallel sessions and repository-owned JIT records change commit
identities independently of measurement behavior. The invoker also explicitly
requests this as a project invariant. The canonical rule is
`@/inv/behavioral-evidence-validity`, projected from `.jit/invariants.toml` into
`AGENTS.md`; the [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
applies it to this epic.

## Review resolution

The round-1 review is preserved at
[the original verdict](../1a379447-zen3-cpu-performance/reviews/f547c394-r1.md).
Its Git-object-availability and Git-ancestry requirements are superseded by the
invoker's explicit clarification. This is an authorized contract amendment,
not a passing verdict on the incomplete implementation.

- F1 requires verification of the exact pinned artifact content. Missing bytes
  and content-digest mismatches reject evidence. Git objects are optional
  retrieval aids; their absence is not a rejection condition when the pinned
  bytes are available and verified.
- F2 requires an immutable content-pinned addendum recorded in the append-only
  execution log before the first measurement event, and a distinct pilot receipt
  identified by content digest. A commit string or ancestry relation is not
  evidence of freezing. Self-referential and missing pilot evidence remain errors.
- Acceptance and checkpoint/resume compare canonical producing-content,
  executable, configuration and input identities. JIT-only commits, unrelated
  edits and repository-wide dirty state do not invalidate these identities.
- Both consumers of shared provenance machinery use its canonical abstraction.
  Validation includes resumed runs across metadata-only commits and detection
  of changed producing inputs or corrupted snapshots.

## Verification status

The implementation stores and verifies receipt-local snapshots for the shared
protocol, contract, schema, family addendum, producing-input closure and pilot
evidence. Shared checkpoint resume compares producing content while preserving
informational source-control locators. The tuning-campaign-support suite passes
147 tests, including independent checkpoint provenance tests and receipt tests
for missing or changed snapshots, metadata-independent acceptance, premeasurement
addendum freezing and distinct pilot evidence.

The durable pilot receipt at
`dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/receipt.json`
has SHA-256
`6ff1d8697eb7846b98702302cd258a1ccc72c0ab9575fc8f0bf74401310757bf`;
its acceptance summary reports zero findings across two resumed sessions. The
separately frozen confirmatory addendum cites that digest. Its confirmatory
receipt at
`dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/receipt.json`
has SHA-256
`e87a36d0358c3583e35125e53b9ae481a9ec6afa21949a64108b081314d35344`
and is accepted with zero findings across two resumed sessions. It preserves
the unavailable 12-core cell observed under the six-core CCX1 affinity. The
current acceptance CLI accepts exactly one receipt directory and preserves the
same evaluation and output semantics as the source captured in both receipts;
clean exports of both committed trees revalidate under the current release
binary. The configured issue gates remain the execution lead's completion step.
This is the first rework attempt under the clarified contract.
