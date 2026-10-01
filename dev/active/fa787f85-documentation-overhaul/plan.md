# Plan: Overhaul documentation for research adoption (fa787f85)

> Planning node: 8dddfc7a. Authoritative graph:
> [breakdown.json](breakdown.json).

Source IDs: `CC-§1` cites the code-pinned-entry section of the [code-consumer scan](dev-path-code-consumers.md); `ARCH-§n` cites section n of the [archive preview results](archive-preview-results.md); `REQ-01`..`REQ-20` are the epic's criteria, `REQ-21`..`REQ-27` the
replacement sweep story's, and `REQ-31`..`REQ-40` the contract story's. `D-xx`
are brief decisions (D-43..D-53 record the 2026-10-01 owner decisions), `DEC-xx` are
3f29e945 decisions, and `OD-08` and `PD-xx` are rows of the decisions table
below. `INV-§n` cites [investigation](investigation.md) section n, `INV-§0.k`
its scope finding k, and `INV-A/B/C` its appendices.

## Outcome and criterion approach

| Criterion | Approach | Evidence / open gap |
|---|---|---|
| REQ-01 | Invariants come from 3f29e945 (three groups) and 9b2886a7 (audience, placement, tone); the permanent-page audit verifies enforcement by address. | DEC-02; INV-§2 |
| REQ-02 | 3f29e945 owns the symlink, scope and line cap; one task retires CONTRIBUTING.md. | INV-§3.6, INV-§0.8 (199/200 lines) |
| REQ-03 | `readme-landing-page` replaces 44c98235 and carries its criteria, including no performance claims; it runs once the index and both tutorials exist. | INV-§0.9 |
| REQ-04 | Index plus quadrant layout; crate READMEs are the four entry pages (D-50); exactly two tutorials (D-45), the second built on a new gf2-core example. | INV-§5.2 |
| REQ-05 | Each page task writes fresh prose from verified facts; the audit compares each page with the sources it mined. | INV-§3.5 topic map |
| REQ-06 | One evidence page with commit-pinned links (D-48); how-tos link its anchors; README and entry pages carry no figures. | INV-§3.10: no class has all fields in-file |
| REQ-07 | a0a29512 removes tautological examples and 153297cf settles accessor gaps, both before the sweep; a final census measures the corpus after it. | audit + census script linked to fa787f85 |
| REQ-08 | Retroactive mapping of the three deleted roadmaps (D-47), split by file and, for the root roadmap, by section group. | INV-§3.9 |
| REQ-09 | Decks move by `git mv` into their owners' archive dirs, and every link target is repaired (D-51); the figure example stops writing under `docs/`. | INV-§3.4, INV-§0.1 |
| REQ-10 | The previews are done ([archive preview results](archive-preview-results.md)). Five re-archive tasks, grouped by artifact count, collect 217 terminal-story artifacts into ten existing epic archives. A separate task resolves b7157be6's unmarked hand archive and then archives it through JIT. The six terminal stories under open epics are not archived now (D-22). | ARCH-§1..§3; INV-§0.7 |
| REQ-11 | The migration universe is frozen at commit `7641561a9`: the five inventory shards list exactly its 77 `dev/active` entries by name. Later entries are outside the migration; `@/inv/active-document-layout` governs them at creation. Generic regroup tasks move only entries without code consumers; the scan's code-pinned entries are excluded by name. f547c394, 02b8137c and 3f664839 have tolerance and move tasks here (D-52, D-53). `code-pinned-entry-plan` files the tolerance and move pairs for the remaining frozen code-pinned entries under this epic, and husks are cleared after it. | INV-A, INV-§0.6, CC-§1 |
| REQ-12 | Admissible-evidence rule (D-24) in the inventory; ownerless material and mined guides move to the legacy mirror by `git mv`. | INV-§0.3: no legacy primitive |
| REQ-13 | a24b2af7 defines the schema and checker; seven inventory shards populate it (five for `dev/active`, frozen at commit `7641561a9`, plus one each for the other dev buckets and the permanent-path sources); `dev/active` entries created after the freeze are outside the migration; the checker retires after the final check. Tooling Markdown under `.agents/`, `contrib/`, `packages/` and `.jit/` is not documentation and is excluded. | INV-A, INV-B |
| REQ-14 | `plans`, `presentations` and `sessions` are eliminated (D-28); `bench_results`, `simulation_results` and `studies` keep consumed files and lose loose narrative Markdown (D-49); `tools` stays as a workspace member (D-27, INV-§0.5); `dev/index.md` is rewritten. | INV-§3.1, INV-§0.4, INV-§0.5 |
| REQ-15 | Temporary widening after the inventory; final narrowing after the content audit. | INV-§3.1 stale entries |
| REQ-16 | 3f29e945 delivers doc-review grounding, docs-mechanical and rustdoc CI; the final policy sets the docs-mechanical footprint. | DEC-04..DEC-06; INV-§0.11 |
| REQ-17 | The sweep story covers source comments and module docs; the audit covers permanent pages and crate-root docs; 12907582 covers factual drift. | INV-§6 |
| REQ-18 | One triage task with two dispositions (D-43); candidates re-derived at execution. | INV-§3.13, INV-§0.10 |
| REQ-19 | Every move task scans its touched files; a final task verifies the permanent footprint, every executed archive and every relocated artifact. | INV-§0.2: archival rewrites no content |
| REQ-20 | The brief records D-01..D-53, including the owner decisions; triage adds the inherited-issue dispositions and carries the credit (PD-06). | brief D-43..D-53 |

## Shared architectural contracts

### `docs-surface-layout` [plan-fixed] — Permanent docs layout

`docs/index.md` is the single navigation page, with one section per quadrant
(`tutorials/`, `how-to/`, `concepts/`, `reference/`) and a crate section. Each
crate's `README.md` is its entry page. A page task creates its file in the
matching quadrant and adds its own index row. Permanent pages link to `dev/`
only through commit-pinned URLs.

### `performance-evidence-page` [implementation-produced] — Performance evidence page

`docs/reference/performance-evidence.md` holds every performance claim, with
one stable anchor per claim. Each claim states methodology, hardware, flags,
workload, baseline and date, and links evidence at a fixed commit. Other pages
link anchors and state no figures. Producer: `reference-performance-evidence`.
The how-tos depend on it directly; `audit-permanent-content` reaches it through
`legacy-move-gf2-core-guides` → `howto-reproduce-evidence`. A direct edge would
be transitively redundant.

### `migration-manifest` [plan-fixed] — Migration manifest and checker

Schema, file location and checker are produced outside this manifest by a24b2af7. Each migration task
updates the rows it executes to complete status and leaves the checker passing
for those rows. Temporary policy entries are recorded in the manifest with
their removal condition.

### `relocation-protocol` [plan-fixed] — Moves outside JIT archival

A move JIT cannot perform (non-`dev/` sources, the legacy mirror, decks) uses
`git mv`, updates affected tracker document references, repoints in-file
human-facing citations in the same commit, and passes a link scan of touched
files (the docs-mechanical check once registered). Digest-pinned machine inputs
keep their bytes: receipt directories, preregistrations, addenda, plans,
journals, schemas, continuation JSON and snapshot copies (INV-§6). An entry
whose path code checks moves only after a tolerance change lets the consumer
accept the recorded historical path. Both steps compare the consumer's verdicts
over committed artifacts against a recorded baseline. In historical artifacts only
link targets change, and every link in a moved artifact resolves afterwards.

