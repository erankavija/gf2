# Plan review r1: fa787f85 documentation overhaul

Planning node 8dddfc7a. Container fa787f85. Reviewed at `c64a5f6f7`. Read-only review; no tracker state changed.

## Mechanical checks

| Check | Result |
|---|---|
| Artifacts linked to 8dddfc7a | plan.md, breakdown.json, investigation.md linked |
| `breakdown_manifest.py validate --deny-warnings` (known sources = every manifest ref; required REQ-01..REQ-20) | exit 0; advisories only (5 uncertainty, 12 overlap) |
| `render --check` | exit 0 (generated region current) |
| `jit issue batch-create --dry-run --json` | valid; 78 issues, 128 edges |
| Source refs | All resolve: REQ-01..20 (epic), REQ-21..27 (`tersification-sweep` body), D-02..D-42 (brief), DEC-01..06 (3f29e945 body), OD-01..08 and PD-01..11 (plan table), INV-§0.1..0.11, §1.1, §1.2, §2, §3.1, §3.4–3.6, §3.9, §3.10, §3.13, §3.14, §4, §5.2, §5.3, §6, INV-A/B/C (investigation headings). No invented refs. |
| Contracts | Plan-fixed contracts have no producer. Implementation-produced contracts have one reachable producer: `performance-evidence-page` (consumers reach `reference-performance-evidence`, audit via `legacy-move-gf2-core-guides`), `sweep-baseline`, `fieldmatrix-example`. |
| Spine | Sources: triage-doc-issues, roadmap-coverage-map, remove-contributing-guide, three inventories, sweep-baseline-census, docs-scaffold-index, fieldmatrix-example-program; each reaches 3f29e945 via the recorded re-homes. Sinks: triage-doc-issues, roadmap-coverage-map, retire-migration-checker. |
| Warning overrides | None present. |

## Coverage-label audit

- `label-coverage` walks dependencies transitively and stops at planning/breakdown types (`just-in-time/crates/jit/src/validation/graph.rs:775-900`; `.jit/rules.toml:78-93`). So any `satisfies:REQ-NN` in the subtree credits epic REQ-NN.
- Contract-story subtree: 495807a3 REQ-31/32/33, 56378e82 REQ-34, dcd38c45 REQ-35, 8f61d6de REQ-36/37/40, 34adff85 REQ-38. None collide with REQ-01..20.
- Sweep labels REQ-21..27 collide with nothing on the epic.
- Externals pulled in: 12907582 → 0b45a5fa (done, no `satisfies`). Their labels credit nothing falsely. 4ad869d6 and ae03bcd0 stay outside, which matters because their `satisfies:REQ-02..16` labels would falsely credit the epic.
- Task-local criterion IDs (REQ-01..05 in leaf bodies) are not evaluated by any rule. Only epics and breakdowns fire.
- One partial false credit: `triage-doc-issues` carries `satisfies:REQ-20`, but it does not deliver what REQ-20 asks for. See B8.

## Assignment simulation (finest tier: 77 tasks)

Legend: P = pass, F = fail. A sweep unit means one of the 24 `sweep-*` module-group tasks other than census and completion. They share one shape: consumer family is the crate's source readers, the test boundary is an ignore-whitespace diff review plus the Rust CI gate, and each covers 1–6k comment lines, which is the size the owner chose in OD-02.

