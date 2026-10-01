# Plan review r3: fa787f85 documentation overhaul

Planning node 8dddfc7a. Container fa787f85. Reviewed at `ba6c199ad`, which includes `728f1343c` (plan and manifest), `ddcc03eff` (brief D-53) and `ba6c199ad` (brief D-28 names `dev/sessions`). Read-only review; no tracker state changed.

## Mechanical checks

| Check | Result |
|---|---|
| `validate --deny-warnings`: known sources = every manifest ref (D-02..D-53, DEC-05, INV-*, OD-08, PD-*, REQ-01..27); required REQ-01..REQ-20 | exit 0; advisories only (8 uncertainty, overlaps) |
| `render --check` | exit 0 |
| `batch-create --dry-run --json` | valid; 84 issues, 145 edges |
| Source refs | All resolve; D-53 is at brief line 90, D-28 at line 65. |
| Contracts and spine | Producers are exact. The three tolerance tasks are declared spine sources with no external edge (plan "External re-homes" row). The other sources reach 3f29e945. |

## Round-2 findings and advisories

| Item | Status | Evidence |
|---|---|---|
| N1 code-pinned entries | Fixed in substance | Owner decision D-53. `rare-event-design-path-tolerance` then `move-3f664839-design` covers `artifact.rs:4245-4250` and `tests/permanent_rare_event_artifacts.rs:55`. `campaign-validation-path-tolerance` then `move-02b8137c-journal` covers `validation.rs:68-74,1359-1367` and `tests/permanent_validation.rs`. Both keep digest checks unchanged (tolerance REQ-01). `regroup-active-remaining` excludes both entries. All three regroup tasks gained REQ-04 (code-consumer entries stay in place). `inventory-dev-active` REQ-05 records non-comment consumers and digest-pinned inputs. All regroup and move tasks keep digest-pinned machine inputs byte-identical. One new defect in the 02b8137c wiring: R1. |
| N2 receipt run undefined | Fixed | `receipt-pin-path-tolerance` REQ-02 names `benchmark-acceptance` (`src/bin/benchmark-acceptance.rs`: `<receipt-dir>`, exit 0/1/2). It covers every committed `zen3-benchmark-receipt-v1` `receipt.json` outside `inputs/`, uses before/after identical verdicts and findings, and declares `receipt-verdict-baseline.md` in `creates`. `move-f547c394-inputs` REQ-04 repeats the run. Both gf2-sim tolerance tasks use the same baseline-relative pattern with their own record files. |
| N3 `receipt.rs` | Fixed | It is in the `move-f547c394-inputs` `touches`. |
| Advisory: tolerance task dependency | Fixed | Now a declared source. |
| Advisory: README ordering | Fixed | `readme-landing-page` depends on `execute-terminal-archives`. |
| Advisory: D-49/D-28 wording | Fixed | D-28 names `dev/sessions`. |

## Assignment simulation (finest tier: 83 tasks)

The 24 sweep units are unchanged since r2. The new tolerance tasks each change one identity check, its historical-path list and two tests, plus one recorded baseline run. The new move tasks each cover one small entry and its readers.

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
| regroup-active-remaining | P | P | P | P | P | P | P | P |
| receipt-pin-path-tolerance | P | P | P | P | P | P | P | P |
| move-f547c394-inputs | P | P | P | P | P | P | P | P |
| rare-event-design-path-tolerance | P | P | P | P | P | P | P | P |
| move-3f664839-design | P | P | P | P | P | P | P | P |
| campaign-validation-path-tolerance | P | P | P | P | P | P | P | P |
| move-02b8137c-journal | P | P | P | P | P | P | P | P for sizing; graph edge defect R1 |
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

## Blocking finding

### R1. The 02b8137c re-home edge pulls another epic's subtree into fa787f85 and falsely credits REQ-02 and REQ-04

The plan's "External re-homes" row adds `move-02b8137c-journal` → 02b8137c. 02b8137c is an in-progress task of epic b8206228 (`epic:permanent-statistics`). Its transitive dependency closure holds 69 issues. Among them:

- d1b6a182, 49cbb131, 6ae9f4ac, 531695da and b494fec8 carry `satisfies:REQ-02`.
- 175972df carries `satisfies:REQ-04`.
- Archived epic ae82bd73 and its subtree are also in the closure.

`label-coverage` walks dependencies transitively and stops only at planning and breakdown types (`just-in-time/crates/jit/src/validation/graph.rs:775-884`; `.jit/rules.toml:78-93`). Those tasks sit directly below 02b8137c, not behind a breakdown node, so both `coverage-preview` and `hard-criteria-covered` would credit fa787f85 REQ-02 (AGENTS/CONTRIBUTING) and REQ-04 (docs layout) from unrelated done work. The edge also makes all 69 issues descendants of fa787f85 in the resolved hierarchy.

The plan already rejects this kind of edge for ae03bcd0 ("an edge into ae03bcd0 would pull another epic's work into this subtree"). It is the defect the review was asked to rule out.

**Correction:**
- Remove the `move-02b8137c-journal` → 02b8137c re-home from the plan.
- Add a hard criterion to `move-02b8137c-journal`: "No file moves until 02b8137c is terminal (done or rejected), as shown by `jit issue show 02b8137c` and recorded in the commit message."
- Restate the "Ordering the graph cannot encode" row as two items: the ae03bcd0 window, and the 02b8137c-terminal precondition.
- Update the D-53 row ("waits for its in-progress owning task") to name this mechanism.
- Regenerate and rerun the checks.

## Advisories (non-blocking)

- `campaign-validation-path-tolerance` edits `crates/gf2-sim/src/permanent_campaign/validation.rs` while 02b8137c, which owns that validation work, is in progress. Expect to coordinate with its worker.
- Regroup REQ-04 leaves any newly found code-consumed entry in place for "its own tolerance-then-move work". D-53 says such entries get new tasks, but nothing in the graph creates them, so REQ-11 could remain open for those entries. `finalize-docs-policy` or the execution lead should confirm that no such entry remains.
- `recorded_in` paths in `dev/scripts/receipt-input-omissions.json` are documentation fields that the check script does not read (`check-receipt-input-snapshots.py` uses only `receipt` and `path`). Treat them as human-facing citations, as the footprints imply, and not as config consumers under REQ-04.
- Advisories carried over: four parallel tasks declare `crates`; the three inventories share one manifest file (stated); `docs/index.md` has many writers (PD-10); f357b3dc and 44c98235 labels will diverge after rejection.
- The plan stays concise: about 170 hand-written lines.

R1 is the only blocking finding. Its correction is mechanical and needs no new owner decision.

VERDICT: FAIL
