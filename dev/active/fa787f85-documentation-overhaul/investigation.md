# fa787f85 investigation: documentation overhaul premises

**Issue:** fa787f85 (epic), planning node 8dddfc7a
**Date:** 2026-10-01
**Scope:** read-only verification of the epic's 20 [hard] REQs, the planning brief (D-01..D-42), and today's owner decisions against the current tree at `fca6df53d`.

Counts below are audit evidence for planning. They are not product facts.

## 0. Findings that change scope

1. **JIT archival does not reach most legacy sources.** `jit archive` moves only files under `development_root = "dev"`. It never moves `crates/*/docs/**`, `CONTRIBUTING.md` or root files, and it can only copy permanent-path sources (`just-in-time/crates/jit/src/domain/artifact_classifier.rs:847-879`; `just-in-time/docs/reference/configuration.md:85-92`). The crate-docs legacy move therefore needs either a policy change (`development_root` widened) or plain `git mv`.
2. **Archival rewrites no file content.** It repoints only issue `documents[]` records (`domain/artifact_plan.rs:350-359`; `repository_state/archive.rs:177-198`; `docs/reference/cli-commands.md:247-252`). In-file citations raise advisory `moving-path-citation` warnings only, and only inside `citation_scan_roots` (unset in gf2, so `dev` plus `permanent_paths`). Citations in README, AGENTS.md, `crates/**` and rustdoc get no warning. REQ-10/REQ-19 "valid rewritten references" is a manual planned step per archive.
3. **No legacy-mirror primitive.** `jit archive document <path> --execute` accepts unowned managed files (`no-owner`), but its destination is `dev/archive/<path minus dev/>`, never `dev/archive/legacy/<path>` (`artifact_classifier.rs:1479-1484`). D-25 needs `git mv` plus manual reference updates, or an untested temporary `archive_root = "dev/archive/legacy"` pass.
4. **`dev/bench_results`, `dev/simulation_results` and `dev/studies` are operational, not documentation buckets.** `bench_results` holds 49,828 tracked files: 145 campaign receipts with frozen `inputs/producing/` snapshots, read by CI (`scripts/cargo-ci.sh:240-243` running `dev/scripts/check-receipt-input-snapshots.py:136`, `check-addendum-schema-versions.py:181,198`). `simulation_results` is read by gf2-sim production code (`crates/gf2-sim/src/permanent_campaign/schema.rs:45` `DATASET_HOME`) and by tests. `studies` is read through `include_bytes!` (`crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:178,214`). D-28 elimination of `bench_results`/`simulation_results` needs purpose-specific relocation with code and CI edits, and the check scripts silently skip receipts moved out of the hard-coded prefix.
5. **`dev/tools` must be kept.** It is a workspace member (`Cargo.toml:9`) and a path dependency of gf2-core and gf2-algebra (`crates/gf2-core/Cargo.toml:32`, `crates/gf2-algebra/Cargo.toml:36`). D-27 omits it.
6. **dev/active is mostly live work, not leftovers.** Of 118 entries, 43 are empty untracked husks from executed archives, 70 belong to non-terminal epics (63 of them flat, not under `<epic>-<slug>/`), 3 hold terminal-owned stray files, and 2 have no defensible owner. The brief's "every mapped issue is terminal done" baseline (2026-07-14) is stale.
7. **b7157be6 was archived by hand.** `dev/archive/b7157be6-osd/` has no `.jit-container` marker, the epic is `done` not `archived`, and it still links `dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md`, shared with four non-terminal epics (commit `e034dfaab`).
8. **Root `AGENTS.md` is 199 lines.** REQ-02's 200-line cap leaves one line for the D-23 convention, the recursive-scope rule (56378e82 REQ-02) and new invariant projection lines. Each projected invariant adds one line to the region.
9. **Brief and child issues contradict the epic on README performance.** Epic REQ-03 bars README performance claims and owner decision D-15 agrees. The brief's REQ-03 text (brief line 36) still requires "an evidence-linked performance summary", its REQ-04 still says "research-grade workflows" instead of exactly two tutorials, and 44c98235 REQ-05 still requires README performance highlights.
10. **REQ-18 candidate list is incomplete.** Six more open doc issues in epic 86b9c719 conflict with D-06/D-07 or are subsumed: 0056e853, 315f4de5, 35007c4c, 5f3d0ff9, be331e20, 68189a0a.
11. **The doc-review gate is already automated.** `.jit/gates.toml:163-180` runs `contrib/gates/ai-review.sh` with a reviewer agent; D-33's "manual placeholder" premise is stale. Today's commit `a47ac3c71` added an archive-exclusion clause that makes the live prompt diverge from the profile asset `packages/sim-research/assets/live/contrib/gates/doc-review-prompt.md`.

## 1. Claim classification

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | REC-10 archival mostly done: 12 epics archived with dirs; b7157be6 done with `dev/archive/b7157be6-osd` | **Partly already-done; partly invalid as stated** | `jit query all --json`: 12 epics `archived` (97bf0879, 026fc832, 2928ccce, ae82bd73, babcf05e, e095a100, bb85c68a, 6efb756b, d4851c3d, 806eb14e, 7be754bd, f9717e7e), each with `dev/archive/<id>-<slug>/.jit-container` holding the full UUID (e.g. `dev/archive/6efb756b-grand/.jit-container`). Executed in `195f8254f` (2026-08-08). b7157be6 is `done`, its dir has **no marker**, it was moved by plain git (`e034dfaab`), and one linked doc stays in `dev/active/aed96ef9-.../`. Terminal-owned linked docs outside `dev/archive`: see §1.1. `jit archive candidates` result: §1.2. |
| 2 | dev/active has ~118 entries | **Valid; composition differs from premise** | `ls dev/active \| wc -l` = 118; tracked files 1,948 (`git ls-files dev/active`). Class counts (a) 70, (b) 46 (43 empty untracked husks + 3 with files), (c) 2. Appendix A. |
| 3 | dev/ bucket inventory and consumers; D-27 keep / D-28 eliminate | **Valid-and-open, with scope corrections** | §3 and Appendix B. `tools` needs keeping; `bench_results`, `simulation_results`, `studies` have hard code/CI consumers; `plans` 12 files (brief said 62); `presentations` 2 css files; `sessions` 3 md files. |
| 4 | Presentation decks belong to terminal epics | **Valid** | All four owner epics `archived`; each deck sits in its owner's `dev/archive/<epic>/docs/presentations/` directory. §3.4. |
| 5 | Crate-local guides are fact sources to mine, then move to `dev/archive/legacy/<path>` | **Valid; move is manual** | 9 + 25 tracked md files (§3.5). JIT never moves `crates/**` (§4). Inbound links from crate READMEs need repointing or removal. |
| 6 | CONTRIBUTING.md removable | **Valid-and-open** | 58 lines, a pointer to AGENTS.md plus commit/PR guidance (`CONTRIBUTING.md:1-58`). Live consumer: `README.md:195` only. §3.6. |
| 7 | JIT doc policy and archive primitives | **Verified; three gaps** | §4: no content rewrite, no non-`dev/` moves, no legacy subdirectory. Container archival is idempotent. |
| 8 | README and docs/index.md vs REQ-03 | **Valid-and-open** | `README.md` is 201 lines; omits gf2-sim from the layout table (`README.md:24-34`); contains an API tour (`:51-106`), marketing ("compete with specialized CAS", `:11`), future-facing "Good first areas" (`:197`), links to `crates/*/docs/` (`:189`) and CONTRIBUTING (`:195`). No `docs/index.md` exists (`find docs -type f`: 24 files, decks plus `docs/lean4-verification-pipeline.md`). |
| 9 | Roadmaps deleted in 61c3f0a6a; retroactive check needed | **Already-done (deletion); valid-and-open (coverage check)** | §3.9: counts and locations. |
| 10 | Evidence for docs/reference performance page | **Valid; no receipt class carries all six REQ-06 fields in-file** | §3.10. |
| 11 | Tutorial feasibility | **Valid; gf2-sim strong, gf2-core has no FieldMatrix example** | §5.2. |
| 12 | Comment volume per crate/module | **Measured** | Appendix C. |
| 13 | Other-epic doc issue states | **Measured; list incomplete** | §3.13. |
| 14 | Concurrency today | **Valid; working tree now clean** | §3.14. |

