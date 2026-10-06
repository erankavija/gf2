# Baseline survey entry: root resolution and move (b23c364a)

`E` is the directory of the entry of issue 4e732b56 and `S` is
`E/baseline-survey`.

## Captures (REQ-02)

Both commands run from the repository root.

| Capture | Command | Output |
| --- | --- | --- |
| Generator check | `python3 -B S/verify-generators.py dev/bench_results/4e732b56/generators.txt` | [`b23c364a-verify-generators.txt`](b23c364a-verify-generators.txt) |
| Makefile variable `EXT` | `realpath -m "$(make --no-print-directory -C S --eval='b23c364a-ext: ; @echo $(EXT)' b23c364a-ext)" \| sed "s#$(git rev-parse --show-toplevel)#<root>#"` | [`b23c364a-ext.txt`](b23c364a-ext.txt) |

The `--eval` goal echoes the variable and runs no target of the Makefile.
`realpath -m` reduces `..` segments without reading the file system, and `sed`
replaces the checkout root, so the capture holds the location `EXT` names and
not the spelling of the assignment.

| State | Commit | `E` | `diff` of each capture against its committed file |
| --- | --- | --- | --- |
| Baseline | `4dfbab9ce` | `dev/active/4e732b56` | the committed files are these outputs |
