# Terminal state of the remaining manifest rows (a0ec34f2)

Record of `@/issue/a0ec34f2`. Commit checked: `ee5494344d5c62075622a7f60f45d161328b4752`.
Base: `338275ff25cf19b275e844894401edef4257f007`, the constant `BASE` of the
script. Tracker state is read on main at `3f6521789`.

| File | Content |
|---|---|
| [a0ec34f2-rows.py](a0ec34f2-rows.py) | Decision and report script; its docstring states the rule per disposition and the exit conditions |
| [a0ec34f2-archive-rows.tsv](a0ec34f2-archive-rows.tsv) | Per archive row: decision, ground and the `file:line` of each holder |
| [a0ec34f2-rows.txt](a0ec34f2-rows.txt) | Output of the script for the commit checked |

## Grounds of the archive rows

The `ground` column of the table takes these values.

| Ground | Meaning | Tracker state |
|---|---|---|
| `unheld` | No tracker reference, digest citation, link or code consumer names the file. It moves by `git mv` under `relocation-protocol` and keeps its blob. | No issue document names the path. |
| `preregistration` | A preregistration states that the file stays at its committed path. | The holder at line 10 of the v5 preregistration is document 0 of `@/issue/ec22205e`, pinned to commit `ac550959`; the issue is done under the open epic `@/issue/b8206228`. |
| `receipt-link` | Receipts whose manifest rows are digest-pinned link the file by relative path. | Documents of `@/issue/047b62ed`, `@/issue/0dffa759`, `@/issue/6c7fcb38` and `@/issue/91605d4d` capture the file as an asset with its SHA-256. |
| `archive-link` | Files of the marker-backed archive of `@/issue/ae82bd73` link the file by relative path as the live provenance record. | `jit archive container ae82bd73 --json` gives the source action `block` with blocker `destination-conflict`: the archive holds another file at the plan destination. |
| `container-plan` | The archive plan of `@/issue/ae82bd73` moves the file; execution belongs to the lead on main. | The same preview gives action `move` with a done, unpinned owner (`@/issue/b293af5a`, `@/issue/a9e461de`) and `eligible` false: blockers `destination-conflict` (the `archive-link` row) and `repository-escape` (`dev/plans/a9e461de/s5_gpu_crossover.md`). |

A `retained` row carries disposition `retained-operational`, an empty
destination and status `complete`, the encoding of
[83f9ce69-rearchive.md](83f9ce69-rearchive.md); the table names its holder. A
`container-plan` row keeps its base fields.

## Criteria

| Criterion | Evidence in `a0ec34f2-rows.txt` | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | `[decisions]`, lines `retained-operational: in place` and `in place, destination cleared`; list `in place, destination cleared` | Each `retained-operational` row that has status `pending` at base names a path that exists at the commit checked and has status `complete` with an empty destination. The script stops on an absent path, so no such row names a moved or removed path. | Why no commit moves a listed file to its base destination. |
| REQ-02 | lines `rewritten <destination>` | Each `rewritten-topic` destination exists and carries a commit of a breakdown issue; the rewrite tasks are the keys `entry-page-*`, `backend-crate-readmes` and `rewrite-dev-index`. | The content of the pages. |
| REQ-03 | lines `moved`, `retained` and `held` with their holder lines | The moved file has the blob of its base path. Each holder line exists and reads as printed; `[digest-pinned row]` marks a holder whose own row is digest-pinned. | Tracker state: the script reads git objects only; the table above states it. |
| REQ-04 | `[pending at checked]`; list `left pending` | The rows with status `pending` at the commit checked, per disposition and by path. | — |
| REQ-05 | `[row changes base -> checked, by base disposition]` | Per base disposition the changed rows and the count per field; no artifact row is added or removed, no other field differs, and each manifest line equals its base line outside the three fields. | Rows of the `policy` table beyond their `pending` count. |

`python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` exits 0
at the commit checked.

## Citations of the moved receipt

`perf-evidence-catalog.md` names the mirror path. The base path stays in:

| File:line | Ground |
|---|---|
| `dev/archive/806eb14e-hip-gpu-prototype/active/806eb14e-feasibility-report.md:56,57,93`, `dev/archive/806eb14e-hip-gpu-prototype/active/9012f8a0/gpu-batch-bch-syndrome-plan.md:276,320` | marker-backed archive; path text, not links |
| `migration/manifest.toml`, `a0ec34f2-archive-rows.tsv`, `a0ec34f2-rows.txt` | `path` field of the row and its decision |

The moved file holds no link.

## Commands

```
python3 dev/active/fa787f85-documentation-overhaul/a0ec34f2-rows.py ee5494344d5c62075622a7f60f45d161328b4752 | diff - dev/active/fa787f85-documentation-overhaul/a0ec34f2-rows.txt
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py
python3 contrib/gates/docs-mechanical.py
```

The first command prints nothing from the repository root and from `/tmp`
(with absolute paths). `docs-mechanical` resolves each local link of the files
this unit touches.