### 1.1 Terminal-owned linked documents outside dev/archive

Method: scan of `.jit/issues/*.json` `documents[].path`, owners resolved to top epic through reverse dependencies.

- **b7157be6 (`done`)**
  - `dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md`, also linked by non-terminal epics aed96ef9, 55087229, c7cfd37e, cce5da8c: a `document-non-terminal-owner` copy case, not a move.
  - `dev/bench_results/2026-08-27-258be082-osd-campaign-worker-scaling.md` (bug 258be082).
  - `dev/simulation_results/osd-ebch-128-64/` (8 files, simulation cef1ae5f). Permanent path, so copy-only; consumed by `crates/gf2-sim/tests/osd_campaign_protocol.rs:807`.
- **97bf0879 / 026fc832 (`archived`), copied not moved by `195f8254f`:** `dev/plans/{flint_promotion_evidence,ntl_promotion_evidence,small_prime_kernel_strategy,sota_target_matrix}.md`, `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md`.
- **Archived epics, code and tooling links (stay in place):** `benchmarks/{Containerfile,README.md,image.lock,run.sh,reference/*}`, `crates/gf2-algebra/{README.md,examples/permanent_demo.rs,tests/data/cas_permanent_f5_f7.csv}`, `crates/gf2-coding/examples/bench_sparse_csv_emitter.rs`, `crates/gf2-core/{examples/m4rm_multiply_perfstat.rs,src/sparse.asm.txt,tests/gf2pow32_constant_drift.rs}`, `proofs/Gf2Algebra/Proofs/RyserBounded.lean`, `scripts/{generate-cas-permanent-vectors.sage,plot_permanent_benchmarks.py}`.
- **Unlinked terminal-owned files still in dev/active:** `babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md`, `e095a100-gfpm-arithmetic/e095a100-presentation/themes/gruvbox.css`, `37e0b235/gpu-batch-ldpc-bp-plan.md` (rejected task under 806eb14e).
- **Unlinked terminal-owned files in dev/presentations:** `babcf05e-*/themes/gruvbox.css`, `f9717e7e-gf2-sim/themes/gruvbox.css`.

### 1.2 `jit archive candidates`

Not obtained. `jit archive candidates` ran for the 30-minute background limit with no output and was stopped; a single-container preview (`jit archive container b7157be6`) was refused by the session's permission policy. The planner must run `jit archive candidates --json` with a longer budget, or preview each container separately, before the archive tasks. Expected blockers from the evidence above: b7157be6 `document-non-terminal-owner` on the shared aed96ef9 review file, no marker on `dev/archive/b7157be6-osd/`. `cli-commands.md:374-389` documents adoption of a markerless bare `<short8>/` directory only, so this markerless slugged directory may produce a destination conflict (unverified), and copy-only permanent-path documents.

## 2. Prior-art sweep

| Artifact | Kind | Relevance |
|---|---|---|
| `dev/active/fa787f85-documentation-overhaul/fa787f85-planning-brief.md` | design brief | D-01..D-42. Stale points: line 36 REQ-03 (README perf summary), line 37 REQ-04 (tutorial count), D-33 (placeholder review), baseline counts (lines 18-24). |
| `dev/active/fa787f85-documentation-overhaul/fa787f85-rustdoc-example-audit.md`, `-example-verdicts.tsv`, `-example-census.py` | REQ-07 audit inputs | Linked to fa787f85 and a0a29512; reusable baseline for the example audit. |
| `3f29e945` description, DEC-01..DEC-06 | story decisions | DEC-04 keeps the AI-review wrapper; DEC-05 docs-mechanical is stdlib Python under `contrib/gates/`; DEC-06 rustdoc/doctests inside `scripts/cargo-ci.sh`. |
| `dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md:138-196` | drift audit | 20 doc-drift findings in CONTRIBUTING, README, decks; the source of 84db2984 and several REQ-18 candidates. |
| `dev/active/DOCUMENTATION_AUDIT.md` | 2026-02-20 audit (no jit tag, commit 13e82164f) | Class (c) legacy; proposes a CONTRIBUTING template (`:526,619`) that conflicts with D-13. |
| `dev/archive/legacy/crates/gf2-core/docs/archive/DOCUMENTATION_AUDIT_{PLAN,REPORT}.md`, `dev/archive/legacy/crates/gf2-coding/docs/archive/QUALITY_AUDIT_*` | pre-JIT audits | Legacy; mine only for verified facts. |
| `dev/archive/b7157be6-osd/active/b7157be6-completion-report.md` and `e034dfaab` message | manual archive precedent | Records that 58 done-issue descriptions name old `dev/active` paths and were left unchanged. |
| `195f8254f` | 12-epic archive execution | Shows R097/R099 renames and hand edits to `crates/gf2-algebra/README.md`: the reference repointing was manual. |
| `.agents/skills/jit-project-lead/scripts/standards-scan.sh:82` | tooling | Defaults permanent paths to `docs/`; a reuse point for the doc footprint. |

No existing study or decision document covers dev-bucket elimination, the legacy mirror, or deck relocation.

## 3. Consumer sweep

### 3.1 dev/ top level (Appendix B has full counts)

| Path | Tracked files | Brief disposition | Hard consumers (code, CI, Cargo) | Verdict |
|---|---|---|---|---|
| `plans` | 12 md | eliminate (D-28) | none reading files; printed/cited paths in `dev/research/f{5,7}_packing/src/main.rs:131`, `proofs/Gf2Algebra/Proofs/*Correctness.lean:4-5`, `RyserBounded.lean:7`, `scripts/asm-artefact-present.sh:104`, `scripts/fix-aeneas-gf2algebra.py:4`, `crates/gf2-algebra/Cargo.toml:112,140,147`, ~30 rustdoc files | archivable; 29 of 33 cited `dev/plans/*.md` paths already dangle |
| `bench_results` | 49,828 (31,145 rs, 5,365 json, 664 md, ...) | eliminate (D-28) | `dev/scripts/check-receipt-input-snapshots.py:136,324,376-377` and `check-addendum-schema-versions.py:181,198,314` (CI `scripts/cargo-ci.sh:240-243`); `dev/scripts/receipt-input-omissions.json` (5 path keys); `dev/scripts/verify-campaign-log.test.sh:14,94-96`; `dev/scripts/perf-stat-c1.sh:14` (writer); `crates/gf2-algebra/tests/rank_event_rates.rs:20`; `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs` (11 lines) | relocation only, with CI/test edits; 5 owner issues open (2037941f, 4c1e441f, ad2a6a58, f63a2464, 3eeb57f6) |
| `simulation_results` | 283 (+14 ignored) | eliminate (D-28) | `crates/gf2-sim/src/permanent_campaign/schema.rs:45`, `validation.rs:66-78`, `provenance.rs:1967`, `permanent_rare_event/artifact.rs:85-90`, `bin/permanent_campaign.rs:55`; tests in gf2-sim and `crates/gf2-coding/tests/grand_phase1_smoke.rs:417-476`; all 50 `dev/campaigns/*.toml` `output_dir`; `.gitignore:91` | operational dataset store; relocation needs code edits |
| `presentations` | 2 css | eliminate | `.jit/config.toml:163,165` only | archivable now (owners babcf05e, f9717e7e archived) |
| `sessions` | 3 md | unclassified | none; 1 relative link from `dev/benchmarks/gf2_algebra_permanent/README.md:26` | 2 name issues (b488f02c, b8206228); 1 research-frontier handoff with no id |
| `studies` | 306 (+394 ignored) | unclassified | `include_bytes!` at `crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:178,214`; `bin/permanent_campaign.rs:57`; `tests/permanent_campaign_bin.rs:13`; `dev/scripts/permanent-campaign-runner.sh:35` | owner epic b8206228 is backlog: stays (D-22); operational inputs |
| `campaigns` | 50 toml | keep (D-27) | `crates/gf2-coding/src/bin/sim_runner.rs`, `tests/grand_phase1_smoke.rs`, `crates/gf2-sim/tests/osd_campaign_protocol.rs` | keep |
| `research` | 141 (+9,828 ignored) | keep | `Cargo.toml:14,21-22`, `crates/gf2-kernels-simd/Cargo.toml:34`, `.jit/gates.toml:195,324,344`, `.github/workflows/ci.yml:178`, `ci-slow.yml:80` | keep |
| `reference_data` | 45 | keep | `include_str!` at `crates/gf2-algebra/tests/tuning_repository_envelopes.rs:12`, `dev/tools/tuning-profile-compose/src/main.rs:545` | keep |
| `benchmarks` | 172 (+88 ignored) | keep | ~60 consumers incl. `crates/gf2-core/src/tuning/baked.rs`, gf2-sim bins/tests | keep; but it is in `managed_paths`, so a container archive can move receipts out of it |
| `scripts` | 25 | keep | `scripts/cargo-ci.sh:237-244`, `AGENTS.md:47,51,121,125`, `Cargo.toml:63` | keep |
| `tools` | 34 (+1,129 ignored) | unclassified | `Cargo.toml:9` member; path deps `crates/gf2-core/Cargo.toml:32`, `crates/gf2-algebra/Cargo.toml:36`; `scripts/cargo-ci.sh:267` | **keep** (add to D-27) |
| `index.md` (117 lines), `authoring-conventions.md` (164 lines) | 2 | unclassified | `.jit/config.toml:164` | `index.md` describes the bucket layout being removed: rewrite or delete |
| `archive/legacy` | absent | target | — | to be created |

