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

The implementation verifies receipt-local snapshots for the protocol, contract,
schema, addendum, producing-input closure and pilot evidence. The complete
`receipt::CampaignFacts` representation binds each receipt to the opening
campaign record, with the same semantic comparison used by runner resume.
Acceptance validates exact saved-plan bytes and their cell/arm connections,
and uses read-only shared checkpoint inspection. Source-control locators remain
informational.

[Revalidation evidence](../../bench_results/f547c394/revalidation.json) records
the release acceptance executable digest, its source/build input identities,
toolchain, commands, results and unchanged raw-evidence digests. It includes
results from both committed receipt directories and their clean Git exports,
plus the preceding acceptance summaries for comparison. The
[pilot](../../bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/receipt.json)
and [confirmation](../../bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/receipt.json)
carry all required opening facts; neither requires regeneration. The frozen
logs, plans, receipts, snapshots and raw samples retain their original bytes.
The pilot supplies exploratory evidence only and does not qualify for
production selection. Confirmation retains every negative and unavailable
outcome, including the cell requiring twelve physical cores.

These receipts check the protocol pipeline and make no gf2 performance claim.
The [rework validation report](rework-validation.md) records behavioral tests,
MSRV validation, CI results, cumulative finding resolutions and the raw audits.
Tracker state and formal review decisions belong to the execution lead.
