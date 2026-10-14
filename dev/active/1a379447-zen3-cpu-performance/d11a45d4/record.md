# Survey tool repairs: addendum identity and failing-build evidence

## Which bytes the logical receipts pin

Every committed receipt of the logical family pins
`logical-buffer-addendum.md` at SHA-256 `290685bf…7074`; its byte copy is the
snapshot under each campaign's `inputs/producing/`. The live addendum holds
`817f1028…ea35`: those bytes with Amendments 2 and 3 (invoker rulings of
2026-09-27 on `@/issue/65c0e13d`), the wording edits Amendment 2 makes in the
body, and two links repointed to the epic directory. Neither the live addendum
nor the harness pin holds the receipt-pinned bytes.

The harness pin `ADDENDUM_SHA256` in `survey/harness/src/cells.rs` holds the
live addendum's digest (invoker ruling, 2026-10-14). Receipts and their
`inputs/` snapshots keep their bytes.

## A failing build leaves the committed records unchanged

`run-logical-harness.sh build` writes its validation record, and with `--isal`
the ISA-L build record and verbose log, into a stage directory and moves them
to their committed paths only after every step succeeds.

Run on 2026-10-15 from the worktree root of this issue: one newline appended
to the live addendum, then
`dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh build --isal`.
The gf2 arms, the harness contract (25 tests) and the ISA-L arm built; the
campaign tool then stopped with exit status 2:

```text
logical-campaign: …/logical-buffer-addendum.md sha256 6433aa73ec5adb79476441f8ee2108f32697a9a4ef111908ebd41298dfe406d8 differs from the pinned 817f10287b4c89f03504c1a6a0b3e26f7e58585f05aa51ca3d4994daf50fea35
```

`git status --porcelain` afterwards listed only the edited addendum; the build
record, build log and validation record were unchanged.