Other CI reads of dev/active: `dev/scripts/check-campaign-producing-closure.py:33` reads `dev/active/f547c394/producing-inputs.json`; `check-addendum-schema-versions.py:53` reads `dev/active/f547c394/addendum*.schema.json` (both in `scripts/cargo-ci.sh:238-243`). Regrouping f547c394 under `dev/active/1a379447-.../` breaks CI unless these paths change in the same commit.

Stale config entries: `managed_paths` lists absent `dev/design`, `dev/experiments`; `permanent_paths` lists absent `dev/architecture`, `dev/eval`, `dev/vision`, `dev/TESTING.md` (`.jit/config.toml:163-164`).

### 3.4 Presentation decks

| Deck | Owner epic (state) | Local assets | Outbound repo links | Inbound refs |
|---|---|---|---|---|
| `dev/archive/6efb756b-grand/docs/presentations/6efb756b-grand-sogrand/` | 6efb756b (archived) | `base.css`, `themes/{rust,gruvbox}.css`, `../figures/fig{4,5,6}_*.svg` | `dev/campaigns/*`, own-epic archive documents | `dev/active/ae03bcd0-general-bch/investigation.md:277-278`, `dev/archive/6efb756b-grand/active/6efb756b-completion-report.md:235`, b4b4b9ee report `:151,154` |
| `dev/archive/ae82bd73-gf2-algebra-permanent/docs/presentations/ae82bd73-gf2-algebra-permanent/` | ae82bd73 (archived) | `base.css`, `themes/{rust,gruvbox}.css` | `dev/benchmarks/gf2_algebra_permanent/README.md`, `dev/benchmarks/permanent_campaign/backend-ordering.md`, `dev/studies/b488f02c/feasibility-study.md#44-measured-throughput` (`talk.html:119-124,162`) | `dev/active/6dc81018-field-capability-dispatch/investigation.md:587`, b4b4b9ee report `:150` |
| `dev/archive/bb85c68a-field-linear-algebra/docs/presentations/bb85c68a-fieldmatrix/` | bb85c68a (archived) | `base.css`, `themes/{rust,gruvbox}.css` | own-epic archive documents (`talk.html:270,448`) | `dev/active/6dc81018-field-capability-dispatch/investigation.md:588`, b4b4b9ee report `:138` |
| `dev/archive/d4851c3d-modem-framework/docs/presentations/d4851c3d-modem-framework/` | d4851c3d (archived) | `base.css`, `themes/{rust,gruvbox}.css`, `../figures/{ber_curves,llr_histograms_16qam,per_bit_mi_16qam}.svg` | own-epic archive documents (`talk.html:353,469`) | — |
| `dev/archive/{6efb756b-grand,d4851c3d-modem-framework}/docs/presentations/figures/` | fig4-6 and `generate_grand_comparison_plots.py` under 6efb756b; the modem SVGs under d4851c3d | — | `generate_grand_comparison_plots.py:36-37` reads `dev/simulation_results` and `dev/reference_data` | **Writer:** `crates/gf2-coding/examples/gen_presentation_figures.rs` writes the modem SVGs into its `--output-dir` (default `presentation_figures` in the Cargo target directory); README lists this example (`README.md:171`) |

A stray deck asset also sits at `dev/active/e095a100-gfpm-arithmetic/e095a100-presentation/themes/gruvbox.css` (no deck). Relative links climbing `../../../dev/` break when decks move depth; JIT blocks `repository-escape` links but rewrites nothing.

### 3.5 Crate-local guides

`dev/archive/legacy/crates/gf2-core/docs`: 25 tracked md (9,544 lines; 13 under `archive/`). `dev/archive/legacy/crates/gf2-coding/docs`: 10 tracked md (3,922 lines; 2 under `archive/`). Line counts: `git ls-files ... | xargs wc -l`.

Inbound references (outside the dirs, excluding archives):
- `crates/gf2-core/README.md:125,155-159` (BENCHMARKS, KERNEL_OPTIMIZATION, GF2M, PRIMITIVE_POLYNOMIALS, COMPUTE_BACKEND_DESIGN, RREF_DESIGN_PLAN, SPARSE_DEDUP_DESIGN).
- `crates/gf2-coding/README.md:111,159-164` (SIMD_PERFORMANCE_GUIDE, PARALLELIZATION, LDPC_PERFORMANCE, DVB_T2, LDPC_VERIFICATION_TESTS, SDR_INTEGRATION, SYSTEMATIC_ENCODING_CONVENTION).
- `README.md:190` (generic `crates/*/docs/`).
- Live dev/active citations: `dev/active/ae03bcd0-general-bch/{investigation.md:261-264,346,369-370, bch-api-design.md:1044, breakdown.json:1802-1804, progress.json:706}`, `dev/active/6dc81018-.../investigation.md:513`, `progress.json:1308`, `dev/active/a83583e0/premeasurement-protocol.md:1030`, `dev/active/eaae1b56/premeasurement-protocol.md:472`, `dev/active/53c5a8c0/survey/source-evidence.json:215`.
- No rustdoc, `include_str!`, or Cargo `readme` reference (`git grep` over `crates/*/src|tests|benches|examples|build.rs`; no `readme` key in any `Cargo.toml`).

Topic map (title; perf-number lines by regex):

| File | Topic | Target |
|---|---|---|
| gf2-core `BENCHMARKS.md` (753 lines, 214 perf lines) | performance vs M4RI/NTL/FLINT | `docs/reference/` evidence page (facts only, re-pinned) |
| `KERNEL_OPTIMIZATION.md` | SIMD kernel architecture | `docs/concepts/` acceleration |
| `GF2M.md` | GF(2^m) strategy selection | `docs/concepts/` fields; gf2-core entry page |
| `PRIMITIVE_POLYNOMIALS.md` | polynomial database | `docs/reference/` |
| `COMPUTE_BACKEND_DESIGN.md`, `SYNC_SOLUTION_COMPARISON.md`, `SPARSE_DEDUP_DESIGN.md`, `RREF_DESIGN_PLAN.md`, `POLAR_IMPLEMENTATION_PLAN.md`, `POLY_UTILITIES_PERFORMANCE.md`, `QUALITY_AUDIT_{PLAN,REPORT}.md`, `README.md` | design history / plans / audits | legacy only |
| gf2-coding `DVB_T2.md` | standard implementation and vector verification | `docs/reference/` standards conformance; tutorial 1 |
| `SYSTEMATIC_ENCODING_CONVENTION.md` | layout convention | `docs/reference/` |
| `LDPC_VERIFICATION_TESTS.md` | conformance evidence | `docs/reference/` |
| `SIMD_PERFORMANCE_GUIDE.md`, `PARALLELIZATION.md`, `LDPC_PERFORMANCE.md` | acceleration + numbers | `docs/how-to/` acceleration; numbers to evidence page |
| `SDR_INTEGRATION.md` | SDR integration (GNU Radio epic 21922c59 is backlog) | check against current code; likely future-facing |

