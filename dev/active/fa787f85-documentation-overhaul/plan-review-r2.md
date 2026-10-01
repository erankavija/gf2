# Plan review r2: fa787f85 documentation overhaul

Planning node 8dddfc7a. Container fa787f85. Reviewed at `ac1844d1f`, after `780a7a2a3` (brief D-43..D-52) and `e2523b09d` (live edge 153297cf → a0a29512). Read-only review; no tracker state changed.

## Mechanical checks

| Check | Result |
|---|---|
| `validate --deny-warnings`: known sources = every manifest ref (D-02..D-52, DEC-05, INV-*, OD-08, PD-*, REQ-01..27); required REQ-01..REQ-20 | exit 0; advisories only |
| `render --check` | exit 0 |
| `batch-create --dry-run --json` | valid; 80 issues, 139 edges |
| Source refs | All resolve. D-43..D-52 are in the brief (`780a7a2a3`). The OD/PD ids in the plan and manifest (OD-08, PD-01..03, 05..07, 09..11) are rows of the plan table. No dangling PD-04/PD-08 or OD-01..07 remain. |
| Contracts and spine | Producers are exact. Sources reach 3f29e945 through the re-homes. 153297cf → a0a29512 is live, so a0a29512 is in the subtree. |
| Coverage labels | No false credit. `readme-landing-page` carries REQ-03. The audit dropped REQ-03. Externals add no foreign `satisfies` labels. REQ-20 is now backed by brief D-43..D-52. |

## Round-1 findings

| Finding | Status | Evidence |
|---|---|---|
| B1 f547c394 | Owner decision D-52 recorded; tooling readers enumerated. | `receipt-pin-path-tolerance` then `move-f547c394-inputs`. The readers in `protocol.rs:25,27,1102`, `benchmark-ab-runner.rs:145`, both tests and both CI scripts are named. The two scripts are path-sensitive only through `SCHEMA_DIR`/`CLOSURE`; schema digests are matched by sha (`check-addendum-schema-versions.py:205-213`). Residual defects in N2 and N3. |
| B2 scaffold contradiction | Fixed | Scaffold REQ-01 links the three existing READMEs; `entry-page-gf2-sim` REQ-06 adds its row. |
| B3 sweep gap | Fixed | All 704 crate `.rs` files map to exactly one unit; no footprint path is missing. |
| B4 sweep footprints | Fixed | Explicit file lists; no overlap between units. |
| B5 external ordering | Fixed | 44c98235 is superseded by `readme-landing-page`, which depends on both tutorials and `remove-contributing-guide`. `sweep-baseline-census` depends on 153297cf and 12907582; a0a29512 comes in through the live edge. The stale 4ad869d6 note is gone. The ae03bcd0 window is stated as the only unencodable ordering. |
| B6 citation repointing vs sweep | Fixed | The three regroup tasks, `execute-terminal-archives` and `legacy-move-gf2-core-guides` depend on `tersification-sweep`. They declare `crates` (with concrete uncertainty) and `receipt-input-omissions.json`. |
| B7 husks | Fixed | Depends on strays, field-dispatch, the f547c394 move and both relocations; the other moves are reached transitively. |
| B8 REQ-20 | Fixed | Brief D-43..D-52 records the interview outcome. |
| B9 audit size | Fixed | Scope is the README, `docs/`, crate READMEs, `AGENTS.md` and the seven crate-root `lib.rs` files. |

## Assignment simulation (finest tier: 79 tasks)

All 24 sweep units share the round-1 assessment: one module group, a diff-review boundary, 1–6k comment lines.