| Key | One outcome | One bounded consumer family | Observable test boundary | Footprint credible | One focused cycle | No inner decomposition | No mixed deliverables | Result |
|---|---|---|---|---|---|---|---|---|
| triage-doc-issues | P | P | P | P | P | P | P | P |
| roadmap-coverage-map | P | P | P | P | P | P | P | P |
| remove-contributing-guide | P | P | P | P (README overlap with external 44c98235, see B5) | P | P | P | P |
| inventory-dev-active | P | P | P | P (dir + concrete uncertainty) | P | P | P | P |
| inventory-dev-buckets | P | P | P | P | P | P | P | P |
| inventory-permanent-sources | P | P | P | P | P | P | P | P |
| expand-managed-paths | P | P | P | P | P | P | P | P |
| link-owned-artifacts | P | P | P | P | P | P | P | P |
| archive-candidate-preview | P | P | P | P | P | P | P | P |
| repair-osd-archive | P | P | P | P | P | P | P | P |
| execute-terminal-archives | P | P | P | F: Rustdoc/README files it repoints are undeclared (B6) | P (only b7157be6 is terminal-unarchived; the rest are extensions of archived epics) | P | P | F |
| remove-active-husks | P | P | P | P | P | P | P | F: ordered before the moves that create husks (B7) |
| regroup-active-zen3-ci-inputs | P | P | F: the Rust CI gate does not cover receipt-pin verification of committed receipts | F: omits `dev/tools/tuning-campaign-support` readers (B1) | F | P | P | F |
| regroup-active-zen3 | P | P | P | F: omits crates/ Rustdoc citations and `dev/scripts/receipt-input-omissions.json` (B6) | P | P | P | F |
| regroup-active-field-dispatch | P | P | P | F (B6) | P | P | P | F |
| regroup-active-remaining | P | P | P | F (B6) | P | P | P | F |
| resolve-dev-strays | P | P | P | P | P | P | P | P |
| sweep-baseline-census | P | P | P | P | P | P | P | P (add external edges, B5) |
| sweep-core-field-poly | P | P | P | P | P | P | P | P |
| sweep-core-field-dense | P | P | P | P | P | P | P | P |
| sweep-core-field-algorithms | P | P | P | P | P | P | P | P |
| sweep-core-field-traits | P | P | P | P | P | P | P | P |
| sweep-core-gf2m | P | P | P | P | P | P | P | P |
| sweep-core-prime-fields | P | P | P | P | P | P | P | P |
| sweep-core-bit-structures | P | P | P | P | P | P | P | P |
| sweep-core-runtime | P | P | P | P | P | P | P | P |
| sweep-core-tests-benches | P | P | P | P | P | P | P | P |
| sweep-coding-ldpc | P | P | P | P | P | P | P | P |
| sweep-coding-bch | P | P | P | P | P | P | P | P (stale sequencing note, B5) |
| sweep-coding-modem | P | P | P | P | P | P | P | P |
| sweep-coding-decoders | P | P | P | P | P | P | P | P |
| sweep-coding-core-src | P | P | P | P | P | P | P | P |
| sweep-coding-tests-benches | P | P | P | P | P | P | P | P |
| sweep-sim-campaigns | P | P | P | P | P | P | P | P |
| sweep-sim-pipeline | P | P | P | P | P | P | P | P |
| sweep-sim-runtime | P | P | P | P | P | P | P | P |
| sweep-sim-tests-bins | P | P | P | F: misses `crates/gf2-sim/benches/` (B3) | P | P | P | F |
| sweep-algebra-packed | P | P | P | P | P | P | P | P |
| sweep-algebra-rest | P | P | P | F: `crates/gf2-algebra/src` contains `src/packed` (B4) | P | P | P | F |
| sweep-simd-x86 | P | P | P | P | P | P | P | P |
| sweep-simd-rest | P | P | P | F: `src` contains `src/x86`; `tests`, `benches` do not exist (B4) | P | P | P | F |
| sweep-hip-stats | P | P | P | P | P | P | P | P |
| sweep-completion-record | P | P | P | P | P | P | P | P |
| eliminate-dev-plans | P | P | P | P | P | P | P | P |
| eliminate-dev-sessions | P | P | P | P | P | P | P | P |
| eliminate-dev-presentations | P | P | P | P | P | P | P | P |
| relocate-bench-narrative | P | P | P | P | P | P | P | P |
| relocate-sim-studies-narrative | P | P | P | P | P | P | P | P |
| move-presentation-decks | P | P | P | P | P | P | P | P |
| retarget-figure-generator | P | P | P | P | P | P | P | P |
| docs-scaffold-index | P | P | F: REQ-01 and REQ-03 contradict each other (B2) | P | P | P | P | F |
| entry-page-gf2-core | P | P | P | P | P | P | P | P |
| entry-page-gf2-coding | P | P | P | P | P | P | P | P |
| entry-page-gf2-algebra | P | P | P | P | P | P | P | P |
| entry-page-gf2-sim | P | P | P | P (needs index row, B2) | P | P | P | P |
| concept-acceleration-architecture | P | P | P | P | P | P | P | P |
| backend-crate-readmes | P | P | P | P | P | P | P | P |
| concept-field-arithmetic | P | P | P | P | P | P | P | P |
| reference-performance-evidence | P | P | P | P | P | P | P | P |
| howto-select-acceleration | P | P | P | P | P | P | P | P |
| howto-reproduce-evidence | P | P | P | P | P | P | P | P |
| howto-run-campaigns | P | P | P | P | P | P | P | P |
| howto-formal-verification | P | P | P | P | P | P | P | P |
| reference-standards-conformance | P | P | P | P | P | P | P | P |
| reference-supported-configurations | P | P | P | P | P | P | P | P |
| tutorial-link-simulation | P | P | P | P | P | P | P | P |
| fieldmatrix-example-program | P | P | P | P | P | P | P | P |
| tutorial-linear-algebra | P | P | P | P | P | P | P | P |
| archive-lean-pipeline-doc | P | P | P | P | P | P | P | P |
| legacy-move-gf2-core-guides | P | P | P | P | P | P | P | F: runs concurrently with sweep units on `src/field` Rustdoc (B6) |
| legacy-move-gf2-coding-guides | P | P | P | P | P | P | P | P |
| verify-rustdoc-examples | P | P | P | P | P | P | P | P |
| audit-permanent-content | P | F: "researchers adopting gf2" across every module doc | P | F: `crates` | F: covers module-level Rustdoc in 330 source files plus unbounded fixes (B9) | F | P | F |
| rewrite-dev-index | P | P | P | P | P | P | P | P |
| finalize-docs-policy | P | P | P | P | P | P | P | P |
| verify-final-links | P | P | P | P | P | P | P | P |
| retire-migration-checker | P | P | P | P (concrete uncertainty) | P | P | P | P |