Other permanent-adjacent markdown not in the brief's inventory: `dev/archive/legacy/crates/gf2-core/benches/{BENCHMARK_RESULTS,field_matrix_fusion_results,strassen_threshold_results}.md` (cited by rustdoc at `crates/gf2-core/src/field/winograd.rs:112,116`, `expr.rs:2916`, and bench headers), `benchmarks/README.md` (281 lines), `crates/gf2-algebra/README.md` (261), `crates/gf2-kernels-simd/README.md` (85), `crates/gf2-stats/README.md` (19), `proofs/README.md`, `proofs/WORKAROUNDS.md`, `docs/lean4-verification-pipeline.md` (429), `crates/gf2-coding/src/ldpc/dvb_t2/table_interpretation.md`, `crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md`. gf2-sim has no README. gf2-stats is a public crate absent from REQ-04's four entry pages.

### 3.6 CONTRIBUTING.md

Content: pointer to AGENTS.md; toolchain and nextest prerequisite; six invariant addresses; `./scripts/cargo-ci.sh`; conventional commit with `jit:<id>` scope; PR content; Rust Code of Conduct link (`CONTRIBUTING.md:1-58`). Everything except the Code of Conduct link and PR-content guidance is already in AGENTS.md (`AGENTS.md:148-155` commits; `:17-24` commands).

Consumers: `README.md:195` (live). Historical mentions only in `dev/archive/legacy/crates/gf2-coding/docs/archive/QUALITY_AUDIT_REPORT.md`, `dev/active/DOCUMENTATION_AUDIT.md:526,619`, `dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md:30,141-196`. No CI, gate, profile or script consumer (`git grep CONTRIBUTING`; `packages/`, `.github/`, `contrib/` clean). `.github/copilot-instructions.md` was deleted today (`e8a68dc73`), so `CLAUDE.md` is the only tool-specific pointer left (7 lines, regular file).

### 3.9 Deleted roadmaps (for the retroactive task)

| Source | Lines | Planned-item locations | Counts |
|---|---|---|---|
| `git show 61c3f0a6a^:ROADMAP.md` | 208 | Planned table `:95-106`; Research Goals `:108-126`; Open Research Questions `:128-155`; Long-Term Vision `:157-162`; Publication `:164-183` | 8 planned milestones (M17-M24), 17 open questions, 89 bullets total, empty In Progress `:90-93` |
| `git show 61c3f0a6a^:crates/gf2-core/ROADMAP.md` | 309 | Planned Phases `:196-230` (Phase 2 wide buffers, 6b SIMD polar, 10 GF(p^m)); Future Directions `:232-248`; Priorities `:287-296` | 3 planned phases, 8 future-direction bullets, 117 bullets total |
| `git show 791beb2b0^:crates/gf2-coding/ROADMAP.md` | 429 | Unchecked items at lines 32, 48-49, 96, 111, 228-230, 278-281, 306-315, 345, 354, 369-397, 408; planned phases C4, C6, C7, C8, C10.7, C11.3-C11.5, C12.1-C12.4, C13 | 46 unchecked, 12 checked, 215 bullets |

Already-tracked overlaps visible in epic titles (not a mapping): GPU production 92acd7b5, FPGA adc75ba7, competitive benchmarking d77176e5, polar b81c239c, neural BP 9a5662ff, GNU Radio 21922c59, GF(p^m) e095a100 (archived).

### 3.10 Performance evidence

| Class | Location | Hardware | Flags | Workload | Baseline | Date | Commit |
|---|---|---|---|---|---|---|---|
| Campaign receipts (145) | `dev/bench_results/<id>/<campaign>/receipt.json` | yes (`00dd43c3/v4-r1-pilot/receipt.json:2293ff`) | yes per arm (`:4278`) | yes (`plan.json`) | control arms | `observed_utc` (`:4`) | none; content digests only |
| Loose reports (17 md) | `dev/bench_results/*.md` | header (`2026-05-27-8df0c501-blocked-invert.md:4`) | `:6` | yes | `:9` | `:3` | none |
| Tuning-profile receipts | `dev/benchmarks/tuning_profiles/*.md` + `.sha256` | yes | yes | yes | yes | yes | sometimes (`gf2-a83583e0-...-evidence.md:7`) |
| gf2-sim / DVB-T2 | `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md:10-16`, `dev/benchmarks/dvb_t2_awgn/*.csv` | md yes | command only | yes | none | none | none |
| Datasets | `dev/simulation_results/permanent-zero-fraction-20260829/manifest.json` | yes | — | yes | — | — | `provenance.git_revision` |
| Raw criterion/perf-stat | `dev/benchmarks/<id>-criterion.txt` | no | no | no | no | no | no |
| Crate-local tables | `dev/archive/legacy/crates/gf2-core/docs/BENCHMARKS.md`, `dev/archive/legacy/crates/gf2-core/benches/*_results.md`, `crates/gf2-core/README.md:125` | mixed | mixed | yes | mixed | mixed | none |

No class carries all six REQ-06 fields in-file. The commit field must come from a commit-pinned link (blob URL at a SHA) on the evidence page. Moving a whole receipt dir keeps its verdict (`dev/tools/tuning-campaign-support/src/receipt.rs:1148-1151,2280-2300`); rewriting bytes inside a receipt dir breaks digests.

### 3.13 Other-epic documentation issues (REQ-18)

| Issue | State | Container | Overlap |
|---|---|---|---|
| 2d65e37f | ready | 86b9c719 | rustdoc cross-refs: sweep/rustdoc drift (12907582) |
| e2c649cd | ready | 86b9c719 | example headers: example audit (a0a29512) |
| 9b3452e9 | ready | 86b9c719 | doctest consolidation: example audit + doctest timing (REQ-07) |
| 99c92597 | ready | 86b9c719 | single panic doc: keep (code-adjacent, independent) |
| 5deee377 | ready | 86b9c719 | false SIMD claim in gf2-algebra docs: 12907582 drift |
| 2c668046 | ready | 86b9c719 | issue-hygiene, not docs content: keep in own epic |
| 23f22f53 | ready | aabc528a → b4b4b9ee | missing doc sections gf2-core: 12907582 / sweep REQ-05 |
| 3d34f504 | ready | aabc528a | doc sections + compile examples gf2-coding: example audit |
| 54278d0c | ready | aabc528a | field docs gf2-kernels-simd: keep (doc completeness) |
| aabc528a | backlog | b4b4b9ee | parent story of the three above |
| ad978596 | ready | b4b4b9ee | safety contracts + clippy: code work, keep |
| cdaf5da9 | ready | aabc528a | Packed5Matrix docs: keep |
| 807ddab2 | backlog | 1362381c (zen3) | comment trim in epic-touched files: subsumed by f357b3dc sweep |
| 4ad869d6 | in_progress (agent:worker) | ae03bcd0 | BCH researcher rustdoc + examples: concurrent with sweep in gf2-coding/bch; sequence, do not absorb |
| 049a89af | ready | **no DAG parent** (label `epic:gf2-algebra-permanent`, epic archived) | archived design links in rustdoc: sweep/drift |
| 157c305c | ready | none | single rustdoc fact: keep or fold into 12907582 |
| 835f34f0 | ready | none | single rustdoc fact |
| c2663ce9 | ready | none | doc/behavior mismatch bug: code decision, keep |
| **0056e853** | ready | 86b9c719 | gf2-coding README "learning path": conflicts with D-06 |
| **315f4de5** | ready | 86b9c719 | visual trace examples: conflicts with D-05/D-07 |
| **35007c4c** | ready | 86b9c719 | standardized example headers: example audit |
| **5f3d0ff9** | ready | 86b9c719 | soft vs hard decoding example: elementary, conflicts with D-06 |
| **be331e20** | ready | 86b9c719 | split hamming_7_4 example: conflicts with D-06 |
| **68189a0a** | ready | 86b9c719 | README omits gf2-sim: subsumed by 44c98235 REQ-02 |