### `archive-execution-protocol` [plan-fixed] — JIT container archival

Preview, resolve each blocker, execute, repoint in-file citations in the same
change, verify marker and hashes, rerun to confirm a no-op (INV-§4). Shared
files of open owners are copied, never moved.

### `active-layout` [plan-fixed] — Active document layout

Open-epic material lives under the directory `jit doc dir <epic> dev/active`
resolves. An entry owned by several epics goes to the epic owning most of its
linked documents; a tie goes to the epic whose issue ID names the entry.

### `sweep-unit-rules` [plan-fixed] — Sweep unit rules

Comment, Rustdoc and whitespace edits only. The shared pattern (story REQ-24)
marks narration and scope markers. Each unit lists its justified matches in its
commit message. Evidence pointers and generated-table provenance stay.

### `sweep-baseline` [implementation-produced] — Pre-sweep comment census

The per-crate comment share and pattern counts, with the exact command and
commit, in the sweep completion record. Producer: `sweep-baseline-census`.

### `fieldmatrix-example` [implementation-produced] — Field linear-algebra example

A tested gf2-core example program for large prime- and extension-field linear
algebra, which the second tutorial uses. Producer:
`fieldmatrix-example-program`.

## Generated decomposition overview

<!-- jit:breakdown-overview:begin -->
| Key | Title | Type | Outcome | Contracts | Sources | Footprint | Landing | Depends on |
|---|---|---|---|---|---|---|---|---|
| triage-doc-issues | Reconcile open docs-remediation issues outside the overhaul | task | Each open docs-remediation issue outside the epic has a recorded disposition with preserved requirements | — | REQ-18, REQ-20, D-43, D-02, D-03, INV-§3.13, INV-§0.10, PD-06 | creates 1, touches 1 | — | — |
| roadmap-map-root-planned | Map the root roadmap's planned milestones and goals | task | Each planned item of the root roadmap's Planned, Research Goals and Long-Term Vision sections maps to an issue, code, or a recorded obsolescence | — | REQ-08, D-47, D-19, INV-§3.9 | creates 1 | — | — |
| roadmap-map-root-questions | Map the root roadmap's open questions and publication items | task | Each planned item of the root roadmap's Open Research Questions and Publication & Validation sections maps to an issue, code, or a recorded obsolescence | — | REQ-08, D-47, D-19, INV-§3.9 | creates 1 | — | — |
| roadmap-map-gf2-core | Map the gf2-core roadmap's planned phases | task | Each planned item of the gf2-core roadmap's open planned phases, future directions and priorities maps to an issue, code, or a recorded obsolescence | — | REQ-08, D-47, D-19, INV-§3.9 | creates 1 | — | — |
| roadmap-map-gf2-coding | Map the gf2-coding roadmap's unchecked items | task | Each planned item of the gf2-coding roadmap's unchecked items and planned phases maps to an issue, code, or a recorded obsolescence | — | REQ-08, D-47, D-19, INV-§3.9 | creates 1 | — | — |
| remove-contributing-guide | Retire CONTRIBUTING.md in favor of AGENTS.md | task | CONTRIBUTING.md is gone and its unique guidance has one home within the AGENTS.md line limit | — | REQ-02, D-13, D-14, INV-§3.6, INV-§0.8, PD-05 | touches 3 | — | — |
| inventory-active-zen3-early | Inventory zen3 dev/active entries 00dd43c3 to 3be770d5 | task | Each early zen3 dev/active artifact has a manifest row with evidence, consumers and a disposition | migration-manifest, active-layout | REQ-13, REQ-12, REQ-11, D-24, D-29, D-53, INV-A, INV-§0.6, PD-03, CC-§1 | touches 1, uncertain | — | — |
| inventory-active-zen3-late | Inventory zen3 dev/active entries 428f2f6b to fcb04d66 | task | Each late zen3 dev/active artifact has a manifest row with evidence, consumers and a disposition | migration-manifest, active-layout | REQ-13, REQ-12, REQ-11, D-24, D-29, D-53, INV-A, INV-§0.6, PD-03, CC-§1 | touches 1, uncertain | — | — |
| inventory-active-field-dispatch | Inventory the field-dispatch dev/active entries | task | Each field-dispatch dev/active artifact has a manifest row with evidence, consumers and a disposition | migration-manifest, active-layout | REQ-13, REQ-12, REQ-11, D-24, D-29, D-53, INV-A, INV-§0.6, PD-03, CC-§1 | touches 1, uncertain | — | — |
| inventory-active-open-epics | Inventory the other open-epic dev/active entries | task | Each other open-epic dev/active artifact has a manifest row with evidence, consumers and a disposition | migration-manifest, active-layout | REQ-13, REQ-12, REQ-11, D-24, D-29, D-53, INV-A, INV-§0.6, PD-03, CC-§1 | touches 1, uncertain | — | — |
| inventory-active-terminal-ownerless | Inventory terminal, ownerless and empty dev/active entries | task | Each terminal, ownerless or empty dev/active artifact has a manifest row with evidence, consumers and a disposition | migration-manifest, active-layout | REQ-13, REQ-12, REQ-11, D-24, D-29, D-53, INV-A, INV-§0.6, PD-03, CC-§1 | touches 1, uncertain | — | — |
| inventory-dev-buckets | Populate the migration manifest for the other dev buckets | task | Each non-active dev artifact has a manifest row with consumers, evidence and a disposition | migration-manifest | REQ-13, REQ-12, REQ-14, D-27, D-28, D-49, INV-§3.1, INV-B, INV-§0.4, INV-§0.5 | touches 1, uncertain | — | — |
| inventory-permanent-sources | Populate the migration manifest for permanent-path sources | task | Each permanent-path source has a manifest row with disposition, destination and inbound references | migration-manifest | REQ-13, REQ-12, REQ-09, D-46, D-09, INV-§3.4, INV-§3.5, INV-§0.1, PD-02 | touches 1, uncertain | — | — |
| expand-managed-paths | Widen the docs policy to manage archival sources | task | The docs policy temporarily manages each container-archive source and lists no absent path | migration-manifest | REQ-15, D-26, INV-§3.1, INV-§4 | touches 1 | — | inventory-active-zen3-early, inventory-active-zen3-late, inventory-active-field-dispatch, inventory-active-open-epics, inventory-active-terminal-ownerless, inventory-dev-buckets, inventory-permanent-sources |
| link-owned-artifacts | Link unlinked owned artifacts to their issues | task | Defensibly owned artifacts carry document references on their owning issues | migration-manifest | REQ-12, REQ-11, D-24, INV-A | touches 1 | — | expand-managed-paths |
| sweep-baseline-census | Record the pre-sweep comment census | task | A reproducible per-crate comment census exists before sweep edits begin | sweep-unit-rules | REQ-27, D-44, INV-C | creates 1 | — | — |
| sweep-core-field-poly | Tersify comments in gf2-core field polynomial and extension modules | task | Comments in gf2-core field polynomial and extension modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 8 | source-tersification | sweep-baseline-census |
| sweep-core-field-dense | Tersify comments in gf2-core dense field matrix modules | task | Comments in gf2-core dense field matrix modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 3 | source-tersification | sweep-baseline-census |
| sweep-core-field-algorithms | Tersify comments in gf2-core field matrix algorithm modules | task | Comments in gf2-core field matrix algorithm modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 5 | source-tersification | sweep-baseline-census |
| sweep-core-field-traits | Tersify comments in gf2-core field trait and vector modules | task | Comments in gf2-core field trait and vector modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 6 | source-tersification | sweep-baseline-census |
| sweep-core-gf2m | Tersify comments in the gf2-core GF(2^m) module | task | Comments in the gf2-core GF(2^m) module carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 1 | source-tersification | sweep-baseline-census |
| sweep-core-prime-fields | Tersify comments in the gf2-core prime-field modules | task | Comments in the gf2-core prime-field modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 2 | source-tersification | sweep-baseline-census |
| sweep-core-bit-structures | Tersify comments in gf2-core bit-level structures | task | Comments in gf2-core bit-level structures carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 11 | source-tersification | sweep-baseline-census |
| sweep-core-runtime | Tersify comments in gf2-core runtime support modules | task | Comments in gf2-core runtime support modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 7 | source-tersification | sweep-baseline-census |
| sweep-core-tests-benches | Tersify comments in gf2-core tests, benches and examples | task | Comments in gf2-core tests, benches and examples carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 3 | source-tersification | sweep-baseline-census |
| sweep-coding-ldpc | Tersify comments in the gf2-coding LDPC module | task | Comments in the gf2-coding LDPC module carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 1 | source-tersification | sweep-baseline-census |
| sweep-coding-bch | Tersify comments in gf2-coding BCH and transform modules | task | Comments in gf2-coding BCH and transform modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C, INV-§3.14 | touches 2 | source-tersification | sweep-baseline-census |
| sweep-coding-modem | Tersify comments in the gf2-coding modem module | task | Comments in the gf2-coding modem module carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 1 | source-tersification | sweep-baseline-census |
| sweep-coding-decoders | Tersify comments in gf2-coding OSD, product, GRAND and GLDPC modules | task | Comments in gf2-coding OSD, product, GRAND and GLDPC modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 4 | source-tersification | sweep-baseline-census |
| sweep-coding-core-src | Tersify comments in the remaining gf2-coding source files | task | Comments in the remaining gf2-coding source files carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 17 | source-tersification | sweep-baseline-census |
| sweep-coding-tests-benches | Tersify comments in gf2-coding tests, benches and examples | task | Comments in gf2-coding tests, benches and examples carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 3 | source-tersification | sweep-baseline-census |
| sweep-sim-campaigns | Tersify comments in gf2-sim executor and campaign modules | task | Comments in gf2-sim executor and campaign modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 4 | source-tersification | sweep-baseline-census |
| sweep-sim-pipeline | Tersify comments in gf2-sim GPU, preset, stage and graph modules | task | Comments in gf2-sim GPU, preset, stage and graph modules carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 4 | source-tersification | sweep-baseline-census |
| sweep-sim-runtime | Tersify comments in the remaining gf2-sim source files | task | Comments in the remaining gf2-sim source files carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 15 | source-tersification | sweep-baseline-census |
| sweep-sim-tests-bins | Tersify comments in gf2-sim tests, benches, binaries and examples | task | Comments in gf2-sim tests, benches, binaries and examples carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 4 | source-tersification | sweep-baseline-census |
| sweep-algebra-packed | Tersify comments in the gf2-algebra packed-field module | task | Comments in the gf2-algebra packed-field module carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 1 | source-tersification | sweep-baseline-census |
| sweep-algebra-rest | Tersify comments in the remaining gf2-algebra files | task | Comments in the remaining gf2-algebra files carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 10 | source-tersification | sweep-baseline-census |
| sweep-simd-x86 | Tersify comments in the gf2-kernels-simd x86 module | task | Comments in the gf2-kernels-simd x86 module carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 1 | source-tersification | sweep-baseline-census |
| sweep-simd-rest | Tersify comments in the remaining gf2-kernels-simd files | task | Comments in the remaining gf2-kernels-simd files carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 23 | source-tersification | sweep-baseline-census |
| sweep-hip-stats | Tersify comments in the gf2-kernels-hip and gf2-stats crates | task | Comments in the gf2-kernels-hip and gf2-stats crates carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-25, REQ-26, D-44, D-04, D-42, INV-§5.3, INV-C | touches 2 | source-tersification | sweep-baseline-census |
| sweep-completion-record | Close the sweep with the after census and justification list | task | The sweep's before-and-after census and justified matches are recorded | sweep-baseline, sweep-unit-rules | REQ-24, REQ-27, D-44, INV-C | touches 1 | — | sweep-core-field-poly, sweep-core-field-dense, sweep-core-field-algorithms, sweep-core-field-traits, sweep-core-gf2m, sweep-core-prime-fields, sweep-core-bit-structures, sweep-core-runtime, sweep-core-tests-benches, sweep-coding-ldpc, sweep-coding-bch, sweep-coding-modem, sweep-coding-decoders, sweep-coding-core-src, sweep-coding-tests-benches, sweep-sim-campaigns, sweep-sim-pipeline, sweep-sim-runtime, sweep-sim-tests-bins, sweep-algebra-packed, sweep-algebra-rest, sweep-simd-x86, sweep-simd-rest, sweep-hip-stats |
| tersification-sweep | Workspace source-comment tersification sweep | story | Workspace Rust comments carry only current, non-obvious content in shortest form | sweep-unit-rules | REQ-17, REQ-21, REQ-22, REQ-23, REQ-24, REQ-25, REQ-26, REQ-27, D-44, D-42, D-04, PD-09 | — | — | sweep-completion-record |
| repair-osd-archive | Bring the hand-archived OSD epic under container archival | task | Epic b7157be6 is archived with a marker, verified bytes and resolvable references | archive-execution-protocol, relocation-protocol | REQ-10, PD-01, INV-§0.7, INV-§1.1, ARCH-§3 | touches 2 | — | link-owned-artifacts, tersification-sweep |
| rearchive-97bf0879 | Re-archive epic 97bf0879 to collect its terminal stories | task | Terminal-story artifacts of 97bf0879 sit in their epic archive with verified bytes | archive-execution-protocol, relocation-protocol, migration-manifest | REQ-10, REQ-19, D-21, D-22, PD-01, PD-02, INV-§0.2, INV-§4, ARCH-§1, ARCH-§2 | touches 6, uncertain | — | link-owned-artifacts, tersification-sweep |
| rearchive-026fc832 | Re-archive epic 026fc832 to collect its terminal stories | task | Terminal-story artifacts of 026fc832 sit in their epic archive with verified bytes | archive-execution-protocol, relocation-protocol, migration-manifest | REQ-10, REQ-19, D-21, D-22, PD-01, PD-02, INV-§0.2, INV-§4, ARCH-§1, ARCH-§2 | touches 6, uncertain | — | link-owned-artifacts, tersification-sweep |
| rearchive-babcf05e-f9717e7e | Re-archive epics babcf05e, f9717e7e to collect their terminal stories | task | Terminal-story artifacts of babcf05e, f9717e7e sit in their epic archive with verified bytes | archive-execution-protocol, relocation-protocol, migration-manifest | REQ-10, REQ-19, D-21, D-22, PD-01, PD-02, INV-§0.2, INV-§4, ARCH-§1, ARCH-§2 | touches 7, uncertain | — | link-owned-artifacts, tersification-sweep |
| rearchive-bb85c68a-6efb756b | Re-archive epics bb85c68a, 6efb756b to collect their terminal stories | task | Terminal-story artifacts of bb85c68a, 6efb756b sit in their epic archive with verified bytes | archive-execution-protocol, relocation-protocol, migration-manifest | REQ-10, REQ-19, D-21, D-22, PD-01, PD-02, INV-§0.2, INV-§4, ARCH-§1, ARCH-§2 | touches 7, uncertain | — | link-owned-artifacts, tersification-sweep |
| rearchive-small-epics | Re-archive epics e095a100, 806eb14e, 2928ccce, d4851c3d to collect their terminal stories | task | Terminal-story artifacts of e095a100, 806eb14e, 2928ccce, d4851c3d sit in their epic archive with verified bytes | archive-execution-protocol, relocation-protocol, migration-manifest | REQ-10, REQ-19, D-21, D-22, PD-01, PD-02, INV-§0.2, INV-§4, ARCH-§1, ARCH-§2 | touches 9, uncertain | — | link-owned-artifacts, tersification-sweep |
| resolve-dev-strays | Resolve stray and ownerless dev entries | task | Stray and ownerless dev entries sit with their owners or in the legacy mirror | relocation-protocol, migration-manifest | REQ-12, REQ-11, D-24, D-25, PD-02, PD-03, INV-§1.1, INV-A, INV-B | creates 1, touches 2 | — | rearchive-97bf0879, rearchive-026fc832, rearchive-babcf05e-f9717e7e, rearchive-bb85c68a-6efb756b, rearchive-small-epics |
| code-pinned-entry-plan | File tolerance and move tasks for code-pinned dev/active entries | task | Each frozen code-pinned dev/active entry has filed tolerance and move tasks or a recorded reason it stays | active-layout, migration-manifest | REQ-11, D-53, CC-§1, INV-§6 | creates 1 | — | inventory-active-zen3-early, inventory-active-zen3-late, inventory-active-field-dispatch, inventory-active-open-epics, inventory-active-terminal-ownerless |
| regroup-active-zen3 | Regroup flat zen3 dev/active entries under their epic dir | task | Flat zen3 entries live under the zen3 epic dir with relinked references | active-layout, relocation-protocol, migration-manifest | REQ-11, D-22, D-23, PD-03, INV-A, INV-§6 | touches 5, uncertain | — | link-owned-artifacts, tersification-sweep |
| regroup-active-field-dispatch | Regroup flat field-dispatch dev/active entries under their epic dir | task | Flat field-dispatch entries live under their epic dir with relinked references | active-layout, relocation-protocol, migration-manifest | REQ-11, D-23, PD-03, INV-A | touches 5, uncertain | — | link-owned-artifacts, tersification-sweep |
| regroup-active-remaining | Regroup the remaining flat dev/active entries under epic dirs | task | Remaining flat entries live under their epic dirs with relinked references | active-layout, relocation-protocol, migration-manifest | REQ-11, D-23, PD-03, INV-A, INV-§3.14 | touches 4, uncertain | — | link-owned-artifacts, tersification-sweep |
| receipt-pin-path-tolerance | Accept historical protocol pin paths in receipt verification | task | Receipt verification accepts recorded historical pin paths, keeping committed receipts valid across relocation | — | REQ-11, D-52, INV-§6, INV-§3.1 | creates 1, touches 3 | — | — |
| move-f547c394-inputs | Move the f547c394 protocol inputs under their epic dir | task | f547c394 lives under its epic dir and its CI and tooling readers use the new path | active-layout, relocation-protocol, migration-manifest | REQ-11, D-23, D-52, D-53, PD-03, INV-§3.1, INV-§6 | touches 11 | — | receipt-pin-path-tolerance, regroup-active-zen3 |
| rare-event-design-path-tolerance | Accept the historical design path in rare-event artifact validation | task | Rare-event artifact validation accepts the recorded historical design path with verdicts unchanged | — | REQ-11, D-53, INV-§6 | creates 1, touches 2 | — | — |
| move-3f664839-design | Move the 3f664839 design entry under its epic dir | task | The 3f664839 design lives under its epic dir and validation reads it there | active-layout, relocation-protocol, migration-manifest | REQ-11, D-23, D-53, PD-03, INV-A, INV-§6 | touches 6 | — | rare-event-design-path-tolerance, regroup-active-remaining |
| campaign-validation-path-tolerance | Accept historical frozen-evidence paths in campaign validation | task | Campaign validation accepts recorded historical frozen-evidence paths with verdicts unchanged | — | REQ-11, D-53, INV-§6 | creates 1, touches 2 | — | — |
| move-02b8137c-journal | Move the 02b8137c validation journal under its epic dir | task | The 02b8137c journal lives under its epic dir and validation reads it there | active-layout, relocation-protocol, migration-manifest | REQ-11, D-23, D-53, PD-03, INV-A, INV-§6 | touches 6 | — | campaign-validation-path-tolerance, regroup-active-remaining |
| relocate-bench-narrative | Relocate loose narrative reports out of dev/bench_results | task | dev/bench_results keeps only operational receipts and data | relocation-protocol, migration-manifest, active-layout | REQ-14, D-49, PD-02, INV-§3.1, INV-§3.10, INV-§6, INV-§0.4 | touches 3 | — | rearchive-97bf0879, rearchive-026fc832, rearchive-babcf05e-f9717e7e, rearchive-bb85c68a-6efb756b, rearchive-small-epics, regroup-active-zen3 |
| relocate-sim-studies-narrative | Relocate loose narrative Markdown out of simulation_results and studies | task | simulation_results and studies keep only consumed operational files | relocation-protocol, migration-manifest, active-layout | REQ-14, D-49, PD-02, INV-§3.1, INV-§0.4 | touches 3 | — | repair-osd-archive, regroup-active-remaining |
| remove-active-husks | Clear empty leftover dirs under dev/active | task | dev/active holds only directories with tracked content | migration-manifest | REQ-11, PD-03, INV-A, INV-§0.6 | touches 1 | — | resolve-dev-strays, code-pinned-entry-plan, regroup-active-field-dispatch, move-f547c394-inputs, move-3f664839-design, move-02b8137c-journal, relocate-bench-narrative, relocate-sim-studies-narrative |
| eliminate-dev-plans | Empty dev/plans through archival or legacy moves | task | dev/plans is gone and no citation of its paths dangles | relocation-protocol, migration-manifest | REQ-14, REQ-12, D-28, PD-02, INV-§3.1, INV-§1.1, INV-§6 | touches 6 | — | rearchive-97bf0879, rearchive-026fc832, rearchive-babcf05e-f9717e7e, rearchive-bb85c68a-6efb756b, rearchive-small-epics |
| eliminate-dev-sessions | Empty dev/sessions into owner dirs or the legacy mirror | task | dev/sessions is gone and its notes sit with owners or in the legacy mirror | relocation-protocol, migration-manifest | REQ-14, D-28, D-25, INV-§3.1 | touches 2 | — | link-owned-artifacts |
| eliminate-dev-presentations | Empty dev/presentations of leftover theme files | task | dev/presentations is gone with its stylesheets resolved against their owners | relocation-protocol, migration-manifest | REQ-14, D-28, INV-§3.1, INV-§6 | touches 2 | — | link-owned-artifacts |
| move-presentation-decks | Move presentation decks into their epics' archive dirs | task | Each deck bundle lives with its archived epic and no permanent page links it | relocation-protocol, migration-manifest | REQ-09, D-20, PD-02, D-51, INV-§3.4, INV-§0.1 | touches 3 | — | inventory-permanent-sources |
| retarget-figure-generator | Point the presentation figure example at a caller-chosen output dir | task | The figure example writes outside the permanent docs tree | — | REQ-09, INV-§3.4 | touches 1 | — | move-presentation-decks, tersification-sweep |
| docs-scaffold-index | Create the docs index and quadrant layout | task | docs/index.md routes readers to the four quadrants and the existing crate entry pages | docs-surface-layout | REQ-04, D-17, D-10, D-50, PD-10 | creates 1 | — | — |
| entry-page-gf2-core | Rewrite the gf2-core README as its entry page | task | The gf2-core README is a concise current-state entry page linking Rustdoc and the docs index | docs-surface-layout | REQ-04, REQ-05, REQ-17, D-18, D-50, INV-§3.5, INV-§6 | touches 1 | — | docs-scaffold-index |
| entry-page-gf2-coding | Rewrite the gf2-coding README as its entry page | task | The gf2-coding README is a concise current-state entry page linking Rustdoc and the docs index | docs-surface-layout | REQ-04, REQ-05, REQ-17, D-18, D-50, INV-§3.5, INV-§6 | touches 1 | — | docs-scaffold-index |
| entry-page-gf2-algebra | Rewrite the gf2-algebra README as its entry page | task | The gf2-algebra README is a concise current-state entry page linking Rustdoc and the docs index | docs-surface-layout | REQ-04, REQ-05, REQ-17, D-18, D-50, INV-§3.5, INV-§6 | touches 1 | — | docs-scaffold-index |
| entry-page-gf2-sim | Write the gf2-sim README as its entry page | task | The gf2-sim README is a concise current-state entry page linking Rustdoc and the docs index | docs-surface-layout | REQ-04, REQ-05, REQ-17, D-18, D-50, INV-§3.5, INV-§6 | creates 1, touches 1 | — | docs-scaffold-index |
| concept-acceleration-architecture | Write the acceleration architecture concepts page | task | A concepts page explains current kernel dispatch, backends and parallelism | docs-surface-layout | REQ-05, D-18, INV-§3.5 | creates 1, touches 1 | — | docs-scaffold-index |
| backend-crate-readmes | Rewrite the SIMD kernel and statistics crate READMEs | task | Backend and statistics crate READMEs state current role and integration concisely | docs-surface-layout | REQ-05, REQ-17, D-18, INV-§3.5 | touches 2 | — | concept-acceleration-architecture |
| concept-field-arithmetic | Write the finite-field arithmetic concepts page | task | A concepts page explains field representation and strategy choices as currently implemented | docs-surface-layout | REQ-05, REQ-04, D-06, D-09, INV-§3.5 | creates 1, touches 1 | — | docs-scaffold-index |
| reference-performance-evidence | Write the performance evidence reference page | task | One reference page holds gf2 performance claims with methodology and commit-pinned evidence | docs-surface-layout | REQ-06, D-48, D-08, D-15, INV-§3.10, INV-§6 | creates 1, touches 1 | — | docs-scaffold-index |
| howto-select-acceleration | Write the acceleration selection how-to | task | A how-to shows how to choose and enable acceleration paths with evidence links | docs-surface-layout, performance-evidence-page | REQ-05, REQ-06, D-07, INV-§3.5 | creates 1, touches 1 | — | concept-acceleration-architecture, reference-performance-evidence |
| howto-reproduce-evidence | Write the evidence reproduction how-to | task | A how-to shows how to reproduce a pinned performance claim | docs-surface-layout, performance-evidence-page | REQ-06, REQ-05, D-07, D-08, INV-§3.10 | creates 1, touches 1 | — | reference-performance-evidence |
| howto-run-campaigns | Write the simulation campaign how-to | task | A how-to shows how to configure, run and resume a simulation campaign | docs-surface-layout | REQ-05, D-07, INV-§5.2 | creates 1, touches 1 | — | docs-scaffold-index |
| howto-formal-verification | Write the formal verification how-to | task | A how-to shows the current Lean 4 extraction and proof workflow | docs-surface-layout | REQ-05, D-07, INV-§3.5 | creates 1, touches 2 | — | docs-scaffold-index |
| reference-standards-conformance | Write the standards conformance reference page | task | A reference page states supported standard codes, conventions and conformance evidence | docs-surface-layout | REQ-05, D-12, INV-§3.5 | creates 1, touches 1 | — | docs-scaffold-index |
| reference-supported-configurations | Write the supported configurations reference page | task | A reference page states supported configurations, installation mechanisms and limits | docs-surface-layout | REQ-05, REQ-17, D-12, D-16 | creates 1, touches 1 | — | docs-scaffold-index |
| tutorial-link-simulation | Write the coded-modulation link simulation tutorial | task | A tutorial reproduces a standards-based coded-modulation link simulation with gf2-sim | docs-surface-layout | REQ-04, REQ-05, D-45, D-07, INV-§5.2 | creates 1, touches 1 | — | reference-standards-conformance, howto-run-campaigns |
| fieldmatrix-example-program | Add a FieldMatrix linear-algebra example program to gf2-core | task | gf2-core ships a tested example of large finite-field linear algebra | — | REQ-04, D-45, INV-§5.2 | creates 2 | — | — |
| tutorial-linear-algebra | Write the finite-field linear algebra at scale tutorial | task | A tutorial demonstrates large-scale finite-field linear algebra with gf2-core | docs-surface-layout, fieldmatrix-example | REQ-04, REQ-05, D-45, D-07, INV-§5.2 | creates 1, touches 1 | — | fieldmatrix-example-program, docs-scaffold-index |
| readme-landing-page | Rewrite README for research adoption | task | The root README is a concise current-state landing page linking the docs index and tutorials | docs-surface-layout | REQ-03, D-15, D-16, D-48, INV-§0.9, INV-§3.5 | touches 1 | — | tutorial-link-simulation, tutorial-linear-algebra, remove-contributing-guide, rearchive-97bf0879, rearchive-026fc832, rearchive-babcf05e-f9717e7e, rearchive-bb85c68a-6efb756b, rearchive-small-epics |
| archive-lean-pipeline-doc | Move the Lean pipeline guide into its epic's archive dir | task | The Lean pipeline guide lives with its archived epic and docs root holds only the index | relocation-protocol, migration-manifest | REQ-04, REQ-12, PD-02, INV-§1.1 | touches 3 | — | howto-formal-verification, inventory-permanent-sources |
| legacy-move-gf2-core-guides | Move gf2-core crate guides into the legacy mirror | task | gf2-core crate guides are preserved in the legacy mirror with no dangling citation | relocation-protocol, migration-manifest | REQ-12, D-46, D-25, PD-02, INV-§3.5 | creates 1, touches 6 | — | entry-page-gf2-core, concept-field-arithmetic, howto-select-acceleration, howto-reproduce-evidence, tutorial-linear-algebra, inventory-permanent-sources, tersification-sweep |
| legacy-move-gf2-coding-guides | Move gf2-coding crate guides into the legacy mirror | task | gf2-coding crate guides are preserved in the legacy mirror with no dangling citation | relocation-protocol, migration-manifest | REQ-12, D-46, D-25, PD-02, INV-§3.5 | creates 1, touches 2 | — | entry-page-gf2-coding, tutorial-link-simulation, howto-select-acceleration, inventory-permanent-sources |
| verify-rustdoc-examples | Record the final Rustdoc example census and doctest timing | task | The final Rustdoc example corpus has a recorded census and doctest timing against baseline | — | REQ-07, D-05, INV-§2 | creates 1 | — | tersification-sweep |
| audit-permanent-content | Audit the permanent surface against the docs invariants | task | The permanent pages conform to the registered docs invariants, with findings fixed | docs-surface-layout, performance-evidence-page | REQ-17, REQ-01, REQ-05, REQ-06, D-11, D-12, D-15, INV-§6, INV-§0.9 | creates 1, touches 16 | — | entry-page-gf2-algebra, entry-page-gf2-sim, backend-crate-readmes, reference-supported-configurations, legacy-move-gf2-core-guides, legacy-move-gf2-coding-guides, archive-lean-pipeline-doc, readme-landing-page, retarget-figure-generator |
| rewrite-dev-index | Rewrite dev/index.md for the final dev layout | task | dev/index.md describes the final dev layout and no obsolete bucket remains | active-layout | REQ-14, D-27, D-28, D-49, PD-11, INV-§3.1 | touches 2 | — | eliminate-dev-plans, eliminate-dev-sessions, eliminate-dev-presentations, relocate-bench-narrative, relocate-sim-studies-narrative, resolve-dev-strays |
| finalize-docs-policy | Narrow the docs policy to post-overhaul paths | task | The docs policy and mechanical-check footprint name only post-overhaul paths | migration-manifest | REQ-15, REQ-16, D-26, D-34, D-35, DEC-05, OD-08, INV-§3.1 | touches 1 | — | rewrite-dev-index, remove-active-husks, audit-permanent-content |
| verify-final-links | Verify links across the permanent surface and executed archives | task | Permanent docs and executed archives finish with no unresolved internal reference | relocation-protocol, archive-execution-protocol | REQ-19, PD-07, PD-02, INV-§0.2, INV-§6 | creates 1 | — | finalize-docs-policy, verify-rustdoc-examples |
| retire-migration-checker | Retire the transient progress checker after the final check | task | The final checker report is recorded and the transient checker is gone | migration-manifest | REQ-13, D-30, PD-07 | uncertain | — | verify-final-links |

