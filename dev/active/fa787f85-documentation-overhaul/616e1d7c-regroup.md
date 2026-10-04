# Re-archive of epic 6dc81018 (616e1d7c)

Issue 616e1d7c covers the Appendix A rows of [investigation.md](investigation.md)
whose top epic includes 6dc81018, less the code-pinned entries a83583e0 and
dbd8787d. The epic is archived, so decision DEC-01 of the issue collects its
entries through container archival. `jit archive container 6dc81018 --execute`
ran on main in commit `50de04a6b`. `AR` is
`dev/archive/6dc81018-field-capability-dispatch`.

| File | Content |
|---|---|
| [616e1d7c-preexec.json](616e1d7c-preexec.json) | The `jit archive container 6dc81018 --json` preview taken on main immediately before execution, reduced to target, destination root, eligibility, blockers, action counts and per artifact source, action, destination, sha256, evidence, relinked references and deleted sources |
| [616e1d7c-verify.py](616e1d7c-verify.py) | Verification script; its docstring states each section |
| [616e1d7c-verify.txt](616e1d7c-verify.txt) | Its output |
| [616e1d7c-rows.py](616e1d7c-rows.py) | Manifest row reconciliation; its docstring states the decision rule |

## Entries

| Entry | State | Files |
|---|---|---|
| 1ac74567, 2a85f728 | archived by the first archive run | 5, 1 under `AR/active/` |
| 34d85cb9 | archived | 180 moved |
| e6ea0dde | archived | 66 moved |
| eb9b324c | archived | 5 moved (`pilot/`) |
| 7d7c647c | archived; `design.md` stays with an archive copy | 87 moved (`probes/`), 1 kept |
| 50b47eae | archived; six files stay with an archive copy | 57 moved, 6 copied |
| 220cab0b, 3fa7c9d0, 7d824b2f, 972e2b88, eaae1b56, fc976a80 | one file each stays with an archive copy | 6 kept |
| 9162956b | stays with an archive copy | 14 kept |
| 6dc81018-field-capability-dispatch | 4 files stay with an archive copy; `handoff-12.md` and `handoff-13.md` stay without one | 6 kept |
| 389aa4de | flat, outside the archive plan | 1 |

## Criteria

| Criterion | Evidence in `616e1d7c-verify.txt` | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | `preview`, `marker`, `bytes`: the plan is eligible without blocker; 401 of 401 published destinations hold the planned sha256 at the execution commit; 395 of 395 move sources are absent; 6 of 6 copy sources hold the planned sha256 | The files the plan moves are under `AR`, byte-identical to their sources at execution | Anything about 389aa4de |
| REQ-02 | `references`: 522 of 522 relinked tracker documents on 25 issues name their archive path | The tracker of the checkout names the archive path for each reference the plan relinks | References of kept files that an issue outside the container owns |
| REQ-03 | `links`: 0 unresolved in the 60 Markdown files of `AR` and in the Markdown files of this issue under the active area | Inline links outside fenced code resolve, except three that resolve from the plan source path only | Anchors, code-span paths, links in non-Markdown files |
| REQ-04 | `excluded`: no path under `dev/active/a83583e0` or `dev/active/dbd8787d` changes, is published or is deleted | The code-pinned entries are in place | The Rust CI verdict; `cargo-ci` runs it |
| REQ-05 | `manifest`: every row of a plan source is `complete`; `python3 616e1d7c-rows.py --check` exits 0 | 395 rows archived, 33 rows retained in place by this unit | — |
| REQ-06 | Commit `03272e29d` | Path text only, per entry in the commit message | — |

The three links that resolve from the source path only are relative links
from `AR` files to files outside the development root
(`handoff-11.md:140`, `2026-09-01-eaae1b56.md:82,98`). The planner evaluates
an archived file's links at its source path, so their targets keep their text.

`rerun`: a fresh preview is eligible, without blocker, and publishes and
deletes nothing. It does not establish the result of a second `--execute`.

## Pins of a83583e0

Unpinned, documents 5 to 11 of a83583e0 make the planner move seven files of
`dev/active/a83583e0/`; `campaign-declaration.json` among them is read by path
by the tuning-extent campaign driver. Each reference carries the commit that
last changed its file, and the plan retains the seven as `pinned-historical`.

| Documents | Files | Pin |
|---|---|---|
| 5 | `campaign-declaration.json` | `87b5b733c` |
| 6 to 11 | six files under `failed-attempts/gf2-a83583e0-20260928T130755Z-3719910/` | `eebbc778c` |

## Files that stay

Each row below is `retained-operational`, empty destination, `complete`.