Bold rows are absent from today's candidate list. 1362381c (backlog) depends on 807ddab2; rejecting 807ddab2 unblocks it only once its zen3 REQ-06 obligation is preserved elsewhere.

### 3.14 Concurrency today

- `5fa988085` (13:14) added an archive-exclusion clause to `contrib/gates/doc-review-prompt.md`; `bb898b0e4` reverted it ("changes only with the invoker's explicit approval"); `a47ac3c71` (13:16) re-applied it. The working tree is now clean (`git status --short` empty), so the uncommitted change in the session snapshot is committed. The clause excludes any path with an `archive` segment, which already covers the planned `dev/archive/legacy/` and `crates/*/docs/archive/`.
- Effect on plan: dcd38c45 (ground doc-review in invariants) must carry this clause forward; the live prompt now diverges from `packages/sim-research/assets/live/contrib/gates/doc-review-prompt.md` (`diff` shows only these lines), so a profile re-apply would drop it unless the asset changes too.
- `e8a68dc73` deleted `.github/copilot-instructions.md`; `6fab5d796` removed the Copilot branch trigger. REQ-02 has one fewer pointer file.
- `61c3f0a6a`, `791beb2b0` deleted roadmaps (also edited `dev/archive/legacy/crates/gf2-core/docs/{KERNEL_OPTIMIZATION,PRIMITIVE_POLYNOMIALS,README}.md`, `crates/gf2-coding/docs/{LDPC_VERIFICATION_TESTS,README}.md`, `README.md`).
- `e1c1135fe` removed a non-goals section from gf2-sim channels rustdoc: sweep-style edits are already landing outside f357b3dc.
- Active parallel sessions: d1b4f85e (ae03bcd0, `fca6df53d` "module doc rework"), f759d724 BCH prose sweep (closed `7ab443c17`), 4ad869d6 claimed by a worker. gf2-coding `bch/` and `transform/` comments are moving now; sweep tasks touching them should depend on ae03bcd0 work or run after it.

## 4. Primitive verification (jit 1.0.0, source `just-in-time` HEAD `8b27bcbfc`, archive code last changed `498b5c797` before the 2026-09-19 build)

| Question | Answer | Source |
|---|---|---|
| Moves files? | Yes, by write-new-then-delete-if-hash-matches; no git calls; caller stages | `repository_state/archive.rs:75-89,245-262` |
| Byte verification | SHA-256 and size checked before publish | `archive.rs:74`; `cli-commands.md:391-400` |
| Rewrites references | Issue `documents[]` only; no file content | `artifact_plan.rs:350-359`; `archive.rs:177-198`; `cli-commands.md:247-252` |
| Citation warnings | advisory `moving-path-citation`, within `citation_scan_roots` (default `dev` + `permanent_paths`) | `artifact_plan.rs:539`; `configuration.md:179-196` |
| Destination | `<archive_root>/<short8>-<slug>/`; slug from the container's membership label value (e.g. `epic:`), lowercased, 48 chars; bare short id without label | `artifact_classifier.rs:1493-1554` |
| Layout | path relative to `development_root`; owner dir dropped for `issue_scoped_areas` (`dev/active/X-slug/plan.md` → `dev/archive/X-slug/active/plan.md`) | `artifact_classifier.rs:1564-1600`; confirmed by `dev/archive/6efb756b-grand/{active,plans,simulation_results}/` |
| Marker | `.jit-container` with full container UUID; matching marker reused; markerless `<short8>/` adopted; two matches = `destination-conflict` | `archive.rs:127-136`; `cli-commands.md:374-389` |
| Bundles | md/html/css parsed for local links (recursive); js checked for dynamic loads; binaries carried opaque; directory or non-file target blocks `unsupported-artifact-type`; symlink blocks `symlink-artifact`; climbing above repo blocks `repository-escape` | `artifact_discovery.rs:550-662`; `cli-commands.md:275-285` |
| Area policy | managed + unheld → move; permanent path, outside owner, active owner, or unmanaged linked → copy; outside `development_root` → stays, links not followed | `artifact_classifier.rs:847-879`; `configuration.md:85-92` |
| Eligibility blockers | policy-unconfigured/incomplete, unmanaged-selected-root, destination-conflict, document-non-terminal-owner, non-terminal-target, missing-source, symlink-artifact, unsupported-artifact-type, repository-escape, unresolvable-edge, unpreservable-layout, pinned-read-failed | `artifact_plan.rs:410-462` |
| Container state | only the container moves to `Archived`, previous state recorded | `archive.rs:205-215` |
| Idempotent | yes; rerun adopts identical destination content, no event when nothing changes | `cli-commands.md:402-406`; test `commands/archive.rs:2634-2641` |
| Resumable deletion | source edited after planning → `deletion-failed` warning, source kept | `archive.rs:245-262` |
| Legacy move | `archive document` works on unowned managed files (`no-owner`) but targets `dev/archive/<path minus dev/>`; no legacy option; non-`dev/` and permanent sources unmovable | `artifact_classifier.rs:1479-1484`; `artifact_inventory.rs:300-308`; `cli-commands.md:323-326` |
| `candidates` | evaluate-only, no `--execute` | `jit archive candidates --help` |
| `jit doc` | add, list, remove, show, dir, conformance, history, diff, assets, check-links; no move; `conformance` reports only | `configuration.md:130-176` |

Conclusions for the plan:
- Container archival is the right mover for `dev/` managed sources and is idempotent. Every execution needs a paired manual commit that repoints in-file citations (README, AGENTS.md, rustdoc, dev/active docs).
- Deck relocation (REQ-09) and crate-doc legacy moves (owner decision) need `git mv` plus reference edits, or a temporary policy widening that must be verified first.
- The legacy mirror (D-25) needs `git mv`, or a temporary `archive_root = "dev/archive/legacy"` pass verified on one file. `dev/archive` is a permanent path, which interacts with that pass.
- Shared-owner files are copied, not moved (`195f8254f` duplicated `babcf05e-handoff-5.md` and bench reports into 026fc832 and 97bf0879). D-24 ownership resolution must happen before execution.
- `jit doc check-links` is a candidate reuse point for docs-mechanical (coverage not tested).
- The CLAUDE.md symlink (56378e82) becomes a `symlink-artifact` blocker for any archived document that links `CLAUDE.md`.

## 5. Architecture fit

### 5.1 Reuse points

- `jit archive container --execute` for terminal epics; `jit archive candidates --json` as the manifest's container-row source.
- `jit doc add/remove` for relinking; `jit doc dir <id> active` prints the canonical `dev/active/<short8>-<slug>` for D-23 regrouping.
- `jit doc check-links` and `jit item show` for docs-mechanical (DEC-05).
- `.jit/projection.invariants` (region in AGENTS.md) for invariants; `.jit/projection.rules-and-gates` already renders `.jit/reference/rules-and-gates.md`.
- `contrib/gates/ai-review.sh` wrapper (DEC-04) for doc-review.
- `dev/active/fa787f85-documentation-overhaul/fa787f85-example-census.py` for REQ-07 counts.
- The `.agents/skills/jit-project-lead/scripts/standards-scan.sh` permanent-path logic for footprint selection.

### 5.2 Tutorial feasibility

**Tutorial 1, coded-modulation link simulation with gf2-sim.**
- Presets: `Pipeline::dvb_t2()` typestate builder (`crates/gf2-sim/src/presets/dvb_t2.rs:791`, `Modcod` `:91`, `Channel` `:192`); `nr_5g()` builder (`crates/gf2-sim/src/presets/nr_5g.rs:1023`, base graph, lifting, rate, decoder, demap, channel, parallelism, seed, `checkpoint_dir` at `:507-649`).
- Examples: `crates/gf2-sim/examples/{dvb_t2_quickstart,dvb_t2_typestate,dvb_t2_graph_api,nr_5g_quickstart,novel_chain_via_graph,gpu_hybrid,parallel_byte_identity}.rs`. `dvb_t2_quickstart.rs:1-16` states ~5 s runtime.
- Campaign binaries: `crates/gf2-sim/src/bin/{dvb_t2_awgn_campaign,ldpc_bler_sweep,checkpoint_sweep,ebch_osd_awgn_campaign,...}.rs` (16 bins). `dvb_t2_awgn_campaign.rs` header carries migration narration (`:9-30`), a sweep target.
- Evidence: `dev/benchmarks/dvb_t2_awgn/*.csv`, `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md:10-16` (no commit/date fields).
- Feasible. A reproducible checkpointed sweep with a modest frame budget fits the fast path; a full curve is a campaign workload outside the 60 s budget.

