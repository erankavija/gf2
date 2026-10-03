# Pre-sweep comment census (JIT 62f0d0e6)

## Definitions

- Files: tracked `.rs` files from `git ls-files 'crates/**/*.rs'`, read from the working tree; crate is the second path component.
- Comment line: a line whose first non-whitespace characters are `//` (`//`, `///`, `//!`). Block comments and trailing comments are not counted.
- Non-blank line: a line containing a non-whitespace character.
- Share: comment lines divided by non-blank lines.
- Pattern lines: comment lines matching the sweep pattern, a Python `re` expression applied with `re.IGNORECASE` (`\b` is a word boundary); the expression is the `PATTERN` constant in the script and equals the sweep pattern of issue 62f0d0e6.
- The census reads only files under `crates/`. The measured commit is the first line of the block below and contains the script; the commit that records the output file leaves every file under `crates/` as the measured commit has it. Check from the root of a checkout holding this record (empty output):

  ```sh
  F=dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.txt
  git diff --stat "$(sed -n 's/^commit: //p;q' "$F")" "$(git log -1 --format=%H -- "$F")" -- crates
  ```

## Command

```sh
python3 dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.py \
  dev/active/fa787f85-documentation-overhaul/62f0d0e6-pattern-matches.txt
```

Run from a repository root, the script reads that checkout's working tree, prints its `git rev-parse HEAD` as the first output line, and writes the matching `path:line:text` lines to the file named by its argument. `62f0d0e6-comment-census.txt` and `62f0d0e6-pattern-matches.txt` are its two outputs in a clean checkout of the measured commit. To reproduce them, run this block from the root of a checkout holding this record; it runs the script in a detached scratch worktree of the measured commit and compares both outputs with the committed files of the checkout it starts in:

```sh
D=dev/active/fa787f85-documentation-overhaul
M=$(sed -n 's/^commit: //p;q' "$D/62f0d0e6-comment-census.txt")
T=$(mktemp -d)
git worktree add --quiet --detach "$T/tree" "$M"
(cd "$T/tree" && python3 "$D/62f0d0e6-comment-census.py" "$T/matches.txt" > "$T/census.txt")
diff "$T/census.txt" "$D/62f0d0e6-comment-census.txt"
diff "$T/matches.txt" "$D/62f0d0e6-pattern-matches.txt"
git worktree remove "$T/tree" && rm -r "$T"
```

The block prints nothing: the script at the measured commit regenerates both committed files byte for byte. The script is `62f0d0e6-comment-census.py`.

## Pattern cross-check

`62f0d0e6-pattern-crosscheck.sh` counts, per crate, comment lines matching the sweep pattern with `git grep -P -i` over the committed tree of the commit given as its argument, independent of the Python script. Its output for the measured commit is `62f0d0e6-pattern-crosscheck.txt`; from the root of a checkout holding this record:

```sh
D=dev/active/fa787f85-documentation-overhaul
M=$(sed -n 's/^commit: //p;q' "$D/62f0d0e6-comment-census.txt")
sh "$D/62f0d0e6-pattern-crosscheck.sh" "$M" | diff - "$D/62f0d0e6-pattern-crosscheck.txt"
diff <(awk 'NR>3 && $1!="total" {print $1, $NF}' "$D/62f0d0e6-comment-census.txt") <(tail -n +2 "$D/62f0d0e6-pattern-crosscheck.txt")
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