## Blocking findings

### B1. The f547c394 regroup breaks receipt verification

The plan states that f547c394 "moves together with its two readers" (plan "Receipt and CI integrity", PD-03, the REQ-11 row). That claim is false. Rust code in a workspace member hard-codes the path and checks it against committed receipts:

- `dev/tools/tuning-campaign-support/src/protocol.rs:25` defines `PROTOCOL_PATH = "dev/active/f547c394/protocol.md"`.
- `:27` defines `ADDENDUM_SCHEMA_PATH`.
- `:1102` hard-codes the default `producing_manifest_path`.
- `src/receipt.rs:1028-1048`, rule P-02, rejects any receipt whose pin path is not exactly equal to `PROTOCOL_PATH` or `ADDENDUM_SCHEMA_PATH`.
- `src/bin/benchmark-ab-runner.rs:145` and `tests/protocol_contracts.rs:58-63,341-466` also read these paths.
- `tests/addendum_schema_versions.rs:35-51` reads them too.
- 783 files under `dev/bench_results/` quote `dev/active/f547c394`.

Changing the constants fails P-02 for every existing receipt. Leaving them unchanged breaks the runner and its tests. This is the `behavioral-evidence-validity` hazard from INV-§6.

**Correction:** get an owner decision, record it as an OD row, and replace `regroup-active-zen3-ci-inputs` accordingly. Two options:

- (a) Keep `dev/active/f547c394/` in place as a retained operational asset. This needs a manifest row, an explicit exception in `active-layout` and the REQ-11 row, and deletion of the task.
- (b) Redesign the task with a footprint that covers `dev/tools/tuning-campaign-support/{src/protocol.rs,src/receipt.rs,src/bin/benchmark-ab-runner.rs,tests/protocol_contracts.rs,tests/addendum_schema_versions.rs}`. Add a criterion that every committed receipt still verifies under P-02, for example by accepting the recorded historical pin path. Make the test boundary the receipt-verification run over committed receipts.

Then fix the plan's integrity row.

### B2. `docs-scaffold-index` cannot pass its own criteria

REQ-01 requires the index to link the gf2-sim entry page. REQ-03 requires that the index link only pages present in the tree. `crates/gf2-sim/README.md` does not exist (INV-§3.5), and `entry-page-gf2-sim` depends on the scaffold.