| Key | One outcome | One bounded consumer family | Observable test boundary | Footprint credible | One focused cycle | No inner decomposition | No mixed deliverables | Result |
|---|---|---|---|---|---|---|---|---|
| triage-doc-issues | P | P | P | P | P | P | P | P |
| roadmap-coverage-map | P | P | P | P | P | P | P | P |
| remove-contributing-guide | P | P | P | P | P | P | P | P |
| inventory-dev-active | P | P | P | P | P | P | P | P |
| inventory-dev-buckets | P | P | P | P | P | P | P | P |
| inventory-permanent-sources | P | P | P | P | P | P | P | P |
| expand-managed-paths | P | P | P | P | P | P | P | P |
| link-owned-artifacts | P | P | P | P | P | P | P | P |
| archive-candidate-preview | P | P | P | P | P | P | P | P |
| repair-osd-archive | P | P | P | P | P | P | P | P |
| sweep-baseline-census | P | P | P | P | P | P | P | P |
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
| sweep-coding-bch | P | P | P | P | P | P | P | P |
| sweep-coding-modem | P | P | P | P | P | P | P | P |
| sweep-coding-decoders | P | P | P | P | P | P | P | P |
| sweep-coding-core-src | P | P | P | P | P | P | P | P |
| sweep-coding-tests-benches | P | P | P | P | P | P | P | P |
| sweep-sim-campaigns | P | P | P | P | P | P | P | P |
| sweep-sim-pipeline | P | P | P | P | P | P | P | P |
| sweep-sim-runtime | P | P | P | P | P | P | P | P |
| sweep-sim-tests-bins | P | P | P | P | P | P | P | P |
| sweep-algebra-packed | P | P | P | P | P | P | P | P |
| sweep-algebra-rest | P | P | P | P | P | P | P | P |
| sweep-simd-x86 | P | P | P | P | P | P | P | P |
| sweep-simd-rest | P | P | P | P | P | P | P | P |
| sweep-hip-stats | P | P | P | P | P | P | P | P |
| sweep-completion-record | P | P | P | P | P | P | P | P |
| execute-terminal-archives | P | P | P | P | P | P | P | P |
| resolve-dev-strays | P | P | P | P | P | P | P | P |
| regroup-active-zen3 | P | P | P | P | P | P | P | P |
| regroup-active-field-dispatch | P | P | P | P | P | P | P | P |
| regroup-active-remaining | F: moving 02b8137c/3f664839 breaks gf2-sim identity checks (N1) | P | F: link scan does not cover the frozen-identity checks | F: code consumers in `crates/gf2-sim` treated as comment citations (N1) | P | P | P | F |
| receipt-pin-path-tolerance | P | P | F: no defined verification run or baseline (N2) | F: harness/record file undeclared (N2) | P | P | P | F |
| move-f547c394-inputs | P | P | F: "Rust CI gate ... receipt checks" does not evaluate receipts (N2) | F: omits `src/receipt.rs` (N3) | P | P | P | F |
| relocate-bench-narrative | P | P | P | P | P | P | P | P |
| relocate-sim-studies-narrative | P | P | P | P | P | P | P | P |
| remove-active-husks | P | P | P | P | P | P | P | P |
| eliminate-dev-plans | P | P | P | P | P | P | P | P |
| eliminate-dev-sessions | P | P | P | P | P | P | P | P |
| eliminate-dev-presentations | P | P | P | P | P | P | P | P |
| move-presentation-decks | P | P | P | P | P | P | P | P |
| retarget-figure-generator | P | P | P | P | P | P | P | P |
| docs-scaffold-index | P | P | P | P | P | P | P | P |
| entry-page-gf2-core | P | P | P | P | P | P | P | P |
| entry-page-gf2-coding | P | P | P | P | P | P | P | P |
| entry-page-gf2-algebra | P | P | P | P | P | P | P | P |
| entry-page-gf2-sim | P | P | P | P | P | P | P | P |
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
| readme-landing-page | P | P | P | P | P | P | P | P |
| archive-lean-pipeline-doc | P | P | P | P | P | P | P | P |
| legacy-move-gf2-core-guides | P | P | P | P | P | P | P | P |
| legacy-move-gf2-coding-guides | P | P | P | P | P | P | P | P |
| verify-rustdoc-examples | P | P | P | P | P | P | P | P |
| audit-permanent-content | P | P | P | P | P | P | P | P |
| rewrite-dev-index | P | P | P | P | P | P | P | P |
| finalize-docs-policy | P | P | P | P | P | P | P | P |
| verify-final-links | P | P | P | P | P | P | P | P |
| retire-migration-checker | P | P | P | P | P | P | P | P |

## Blocking findings

### N1. Other dev/active entries are pinned by production code, and regrouping breaks them

Round 1 missed this; it is the same class as B1. `regroup-active-remaining` moves the flat b8206228 entries 02b8137c (task `in_progress`) and 3f664839 (INV-A rows 294, 323). It treats crate references to them as comment citations. They are not comments:

