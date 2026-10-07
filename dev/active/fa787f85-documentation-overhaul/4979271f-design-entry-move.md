# 4979271f channel-gain design entry move record

Entry of @/issue/0b0714e0, epic @/issue/cffc15dd. Commit checked: `cfb0498d1`
(`git show --stat cfb0498d1`).

## Commands

Run from the repository root.

```sh
jit doc dir cffc15dd dev/active
git ls-files dev/active/cffc15dd-channel-models/0b0714e0
git ls-files dev/active | grep 0b0714e0
jit doc list 0b0714e0
git grep -n 0b0714e0 -- . ':!.jit'
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py
python3 contrib/gates/docs-mechanical.py
```

## Criteria

| Criterion | Command result | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | `jit doc dir` prints `dev/active/cffc15dd-channel-models`. The first `git ls-files` lists the design file; the second lists the same file and no other. `jit doc list` names that file. | The entry's one tracked file lives under the canonical directory and the tracker reference names it | Untracked files |
| REQ-02 | Each `git grep` hit is in the table below. `docs-mechanical.py` scans the moved design and this record and reports no finding for them. | No citation outside the grounds below names another path; the two Markdown files this unit touches hold no unresolved local link | Links of files this unit leaves untouched. The manifest is TOML: its row holds path fields and no link, and `check.py` asserts them. |
| REQ-03 | `check.py` exits 0; the row of the entry carries `status = "complete"` and the design file's path as `destination` | The destination exists and the source is absent | The state of other rows |

## Citations outside `.jit/`

| File | Decision | Ground |
|---|---|---|
| `a0ec34f2-rows.txt` | keeps the path | tool output of `a0ec34f2-rows.py` at the commit its record names |
| `migration/manifest.toml` | `path` of the row keeps the source; `destination` names the design file | manifest schema fields |
| `breakdown.json` | unchanged | names the entry by issue id in a frozen entry list; no path |
| `plan-precheck-gate2.md` | unchanged | reports an untracked file observed at the snapshot of that review; the sentence names a path that differs from the tracked one and is dated by its context |
| the design file | unchanged | cites the issue by id only |
| this record | names the destination | — |
