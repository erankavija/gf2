# Archive preview results

Previews run 2026-10-01 with `jit archive container <id> --json` over every effectively terminal, unarchived container (72). Raw JSON is not committed; the commands reproduce it.

## Terminal stories under archived epics (re-archive the epic to sweep them in)

| Archived epic | Terminal stories | Stories with artifacts | Artifacts |
|---|---|---|---|
| 026fc832 | 3 | 3 | 65 |
| 2928ccce | 3 | 1 | 4 |
| 6efb756b | 13 | 5 | 11 |
| 806eb14e | 5 | 5 | 5 |
| 97bf0879 | 6 | 6 | 68 |
| babcf05e | 7 | 6 | 22 |
| bb85c68a | 9 | 4 | 15 |
| d4851c3d | 8 | 2 | 3 |
| e095a100 | 8 | 4 | 9 |
| f9717e7e | 3 | 3 | 15 |

A re-run of `jit archive container 6efb756b` previewed eligible into the existing marker-backed `dev/archive/6efb756b-grand` with 15 artifacts, so re-archiving an archived epic sweeps its terminal stories' documents into the epic archive.

## Terminal stories under open epics (stay until the epic is terminal, D-22)

| Story | Artifacts | Open epic | State |
|---|---|---|---|
| 055bda14 | 0 | 7f809931 | backlog |
| 0de41c82 | 95 | b8206228 | backlog |
| 5d0a3fad | 1 | d77176e5 | ready |
| 9effa2e2 | 0 | 7f809931 | backlog |
| c04dd4ac | 170 | 1a379447 | backlog |
| c3ea6855 | 0 | 0fc3c9d0 | backlog |

## Blocked

| Container | Artifacts | Blocker |
|---|---|---|
| b7157be6 (epic, done) | 31 | destination-conflict: `dev/archive/b7157be6-osd` exists without an archive marker (hand archive) |
