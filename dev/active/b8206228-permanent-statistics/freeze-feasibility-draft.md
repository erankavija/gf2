# Phase-1 freeze-feasibility inventory and draft manifest

Status: draft. This artifact set supports feasibility review for JIT issue `7a816262`; it is not a final freeze and it does not authorize campaign draws.

## Outcome

The draft enumerates all 63 protocol cells, uses the two fixed N tiers, evaluates the determinant companion on every cell, and parses through the real `CampaignManifest` reader. Exactly three cells are dischargeable now: q=3 has 1, q=5 has 1, and q=7 has 1. The remaining 60 cells carry `generic_ryser` only as a schema-valid placeholder and are listed in the gap document. The protocol's same-cohort remeasurement clause therefore still blocks final freeze.

The selected cells are q3 n=28, q5 n=24, and q7 n=20 from the exact 296a41c9 cohort. The q3 GPU-study n=16,20,24 interior window remains the confounded `fold-gf3` finding in `dev/studies/0dffa759/findings.md` §3.2; §3 records the q3 no-go, so no q3 accelerator configuration is retained. Owner decision (2026-08-17), recorded by the owner: retained `f5-three-plane` and `f7-three-plane-permanent` study-cohort prototypes are not selectable at freeze, and their cells remain gaps settled empirically by premeasurement. Their factual receipt rows remain in the inventory. The selection receipt records the candidate provenance and within-cohort ranking for the three selected groups; exclusion records distinguish rule 1, rule 2, and rule 3 exclusions.

## Draft identity and addressing

The draft campaign id is `permanent-zero-fraction-20260817`. It is date-qualified, valid under `CampaignId`, and names the dataset directory the final campaign uses if the protocol and cell universe remain unchanged. The root seed is `0x7a81626200000001`. Purpose tags use the sampler's reserved namespace: validation=1, timing=2, campaign-cells=3, rare-event=4. Every shard uses the campaign-cell purpose tag 3.

The stream index is allocated as `((cell_ordinal as u64) << 32) | shard_id`, with cells ordered lexicographically by `(q,n)` and `shard_id` starting at zero. The low-56-bit schema limit and the scheduler's shard-id limit hold. Distinct purpose tags occupy separate seed-address domains; distinct cells have distinct `(q,n)` coordinates and distinct high-order stream-index slots; distinct shards have distinct low-order slots. This is the construction claimed for REQ-08, not an after-the-fact collision inspection.

## N, error budget, and determinant plan

The cell counts are the protocol's exact values: q=3 uses 20,000,000 for n=4..20 and 222,223 for n=21..28; q=5 uses 16,000,000 for n=4..16 and 160,000 for n=17..24; q=7 uses 12,244,898 for n=4..16 and 122,449 for n=17..20. The multiplicity is K=63. The permanent-floor and determinant families each use family budget 0.025 and per-cell level 1/2520. The manifest schema has no error-budget fields, so the freeze document binds these protocol values by citation rather than adding unknown JSON fields.

`determinant_companion` is `evaluate` for all 63 cells. The determinant-cost receipt supplies marginal-cost evidence but never acts as a backend ranking receipt.
`dev/benchmarks/permanent_campaign/determinant-cost.md` §Verdict and §Twelve-hour budget projection record no marginal-cost failures; the largest measured addition is 0.014288 h at (q,n)=(3,20), with 0.000328 h at (3,28), 0.000187 h at (5,24), and 0.000098 h at (7,20). The receipt's §Measured ratio and §Twelve-hour budget projection tables record the other measured anchor cells and the fixed-N calculation.

## Integrity and reproducibility

The draft uses the implemented integrity contract: `manifest.json` carries no self-hash field; `provenance::manifest_content_hash` recomputes SHA-256 from the manifest bytes, and `recorded_manifest_hash` reads the `manifest.json` entry from `checksums.sha256`. The draft sidecar records that digest. A final freeze generates the complete raw-data integrity file through `permanent_dataset checksums`, and verification compares the recomputed manifest hash with the sidecar entry. No new hash mechanism is introduced.

The manifest records `rng_algorithm` as the schema token `cha_cha20` (ChaCha20), `rng_version` as `rand_chacha 0.9.0`, and an argument-token invocation. The exact patch-version derivation is `dev/studies/0dffa759/rng-provenance-addendum.md` §4 and `@/citation/RustRandom2025`.

REQ-07 is a freeze rule: adding a cell after this draft requires a new campaign id and a restated multiplicity adjustment; the added cell does not reuse this campaign identity or its 63-cell correction.

## Gap premeasurement totals

The gap list nominates exactly two candidates per gap: 120 configurations and 1440 fresh processes. q7 n=17,18,19 are forced because capability leaves only `accelerator` and `generic_ryser`; the other 57 cells use evidence-based nominations. The b488f02c throughput rows, the q3/q5/q7 GPU-study §4.2 rows, and the 296a41c9 frontier table rows provide nomination basis only. Non-nominated candidates are rule-3-excluded with recorded exclusions; no measurement runs in this draft.

## Validation record

The draft reader validation is recorded in [`validation.md`](validation.md). It invokes the real `read_manifest` through the `gf2-sim` binary test harness and returns success. It does not run a campaign arm, create shards, or run a benchmark.

## Artifact hashes

| Artifact | SHA-256 | Role |
|---|---|---|
| [`manifest.json`](manifest-draft/manifest.json) | `d9bbcd6d8bddd51bab0e232732bb0e16d4710e5dce4ff48932d4879afea00cce` | schema-valid 63-cell draft |
| [`backend-selection-draft.md`](backend-selection-draft.md) | `f009e79b2ae2c4a7ea16415fdc14cd98da9e2e07a2796fde482fdfbd93bc1184` | manifest-bound draft selection receipt |
| [`receipt-inventory.md`](receipt-inventory.md) | `42ab7d118081fcb7956105dec7a2d4a0872119259c6215331af774bef8c41efd` | committed receipt inventory |

See [`receipt-inventory.md`](receipt-inventory.md), [`backend-selection-draft.md`](backend-selection-draft.md), [`backend-exclusions-draft.md`](backend-exclusions-draft.md), and [`gap-list.md`](gap-list.md) for the evidence records. Unverifiable items are the uncommitted scratch raw paths named inside `backend-ordering.csv`, plus any final selection receipt hashes that depend on post-draft edits.
