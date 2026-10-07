# 4979271f channel-gain design entry move record

Entry of @/issue/0b0714e0, epic @/issue/cffc15dd. Commit checked: `cfb0498d1`
(`git show --stat cfb0498d1`). `E` is
`dev/active/cffc15dd-channel-models/0b0714e0`.

## Criteria

| Criterion | Command | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | `jit doc dir cffc15dd dev/active` prints the parent of `E`. `git ls-files E` lists the design file. `git ls-files dev/active` lists no file of the entry outside `E`. `jit doc list 0b0714e0` names `E/0b0714e0-channel-gain-design.md` | The entry's one tracked file lives under the canonical directory and the tracker reference names it | Untracked files |
| REQ-02 | `git grep -n 0b0714e0 -- . ':!.jit'`; link scan below | Every remaining hit is a keeper (table below) | A scan of files outside the touched set |
| REQ-03 | `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` exits 0; the row of the entry carries `status = "complete"` and the destination `E/0b0714e0-channel-gain-design.md` | The destination exists and the source is absent | The state of other rows |
| REQ-04 | this record | — | — |

Link scan of the touched files (the moved design, the manifest, this record):
`python3 contrib/gates/docs-mechanical.py` reports no unresolved local link for
them.

## Citations outside `.jit/`

| File | Decision | Ground |
|---|---|---|
| `a0ec34f2-rows.txt` | keeps the path | tool output of `a0ec34f2-rows.py` at the commit its record names |
| `migration/manifest.toml` | `path` of the row keeps the source; `destination` names `E` | manifest schema fields |
| `breakdown.json` | unchanged | names the entry by issue id in a frozen entry list; no path |
| `plan-precheck-gate2.md` | unchanged | reports an untracked file observed at the snapshot of that review; the sentence names a path that differs from the tracked one and is dated by its context |
| the design file | unchanged | cites the issue by id only |
