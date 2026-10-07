# Re-archive of epic 6dc81018 (616e1d7c)

Issue 616e1d7c covers the Appendix A rows of [investigation.md](investigation.md)
whose top epic includes 6dc81018, less the code-pinned entries a83583e0 and
dbd8787d. The epic is archived, so decision DEC-01 of the issue collects its
entries through container archival. `jit archive container 6dc81018 --execute`
ran on main in commit `50de04a6b`. `AR` is
`dev/archive/6dc81018-field-capability-dispatch`. Decision DEC-02 places entry
389aa4de under `Q`, `dev/active/86b9c719-quality-documentation-tech-debt`.

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
| 389aa4de | under `Q/389aa4de/` | 1 moved |

A file that stays is under `dev/active/<entry>/`. The `manifest`
section of the output lists, per entry, the directory holding it and the row
status.

## Criteria

| Criterion | Evidence in `616e1d7c-verify.txt` | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | `preview`, `marker`, `bytes`: the plan is eligible without blocker; 401 of 401 published destinations hold the planned sha256 at the execution commit; 395 of 395 move sources are absent; 6 of 6 copy sources hold the planned sha256 | The files the plan moves are under `AR`, byte-identical to their sources at execution; `manifest` adds that the file of each of the 465 rows is at its destination, or at its path when the destination is empty | — |
| REQ-02 | `references`: 522 of 522 relinked tracker documents on 25 issues name their archive path | The tracker of the checkout names the archive path for each reference the plan relinks | References of kept files that an issue outside the container owns; the reference of issue 389aa4de to its note, which `jit doc check-links --scope all` covers |
| REQ-03 | `links`: 239 of 239 local links in the 60 Markdown files of `AR` resolve, and 26 of 26 in the 11 Markdown files of this issue under the active area; `citations` | Each inline link outside fenced code resolves from its file's directory or, with a leading `/`, from the checkout root | Anchors, code-span paths, links in non-Markdown files |
| REQ-04 | `consumers`: the 222 manifest consumers on 19 rows of the entries lie inside entries 1ac74567, 34d85cb9, 50b47eae, 7d7c647c, e6ea0dde and eb9b324c, each of which has archived rows; `excluded`: no path under `dev/active/a83583e0` or `dev/active/dbd8787d` changes, is published or is deleted | No entry has a manifest consumer outside the entries this unit moves or archives together (decision DEC-04); the code-pinned entries are in place | The Rust CI verdict; `cargo-ci` runs it |
| REQ-05 | `manifest`: each of the 465 rows is `complete`; `python3 616e1d7c-rows.py --check` exits 0 | 431 rows archived, 33 retained in place, 1 moved under `Q` | — |
| REQ-06 | Commit `03272e29d` | Path text only, per entry in the commit message | — |

The receipt `dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` and its copy
`AR/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` carry root-relative
targets on lines 82 and 98 (decision DEC-03 of the issue) and hold equal bytes.
With the same targets in both files the preview is eligible; with them in the
archive copy alone it blocks on `destination-conflict` for the receipt and
`unpreservable-layout` for `dev/active/7d824b2f/design.md`.

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

Four kept files differ from their archive file by edits of other units after
the first archive run: `6dc81018-field-capability-dispatch/{breakdown.json,classification.md,investigation.md}`
and `eaae1b56/premeasurement-protocol.md`. The plan reports none as a
conflict. `AR/active/7d7c647c/design.md` holds the bytes of its source
(commit `55fbea37a`): an archive file that differs both from its source and
from its recorded publication blocks the plan with `destination-conflict`.

## Citations

Repointed in commit `44c701933`, path text only:

| File | Cites |
|---|---|
| `dev/active/7d7c647c/design.md` and `AR/active/7d7c647c/design.md` | e6ea0dde, 34d85cb9, `7d7c647c/probes` |
| `dev/active/ae03bcd0-general-bch/investigation.md` | `7d7c647c/probes/AS1_lean/Funs.lean` |
| `AR/active/1ac74567/proof-sketch.md`, `34d85cb9/findings.md`, `e6ea0dde/record.md`, `handoff-2.md` to `handoff-5.md`, `handoff-8.md` | 34d85cb9, e6ea0dde, 50b47eae, eb9b324c, `7d7c647c/probes` |
| Comments in `AR/active/7d7c647c/probes/trim-logs.sh`, `e6ea0dde/extraction/trim-logs.sh`, `eb9b324c/pilot/pilot-build.sh`, `1ac74567/elaboration/elaborate.sh` | sibling entries |
| Input paths `LED` in `AR/active/eb9b324c/pilot/pilot-analyse.py` and `SRC` in `AR/active/1ac74567/elaboration/elaborate.sh` | `50b47eae/s5-session`, `34d85cb9/extraction/A8b_lean` |

Repointed in commit `7d18de5f4`, path and link-target text only:

