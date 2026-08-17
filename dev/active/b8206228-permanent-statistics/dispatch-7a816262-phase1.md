# JIT issue 7a816262, phase 1 — freeze-feasibility inventory and draft manifest

You are a research worker in /home/vkaskivuo/Projects/gf2 (work from its
root). Do not stage, do not commit, never touch `.jit/` or run state-mutating
`jit` commands — the lead reviews and commits. Read AGENTS.md and
`.jit/reference/content-standards.md` first.

## Context

Issue 7a816262 freezes the campaign's root manifest. Its criteria (REQ-01..08)
are in the issue description; the controlling contract is
`dev/simulation_results/permanent-zero-fraction/protocol.md` — read it IN FULL,
especially "Frozen cell universe and sample sizes" (63 cells, two N tiers),
"Global error budget" (1/2520 per-cell levels), "Backend freeze" (per-cell
`backend_receipt` = ArtifactIdentity of a committed selection receipt binding
hashed raw timing receipts, cohort comparability rules, and the three-step
selection order), "Reproducibility identity", and "Shard validity".

The manifest must deserialize through
`crates/gf2-sim/src/permanent_campaign/schema.rs::read_manifest`
(`CampaignManifest`/`CellSpec`/`ShardSpec`/`StreamPurpose`/`Provenance`,
serde `deny_unknown_fields`; read the validation functions for every
constraint, including shard/stream-index rules). The integrity mechanism is in
`provenance.rs` (`MANIFEST_FILE`, `computed_manifest_hash`,
`recorded_manifest_hash`, `INTEGRITY_FILE`) — REQ-06's recomputable hash is
already the implemented contract; describe how the freeze uses it rather than
inventing a new one.

## Phase-1 deliverables (this dispatch produces documents and a draft, no
final freeze)

1. **Receipt inventory against the protocol's cohort rules.** Enumerate every
   committed measurement receipt relevant to backend selection:
   `dev/benchmarks/permanent_campaign/backend-ordering.{md,csv}` (296a41c9;
   four configurations, twelve processes each),
   `dev/studies/b488f02c/envelope-2026-08-07.csv` and the other b488f02c
   artifacts, the determinant timing receipt from jit 8cb4def5 (find it via
   its commits: `git log --oneline --grep 8cb4def5`), and the GPU story
   receipts under `dev/studies/{047b62ed,91605d4d,6c7fcb38}/`. For EVERY one
   of the 63 cells, state which backends have cell-applicable,
   cohort-comparable committed evidence under the protocol's rules
   (host/build/workload/timing comparability), which backends are excluded by
   rules 1-2 (behavioural-suite passage from the existing shared suites and
   equivalence receipts; safety/capability conditions), and whether rule 3's
   ranking or the only-one-eligible exclusion path applies. Be exact about
   what the protocol's cohort discipline does NOT allow you to compare.
2. **Gap list.** Cells (if any) where no backend can be selected from
   committed evidence — i.e. where the protocol's "cannot freeze until a
   same-cohort remeasurement is committed" clause bites. For each, state the
   minimal premeasurement that would close it (configurations, process
   counts, stopping rule) in the 296a41c9 pattern. Do NOT run any
   measurement: bench execution is lead-scheduled after 02:00 local.
3. **Draft manifest.** Write a complete draft `manifest.json` (all 63 cells,
   protocol N values, shard sizes and ShardSpec stream indices satisfying the
   schema's disjointness-by-construction and REQ-08, stream purposes with
   tags, determinant_companion Evaluate on all cells per the protocol,
   provenance fields with the RNG identity — algorithm ChaCha20, crate
   rand_chacha, resolved version 0.9.0 per the registry derivation in
   `dev/studies/0dffa759/rng-provenance-addendum.md` §4 and
   `@/citation/RustRandom2025`). Backends: fill cells whose selection is
   dischargeable now; mark undischargeable cells explicitly in the
   accompanying document (the draft may carry a placeholder backend but the
   document must list them). Choose and justify the campaign id per the issue
   Notes. Validate the draft by a recorded command that parses it through the
   real reader (e.g. a `cargo run`/test invocation you name and run), and
   record the invocation and outcome.
4. **Freeze document draft** under `dev/active/b8206228-permanent-statistics/`
   or the campaign home as the issue's linked artifact: the selection
   receipt(s) drafted per the protocol's required per-candidate provenance
   fields, the exclusion records, the inventory, and the gap list. Cite every
   number to its committed source (path + section); external claims cite
   registry keys.

Write everything in present tense; a draft is labelled a draft. Report: the
inventory summary (how many of 63 cells are dischargeable now, per field),
the gap list, the draft's validation outcome, and anything unverifiable.
