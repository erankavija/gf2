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

The invariant, measurement contract and issue criteria record this direction.
Implementation rework and all configured gates remain required before completion.
The rework attempt remains the first retry under the clarified contract.