| File | Cites |
|---|---|
| `Q/389aa4de/receipt-notes.md:21,39,70` and `:38,43,147` | `AR/active/6dc81018-field-capability-dispatch/handoff.md`, `AR/benchmarks/tuning_profiles/2026-08-20-host-calibration.md` |
| `AR/active/6dc81018-field-capability-dispatch/handoff-6.md`, `handoff-7.md`, `handoff-8.md` | `Q/389aa4de/receipt-notes.md` |
| `AR/active/6dc81018-field-capability-dispatch/handoff-11.md:140` | the execution-lead skill, by a root-relative target |

Two repointed command quotations name generated files that no commit holds
(`34d85cb9/extraction/R2_gf2_core.llbc`, `e6ea0dde/extraction/X3_gf2_core.llbc`).

The `citations` section of the output counts, per citing file and ground, the
citations of a path under an entry of this unit that the tree lacks, found by
`git grep -F` for `dev/active/<entry>` and `../<entry>/` outside `.jit/`: 3014
outside the tracker documents of fa787f85. Commit `55fbea37a` repoints 52.

Repointed in commit `55fbea37a`, path text only:

| File | Cites |
|---|---|
| `dev/active/7d7c647c/design.md:12,107,110,552` and `AR/active/7d7c647c/design.md`, equal bytes | `AR/active/1ac74567/proof-sketch.md`, `progress.json` of the archived epic directory |
| `AR/active/1ac74567/proof-sketch.md`, `AR/active/e6ea0dde/record.md` | files of 1ac74567 |
| `AR/active/1ac74567/elaboration/elaborate.sh:8,9,30` | its own directory, in two comment lines and in `HERE` |
| `handoff-7.md:51`, `handoff-8.md:5,60` of the archived epic directory; `handoff-12.md:114`, `handoff-13.md:84` of `dev/active/6dc81018-field-capability-dispatch` | `progress.json`, `handoff.md` of the archived epic directory |
| `dev/active/f547c394/research-r3-sweep-resolutions.md:775,776,870,1021` and the `path` fields at `research-r3-sweeps.json:3982,3987,4457,5258` | `AR/active/2a85f728/asm-delta.md`, `progress.json`, `handoff.md` |
| `8f61d6de-baseline-findings.md:9,10`, `d1f6b2c9-inventory-notes.md:3` | `handoff-4.md`, `handoff-17.md` of the archived epic directory |

`design.md:12` cites `progress.json` without a line suffix; line 785 of the
archived file holds other content. The words that `design.md:104-107` quotes
differ from the wording of lines 900 to 907 of the sketch, which state the
same fact.

Citations that keep their bytes, each on one ground:

| Citing file | Citations | Ground |
|---|---|---|
| `migration/manifest.toml` | 938 | Manifest schema fields |
| `616e1d7c-preexec.json` | 828 | Digest pin: content hash in `.jit/issues/616e1d7c-1e65-45f0-91d4-2eb194746882.json` |
| `67048b47-linked-pairs.txt`, `96cea1b9-archive-rows.txt`, `7bac1303-harness-path-trials.txt` | 395, 431, 3 | Tool output: listings that units 67048b47, 96cea1b9 and 7bac1303 commit beside their scripts |
| Files under `AR`, the `container root` line of the output: logs, excerpts, Lean files, `progress.json`, `breakdown.json`, `handoff.md`, two receipts | 412 | Marker-backed archive files |
| `dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt-3.md:136`, `2026-08-22-post-cutover-receipt-4.md:138,139` | 3 | Receipts, equal in bytes to their archive files |
| `dev/active/6dc81018-field-capability-dispatch/breakdown.json:42,126,457,458` | 4 | Tracker state: the breakdown manifest of 6dc81018; the tree holds none of the four cited paths |
| Session handoffs and `progress.json` of this epic directory | not counted | Tracker state: tracker documents of fa787f85 |

The two Lean files among them that a person wrote,
`AR/active/1ac74567/elaboration/FunsExternal.lean` and
`AR/active/e6ea0dde/elaboration/Statements.lean`, are the inputs of the
committed elaboration logs.

`git grep -F` for the six moved entry directories finds no citation in a
README, `AGENTS.md`, `docs/` page or Rust source outside `dev/archive/` and
receipt `inputs/` trees. `scripts/cargo-ci.sh:268` and
`crates/gf2-core/benches/tuning_calibration.rs:607` cite
`7d824b2f/design.md` and `eaae1b56/premeasurement-protocol.md` in comments;
both files stay.

## Commands

```sh
python3 dev/active/fa787f85-documentation-overhaul/616e1d7c-verify.py
python3 dev/active/fa787f85-documentation-overhaul/616e1d7c-rows.py --check
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py
python3 contrib/gates/docs-mechanical.py
```

`migration/check.py` reports no finding on a row of this unit; it exits 1 on
six rows of entries 3be770d5, 4c1e441f, ad2a6a58 and f63a2464.