**Tutorial 2, finite-field linear algebra at scale with gf2-core.**
- No FieldMatrix or GF(p^m) example exists. `crates/gf2-core/examples/` has 14 files: BitVec/BitMatrix basics, primitive polynomials, perf-stat and emitter tools.
- Benches cover the target workload: `crates/gf2-core/benches/{field_matrix,field_matrix_gemm,fieldmatrix_gemm,fieldmatrix_gemm_delayed,fieldmatrix_gf2m_batch_gemm,fieldmatrix_ple,fieldmatrix_solve,fieldmatrix_charpoly,field_sparse_matrix,sparse_field_matmul,fp_montgomery,fp_specialized,strassen_threshold,...}.rs`.
- Source: `crates/gf2-core/src/field/{matrix,ple,charpoly,inverse,triangular,sparse_matrix,winograd,extension,extension_wiedemann,poly}.rs`; `gfp/`, `gfpn/`, `gf2m/`.
- Evidence: `dev/archive/legacy/crates/gf2-core/benches/{field_matrix_fusion_results,strassen_threshold_results}.md`, tuning receipts under `dev/benchmarks/tuning_profiles/`, campaign receipts in `dev/bench_results/`.
- Feasible, but the tutorial needs a new tested example program; it cannot build on an existing one.

### 5.3 Sweep sizing (Appendix C)

Total comment lines (`//`, `///`, `//!`): gf2-core 40,784; gf2-coding 29,111; gf2-sim 18,559; gf2-algebra 9,157; gf2-kernels-simd 8,178; gf2-kernels-hip 5,060; gf2-stats 641 (701 tracked `.rs` files under `crates/`). gf2-core `src/field` alone has 18,821 lines across 22 files (largest `poly.rs` 2,693, `matrix.rs` 2,331). Natural worker units at roughly 3-6k comment lines: gf2-core split into field (3-4 units), gf2m+gfp+gfpn, remaining src, tests+benches+examples; gf2-coding split into bch+ldpc, modem+osd+product+grand+gldpc, remaining src, tests+benches+examples; gf2-sim split into src (2 units) and tests+bins; gf2-algebra, kernels-simd, kernels-hip+stats one unit each.

## 6. Invariant check

| Invariant | Implication |
|---|---|
| `behavioral-evidence-validity` | Moving a whole receipt dir keeps its verdict; rewriting references inside any receipt dir (`receipt.json`, `acceptance-summary`, `plan.json`, `inputs/producing/`) breaks digests. `check-receipt-input-snapshots.py:136` and `check-addendum-schema-versions.py:181,198` hard-code `dev/bench_results/`; moved receipts drop out of CI silently. `receipt-input-omissions.json` keys by full path. Receipt dirs must be excluded from any bulk rewrite and their checks updated in the same commit as any move. |
| `single-source-prose` | README's example list (`README.md:162-173`) and feature table (`:108-125`) are hand copies that drift: the gf2-core list names 6 of 14 example files, and the gf2-coding list omits `dvb_t2_bicm_chain` and `bch_oracle_messages`. The evidence page should cite receipts, not copy tables. |
| `benchmark-backed-performance` | `crates/gf2-core/README.md:125` (3.4-3.6x), `dev/archive/legacy/crates/gf2-core/docs/BENCHMARKS.md` (214 perf lines), gf2-coding perf guides carry numbers without commit-pinned receipts. |
| `present-tense-prose` | `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:9-30` migration narration; crate docs named `*_PLAN.md`; README "not yet published", "Good first areas". |
| `no-deferred-defects` | 29 dangling `dev/plans/*.md` citations in code/rustdoc. |
| `runtime-observed-provenance` | Receipts pin `dev/active/00dd43c3/...` paths as provenance strings (`dev/bench_results/00dd43c3/v4-r1-pilot/receipt.json:3100,3444`); regrouping dev/active does not break verification (snapshot copy) but makes the string historical. |
| `canonical-cutover` | Two `gruvbox.css` stray copies and a third in dev/active are leftovers. |

## Appendix A: dev/active entries

Method: Python scan of `.jit/issues/*.json` (893 issues) mapping `documents[].path` to owners; reverse dependencies to `type:epic`; name-embedded short ids; `git log --format=%s -- <path>` for `jit:<id>`. Classes: a = non-terminal epic [epic-dir or flat]; b = terminal with files; b-empty = empty untracked husk; c = no defensible owner.

Counts: a 70 (7 epic-dir, 63 flat); b 3; b-empty 43; c 2. Multi-epic entries blocking a clean per-epic move: 220cab0b, 389aa4de, 3fa7c9d0, `6dc81018-field-capability-dispatch` (6dc81018 + 86b9c719); `1a379447-zen3-cpu-performance`, 3be770d5, c077a88b, f547c394 (1a379447 + d77176e5). Name-id-only (no doc link): 50b47eae (63 files), d1b4f85e (2). Untracked `__pycache__`: `4e732b56/baseline-survey`, `c04dd4ac-zen3-shifts-and-permutations/survey`.

