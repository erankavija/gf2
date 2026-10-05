# Dense-parity source-evidence ledger verification (jit:6833c5b5)

> **Diátaxis Type:** Reference

The dense-parity
[source-evidence ledger](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-parity-source-evidence.json)
is a record pinned to the commits its rows record. Each row names a commit, a
path, a line, the verbatim line and the digest of the whole file, and each
citation resolves at that commit: the file there has the recorded digest and
carries the cited text at the cited line. The ledger is a producing input the
dense-parity receipts pin by digest, so its bytes stay as committed while the
cited sources change in later trees.

## Check

Every command runs from the repository root.

```sh
python3 -B dev/scripts/verify-source-evidence.py \
    dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-parity-source-evidence.json \
    --project gf2 --frame recorded-commits
```

The command exits zero when every row holds at its recorded commit. Its
`class:` line distinguishes the two kinds of ledger: `recorded-commits` for a
ledger whose rows all hold at their recorded commits while some differ from the
compared tree, and `tree` for a ledger whose rows all hold in the compared
tree. `--frame tree` is the check for a ledger that tracks a tree and refuses
this one. The verifier's header states the row contract;
`bash dev/scripts/verify-source-evidence.test.sh` exercises both frames, both
classes and each refusal on this ledger.

## Record

```sh
python3 -B dev/scripts/verify-source-evidence.py \
    dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-parity-source-evidence.json \
    --project gf2 --frame recorded-commits \
    --tree fcb972da393c878665b1653856d76ddf5b299731 \
    --record dev/active/1a379447-zen3-cpu-performance/6833c5b5/dense-parity-source-evidence-verification.json
```

writes the
[verification record](dense-parity-source-evidence-verification.json). It pins
the ledger by path and digest and names the compared revision. Its
`holding_at_recorded_commits` and `holding_in_compared_tree` fields count the
rows that hold in each frame, and `differing_from_compared_tree` lists each row
that differs from the compared revision with the file digest and the lines of
its text there, the first commit after the recorded one that changes the file
and the first whose content displaces the cited line.

## Limits

[`make-dense-parity-source-evidence.py`](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/make-dense-parity-source-evidence.py)
reads each cited file at the commit its ledger row records and reproduces the
committed ledger byte for byte. It refuses to write a ledger that differs, and
`--check` compares without writing. The recorded-commits check above verifies
the ledger independently of the generator.

The documents that cite the ledger and keep their bytes carry their tree
statement in the
[dense-parity corrections](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-corrections.md).
