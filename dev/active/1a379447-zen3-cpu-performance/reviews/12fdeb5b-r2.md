# Review: Establish 5G NR rate-matched encoder baselines (12fdeb5b)

**Verdict:** PASS (after two rework rounds and one invoker decision)
**Reviewed commit:** 5b377150 (worker branch merged at e6a54397 and 5b377150)

### Gate status

All passed on 5b377150: cargo-ci (5987 tests, 0 failed), code-review (0 findings), doc-review (0), research-review (0). Round-1 gate runs on 4ade51f8: cargo-ci and doc-review passed; code-review F1 (no RV3 validation) and research-review F1 (retry not preregistered) failed.

### Prior-findings regression table (Tier 1.5)

| Round | Finding | Status at HEAD | file:line |
|---|---|---|---|
| lead R1 | ledger reservation of the aborted confirmation removed by hand | superseded by DEC-01 (invoker ruling): aborted attempt voided, stage preserved | `dev/bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation-abandoned/`, `dev/bench_results/12fdeb5b/v4-abandoned-confirmation-attempt.json`, protocol paragraph in `dev/active/f547c394/protocol.md` and `amendment-v4.md` |
| lead R2 | three restated finding counts | closed | pointers at the tables' Source lines |
| code-review R1 F1 | REQ-03 lacks an RV3 srsRAN configuration | closed | `dev/active/12fdeb5b/survey/nr-encode/src/lib.rs:105-108`; RV3 rows in `survey/nr-encode-validation.json`, tables "Bit-exact equivalence outcomes", findings § Where the projects diverge |
| research-review R1 F1 | void rule postdates the measurement | foreclosed by DEC-01; premise recorded at `dev/active/12fdeb5b/findings.md:235`, `plan.md:175-180` |

No regression.

### Success criteria

- [x] REQ-01 — smoke, pilot and confirmation receipts accepted under protocol v4 (finding counts on the tables' Source lines); the confirmation's `fail` cell (AFF3CT materially slower at BG2 n256 k121) and the pilot's control outcomes preserved as the evaluator states them; aborted attempt preserved with its record; DEC-01 recorded on the issue.
- [x] REQ-02 — `survey/build-evidence.json`, `survey/source-evidence.json` (pins, licences, flags, backend), plan.md.
- [x] REQ-03 — `survey/nr-encode-validation.json`: 26 configurations, 39 matched arms bit-exact, both base graphs, fillers, lifting boundaries, RV0–RV3 for srsRAN (RV1–RV3 non-equivalent to gf2's RV0-only selection, as expected), AFF3CT unavailable beyond RV0; non-equivalences recorded, not timed.
- [x] REQ-04 — frozen pilot and confirmation addenda; pilot (8 cells) and confirmation (6 cells, 24 pairs each, verified by the lead from the execution logs) receipts committed; unavailable and non-equivalent arms recorded in the tables.

Citation keys Cassagne2019, Srsran2026, ThreeGpp2017 appear in the report text.

### Stale-narrative sweep (Tier 2.5)

No forward-looking reference to 12fdeb5b outside its directories.

### Deferred-items audit (Tier 2.75)

No deferred/TODO/follow-up/open-question markers; the `srsran-interleave-identity` claim is stated as deliberately outside the validation grid with its reason.

### Holistic findings

- The canonical freezer `dev/active/c7113c5a/survey/freeze-confirmation.py` was extended additively (cell selection, rationale, resolution override with the pilot-alpha width as a floor); existing callers reproduce byte-identical addenda (worker-verified; 1c602857 froze with it the same morning).
- Protocol v4 gains the voided-attempt paragraph without a version bump (acceptance behaviour unchanged; P-22 reads no live ledger), recorded in amendment-v4.md and scoped to f547c394.
- Scope: every changed path is under the issue's directories, `dev/active/f547c394/` (the amendment) or the shared freezer.
- 34 artifacts linked; tables regenerate byte for byte on the merged tree.