| Entry | Files | Owner | Evidence | Owner state | Top epic(s) | Class |
|---|---|---|---|---|---|---|
| 00dd43c3 | 15 | 00dd43c3 sim | link+name | done | 1a379447 | a flat |
| 026fc832-gf2-core-sota-stretch | 0 | 026fc832 | name | archived | 026fc832 | b-empty |
| 02b8137c | 26 | 02b8137c | link+name | in_progress | b8206228 | a flat |
| 04b85d10 | 47 | 04b85d10 | link+name | done | 1a379447 | a flat |
| 0606186a | 0 | 0606186a | name | done | ae82bd73 | b-empty |
| 07ca8585 | 42 | 07ca8585 | link+name | done | 1a379447 | a flat |
| 085cbd0b | 0 | 085cbd0b | name | done | babcf05e | b-empty |
| 0de41c82 | 4 | 0de41c82, 1b113b9d | link+name | done | b8206228 | a flat |
| 0fb99491-0fb994 | 0 | 0fb99491 | name | done | e095a100 | b-empty |
| 12fdeb5b | 32 | 12fdeb5b | link+name | done | 1a379447 | a flat |
| 150d7d79 | 1 | 150d7d79 | link+name | done | 86b9c719 | a flat |
| 152388f4 | 0 | 152388f4 | name | done | 2928ccce | b-empty |
| 19513245 | 68 | 19513245 | link+name | done | 1a379447 | a flat |
| 1a379447-zen3-cpu-performance | 99 | 57 issues | link+name | mixed | 1a379447, d77176e5 | a epic-dir |
| 1ac74567 | 5 | 1ac74567 | link+name | done | 6dc81018 | a flat |
| 1c602857 | 18 | 1c602857 | link+name | done | 1a379447 | a flat |
| 1d0da41f | 33 | a203a23c, 1d0da41f, 428f2f6b | link+name | done | 1a379447 | a flat |
| 1d4fd63d | 37 | 1d4fd63d | link+name | done | 1a379447 | a flat |
| 2037941f-profile-and-optimize-mid-range-buffer-operations | 153 | 14 issues | link+name | mixed | 1a379447 | a flat |
| 220cab0b | 1 | 220cab0b, f35daec0, 2a85f728, 676f55a2 | link+name | done | 6dc81018, 86b9c719 | a flat |
| 24a93e4e | 0 | 24a93e4e | name | done | 026fc832 | b-empty |
| 26465e6c | 68 | 26465e6c | link+name | done | 1a379447 | a flat |
| 2928ccce-dvb-t2-awgn-campaign | 0 | 2928ccce | name | archived | 2928ccce | b-empty |
| 2a85f728 | 1 | 2a85f728 | link+name | done | 6dc81018 | a flat |
| 2e8c5a29 | 0 | 2e8c5a29 | name | done | 026fc832 | b-empty |
| 34d85cb9 | 181 | 34d85cb9 | link+name | done | 6dc81018 | a flat |
| 363556e6 | 0 | 363556e6 | name | done | ae82bd73 | b-empty |
| 37e0b235 | 1 | 37e0b235 | name | rejected | 806eb14e | b |
| 389aa4de | 1 | 389aa4de | link+name | done | 6dc81018, 86b9c719 | a flat |
| 3be770d5 | 46 | 3be770d5 | link+name | done | 1a379447, d77176e5 | a flat |
| 3d522ba0 | 1 | 3d522ba0 | link+name | backlog | b4b4b9ee | a flat |
| 3f664839 | 1 | 3f664839 | link+name | done | b8206228 | a flat |
| 3fa7c9d0 | 1 | 3fa7c9d0 | link+name | done | 6dc81018, 86b9c719 | a flat |
| 41c3d91d | 1 | 41c3d91d | link+name | done | b8206228 | a flat |
| 428f2f6b | 4 | 428f2f6b | link+name | done | 1a379447 | a flat |
| 43fb19e2 | 0 | 43fb19e2 | name | done | f9717e7e, 1a379447, d77176e5 | b-empty |
| 45649554-run-phase-2-awgn-simulations-figs-4-6-additional | 0 | 45649554 | name | done | 6efb756b | b-empty |
| 4c1e441f | 20 | 4c1e441f | link+name | in_progress | 1a379447 | a flat |
| 4e732b56 | 19 | 4e732b56 | link+name | done | ae03bcd0 | a flat |
| 50b47eae | 63 | 50b47eae | name | done | 6dc81018 | a flat |
| 50f0bd42 | 3 | 50f0bd42 | link+name | in_progress | 1a379447 | a flat |
| 52cce970 | 0 | 52cce970 | name | done | 026fc832 | b-empty |
| 53c5a8c0 | 48 | 53c5a8c0 | link+name | done | 1a379447 | a flat |
| 591a1c5e | 1 | 591a1c5e | link+name | done | ae03bcd0 | a flat |
| 5cbb6545 | 39 | 5cbb6545 | link+name | done | 1a379447 | a flat |
| 5ce13bae | 0 | 5ce13bae | name | done | 026fc832 | b-empty |
| 613574db | 2 | 613574db | link+name | done | 1a379447 | a flat |
| 615db3b9 | 0 | 615db3b9 | name | done | 026fc832 | b-empty |
| 64fd3afd | 1 | 64fd3afd, 94597a51, b1bd75ca | link+name | done | ae03bcd0 | a flat |
| 68cdf4c8 | 0 | 68cdf4c8 | name | done | 026fc832 | b-empty |
| 6c6b09b1 | 73 | 6c6b09b1, ad2a6a58, 4c1e441f, d45aff82 | link+name | mixed | 1a379447 | a flat |
| 6dc81018-field-capability-dispatch | 21 | 6dc81018, 265997f9, 663965f6 | link+name | mixed | 6dc81018, 86b9c719 | a epic-dir |
| 6efb756b-grand | 0 | 6efb756b | name | archived | 6efb756b | b-empty |
| 6fb89a3c | 52 | 6fb89a3c | link+name | done | 1a379447 | a flat |
| 7be754bd-coverage-90 | 0 | 7be754bd | name | archived | 7be754bd | b-empty |
| 7d7c647c | 88 | 7d7c647c | link+name | done | 6dc81018 | a flat |
| 7d824b2f | 1 | 7d824b2f, eaae1b56, a83583e0 | link+name | done | 6dc81018 | a flat |
| 806eb14e-hip-gpu-prototype | 0 | 806eb14e | name | archived | 806eb14e | b-empty |
| 82dd7384 | 0 | 82dd7384 | name | done | 2928ccce | b-empty |
| 831bfc4a-run-phase-4-fading-channel-simulations-figs-8-10 | 0 | 831bfc4a | name | done | 6efb756b | b-empty |
| 873cbec1 | 0 | 873cbec1 | name | done | 026fc832 | b-empty |
| 8b1609a8 | 0 | 8b1609a8 | name | done | 6efb756b | b-empty |
| 8b3fe657 | 1 | 8b3fe657 | link+name | backlog | b4b4b9ee | a flat |
| 9012f8a0 | 0 | 9012f8a0 | name | done | 806eb14e | b-empty |
| 9162956b | 14 | 9162956b | link+name | done | 6dc81018 | a flat |
| 972e2b88 | 1 | 972e2b88 | link+name | done | 6dc81018 | a flat |
| 97bf0879-gf2-core-sota-performance | 0 | 97bf0879 | name | archived | 97bf0879 | b-empty |
| 9c37ec8c | 0 | 9c37ec8c | name | done | d4851c3d | b-empty |
| 9f0e385e | 1 | 9f0e385e, 9fb40c83 | link+name | done | 1a379447 | a flat |
| DOCUMENTATION_AUDIT.md | 1 | — | 13e82164f untagged | — | — | c |
| a1ad6d4e | 2 | a1ad6d4e | link+name | done | 1a379447 | a flat |
| a2026f8c | 0 | a2026f8c | name | done | 6efb756b | b-empty |
| a203a23c | 10 | a203a23c, 7cdc28e9 | link+name | done | 1a379447 | a flat |
| a4d86b3d-configurable-simulation-campaign-runner | 0 | a4d86b3d | name | done | 6efb756b | b-empty |
| a7886bd8 | 0 | a7886bd8 | name | done | ae82bd73 | b-empty |
| a83583e0 | 9 | a83583e0 | link+name | done | 6dc81018 | a flat |
| aaa847cf | 0 | aaa847cf | name | done | 026fc832 | b-empty |
| aabc528a | 1 | aabc528a | link+name | backlog | b4b4b9ee | a flat |
| ab791e27-design-fieldmatrix-f-finitefield-dense-matrix-ty | 0 | ab791e27 | name | done | bb85c68a | b-empty |
| ad2a6a58 | 20 | ad2a6a58 | link+name | in_progress | 1a379447 | a flat |
| ae03bcd0-general-bch | 40 | ae03bcd0 + 4 children | link+name | mixed | ae03bcd0 | a epic-dir |
| ae82bd73-gf2-algebra-permanent | 0 | ae82bd73 | name | archived | ae82bd73 | b-empty |
| aed96ef9-finite-blocklength-bounds | 1 | b7157be6, aed96ef9, 55087229, cce5da8c, c7cfd37e | link+name | mixed | same | a epic-dir (shared with done b7157be6) |
| b293af5a | 0 | b293af5a | name | done | ae82bd73 | b-empty |
| b4b4b9ee-tech-debt-2026-06-30 | 1 | b4b4b9ee | link+name | backlog | b4b4b9ee | a epic-dir |
| b8206228-permanent-statistics | 53 | b8206228 + 4 children | link+name | mixed | b8206228 | a epic-dir |
| babcf05e-gf2-core-ppc-spiral | 1 | babcf05e | name | archived | babcf05e | b |
| bb85c68a-field-linear-algebra | 0 | bb85c68a | name | archived | bb85c68a | b-empty |
| bc091474 | 17 | bc091474 | link+name | done | 1a379447 | a flat |
| bd9c6e13 | 0 | bd9c6e13 | name | done | 026fc832 | b-empty |
| bdc507a3 | 2 | bdc507a3 | link+name | done | 1a379447 | a flat |
| c04dd4ac-zen3-shifts-and-permutations | 72 | c04dd4ac + 4 | link+name | done | 1a379447 | a flat |
| c077a88b | 32 | c077a88b, a203a23c | link+name | done | 1a379447, d77176e5 | a flat |
| c3f8c1cb-implement-ple-decomposition-and-echelon-forms-ov | 0 | c3f8c1cb | name | done | bb85c68a | b-empty |
| c7113c5a | 43 | c7113c5a, 12fdeb5b | link+name | done | 1a379447 | a flat |
| c73ffa25 | 1 | c73ffa25 | link+name | done | 1a379447 | a flat |
| c87c5043 | 0 | c87c5043 | name | done | d4851c3d | b-empty |
| charon-patch-backup-2026-05-15 | 9 | — | ambiguous jit:9efd9c39 / jit:150d7d79 | — | (86b9c719) | c |
| d1b4f85e | 2 | d1b4f85e | name | in_progress | ae03bcd0 | a flat |
| d1dd266c | 0 | d1dd266c | name | done | 97bf0879 | b-empty |
| d4851c3d-modem-framework | 0 | d4851c3d | name | archived | d4851c3d | b-empty |
| dbd8787d | 3 | dbd8787d | link+name | in_progress | 6dc81018 | a flat |
| e095a100-gfpm-arithmetic | 1 | e095a100 | name | archived | e095a100 | b |
| e6ea0dde | 67 | e6ea0dde | link+name | done | 6dc81018 | a flat |
| e8a0c47a | 0 | e8a0c47a | name | done | 026fc832 | b-empty |
| eaae1b56 | 1 | eaae1b56 | link+name | done | 6dc81018 | a flat |
| eb9b324c | 18 | eb9b324c | link+name | done | 6dc81018 | a flat |
| ec530af9 | 0 | ec530af9 | name | done | f9717e7e | b-empty |
| eda07788 | 58 | eda07788, 3e59cb9a, 12fdeb5b, 9fb40c83 | link+name | done | 1a379447 | a flat |
| f547c394 | 96 | f547c394, bdc507a3, f63a2464, 1a379447, 2c487595 | link+name | mixed | 1a379447, d77176e5 | a flat (CI reads it) |
| f63a2464 | 39 | f63a2464 | link+name | in_progress | 1a379447 | a flat |
| f8dd4dde | 6 | f8dd4dde, 00dd43c3 | link+name | done | 1a379447 | a flat |
| fa787f85-documentation-overhaul | 4 (+ this file) | fa787f85, a0a29512 | link+name | backlog | fa787f85 | a epic-dir |
| fc182ed5 | 0 | fc182ed5 | name | done | 026fc832 | b-empty |
| fc976a80 | 1 | fc976a80 | link+name | done | 6dc81018 | a flat |
| fcb04d66 | 4 | fcb04d66 | link+name | done | 1a379447 | a flat |
| fd73e8a8 | 0 | fd73e8a8 | name | done | 2928ccce | b-empty |
| feb15da9 | 0 | feb15da9 | name | done | 026fc832 | b-empty |

