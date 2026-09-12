# Batch `cat-file` timing

`dev/scripts/check-receipt-input-snapshots.py`'s `committed_reader` and
`dev/active/a203a23c/restore-receipt-inputs.py`'s `sources_by_digest` read
committed blobs by path. A per-file form spawns one `git cat-file blob`
process per read; a batch form serves every read through one `git cat-file
--batch` process. Commit `76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31` carries
the batch form; its parent carries the per-file form. `measure.py` times both
forms of both call sites directly against this repository's committed
objects, without writing to the repository: it loads each form's exact
historical source with `git show <revision>:<path>` and `exec`s it as an
in-memory module, so the per-file form runs unmodified from `76812a4e`'s
parent and the batch form runs unmodified from `76812a4e` itself, both against
the same workload revision (`76812a4e`).

Two call sites are timed:

- `check()`, the read path `check-receipt-input-snapshots.py`'s `main()`
  invokes — a direct proxy for that tool's wall time.
- `sources_by_digest()`, `restore-receipt-inputs.py`'s dominant cost, since it
  hashes every file the workload revision tracks rather than only the files
  receipts pin. `check() + sources_by_digest()`, summed per repetition index,
  is reported as a restoration proxy; it excludes a second, redundant read of
  the pinned files that `restore-receipt-inputs.py`'s own `main()` performs
  while assembling its pins dictionary (same mechanism and revision as
  `check()`, so already represented) and the small file-copy tail (unchanged
  between the two revisions).

`timing.json` records the invocation, host, toolchain, and five repetitions
per form, with the min and median of each series. It is produced by:

```
./dev/scripts/ccx1-bench-flock.sh --full-host \
  python3 -B dev/active/a203a23c/timing/measure.py --repetitions 5 \
  > dev/active/a203a23c/timing/timing.json
```

`ccx1-bench-flock.sh --full-host` holds this host's benchmark mutex without
pinning to a core subset, keeping other benchmark and build traffic off the
host for the run's duration.
