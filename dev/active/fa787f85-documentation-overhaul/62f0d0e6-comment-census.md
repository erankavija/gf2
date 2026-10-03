# Pre-sweep comment census (JIT 62f0d0e6)

## Definitions

- Files: tracked `.rs` files from `git ls-files 'crates/**/*.rs'`, read from the working tree; crate is the second path component.
- Comment line: a line whose first non-whitespace characters are `//` (`//`, `///`, `//!`). Block comments and trailing comments are not counted.
- Non-blank line: a line containing a non-whitespace character.
- Share: comment lines divided by non-blank lines.
- Pattern lines: comment lines matching the sweep pattern, a Python `re` expression applied with `re.IGNORECASE` (`\b` is a word boundary); the expression is the `PATTERN` constant in the script and equals the sweep pattern of issue 62f0d0e6.
- The record commit adds only files under `dev/active/`, which the census does not read; the measured commit is the first line of the block below and contains the script. Check from the repository root at the record commit (empty output):

  ```sh
  git diff --stat "$(sed -n 's/^commit: //p;q' dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.txt)" HEAD -- crates
  ```

## Command

```sh
python3 dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.py \
  dev/active/fa787f85-documentation-overhaul/62f0d0e6-pattern-matches.txt
```

Run from the repository root at the measured commit. The script prints `git rev-parse HEAD` as the first output line, so the output there equals `62f0d0e6-comment-census.txt` byte for byte, and the matching `path:line:text` lines equal `62f0d0e6-pattern-matches.txt`. To reproduce, check out the measured commit in a clean working tree, run the command with the matches file written to a scratch path, and compare:

```sh
python3 dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.py /tmp/matches.txt > /tmp/census.txt
diff /tmp/census.txt dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.txt
diff /tmp/matches.txt dev/active/fa787f85-documentation-overhaul/62f0d0e6-pattern-matches.txt
```

Both `diff` commands print nothing. The script is `62f0d0e6-comment-census.py`.

## Pattern cross-check

`62f0d0e6-pattern-crosscheck.sh` counts, per crate, comment lines matching the sweep pattern with `git grep -P -i` over the committed tree of the commit given as its argument, independent of the Python script. Its output is `62f0d0e6-pattern-crosscheck.txt`; from the repository root at the record commit:

```sh
cd dev/active/fa787f85-documentation-overhaul
M=$(sed -n 's/^commit: //p;q' 62f0d0e6-comment-census.txt)
sh 62f0d0e6-pattern-crosscheck.sh "$M" | diff - 62f0d0e6-pattern-crosscheck.txt
diff <(awk 'NR>3 && $1!="total" {print $1, $NF}' 62f0d0e6-comment-census.txt) <(tail -n +2 62f0d0e6-pattern-crosscheck.txt)
```

Both `diff` commands print nothing: the cross-check output reproduces, and its per-crate counts equal the pattern column of the census.


## Measurement

The first line of the block names the measured commit.

```text
commit: e3bfdfae782161c1f5031daac98f77ef48db2281
files: 706
crate                       comment  non-blank   share  pattern
crates/gf2-algebra             7788      22990  33.88%        9
crates/gf2-coding             26946      94570  28.49%       74
crates/gf2-core               37360     135122  27.65%      279
crates/gf2-kernels-hip         4799      10239  46.87%       33
crates/gf2-kernels-simd        7984      23772  33.59%       48
crates/gf2-sim                18038      70819  25.47%      197
crates/gf2-stats                623       3410  18.27%        0
total                        103538     360922  28.69%      640
```

The block equals the committed output file; this command prints nothing when it holds:

```sh
cd dev/active/fa787f85-documentation-overhaul && \
  diff <(awk '/^```text$/{f=1;next} /^```$/{f=0} f' 62f0d0e6-comment-census.md) 62f0d0e6-comment-census.txt
```
