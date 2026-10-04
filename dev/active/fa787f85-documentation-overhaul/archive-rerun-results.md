# Archive rerun results

`jit archive container <epic> --execute --json`, run on main at the commit of
each row after the unit's branch is merged and its tracker script has run.
Each row is one run; the fields are those of the command's JSON result. Every
run exits 0.

| Epic | Issue | Main | Run | publications | reference_changes | deleted_sources | event_appended |
|---|---|---|---|---|---|---|---|
| babcf05e | 3759f995 | `04f6fd190` | 1 | 0 | 0 | 0 | false |
| f9717e7e | 3759f995 | `04f6fd190` | 1 | 0 | 0 | 0 | false |
| 97bf0879 | 83f9ce69 | `04f6fd190` | 1 | 0 | 0 | 0 | false |
| 026fc832 | f29a9225 | `04f6fd190` | 1 | 4 adopted | 4 | 0 | true |
| 026fc832 | f29a9225 | `04f6fd190` | 2 | 0 | 0 | 0 | false |
| 026fc832 | f29a9225 | `e15dbdaef` | 3 | 1 adopted | 0 | 0 | true |
| 026fc832 | f29a9225 | `e15dbdaef` | 4 | 0 | 0 | 0 | false |
| b7157be6 | f902240f | `04f6fd190` | 1 | 1 adopted | 0 | 0 | true |
| b7157be6 | f902240f | `04f6fd190` | 2 | 0 | 0 | 0 | false |
| bb85c68a | 42037c91 | `e15dbdaef` | 1 | 2 adopted | 0 | 0 | true |
| bb85c68a | 42037c91 | `e15dbdaef` | 2 | 0 | 0 | 0 | false |
| 6efb756b | 42037c91 | `e15dbdaef` | 1 | 5 adopted | 0 | 0 | true |
| 6efb756b | 42037c91 | `e15dbdaef` | 2 | 0 | 0 | 0 | false |
| e095a100 | 1ca94ec2 | `e15dbdaef` | 1 | 1 adopted | 1 | 0 | true |
| e095a100 | 1ca94ec2 | `e15dbdaef` | 2 | 0 | 0 | 0 | false |
| 806eb14e | 1ca94ec2 | `e15dbdaef` | 1 | 0 | 0 | 0 | false |
| 2928ccce | 1ca94ec2 | `e15dbdaef` | 1 | 0 | 0 | 0 | false |
| d4851c3d | 1ca94ec2 | `e15dbdaef` | 1 | 5 adopted | 0 | 0 | true |
| d4851c3d | 1ca94ec2 | `e15dbdaef` | 2 | 0 | 0 | 0 | false |
| 6dc81018 | 616e1d7c | `e15dbdaef` | 1 | 122 adopted | 0 | 0 | true |
| 6dc81018 | 616e1d7c | `e15dbdaef` | 2 | 0 | 0 | 0 | false |

An adopted publication records the identity of a file that already sits at its
archive destination; it moves no file. The adopting runs follow edits the units
made inside the archives (link targets, self-path literals in moved scripts,
the BLAS route-B crate files and its `lib.rs`) and deck assets the tracker
scans. The last run of each epic publishes, relinks and deletes nothing.

The table does not establish link validity; the unit records hold the link
scans.