```mermaid
flowchart LR
    N0["triage-doc-issues: Reconcile open docs-remediation issues outside the overhaul"]
    N1["roadmap-map-root-planned: Map the root roadmap's planned milestones and goals"]
    N2["roadmap-map-root-questions: Map the root roadmap's open questions and publication items"]
    N3["roadmap-map-gf2-core: Map the gf2-core roadmap's planned phases"]
    N4["roadmap-map-gf2-coding: Map the gf2-coding roadmap's unchecked items"]
    N5["remove-contributing-guide: Retire CONTRIBUTING.md in favor of AGENTS.md"]
    N6["inventory-active-zen3-early: Inventory zen3 dev/active entries 00dd43c3 to 3be770d5"]
    N7["inventory-active-zen3-late: Inventory zen3 dev/active entries 428f2f6b to fcb04d66"]
    N8["inventory-active-field-dispatch: Inventory the field-dispatch dev/active entries"]
    N9["inventory-active-open-epics: Inventory the other open-epic dev/active entries"]
    N10["inventory-active-terminal-ownerless: Inventory terminal, ownerless and empty dev/active entries"]
    N11["inventory-dev-buckets: Populate the migration manifest for the other dev buckets"]
    N12["inventory-permanent-sources: Populate the migration manifest for permanent-path sources"]
    N13["expand-managed-paths: Widen the docs policy to manage archival sources"]
    N14["link-owned-artifacts: Link unlinked owned artifacts to their issues"]
    N15["sweep-baseline-census: Record the pre-sweep comment census"]
    N16["sweep-core-field-poly: Tersify comments in gf2-core field polynomial and extension modules"]
    N17["sweep-core-field-dense: Tersify comments in gf2-core dense field matrix modules"]
    N18["sweep-core-field-algorithms: Tersify comments in gf2-core field matrix algorithm modules"]
    N19["sweep-core-field-traits: Tersify comments in gf2-core field trait and vector modules"]
    N20["sweep-core-gf2m: Tersify comments in the gf2-core GF(2^m) module"]
    N21["sweep-core-prime-fields: Tersify comments in the gf2-core prime-field modules"]
    N22["sweep-core-bit-structures: Tersify comments in gf2-core bit-level structures"]
    N23["sweep-core-runtime: Tersify comments in gf2-core runtime support modules"]
    N24["sweep-core-tests-benches: Tersify comments in gf2-core tests, benches and examples"]
    N25["sweep-coding-ldpc: Tersify comments in the gf2-coding LDPC module"]
    N26["sweep-coding-bch: Tersify comments in gf2-coding BCH and transform modules"]
    N27["sweep-coding-modem: Tersify comments in the gf2-coding modem module"]
    N28["sweep-coding-decoders: Tersify comments in gf2-coding OSD, product, GRAND and GLDPC modules"]
    N29["sweep-coding-core-src: Tersify comments in the remaining gf2-coding source files"]
    N30["sweep-coding-tests-benches: Tersify comments in gf2-coding tests, benches and examples"]
    N31["sweep-sim-campaigns: Tersify comments in gf2-sim executor and campaign modules"]
    N32["sweep-sim-pipeline: Tersify comments in gf2-sim GPU, preset, stage and graph modules"]
    N33["sweep-sim-runtime: Tersify comments in the remaining gf2-sim source files"]
    N34["sweep-sim-tests-bins: Tersify comments in gf2-sim tests, benches, binaries and examples"]
    N35["sweep-algebra-packed: Tersify comments in the gf2-algebra packed-field module"]
    N36["sweep-algebra-rest: Tersify comments in the remaining gf2-algebra files"]
    N37["sweep-simd-x86: Tersify comments in the gf2-kernels-simd x86 module"]
    N38["sweep-simd-rest: Tersify comments in the remaining gf2-kernels-simd files"]
    N39["sweep-hip-stats: Tersify comments in the gf2-kernels-hip and gf2-stats crates"]
    N40["sweep-completion-record: Close the sweep with the after census and justification list"]
    N41["tersification-sweep: Workspace source-comment tersification sweep"]
    N42["repair-osd-archive: Bring the hand-archived OSD epic under container archival"]
    N43["rearchive-97bf0879: Re-archive epic 97bf0879 to collect its terminal stories"]
    N44["rearchive-026fc832: Re-archive epic 026fc832 to collect its terminal stories"]
    N45["rearchive-babcf05e-f9717e7e: Re-archive epics babcf05e, f9717e7e to collect their terminal stories"]
    N46["rearchive-bb85c68a-6efb756b: Re-archive epics bb85c68a, 6efb756b to collect their terminal stories"]
    N47["rearchive-small-epics: Re-archive epics e095a100, 806eb14e, 2928ccce, d4851c3d to collect their terminal stories"]
    N48["resolve-dev-strays: Resolve stray and ownerless dev entries"]
    N49["code-pinned-entry-plan: File tolerance and move tasks for code-pinned dev/active entries"]
    N50["regroup-active-zen3: Regroup flat zen3 dev/active entries under their epic dir"]
    N51["regroup-active-field-dispatch: Regroup flat field-dispatch dev/active entries under their epic dir"]
    N52["regroup-active-remaining: Regroup the remaining flat dev/active entries under epic dirs"]
    N53["receipt-pin-path-tolerance: Accept historical protocol pin paths in receipt verification"]
    N54["move-f547c394-inputs: Move the f547c394 protocol inputs under their epic dir"]
    N55["rare-event-design-path-tolerance: Accept the historical design path in rare-event artifact validation"]
    N56["move-3f664839-design: Move the 3f664839 design entry under its epic dir"]
    N57["campaign-validation-path-tolerance: Accept historical frozen-evidence paths in campaign validation"]
    N58["move-02b8137c-journal: Move the 02b8137c validation journal under its epic dir"]
    N59["relocate-bench-narrative: Relocate loose narrative reports out of dev/bench_results"]
    N60["relocate-sim-studies-narrative: Relocate loose narrative Markdown out of simulation_results and studies"]
    N61["remove-active-husks: Clear empty leftover dirs under dev/active"]
    N62["eliminate-dev-plans: Empty dev/plans through archival or legacy moves"]
    N63["eliminate-dev-sessions: Empty dev/sessions into owner dirs or the legacy mirror"]
    N64["eliminate-dev-presentations: Empty dev/presentations of leftover theme files"]
    N65["move-presentation-decks: Move presentation decks into their epics' archive dirs"]
    N66["retarget-figure-generator: Point the presentation figure example at a caller-chosen output dir"]
    N67["docs-scaffold-index: Create the docs index and quadrant layout"]
    N68["entry-page-gf2-core: Rewrite the gf2-core README as its entry page"]
    N69["entry-page-gf2-coding: Rewrite the gf2-coding README as its entry page"]
    N70["entry-page-gf2-algebra: Rewrite the gf2-algebra README as its entry page"]
    N71["entry-page-gf2-sim: Write the gf2-sim README as its entry page"]
    N72["concept-acceleration-architecture: Write the acceleration architecture concepts page"]
    N73["backend-crate-readmes: Rewrite the SIMD kernel and statistics crate READMEs"]
    N74["concept-field-arithmetic: Write the finite-field arithmetic concepts page"]
    N75["reference-performance-evidence: Write the performance evidence reference page"]
    N76["howto-select-acceleration: Write the acceleration selection how-to"]
    N77["howto-reproduce-evidence: Write the evidence reproduction how-to"]
    N78["howto-run-campaigns: Write the simulation campaign how-to"]
    N79["howto-formal-verification: Write the formal verification how-to"]
    N80["reference-standards-conformance: Write the standards conformance reference page"]
    N81["reference-supported-configurations: Write the supported configurations reference page"]
    N82["tutorial-link-simulation: Write the coded-modulation link simulation tutorial"]
    N83["fieldmatrix-example-program: Add a FieldMatrix linear-algebra example program to gf2-core"]
    N84["tutorial-linear-algebra: Write the finite-field linear algebra at scale tutorial"]
    N85["readme-landing-page: Rewrite README for research adoption"]
    N86["archive-lean-pipeline-doc: Move the Lean pipeline guide into its epic's archive dir"]
    N87["legacy-move-gf2-core-guides: Move gf2-core crate guides into the legacy mirror"]
    N88["legacy-move-gf2-coding-guides: Move gf2-coding crate guides into the legacy mirror"]
    N89["verify-rustdoc-examples: Record the final Rustdoc example census and doctest timing"]
    N90["audit-permanent-content: Audit the permanent surface against the docs invariants"]
    N91["rewrite-dev-index: Rewrite dev/index.md for the final dev layout"]
    N92["finalize-docs-policy: Narrow the docs policy to post-overhaul paths"]
    N93["verify-final-links: Verify links across the permanent surface and executed archives"]
    N94["retire-migration-checker: Retire the transient progress checker after the final check"]
    N6 --> N13
    N7 --> N13
    N8 --> N13
    N9 --> N13
    N10 --> N13
    N11 --> N13
    N12 --> N13
    N13 --> N14
    N15 --> N16
    N15 --> N17
    N15 --> N18
    N15 --> N19
    N15 --> N20
    N15 --> N21
    N15 --> N22
    N15 --> N23
    N15 --> N24
    N15 --> N25
    N15 --> N26
    N15 --> N27
    N15 --> N28
    N15 --> N29
    N15 --> N30
    N15 --> N31
    N15 --> N32
    N15 --> N33
    N15 --> N34
    N15 --> N35
    N15 --> N36
    N15 --> N37
    N15 --> N38
    N15 --> N39
    N16 --> N40
    N17 --> N40
    N18 --> N40
    N19 --> N40
    N20 --> N40
    N21 --> N40
    N22 --> N40
    N23 --> N40
    N24 --> N40
    N25 --> N40
    N26 --> N40
    N27 --> N40
    N28 --> N40
    N29 --> N40
    N30 --> N40
    N31 --> N40
    N32 --> N40
    N33 --> N40
    N34 --> N40
    N35 --> N40
    N36 --> N40
    N37 --> N40
    N38 --> N40
    N39 --> N40
    N40 --> N41
    N14 --> N42
    N41 --> N42
    N14 --> N43
    N41 --> N43
    N14 --> N44
    N41 --> N44
    N14 --> N45
    N41 --> N45
    N14 --> N46
    N41 --> N46
    N14 --> N47
    N41 --> N47
    N43 --> N48
    N44 --> N48
    N45 --> N48
    N46 --> N48
    N47 --> N48
    N6 --> N49
    N7 --> N49
    N8 --> N49
    N9 --> N49
    N10 --> N49
    N14 --> N50
    N41 --> N50
    N14 --> N51
    N41 --> N51
    N14 --> N52
    N41 --> N52
    N53 --> N54
    N50 --> N54
    N55 --> N56
    N52 --> N56
    N57 --> N58
    N52 --> N58
    N43 --> N59
    N44 --> N59
    N45 --> N59
    N46 --> N59
    N47 --> N59
    N50 --> N59
    N42 --> N60
    N52 --> N60
    N48 --> N61
    N49 --> N61
    N51 --> N61
    N54 --> N61
    N56 --> N61
    N58 --> N61
    N59 --> N61
    N60 --> N61
    N43 --> N62
    N44 --> N62
    N45 --> N62
    N46 --> N62
    N47 --> N62
    N14 --> N63
    N14 --> N64
    N12 --> N65
    N65 --> N66
    N41 --> N66
    N67 --> N68
    N67 --> N69
    N67 --> N70
    N67 --> N71
    N67 --> N72
    N72 --> N73
    N67 --> N74
    N67 --> N75
    N72 --> N76
    N75 --> N76
    N75 --> N77
    N67 --> N78
    N67 --> N79
    N67 --> N80
    N67 --> N81
    N80 --> N82
    N78 --> N82
    N83 --> N84
    N67 --> N84
    N82 --> N85
    N84 --> N85
    N5 --> N85
    N43 --> N85
    N44 --> N85
    N45 --> N85
    N46 --> N85
    N47 --> N85
    N79 --> N86
    N12 --> N86
    N68 --> N87
    N74 --> N87
    N76 --> N87
    N77 --> N87
    N84 --> N87
    N12 --> N87
    N41 --> N87
    N69 --> N88
    N82 --> N88
    N76 --> N88
    N12 --> N88
    N41 --> N89
    N70 --> N90
    N71 --> N90
    N73 --> N90
    N81 --> N90
    N87 --> N90
    N88 --> N90
    N86 --> N90
    N85 --> N90
    N66 --> N90
    N62 --> N91
    N63 --> N91
    N64 --> N91
    N59 --> N91
    N60 --> N91
    N48 --> N91
    N91 --> N92
    N61 --> N92
    N90 --> N92
    N92 --> N93
    N89 --> N93
    N93 --> N94
```
<!-- jit:breakdown-overview:end -->

