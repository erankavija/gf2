# eace5009 regroup record

Scope: `investigation.md` Appendix A rows whose top epics include b8206228,
86b9c719, ae03bcd0, b4b4b9ee or d77176e5. Base commit `98e238917`; moves in
`917b5530a`, citation repoints in `db27d21a1`.

## Entries

"Epics" lists each `type:epic` issue reached over reverse dependency edges from
the entry's owners, with the count of the entry's linked documents it owns.
Rule L is the `active-layout` membership-label tie-break: the tied epic named by
the owners' single applicable `epic:` label.

| Entry | Owners | Epics (linked documents) | Rule applied | Destination under `dev/active/` | Result |
|---|---|---|---|---|---|
| 0de41c82 | 0de41c82, 1b113b9d | b8206228 (6) | single epic | `b8206228-permanent-statistics/0de41c82/` | moved |
| 41c3d91d | 41c3d91d | b8206228 (1) | single epic | `b8206228-permanent-statistics/41c3d91d/` | moved |
| 150d7d79 | 150d7d79 | 86b9c719 (1) | single epic | `86b9c719-quality-documentation-tech-debt/150d7d79/` | moved |
| 3d522ba0 | 3d522ba0 | 86b9c719 (1), b4b4b9ee (1) | L: `epic:tech-debt-2026-06-30` | `b4b4b9ee-tech-debt-2026-06-30/3d522ba0/` | moved |
| 8b3fe657 | 8b3fe657 | 86b9c719 (1), b4b4b9ee (1) | L: `epic:tech-debt-2026-06-30` | `b4b4b9ee-tech-debt-2026-06-30/8b3fe657/` | moved |
| aabc528a | aabc528a | 86b9c719 (1), b4b4b9ee (1) | L: `epic:tech-debt-2026-06-30` | `b4b4b9ee-tech-debt-2026-06-30/aabc528a/` | moved |
| 64fd3afd | 64fd3afd, 94597a51, b1bd75ca | 86b9c719 (3), ae03bcd0 (3) | L: `epic:general-bch` | `ae03bcd0-general-bch/64fd3afd/` | moved |
| 4e732b56 | 4e732b56 | 86b9c719 (18), ae03bcd0 (18) | L: `epic:general-bch` | `ae03bcd0-general-bch/4e732b56/` | left: six manifest `consumers` rows (table below) |
| 02b8137c, 3f664839, d1b4f85e, 591a1c5e, fd9d5416 | — | — | — | — | left: excluded by the issue description |
| 220cab0b, 389aa4de, 3fa7c9d0 | — | 6dc81018, 86b9c719 | — | manifest epic 6dc81018 | left: scope of 616e1d7c |
| 3be770d5, c077a88b, f547c394 | — | 1a379447, d77176e5 | — | manifest epic 1a379447 | left: scope of 4f161183 and its exclusions |

Each moved entry's destination equals the `destination` field of its manifest
rows. No entry of this scope resolves to d77176e5. No moved entry holds a
script, so REQ-06 has no edit.

## Checks