| Files | What keeps them |
|---|---|
| `50b47eae/s{4,5,6}-session/{comparison,layout-audit}.txt` | Root-relative link targets of post-cutover receipts 4, 5 and 6, which keep their bytes |
| `9162956b/ensemble-axes-pilot-receipt.md`, `972e2b88/ensemble-axis-verification.md`, `fc976a80/findings.md`, `220cab0b/design.md` | Root-relative link targets of the verdicts and the selector plan under `dev/benchmarks/tuning_profiles/` and of the 9162956b pilot receipt |
| 13 further files of 9162956b | Relative link targets of the 9162956b pilot receipt and protocols |
| `3fa7c9d0/design.md`, `7d824b2f/design.md`, `eaae1b56/premeasurement-protocol.md`, `7d7c647c/design.md` | Relative link targets of the a83583e0 and dbd8787d protocols, which stay by exclusion; issue 3fa7c9d0 resolves under the open epic 86b9c719 |
| `classification.md`, `plan.md`, `investigation.md`, `breakdown.json` of the epic directory | Relative link targets of the designs above and of each other |
| `handoff-12.md`, `handoff-13.md` of the epic directory | References of 6dc81018 pinned to commits `623f5568` and `4c70534c` |

Two kept files differ from their archive file by citation repoints of other
units after the first archive run: `6dc81018-field-capability-dispatch/investigation.md`
and `eaae1b56/premeasurement-protocol.md`. The plan reports neither as a
conflict. `AR/active/7d7c647c/design.md` holds the bytes of its source
(commit `1be94823d`): an archive file that differs both from its source and
from its recorded publication blocks the plan with `destination-conflict`.

## 389aa4de

| File | Tracker parent of issue 389aa4de | Plan row "Active-layout ties" | State |
|---|---|---|---|
| `dev/active/389aa4de/receipt-notes.md` | 86b9c719 (`jit graph tree`), state backlog | assigns the entry to 6dc81018 | flat; in no archive plan; manifest row `pending` |

## Citations

Repointed in commit `44c701933`, path text only:

| File | Cites |
|---|---|
| `dev/active/7d7c647c/design.md` and `AR/active/7d7c647c/design.md` | e6ea0dde, 34d85cb9, `7d7c647c/probes` |
| `dev/active/ae03bcd0-general-bch/investigation.md` | `7d7c647c/probes/AS1_lean/Funs.lean` |
| `AR/active/1ac74567/proof-sketch.md`, `34d85cb9/findings.md`, `e6ea0dde/record.md`, `handoff-2.md` to `handoff-5.md`, `handoff-8.md` | 34d85cb9, e6ea0dde, 50b47eae, eb9b324c, `7d7c647c/probes` |
| Comments in `AR/active/7d7c647c/probes/trim-logs.sh`, `e6ea0dde/extraction/trim-logs.sh`, `eb9b324c/pilot/pilot-build.sh`, `1ac74567/elaboration/elaborate.sh` | sibling entries |
| Input paths `LED` in `AR/active/eb9b324c/pilot/pilot-analyse.py` and `SRC` in `AR/active/1ac74567/elaboration/elaborate.sh` | `50b47eae/s5-session`, `34d85cb9/extraction/A8b_lean` |

Two repointed command quotations name generated files that no commit holds
(`34d85cb9/extraction/R2_gf2_core.llbc`, `e6ea0dde/extraction/X3_gf2_core.llbc`).

Citations that keep their bytes:

| Location | Reason |
|---|---|
| `dev/benchmarks/tuning_profiles/2026-08-22-post-cutover-receipt-{4,5}.md` and the three receipts under `AR/benchmarks/tuning_profiles/` | Receipts |
| 19 scripts under `AR/active/50b47eae/s{4,5,6}-session/`, three of which name their own directory | Manifest `digest_pinned`; the post-cutover receipts record their digests |
| Logs, excerpts and Lean outputs under `AR/active/`, `progress.json` | Tool outputs and continuation JSON |
| `67048b47-linked-pairs.txt`, `7bac1303-harness-path-trials.txt`, `96cea1b9-archive-rows.txt` | Committed script outputs |
| `migration/manifest.toml`, `path` fields | The field names the source by schema |

`git grep -F` for the five moved entry directories finds no citation in a
README, `AGENTS.md`, `docs/` page or Rust source outside `dev/archive/` and
receipt `inputs/` trees. `scripts/cargo-ci.sh:268` and
`crates/gf2-core/benches/tuning_calibration.rs:607` cite
`7d824b2f/design.md` and `eaae1b56/premeasurement-protocol.md` in comments;
both files stay.

`96cea1b9-archive-rows.py` requires exactly one archive event for the
container and exits on the two the event log holds.

## Commands

```sh
python3 dev/active/fa787f85-documentation-overhaul/616e1d7c-verify.py
python3 dev/active/fa787f85-documentation-overhaul/616e1d7c-rows.py --check
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py
python3 contrib/gates/docs-mechanical.py
```

`migration/check.py` reports no finding on a row of this unit.
