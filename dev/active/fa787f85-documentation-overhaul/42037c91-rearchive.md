# Re-archive of epics bb85c68a and 6efb756b

Issue 42037c91. Container archival of both epics is executed in commit
`02984b612`, bb85c68a first. `BB` is `dev/archive/bb85c68a-field-linear-algebra`,
`GR` is `dev/archive/6efb756b-grand`.

## Evidence files

| File | Content |
|---|---|
| `42037c91-preexec.json` | Per epic, the `jit archive container <epic> --json` preview taken immediately before execution, reduced to target, destination root, eligibility, blockers, action counts and per artifact source, action, destination, sha256 and deleted sources. |
| `42037c91-verify.py` | Produces the report; `--reduce` produces `42037c91-preexec.json` from the raw previews. It takes archive roots from the tracker, expected files and hashes from `42037c91-preexec.json`, and repointed files from the commits tagged `jit:42037c91`. |
| `42037c91-verify.txt` | Report of `python3 dev/active/fa787f85-documentation-overhaul/42037c91-verify.py`, exit 0, run at the commit that adds the report. It reads the working tree and the tracker of the checkout. |

## Criteria

| Criterion | Evidence in `42037c91-verify.txt` | The check establishes | The check does not establish |
|---|---|---|---|
| REQ-01 | `pre-execution preview` lines: both epics `eligible=True`, `blockers=0`, destination roots `BB` and `GR` | The preview before execution is eligible, without plan-level or artifact-level blocker, into the existing directory. | That the reduced file equals the raw preview; the raw JSON is not committed. |
| REQ-02 | `marker` lines; the two `artifact_archive_executed` events that `02984b612` appends to the tracker event log | `BB/.jit-container` names `bb85c68a-f88e-46fb-b0ab-cddc3487cbc9`; `GR/.jit-container` names `6efb756b-bee0-4dc1-9ee6-a1a0a61034d4`. | – |
| REQ-03 | `byte verification`: 17/17 and 17/17 destinations, 0 outside the archive directory, 0/0 and 1/1 deleted sources; `tracker references`: 18/18 and 21/21 | Each destination of the pre-execution plan lies in the epic's archive directory and has the sha256 the plan records; the source the plan deletes is absent; each document reference the plan relinks names its archive path. | Anything about the 11 files the bb85c68a plan retains in the archive of 97bf0879 (below). |
| REQ-04 | Table "Citations" below | – | – |
| REQ-05 | `fresh preview` lines: 0 artifacts left to move, copy or delete; `link scan` lines: 0 unresolved; `manifest rows`: 7 and 15 rows, each `complete`; `archive-rerun-results.md`, rows of bb85c68a and 6efb756b for issue 42037c91 | A preview of each archived epic plans no publication of an unarchived file and no deletion; no inline Markdown link and no HTML `href` or `src` with a local target in `BB`, `GR` or the files of the `jit:42037c91` commits is unresolved; each manifest row whose `epic` is one of the two epics is complete. | The result of a second `--execute`, which `archive-rerun-results.md` holds; reference-style Markdown links, `url()` targets in stylesheets and paths in code spans. |

## What the execution changed

The two event records of `02984b612` and its diff:

| Epic | Adopted publications | Reference changes | Sources deleted | Files moved or copied |
|---|---|---|---|---|
| bb85c68a | 2 | 12 | 0 | 0 |
| 6efb756b | 4 | 11 | 1 | 0 |

An adopted publication records a file that is present at its destination with
the planned bytes. The diff of `02984b612` holds three files: the event log,
the record of issue 831bfc4a, and the deleted
`dev/simulation_results/phase4_comparison_report.md`. Document 2 of 831bfc4a
names `GR/simulation_results/phase4_comparison_report.md`; that copy is in the
archive since `195f8254f` and has the sha256 of the deleted source.

Issue 831bfc4a holds two references to that archive path (documents 0 and 2,
labels "Phase 4 Comparison Report" and "phase4_comparison_report.md"); the
report lists them. `jit validate` exits 0 and reports nothing on the issue, and
the fresh preview has no blocker.

The deck files under `BB/docs/presentations/` and `GR/docs/presentations/` keep
their location. The plans derive `dev/docs/presentations/...` as their sources,
a managed root under DEC-01 of the issue, and no such directory exists. The
fresh previews list, beyond the pre-execution artifacts, the stylesheets and
figures that the two `talk.html` files embed (2 and 5 entries), each archived at
its destination.

## Retained by the bb85c68a plan

| Entry | Reason |
|---|---|
| `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-04-30-post-ppc-delta-appendix.md` | Document of 64c88ae4 and a9ab0a4f of this epic; the file is in the marker-backed archive of 97bf0879, whose issues 0fd48627, 974a85bd, b0434149 and b8189dbf also hold it (plan evidence `archived-source`, `outside-owner`). |
| 10 files of the same archive | Relative link targets of the appendix; same evidence. |

No manifest row corresponds to a retained entry under `epic` bb85c68a.

## Citations

| File | Line | Target |
|---|---|---|
| `dev/active/ae03bcd0-general-bch/investigation.md` | 274 | `GR/simulation_results/phase4_comparison_report.md` |

`git grep -F phase4_comparison_report` finds no citation of the deleted path in
a README, `AGENTS.md`, Rust source or page under `docs/`.

Citations that keep their bytes:

| Location | Reason |
|---|---|
| `67048b47-linked-pairs.txt:830` | Committed output of `67048b47-doc-links.py`. |
| `migration/manifest.toml`, `path` field of the row | The field names the source by schema. |
| `GR/active/831bfc4a-run-phase-4-fading-channel-simulations-figs-8-10/831bfc4a-completion.md:9` | Archived historical artifact; a code span, not a link. |

## Manifest

| Row | Field | Value | Evidence |
|---|---|---|---|
| `dev/simulation_results/phase4_comparison_report.md` | `status` | `complete` | Source absent; destination `GR/simulation_results/phase4_comparison_report.md` has the pre-execution sha256. |

## Verification

| Command | Result |
|---|---|
| `python3 dev/active/fa787f85-documentation-overhaul/42037c91-verify.py` | exit 0 |
| `python3 contrib/gates/docs-mechanical.py` | PASS |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding on the row of this unit |
