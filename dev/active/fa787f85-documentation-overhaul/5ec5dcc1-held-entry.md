# Held entry dev/active/02b8137c (5ec5dcc1)

Issue 5ec5dcc1 moves the flat entry `dev/active/02b8137c/` only when issue
02b8137c is done or rejected (DEC-01 of 5ec5dcc1). The entry stays in place;
no file of it is moved or edited.

| Fact | Value | Check |
|---|---|---|
| State of 02b8137c | `in_progress` | `jit issue show 02b8137c --json` |
| Unmerged commit on the entry | `fa8a9d9cd` (`chore(jit:02b8137c): pin continuation runbook`) adds `run-validation-v2-continuation.sh` on branch `worktree-agent-02b8137c-run` | `git cherry main worktree-agent-02b8137c-run` |
| Manifest rows under `dev/active/02b8137c/` | each `retained-operational`, empty destination, `complete` | parse `migration/manifest.toml` with Python, filter on `path` |
| Tracking issue of the move | the move is tracked by 5b3e7c81 (`jit issue show 5b3e7c81`), the retaining reference of those rows | `jit issue show 5b3e7c81` |

`python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` reports
no finding on those rows.