**Correction:** REQ-01 links the three existing crate READMEs and states that the gf2-sim row is added by its entry-page task. Add to `entry-page-gf2-sim` a criterion: "`docs/index.md` crate section links `crates/gf2-sim/README.md`". Add `docs/index.md` to its `touches`. This follows PD-10.

### B3. Sweep coverage gap

Story REQ-21 covers every Rust file under `crates/`, including benches. `crates/gf2-sim/benches/nr_5g_realtime.rs` is in no unit's scope or footprint. This was checked against `git ls-files` for all 704 crate `.rs` files.

**Correction:** add `crates/gf2-sim/benches/` to the `sweep-sim-tests-bins` scope sentence, title/outcome ("tests, benches, binaries and examples") and footprint.

### B4. Two sweep footprints are not credible

- `sweep-algebra-rest` touches `crates/gf2-algebra/src`, which contains `src/packed`, the scope of `sweep-algebra-packed`. **Correction:** use `crates/gf2-algebra/src/{gpu,gray,lib,parallel,testutil,tuning}.rs`, `src/permanent`, `tests`, `benches`, `examples`.
- `sweep-simd-rest` touches `crates/gf2-kernels-simd/src`, which contains `src/x86`. It also lists nonexistent `tests` and `benches`. **Correction:** use `crates/gf2-kernels-simd/src/bipedal` plus the top-level `src/*.rs` files: `bch_encode`, `clmul_scalar`, `fp65537`, `fp_generic`, `fp_medium`, `fp_medium_f64`, `fp_medium_ple`, `fp_small`, `fp_small_f32`, `fp_small_panel`, `fp_small_ple`, `gf2m`, `gf2m_batch`, `gf2m_gemm`, `gf2m_wide`, `lib`, `llr`, `mersenne`, `modem`, `prefetch`, `shift_funnel`, `transpose`. Drop `tests` and `benches`.

### B5. Unenforced external ordering is not minimized, and part of it is stale or unstated

- **a0a29512 and 153297cf.** Both edit Rustdoc examples across the workspace while the 24 sweep units edit the same doc blocks. Nothing orders them, and the plan does not state the overlap. 153297cf's own Notes require it to run after a0a29512, and that ordering is neither encoded nor listed. Inbound edges can enforce most of this. **Correction:**
  - Add re-home edges `sweep-baseline-census` → a0a29512, `sweep-baseline-census` → 153297cf, and `sweep-baseline-census` → 12907582. These are key-depends-on-external, so they stay inside the spine contract.
  - Drop the "12907582 after the sweep units" sentence.
  - State "153297cf after a0a29512" as the remaining ordering the graph cannot encode.
- **44c98235 (README).** Its REQ-06/REQ-07 need `docs/index.md` and the tutorials to exist. It also edits `README.md` concurrently with `remove-contributing-guide`. It becomes claimable as soon as 3f29e945 closes. **Correction:**
  - Supersede it as with f357b3dc: add manifest task `readme-landing-page`. Carry 44c98235's seven criteria as standalone criteria and add `satisfies:REQ-03`.
  - Make it depend on `docs-scaffold-index`, `tutorial-link-simulation`, `tutorial-linear-algebra` and `remove-contributing-guide`. Make `audit-permanent-content` depend on it.
  - After creation, reject 44c98235 with `resolution:obsolete` and a comment naming the replacement.
  - Remove 44c98235 from the re-home and label-credit rows.
- **4ad869d6.** "sweep-coding-bch after 4ad869d6" is stale: 4ad869d6 is done (`d0a28afec`). **Correction:** restate against the current open ae03bcd0 work, or delete it.
- **ae03bcd0 quiet window.** `regroup-active-remaining` requires a quiet window for ae03bcd0 entries, but the plan's sequencing row does not list it. **Correction:** add it to the "External sequencing the graph cannot encode" row.

### B6. Citation-repointing tasks declare incomplete footprints and race the sweep

`regroup-active-zen3`, `regroup-active-field-dispatch`, `regroup-active-remaining` and `execute-terminal-archives` must repoint in-file citations in Rustdoc. Examples:

- `dev/active/7d824b2f` in `crates/gf2-core/src/compute/field.rs:123`, `crates/gf2-core/src/field/vec.rs:28` and `crates/gf2-algebra/src/permanent/parallel_bipedal3.rs:26,51`.
- `dev/active/fc182ed5` in `crates/gf2-core/src/lib.rs:336`, `crates/gf2-core/src/gfp/simd_ops.rs:905` and `crates/gf2-kernels-simd/src/fp_small_panel.rs:15,66`.
- `dev/active/4e732b56` in gf2-coding `bch/` and benches.
- `recorded_in` paths in `dev/scripts/receipt-input-omissions.json`.

`legacy-move-gf2-core-guides` edits `src/field/winograd.rs` and `expr.rs`. None of these tasks declares those files, so the validator's overlap analysis cannot see the conflicts. All of them run in parallel with sweep units that remove design-history provenance from the same lines (story REQ-22). `eliminate-dev-plans` is already ordered after the sweep for exactly this reason.

**Correction:**
- Add `tersification-sweep` to `depends_on` of the three regroup tasks, `execute-terminal-archives` and `legacy-move-gf2-core-guides`. No cycle results.
- Add the repointed crate paths (or `crates` with a concrete uncertainty) and `dev/scripts/receipt-input-omissions.json` to their `touches`.

### B7. `remove-active-husks` runs before the moves that create husks

It depends only on `inventory-dev-active`. Archive execution, regrouping, stray resolution and narrative relocation all leave new empty directories after it runs.

**Correction:** make it depend on `execute-terminal-archives`, `resolve-dev-strays`, all regroup tasks, `relocate-bench-narrative` and `relocate-sim-studies-narrative`.

### B8. REQ-20 is under-delivered

REQ-20 requires the epic-linked planning brief to record the complete interview outcome, including every approved scope decision and deferred breakdown concern. Today's OD-01..08 and the owner-confirmed PD-04 and PD-08 exist only in plan.md. plan.md is linked to 8dddfc7a, not the epic, and it is not the brief. Those decisions include the tutorial choice, `dev/sessions` elimination and the crate-README entry pages. `triage-doc-issues` only adds dispositions (its REQ-05), yet it carries `satisfies:REQ-20`. The brief also gives the reviewer no way to verify that the owner confirmed these decisions.

**Correction:** before approval, the planner adds an "Owner decisions (2026-10-01)" section to `fa787f85-planning-brief.md`. It states OD-01..08, PD-04 and PD-08, links the INV-§0 scope findings, and records the f357b3dc supersession plus the re-homes. Then update the plan's REQ-20 row. Alternatively, add an equivalent hard criterion to `triage-doc-issues`.

### B9. `audit-permanent-content` is not worker-sized

REQ-01 audits crate- and module-level Rustdoc. Module docs appear in 330 source files (`git grep -l '^//!'`). REQ-02 then requires every finding to be fixed in the same change. Module-level doc conformance is already the sweep story's REQ-21/REQ-22, and the story credits REQ-17.

**Correction:** limit REQ-01 to the root README, `docs/`, crate READMEs, `AGENTS.md` and crate-root `lib.rs` docs. Narrow the footprint from `crates` to those files.

## Advisories (non-blocking)

- The three inventory tasks write one manifest file in parallel. Expect merge conflicts; a24b2af7's schema could allow per-scope files.
- Eleven page tasks touch `docs/index.md`. PD-10 accepts this.
- `regroup-active-zen3` and `regroup-active-zen3-ci-inputs` move files into the same directory in parallel.
- `migration-manifest` is labelled plan-fixed but is produced by external a24b2af7. State the external producer in the contract text.
- `retarget-figure-generator` and `sweep-coding-tests-benches` both edit `gen_presentation_figures.rs`.
- Tooling Markdown under `.agents/`, `contrib/`, `packages/` and `.jit/` is outside every inventory. State the exclusion in the REQ-13 row.
- Plan lines 15–21 restate graph order. PD-07 already holds it, so the paragraph can be cut. Otherwise the plan is concise: about 150 hand-written lines, with no copied bodies or review history.
- After rejection, f357b3dc keeps `epic:documentation-overhaul` outside the DAG, so `jit query divergence` will report it.

## Required follow-up

Fix the manifest first, then the plan rows named above. Regenerate the overview and rerun validate (`--deny-warnings`), `render --check` and the batch-create dry run.

VERDICT: FAIL