Per-epic counts of class (a) entries (multi-epic entries count once per epic): 1a379447 35, 6dc81018 18, b8206228 5, 86b9c719 5, ae03bcd0 5, d77176e5 4, b4b4b9ee 4, fa787f85 1; aed96ef9/55087229/c7cfd37e/cce5da8c share 1.

## Appendix B: dev/ inventory

| Path | Tracked | Ignored on disk | Composition |
|---|---|---|---|
| active | 1,948 | 2 | see Appendix A |
| archive | 399 | 0 | 13 container dirs + `packed_field_stub/` (unmarked code stub, `68dba33d1`, a legacy candidate) |
| bench_results | 49,828 | 0 | 31,145 rs, 5,365 json, 4,025 txt, 1,819 csv, 1,355 err, 1,352 status, 1,050 toml, 664 md; 136 top-level entries (35 issue dirs, dated campaign dirs, 88 loose files) |
| benchmarks | 172 | 88 | tuning_profiles 60, permanent_campaign 46, dvb_t2_awgn 31, gf2_algebra_permanent 13, gf2-sim 11, 11 loose |
| campaigns | 50 | 0 | toml campaign configs |
| plans | 12 | 0 | md |
| presentations | 2 | 0 | 2 `themes/gruvbox.css` |
| reference_data | 45 | 0 | 31 csv, 7 json, 4 md, 3 py |
| research | 141 | 9,828 | 18 prototype crates + `rns_representation.md` |
| scripts | 25 | 0 | CI and campaign scripts |
| sessions | 3 | 0 | handoff/RCA md |
| simulation_results | 283 | 14 | 106 json, 95 csv, 61 jsonl, 7 md, 4 sha256 |
| studies | 306 | 394 | 6 issue dirs (047b62ed, 0dffa759, 6c7fcb38, 91605d4d, a9284086, b488f02c), all done under backlog epic b8206228 |
| tools | 34 | 1,129 | tuning-campaign-support, tuning-profile-compose |
| index.md / authoring-conventions.md | 1 / 1 | — | 117 / 164 lines |

Commands: `git ls-files <dir> | wc -l`, `find <dir> -type f | wc -l`.

## Appendix C: comment volume

Command: `git ls-files <path> | grep '\.rs$' | xargs cat | awk` counting lines whose first non-blank token is `//!`, `///`, or `//`.

| Path | Comment | `///` | `//!` | `//` | Non-blank |
|---|---|---|---|---|---|
| crates/gf2-core | 40,784 | 26,736 | 5,673 | 8,375 | 138,406 |
| crates/gf2-coding | 29,111 | 19,900 | 4,727 | 4,484 | 96,504 |
| crates/gf2-sim | 18,559 | 12,533 | 3,726 | 2,300 | 71,253 |
| crates/gf2-algebra | 9,157 | 6,301 | 1,285 | 1,571 | 24,354 |
| crates/gf2-kernels-simd | 8,178 | 4,644 | 1,913 | 1,621 | 23,966 |
| crates/gf2-kernels-hip | 5,060 | 3,847 | 431 | 782 | 10,500 |
| crates/gf2-stats | 641 | 596 | 36 | 9 | 3,428 |

gf2-core by module (comment / non-blank): `src/field` 18,821 / 47,460; `src/gf2m` 4,629 / 11,685; `src/gfpn` 2,603 / 6,835; `src/gfp` 2,598 / 7,104; `benches` 2,414 / 24,278; `tests` 2,269 / 14,844; `src/sparse.rs` 1,007; `src/bitvec.rs` 993; `src/alg` 956; `src/matrix.rs` 851; `src/tuning` 641; `src/kernels` 556; `src/io` 473; `src/compute` 469; `examples` 338; `src/primitive_polys.rs` 264; `src/lib.rs` 210; `src/bench_seed.rs` 197; `src/rng.rs` 179; other files under 120 each.

gf2-core `src/field` by file: poly 2,693; matrix 2,331; charpoly 1,670; ple 1,572; extension 1,480; triangular 1,132; sparse_matrix 1,098; expr 934; inverse 868; traits 797; vec 767; axiom_tests 666; poly_interpolate 661; winograd 516; two_adic 425; extension_wiedemann 325; batch_ops 306; modulus_select 167; ntt 139; test_random_matrix 123; irreducibility 99; mod 52.

gf2-coding by module: `src/modem` 4,784; `src/ldpc` 4,699; `src/bch` 3,808; `tests` 2,658; `src/simulation.rs` 1,720; `src/osd` 1,447; `examples` 1,333; `src/grand` 1,276; `src/product` 1,204; `src/gldpc` 784; `src/traits.rs` 739; `src/fading.rs` 623; `src/transform` 578; `src/test_support.rs` 475; `src/llr.rs` 416; `benches` 395; `src/drm.rs` 386; `src/linear.rs` 296; `src/bcjr` 277; `src/dvb_t2_bicm_harness.rs` 251; `src/bin` 234; `src/crc.rs` 231; others under 120.

gf2-sim by module: `tests` 3,341; `src/executor` 2,046; `src/permanent_campaign` 1,919; `src/gpu` 1,676; `src/bin` 1,171; `src/presets` 1,134; `src/permanent_rare_event` 1,072; `src/stages` 797; `src/graph` 696; `src/snr_checkpoint.rs` 690; `src/parallel` 674; `src/channels` 597; `src/osd_campaign.rs` 514; `src/stage.rs` 331; `src/frame_sim.rs` 286; `src/pipeline.rs` 253; `src/testutil.rs` 215; `src/error.rs` 185; `examples` 178; `src/batch.rs` 171; `src/checkpoint` 158; others under 120.
