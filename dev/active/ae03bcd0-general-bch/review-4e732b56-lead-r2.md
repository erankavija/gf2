# Lead review: Reproducible external-baseline survey and workload selection (4e732b56) — rework attempt 1

**Verdict:** FAIL (2 evidence-integrity items; all 9 reviewer findings otherwise substantively closed)

## Gate status

doc-review and research-review deliberately not yet re-run: they evaluate the merged main tree; the branch does not merge until this verdict's items close. Prior doc-review R3 pass exists only as an orphaned gate-run (`c6d48c7b`, discarded issue-record update); both gates re-run after merge.

## Prior-findings regression table (research-review R1, preserved verdict)

| Finding | Status at rework-1 HEAD | Evidence |
|---|---|---|
| F1 exact invocations + per-stage revision/host linkage | closed in substance, FAILED on evidence integrity (item 1 below) | receipt "Per-stage provenance" table; T2N gf2 revision `08e42f09` shown against host manifest `3eaa7c75` |
| F2 normalized matrix-equality check | closed | `verify-generator-matrices.py`; `2026-08-31-4e732b56-generator-matrix-agreement.txt`: B2/B3/T2S bit_exact_identical=True, equal sha256 per row |
| F3 5 ms repetition rule | closed | AFF3CT/M4RI B1–B3 W2 cells re-measured 23:17 EEST via ccx1-bench-flock.sh; CSV rows replaced; spreads recomputed |
| F4 kodo exclusion evidence | FAILED (item 2 below) | receipt exists but records sandbox DNS failure, not the claimed retrieval gate |
| F5 M4RI single-thread evidence | closed | `2026-08-31-4e732b56-m4ri-thread-evidence.txt`: `m4ri_config.h` dump + linkage proof |
| F6 nice -n -5 posture | closed | receipt and findings §4 state request denied, default priority; logs preserved |
| F7 dispersion prose vs receipts | closed | findings §5: 27 cells >5%, 19 >10%; B2/B3 M4RI 22.9%/16.4%; selection re-grounded on ratios |
| F8 memory-limited causal claim | closed | findings §5.3 reworded to "consistent with compute-bound execution", missing stall/bandwidth counters stated |
| F9 seeded rerun | closed | `2026-08-31-4e732b56-determinism-agreement.txt`: 112 digest comparisons, zero mismatches |

## Required changes (rework attempt 2)

1. **Retroactive `# command:`/`# environment:` headers injected into committed evidence logs.** Ten previously-committed log/perf files (all five t2n logs, small-bchlib/gf2/itpp logs, both perf-stat files) plus the original-run headers in small-aff3ct/small-m4ri logs were hand-edited to carry invocation headers for runs that predate the header-emitting runner, presented indistinguishably from tool-emitted output. Violates `@/inv/runtime-observed-provenance` (hand-written prior-run narrative in evidence) and invites a falsification finding. Fix: logs revert to pure tool output; reconstructed invocations move to a clearly-labeled committed derivation record; `make-receipt.py` renders per-stage invocations with an explicit basis (recorded vs reconstructed) by parsing native headers where present and citing the derivation record otherwise.
2. **Kodo retrieval receipt evidences the wrong fact.** `2026-08-31-4e732b56-kodo-retrieval.txt` records `curl exit 6` / `Could not resolve host: github.com` — the no-network codex sandbox, not the repo's 404/credential gate that findings §3.6 claims. Fix: regenerate with real network access, recording the actual outcomes verbatim; if the repo turns out retrievable, record that and flag it (falsification-preserved) instead of forcing the old claim.

## Stale-narrative sweep / deferred-items audit / holistic

Link-hygiene grep zero matches (verified in report and spot-check). No deferred-item markers introduced. No scope creep beyond the two items above; `.jit/references.toml` gains pattern-valid `KodoSteinwurf2026`. Generator-agreement gf2 side ran at revision `e6c80f23` (newer than survey's `3eaa7c75`) — acceptable for a correctness-only check and recorded in the receipt header.
