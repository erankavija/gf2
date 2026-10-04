# Archive rerun results

`jit archive container <epic> --execute --json` on main at `04f6fd190`, after
the merges of the re-archive branches and after the tracker script of f29a9225
added four references on 91429c1c. Each row is one run; the fields are those of
the command's JSON result.

| Epic | Issue | Run | publications | reference_changes | deleted_sources | event_appended |
|---|---|---|---|---|---|---|
| babcf05e | 3759f995 | 1 | 0 | 0 | 0 | false |
| f9717e7e | 3759f995 | 1 | 0 | 0 | 0 | false |
| 97bf0879 | 83f9ce69 | 1 | 0 | 0 | 0 | false |
| 026fc832 | f29a9225 | 1 | 4 adopted | 4 | 0 | true |
| 026fc832 | f29a9225 | 2 | 0 | 0 | 0 | false |
| b7157be6 | f902240f | 1 | 1 adopted | 0 | 0 | true |
| b7157be6 | f902240f | 2 | 0 | 0 | 0 | false |

Run 1 of 026fc832 adopts the four BLAS route-B crate files that f29a9225 placed
in the archive (`Cargo.toml`, `build.rs`, `.gitignore`,
`src/bin/bench_blas_gf251.rs`) and records their references. Run 1 of b7157be6
adopts `dev/archive/b7157be6-osd/active/plan.md`, whose link targets f902240f
changed. An adopted publication moves no file. Every run exits 0.

The table establishes that a further execution of each archive publishes,
relinks and deletes nothing. It does not establish link validity; the unit
records hold the link scans.