## Material risks and owner decisions

| Risk / decision | Resolution and rationale |
|---|---|
| D-43 REQ-18 triage | Chosen: decide case by case; an overlapping requirement moves into an overhaul child before rejection, and item-level API fixes stay in their epics with the filter label. Rejected: absorb all; leave all. |
| D-44 sweep breakdown | Chosen: break down here, as 24 module-group units of about 2.7–6k comment lines sized from INV-C. Rejected: separate later planning. |
| D-45 tutorials | Chosen: exactly two, gf2-sim link simulation and gf2-core linear algebra at scale; the library example comes first. Rejected: short-code benchmarking; algebra/permanent workflows. |
| D-46 crate-local guides | Chosen: mine them, then `git mv` to the legacy mirror with citations repointed. Rejected: delete; keep in place. |
| D-47 roadmaps | Chosen: retroactive mapping, filing gap issues. Rejected: accept the deletion as complete. |
| D-48 performance claims | Chosen: only the `docs/reference/` evidence page. Rejected: README summary; receipts only. |
| D-49 operational dirs | Chosen: `dev/bench_results`, `dev/simulation_results` and `dev/studies` are operational and remain; only their loose narrative Markdown is archived or relocated. Rejected: relocation with code and CI path changes. `dev/tools` stays under D-27 (INV-§0.5). |
| D-50 entry pages | Owner-confirmed: crate `README.md` files are the four entry pages. Rejected: separate `docs/reference/crates/*` pages. gf2-stats and the SIMD crate get concise READMEs; HIP is covered by the acceleration pages (D-18). |
| D-51 decks | Owner decision: link targets inside moved decks are repaired so every link resolves, including links broken before the move; prose and claims stay unedited (REQ-09). Rejected: recording broken links instead of repairing them, because REQ-19 requires resolved links. |
| D-52 f547c394 | Owner decision: the entry moves under its epic dir. First, rule P-02 is changed to accept recorded historical pin paths. The baseline is benchmark-acceptance verdicts and findings over every committed `zen3-benchmark-receipt-v1` `receipt.json` outside `inputs/`, recorded in `receipt-verdict-baseline.md`; verdicts must be identical after the change and again after the move. CI does not run this check. The move then switches the tooling, runner, tests and CI scripts to the new path. Rejected: keeping it as an operational exception. |
| D-53 other code-pinned entries | Owner decision: 3f664839 (rare-event design, pinned by `permanent_rare_event/artifact.rs`) and 02b8137c (frozen validation journal, pinned by `permanent_campaign/validation.rs`) follow the D-52 pattern. Each has its own validator verdict baseline record. The 02b8137c move starts only after issue 02b8137c is done or rejected, checked against the tracker at execution (no edge, for the coverage reason in the external-wiring row). For the other frozen code-pinned entries (CC-§1), `code-pinned-entry-plan` files the per-entry tolerance and move pairs under this epic; this keeps the graph bounded. Rejected: keeping code-pinned entries in place. |
| OD-08 gates | 3f29e945 DEC-01..DEC-06 hold. Manifest gates use only registered keys (D-35): leaves get `cargo-ci`, `code-review` and `doc-review`; the story adds `repo-validate` and `holistic-review`. Once 8f61d6de registers `docs-mechanical`, the execution lead adds it to every open overhaul issue. |
| PD-01 archive scope | Owner-supplied previews of all 72 terminal unarchived containers bound the work: ten archived epics are re-archived to collect their terminal stories (ARCH-§1), b7157be6 is repaired from its destination conflict (ARCH-§3), and the six open-epic stories wait for their epics (ARCH-§2, D-22). Each task re-previews before execution because ownership linking adds documents. |
| PD-02 non-JIT moves | Default: JIT archival rewrites only `documents[]`, never leaves `dev/` and cannot target the legacy mirror, so legacy, deck and Lean-guide moves follow `relocation-protocol`. |
| PD-03 dev/active cleanup | Default: generic regroup tasks move only entries with no non-comment consumer recorded by the inventory, after the sweep, because Rustdoc cites `dev/active` paths. Strays and ownerless entries follow D-24/D-25; husks are cleared after every move. |
| PD-05 AGENTS.md at 199/200 lines | Risk. Mitigation: 495807a3 REQ-05 removes superseded hand-written prose; CONTRIBUTING guidance moves in only if it fits. |
| PD-06 REQ-20 credit | Default: the triage task carries `satisfies:REQ-20` because it completes the brief's inherited-issue record; the brief already records D-43..D-52. |
| PD-07 final verification | Default: dedicated tasks for policy narrowing (REQ-15), link verification (REQ-19) and checker retirement (REQ-13). |
| PD-09 story criterion IDs | Coverage credit is transitive, so story-level criteria use ID ranges that do not collide with the epic's REQ-01..REQ-20: contract story 3f29e945 uses REQ-31..REQ-40 and the sweep story REQ-21..REQ-27. The sweep story itself carries `satisfies:REQ-17`. |
| PD-10 index ownership | Default: each page task, including the gf2-sim entry page, adds its own `docs/index.md` row; the scaffold links only pages that exist. |
| PD-11 dev orientation | Default: `dev/index.md` is rewritten for the final layout; `dev/authoring-conventions.md` stays as operational guidance. |
| External wiring | [external-wiring.json](external-wiring.json) is authoritative, and the breakdown step applies it verbatim: edges from manifest keys to existing issues, the f357b3dc and 44c98235 supersessions (each rejected `resolution:obsolete` once its replacement exists; the replacement carries its criteria), and the external `satisfies` labels, which are already present on the live issues. Every `from_key` exists in the manifest, and no edge closes a cycle. Every `to_issue` is an overhaul issue outside other epics' closures; the only foreign issue reached transitively is 0b45a5fa, a done task with no `satisfies` label and no dependencies. No edge targets another epic's issue (for example 02b8137c or ae03bcd0), because coverage credit is transitive. The three tolerance tasks are code changes that need no documentation contract, so they are spine sources with no external edge. |
| Ordering the graph cannot encode | Two preconditions. `regroup-active-remaining` (ae03bcd0 entries) and `sweep-coding-bch` run in a window with no in-flight ae03bcd0 edits to its `dev/active` dir or to gf2-coding `bch/` and `transform/` (INV-§3.14). `move-02b8137c-journal` moves nothing until issue 02b8137c is done or rejected; this is a hard criterion of the task, verified against the tracker. An edge into either epic would pull its work and labels into this subtree. |
| README ordering | `readme-landing-page` runs after the re-archive tasks, the only other tasks that edit `README.md`. Regroup and archive tasks declare `crates`, because their citation scans determine the exact files; expect rebases. |
| Concurrent edits | `contrib/gates/doc-review-prompt.md` is being edited in another session and is outside this plan's footprints. |
| Receipt and evidence integrity | Digest-pinned inputs and `dev/bench_results/` prefixes never change. Each code-pinned move follows its tolerance change and repeats the recorded verdict baseline (D-52, D-53). |
| Shared manifest file | The seven inventory shards write one manifest file; each writes a disjoint row scope, and merges are row-level. |
| Doctest timing noise | The final census records toolchain, features, cache state and host load (REQ-07). |

## Investigation sources

- [Investigation](investigation.md) holds the full inventories, consumers and
  counts. This plan cites it and does not copy them.
- [Planning brief](fa787f85-planning-brief.md) holds D-01..D-53.
- [Archive preview results](archive-preview-results.md) hold the per-epic preview counts.
- [Rustdoc example audit](fa787f85-rustdoc-example-audit.md) is the REQ-07
  baseline.
