# Pre-sweep comment census (JIT 62f0d0e6)

## Definitions

- Files: tracked `.rs` files from `git ls-files 'crates/**/*.rs'`, read from the working tree; crate is the second path component.
- Comment line: a line whose first non-whitespace characters are `//` (`//`, `///`, `//!`). Block comments and trailing comments are not counted.
- Non-blank line: a line containing a non-whitespace character.
- Share: comment lines divided by non-blank lines.
- Pattern lines: comment lines matching the sweep pattern, a Python `re` expression applied with `re.IGNORECASE` (`\b` is a word boundary); the expression is the `PATTERN` constant in the script and equals the sweep pattern of issue 62f0d0e6.
- Rust sources are unchanged between the measured commit and the commit adding this record; the record commit adds only files under `dev/active/`, which the census does not read.

## Command

```sh
python3 dev/active/fa787f85-documentation-overhaul/62f0d0e6-comment-census.py \
  dev/active/fa787f85-documentation-overhaul/62f0d0e6-pattern-matches.txt
```

Run from the repository root at the measured commit. Standard output is `62f0d0e6-comment-census.txt`; the matching `path:line:text` lines are `62f0d0e6-pattern-matches.txt`. The script is `62f0d0e6-comment-census.py`.

## Measurement

The first line of the block names the measured commit.

```text
commit: c42507c026c195d1f3388ce573019d298ffd5cd5
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
