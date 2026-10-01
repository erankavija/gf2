# Plan-review pre-check (gate 2), planning node 8dddfc7a

Snapshot: HEAD 66dc0910a, 2026-10-01. Read-only; F5 skipped as instructed.

## Verdict

Blocking: none confirmed. One likely reviewer finding (A1) worth fixing before the gate.

## Deterministic commands

- `breakdown_manifest.py validate ... --deny-warnings` with known sources for every REQ/D/OD/PD/DEC id in plan.md and breakdown.json, plus the INV refs (INV-A/B/C and the 27 `INV-§x.y` section refs), and required sources REQ-01..REQ-20: exit 0. Output is 49 advisories (footprint uncertainty and overlap), no warnings or errors. Without the `INV-§` known sources, validation fails as unknown refs, so the gate run must pass them.
- `render ... --check`: exit 0.
- `jit issue batch-create --from-json breakdown.json --dry-run --json`: `valid: true`, 91 issues, 149 edges.

## F1 external wiring: resolved

- `external-wiring.json` parses. It is linked to 8dddfc7a as document "External wiring".
- All 17 edge `from_key`s and both `replaced_by_key`s (tersification-sweep, readme-landing-page) exist in breakdown.json.
- All targets exist live (3f29e945, a24b2af7, 9b2886a7, 153297cf, 12907582, f357b3dc, 44c98235) and carry `epic:documentation-overhaul`.
- `labels_applied` matches the live labels exactly: 153297cf has `satisfies:REQ-07`, 3f29e945 has REQ-01 and REQ-16, 12907582 has REQ-17.
- Transitive closures of the targets contain only overhaul issues (495807a3, 34adff85, dcd38c45, 56378e82, 8f61d6de, a0a29512, 3f29e945). Their `satisfies:` labels are REQ-31..REQ-40 (contract story, PD-09 range) plus REQ-07. The one foreign node is 0b45a5fa (tech-debt epic): done, no deps, no `satisfies:` label. This matches the plan's claim at plan.md:474.
- No other epic transitively contains any target. The only ancestor of any target is f357b3dc, which is superseded and has no dependents. No edge can close a cycle, since the targets' closures contain no manifest or bracket node.
- After wiring, every open `epic:documentation-overhaul` issue is reachable from fa787f85 plus the targets, so there is no orphan.
- plan.md:474 names the file as authoritative and applied verbatim. No leftover `jit dep add` or post-batch prose remains. The only `jit` mention left is the unrelated plan.md:86.

## F2: resolved

Path: audit-permanent-content -> legacy-move-gf2-core-guides -> howto-select-acceleration -> reference-performance-evidence.

## F3 roadmap shards: resolved

| Leaf | Stated | Recount (`git show 61c3f0a6a^` / `791beb2b0^`) |
|---|---|---|
| roadmap-map-root-planned | ~26 | 10 + 12 + 4 list/table lines (:95-106, :108-126, :157-162) = 26 |
| roadmap-map-root-questions | ~29 | 17 + 12 (:128-155, :164-183) = 29 |
| roadmap-map-gf2-core | ~78 | 20 + 12 + 6 = 38 list lines, many of them status bullets; investigation says 3 phases + 8 future bullets |
| roadmap-map-gf2-coding | ~46 | 46 unchecked `- [ ]` items |

All four are one-worker sized. Each has standalone REQ-01 (mapping record with revision, line and disposition) and REQ-02 (filed issue per untracked item). Only root-planned carries REQ-03 ("no roadmap file exists"), which assigns it to a single owner.

## F4 inventory shards: resolved

Appendix A partition, recomputed from the table:

| Shard | Entries | Files |
|---|---|---|
| zen3-early | 12 | 658 |
| zen3-late | 23 | 644 |
| field-dispatch | 18 | 477 |
| open-epics | 17 | 158 |
| terminal-ownerless | 48 | 13 |

The total is 118 entries and 1,950 files, matching Appendix A with no overlap or gap at the snapshot. The 1a379447 epic dir sorts inside early, as stated. Each shard has REQ-01..REQ-05 standalone. The code-consumer criterion (REQ-04, non-comment consumers by file and line plus digest-pinned marking) appears verbatim in all five. inventory-dev-buckets has the equivalent in REQ-02, and inventory-permanent-sources has it in REQ-03/REQ-04.

## New findings

- **A1 (medium, likely flagged as source-universe or coverage):** the five active shards partition by a closed list of named epics plus terminal or ownerless entries. An entry owned by any other non-terminal epic has no shard, and the checker (a24b2af7 REQ-02) validates only rows that exist, so it does not check universe completeness. A live instance exists: untracked `dev/active/0b0714e0-channel-gain-design.md` (bug 0b0714e0, top epic cffc15dd, which no shard names). If it is committed, it falls outside every shard. `git ls-files dev/active` is now 1,959 (+9 since the snapshot; `fd9d5416/` resolves to ae03bcd0 + 86b9c719, so open-epics covers it). Fix: give one shard (open-epics) a catch-all clause, "and every other entry owned by a non-terminal epic not named in another shard".
- **A2 (low):** roadmap-map-gf2-core's `worker_sized_reason` says "About 78 items". Neither investigation §3.9 nor the source supports that number: the sections have about 38 list lines, and the investigation counts 3 phases plus 8 future bullets. It overstates, which is harmless for sizing but is an unverifiable figure.
- **A3 (low, pre-existing since D-53):** move-02b8137c-journal REQ-01 gates on another epic's issue being terminal via a tracker check rather than an edge. The rationale is recorded in D-53 and plan.md:474, and earlier rounds accepted it.
- **Bodies:** none of the 91 bodies references another manifest key. The tracker mechanics in triage-doc-issues (filter label, `resolution:obsolete`) are that task's subject matter. "Rust CI gate passes" criteria name a check, not tracker state.