- `crates/gf2-sim/src/permanent_campaign/validation.rs:68-74` defines `FROZEN_VALIDATION_JOURNAL_DIRECTORY` and the frozen receipt/continuation paths under `dev/active/02b8137c/`.
- `validation.rs:1359-1367` rejects producer-0 evidence unless `preregistration_identity.path == "dev/active/02b8137c/pre-draw-validation-v1-preregistration.json"` and its sha equals `AUTHORIZED_LEGACY_PREREGISTRATION_SHA256`.
- `crates/gf2-sim/src/permanent_rare_event/artifact.rs:4245-4250` rejects any design identity whose path is not `dev/active/3f664839/design.md`.
- `tests/permanent_validation.rs:39-40,653,680,1636-1672` and `tests/permanent_rare_event_artifacts.rs:55` read these paths.
- 124 committed snapshot files under `dev/bench_results/` carry copies of these sources.

Moving the entries breaks the frozen-identity checks and the tests. Changing the constants invalidates the recorded identities, exactly as with P-02. INV-§6 ("regrouping dev/active does not break verification") is wrong for these entries.

Repointing in-file citations is also unsafe here. `relocation-protocol` and the regroup tasks' REQ-03 exempt only receipt directories, yet some files in moved entries are digest-pinned inputs, for example the 02b8137c preregistration and continuation JSON.

**Correction:**
- Get an owner decision per entry, recorded as D-xx in the brief. Either:
  - exclude 02b8137c and 3f664839 from regrouping as retained operational inputs, with an `active-layout` exception; or
  - add a tolerance task and a move task, as for D-52, covering `validation.rs`, `artifact.rs`, both tests and the frozen evidence. Do not move 02b8137c while its owning task is in progress.
- Add an inventory criterion to `inventory-dev-active`: every entry with a non-comment code, test, script or config consumer is recorded with that consumer, and regroup tasks act only on entries without one.
- Extend `relocation-protocol` and each regroup REQ-03 so digest-pinned machine inputs (preregistrations, addenda, plans, journals, schemas) keep their bytes, like receipt directories.

### N2. "Every committed receipt verifies" has no defined run or baseline

`receipt-pin-path-tolerance` REQ-02 (test boundary "committed receipt verification run") and `move-f547c394-inputs` REQ-04 assert that every committed receipt verifies. No CI step evaluates committed receipts: `scripts/cargo-ci.sh:235-244` runs synthetic-repo tests (`tests/protocol_contracts.rs` stages scratch roots) and snapshot/schema scripts only. `src/bin/benchmark-acceptance.rs` exists but is not invoked. Whether every one of the ~190 receipts pinning `dev/active/f547c394/protocol.md` passes today is unestablished.

**Correction:**
- Define the run: `benchmark-acceptance` or `receipt::evaluate` over every committed `dev/bench_results/**/receipt.json` outside `inputs/` with schema `zen3-benchmark-receipt-v1`.
- Make the criterion baseline-relative: identical verdicts and findings before the change and after it, with the baseline recorded.
- Declare where the run lives, either a committed test (`creates`) or a linked record file.
- Drop the move task's claim that "the Rust CI gate ... receipt checks" covers this.

### N3. `move-f547c394-inputs` footprint omits `src/receipt.rs`

REQ-05 repoints in-file citations, and `dev/tools/tuning-campaign-support/src/receipt.rs:921` cites `dev/active/f547c394/protocol.md`. The historical-pin list from `receipt-pin-path-tolerance` REQ-01 is consulted by P-02 in that file.

**Correction:** add `dev/tools/tuning-campaign-support/src/receipt.rs` to `touches`.

## Advisories (non-blocking)

- `receipt-pin-path-tolerance` depends on `inventory-dev-buckets` with no stated reason. Use a re-home to 3f29e945 or a24b2af7 instead, if the only intent is sequencing.
- `readme-landing-page` and `execute-terminal-archives` both touch `README.md` and are unordered.
- Four parallel tasks declare `crates` (three regroups and the archive execution). Their concrete uncertainty is acceptable, but expect rebases.
- Brief D-49 lists three operational dirs. `tools` and the `sessions` elimination come from D-27/D-28, and the plan's D-49 row adds `tools`. Align the wording.
- Earlier advisories that still apply: the shared manifest file (now stated), the 13 `docs/index.md` writers (PD-10), and the f357b3dc/44c98235 label divergence after rejection.
- The plan stays concise: about 160 hand-written lines; the rest is the generated overview.

## Required follow-up

Fix the manifest (N1 also needs an owner decision recorded in the brief), update the plan rows for REQ-11, PD-03, "Receipt and CI integrity" and `relocation-protocol`, regenerate, and rerun validate, `render --check` and the dry run.

VERDICT: FAIL