| Check | Command | Result | Establishes |
|---|---|---|---|
| Manifest consumers | `consumers` field of each row under the entry, parsed with `tomllib` | empty for the seven moved entries; six rows for 4e732b56 | the manifest's code-consumer inventory |
| Consumer grep | `git grep -l -F "dev/active/<entry>/" 98e238917 -- . ':!dev/archive' ':!.jit' ':(exclude,glob)dev/bench_results/**/inputs/**'` | moved entries: Markdown, JSON data strings and one Lean header comment | no script, test, Rust source or build file names a moved entry; it does not detect a path assembled at run time |
| Unmerged work | `git log --oneline main..<branch> -- dev/active/<entry>` for each branch of `git worktree list` | empty for the eight entries | no worker branch changes an entry |
| ae03bcd0 quiet window | `git -C <main checkout> status --short -- dev/active/64fd3afd`; `git log --since='24 hours ago' main -- dev/active/64fd3afd` | both empty; last commit on main `976b4cd1c` (2026-10-01) | 64fd3afd is idle on main; it does not see uncommitted files in other worktrees |
| Link scan | `sed 's/jit:eb2a833d/jit:eace5009/' dev/active/fa787f85-documentation-overhaul/eb2a833d-link-scan.py \| python3 -` | `unresolved local links: 0` | each inline link in the Markdown files of the `jit:eace5009` commits resolves; anchors are outside it |
| Mechanical check | `python3 contrib/gates/docs-mechanical.py` | PASS, 0 findings | links and anchors of `dev/active/**/*.md` |
| Manifest | `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0; ten rows complete | source absent and destination present for each moved row |

## Left in place, reported for decoupling

Paths are relative to `dev/active/4e732b56/baseline-survey/`. Each consumer is a
depth-dependent path under `no-dev-path-coupling`: it holds at the entry's
current depth and at no other.

| Entry | Consumer | Construct | Decoupling under `no-dev-path-coupling` |
|---|---|---|---|
| 4e732b56 | `Makefile:10` | `$(CURDIR)/../../../../.agents/ext` | `EXT` derives from one runtime-resolved repository root |
| 4e732b56 | `fetch-build.sh:20` | `${HERE}/../../../..` | `REPO` is the runtime-resolved repository root |
| 4e732b56 | `run-survey.sh:25` | `${HERE}/../../../..` | `REPO` is the runtime-resolved repository root |
| 4e732b56 | `gf2-side/Cargo.toml:16-17` | `path = "../../../../../crates/gf2-coding"`, `.../gf2-core` | the two path dependencies resolve independently of the directory depth |
| 4e732b56 | `verify-generator-matrices.py:29` | `pathlib.Path(__file__).resolve().parents[4]` | `REPO` is the runtime-resolved repository root |
| 4e732b56 | `verify-generators.py:27` | `pathlib.Path(__file__).resolve().parents[4]` | `REPO` is the runtime-resolved repository root |

`make-receipt.py:264-266` prints three literals of the entry's own directory;
they are REQ-06 edits of the move.

## Digest-pinned files that keep the flat path

Sweep: `git grep -nF 'dev/active/<entry>/'` and `git grep -nF '../active/<entry>/'`
for each of the seven moved entries, over the tree outside `.jit/`, the
migration manifest and receipt `inputs/` trees. Each hit outside this record
lies in a file of this table; a digest pins each file.

| File | Hits | Pin |
|---|---|---|
| `dev/studies/6c7fcb38/receipts.md:970,1036,2094` | 3 inline links, targets `../../active/0de41c82/plan.md`, `../../active/0de41c82/investigation.md`, `../../active/0de41c82/plan.md` | SHA-256 at `dev/active/b8206228-permanent-statistics/receipt-inventory.md:44` |
| `dev/studies/91605d4d/receipts.md:908,1449` | 2 inline links, both to `../../active/0de41c82/plan.md` | SHA-256 at `dev/active/b8206228-permanent-statistics/receipt-inventory.md:35` |
| `dev/archive/6dc81018-field-capability-dispatch/active/6dc81018-field-capability-dispatch/investigation.md:524-525` | 2 code spans of 0de41c82 | content hash in the container-archive event at `.jit/events.jsonl:15839` |
| `dev/archive/6dc81018-field-capability-dispatch/active/34d85cb9/findings.md:492` | 1 code span of 150d7d79 | content hash in the container-archive event at `.jit/events.jsonl:15839` |

The five receipt links do not resolve.
`jit doc check-links --scope issue:6c7fcb38` reports 3 `broken_link` and
`--scope issue:91605d4d` reports 2.

## Repointed files outside the moved entries

Each file cites a moved path, and no digest pins it: `git grep -n -F` of its
SHA-256 and of its git blob id over the tree outside `.jit/` returns no line,
its manifest row has `digest_pinned = false`, and inside `.jit/` the digests
occur only as `content_hash` of scanned document assets.

| File | Tool that reads it | Effect of the path edit on the tool |
|---|---|---|
| `dev/studies/0dffa759/findings.md`, `req08-amendment-draft.md` | none | — |
| `dev/studies/a9284086/receipt.md` | `dev/studies/a9284086/analysis.py:1092` | none: it matches stated count phrases |
| `dev/active/ae03bcd0-general-bch/progress.json` | none | — |
| `dev/active/f547c394/research-r3-sweep-resolutions.md`, `research-r3-sweeps.json` | none | — |
| `dev/active/6dc81018-field-capability-dispatch/investigation.md` | none | — |
| `dev/active/ae03bcd0-general-bch/handoff-4.md` to `handoff-7.md`, two `b1bd75ca` session prompts | none | — |
| `dev/active/b8206228-permanent-statistics/sessions/2026-08-09-b8206228-planning-handoff.md` | none | — |
| `proofs/Gf2Core/Proofs/BchSystematicEncoding.lean` (header comment, line 6) | Lean build | none: comment text |

The moved 0de41c82 files `investigation.md`, `plan.md` and `breakdown.json`
cite their own directory by its destination path.
