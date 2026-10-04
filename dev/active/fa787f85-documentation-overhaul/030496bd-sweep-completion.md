# Source-comment sweep completion record (JIT 030496bd)

The definitions of file set, comment line, non-blank line, share and pattern line are those of the [baseline record](62f0d0e6-comment-census.md); the census script and the cross-check script are the baseline's, unchanged. The sweep units are the dependencies of issue 030496bd (`jit issue show 030496bd`). Every command below runs from the root of a checkout holding this record, with:

```sh
D=dev/active/fa787f85-documentation-overhaul
M=$(sed -n 's/^commit: //p;q' "$D/030496bd-comment-census.txt")
units() { jit issue show 030496bd --json | jq -r '.dependencies[].short_id'; }
```

`M` is the measured commit. It contains the census script, and the commit that records the output files leaves every file under `crates/` as the measured commit has it (empty output):

```sh
git diff --stat "$M" "$(git log -1 --format=%H -- "$D/030496bd-comment-census.txt")" -- crates
```

## After census

```sh
python3 "$D/62f0d0e6-comment-census.py" "$D/030496bd-pattern-matches.txt"
```

`030496bd-comment-census.txt` and `030496bd-pattern-matches.txt` are the two outputs of this command in a clean checkout of the measured commit, and `030496bd-pattern-crosscheck.txt` is the output of `62f0d0e6-pattern-crosscheck.sh` for it. This block reruns both scripts in a detached scratch worktree of the measured commit and compares the results with the committed files of the checkout it starts in:

```sh
T=$(mktemp -d)
git worktree add --quiet --detach "$T/tree" "$M"
(cd "$T/tree" && python3 "$D/62f0d0e6-comment-census.py" "$T/matches.txt" > "$T/census.txt")
diff "$T/census.txt" "$D/030496bd-comment-census.txt"
diff "$T/matches.txt" "$D/030496bd-pattern-matches.txt"
git worktree remove "$T/tree" && rm -r "$T"
sh "$D/62f0d0e6-pattern-crosscheck.sh" "$M" | diff - "$D/030496bd-pattern-crosscheck.txt"
diff <(awk 'NR>3 && $1!="total" {print $1, $NF}' "$D/030496bd-comment-census.txt") <(tail -n +2 "$D/030496bd-pattern-crosscheck.txt")
```

The block prints nothing: both outputs regenerate byte for byte, the cross-check output reproduces, and its per-crate counts equal the pattern column of the census.

## Comment share before and after

`030496bd-census-compare.py` joins the baseline census and the after census; `030496bd-census-compare.txt` is its output.

```text
baseline: e3bfdfae782161c1f5031daac98f77ef48db2281
after:    24bdf9068e09144c82b82009cc4bd1c80f0c1e4d
                                   comment         non-blank             share           pattern
crate                       before   after    before   after    before   after    before   after
crates/gf2-algebra            7788    2015     22990   17205    33.88%  11.71%         9       0
crates/gf2-coding            26946   10944     94570   78578    28.49%  13.93%        74       0
crates/gf2-core              37360   12728    135122  110467    27.65%  11.52%       279       0
crates/gf2-kernels-hip        4799    2056     10239    7496    46.87%  27.43%        33       0
crates/gf2-kernels-simd       7984    3992     23772   19780    33.59%  20.18%        48       0
crates/gf2-sim               18038    7702     70819   60474    25.47%  12.74%       197       5
crates/gf2-stats               623     541      3410    3328    18.27%  16.26%         0       0
total                       103538   39978    360922  297328    28.69%  13.45%       640       5
```

The block equals the committed output and the output regenerates from the two census files; both commands print nothing:

```sh
block() { awk -v n="$1" '/^```text$/{k++;f=(k==n);next} /^```$/{f=0} f' "$D/030496bd-sweep-completion.md"; }
block 1 | diff - "$D/030496bd-census-compare.txt"
python3 "$D/030496bd-census-compare.py" "$D/62f0d0e6-comment-census.txt" "$D/030496bd-comment-census.txt" | diff - "$D/030496bd-census-compare.txt"
```

## Justified pattern lines

Each line of `030496bd-pattern-matches.txt` is listed by `path:line`. The file they are in is the one path that `cut -d: -f1 "$D/030496bd-pattern-matches.txt" | sort -u` prints; there a wave is one iteration of Kahn's topological scheduling: the set of stages whose producers have all run. The code names it (`wave`, `WaveInput`, `WaveResult`).

| Line | Justification |
|---|---|
| `crates/gf2-sim/src/executor/topology.rs:59` | Rustdoc of `WaveResult`: "wave member" is a stage of the current Kahn wave. |
| `crates/gf2-sim/src/executor/topology.rs:711` | Rustdoc of `WaveInput`: "per-wave input" is the input a stage receives in its Kahn wave. |
| `crates/gf2-sim/src/executor/topology.rs:802` | Complexity contract of `TopologyExecutor::run`: stages of one Kahn wave run in parallel. |
| `crates/gf2-sim/src/executor/topology.rs:816` | Execution contract of `TopologyExecutor::run_with_handle`: the sentence that defines Kahn waves and their parallel execution. |
| `crates/gf2-sim/src/executor/topology.rs:884` | Comment on the loop over the local `wave` vector: inputs are prepared outside the parallel region. |

The table's set of lines equals the file's set (empty output):

```sh
diff <(grep -o '^| `crates/[^`]*`' "$D/030496bd-sweep-completion.md" | tr -d '|` ') <(cut -d: -f1,2 "$D/030496bd-pattern-matches.txt")
```

## Tracked issues for removed statements

`030496bd-planned-work.py` reads the `planned work:` field of every commit up to the measured commit whose subject scope is a sweep unit; `030496bd-planned-work.txt` is its output. Section 1 gives unit, commit, removed statement and issue id for each entry; section 3, embedded here, gives each issue once with the units that name it.

```text
# 3. jit issue show <id> | units
02b8137c | 72768e37
02ed3c51 | f46046f0
0d9cb8e3 | 72768e37
0ecf6fcd | 93aed46a
194b902a | c6aa0e84
1e272575 | 13dfe1a5
23d3525f | 72768e37
252814d8 | 8f451167
2a9cc72c | ee2b0c0c
2c109c8d | 57436474
2cfc4372 | eba21481
3931ac6f | aa0558c1
39cbde20 | 13dfe1a5 ee2b0c0c
3fcb7025 | 72768e37
42eac5cc | 72768e37
47698404 | eba21481
4b5d8948 | 9f47bb94
51334873 | 13dfe1a5
52112411 | 13dfe1a5
54a483a3 | 7bfad039
63bad95d | 290b8716
64c88ae4 | eba21481
662f7a15 | eba21481
6639435f | 72768e37
6ed7f050 | eba21481
6fb4abad | 67b5d1d8 eba21481
726bfeee | c6aa0e84
75c22fa8 | 72768e37
7a3a6738 | aa0558c1
7c8ab1e2 | 9f47bb94
81d05bab | 72768e37
90a88fa9 | f22ec590
96fde7c7 | 13dfe1a5
97410c80 | 13dfe1a5
9c37ec8c | 13dfe1a5
9e12659b | eba21481
a03b2556 | eba21481
a9ab0a4f | eba21481
ad597ede | f2064e90
ae03bcd0 | aa0558c1
b05c908b | f46046f0
b53a6036 | 9dda3958
b87362a3 | eba21481
b9a352b4 | aa0558c1
babcf05e | eba21481
bb64ee0e | 9f47bb94
c04d3f1c | f46046f0
c09d3e95 | 72768e37
c0bb2ab1 | 4997a3ed
c11640f2 | c6aa0e84
c1b253cb | f22ec590
c3f8c1cb | f2064e90
c5cee991 | 13dfe1a5
c5e01de3 | 50201002
c7c0e991 | 290b8716 9f47bb94 eba21481
ca9a5194 | c6aa0e84
cdcebf6a | f2064e90
cfc2e9a9 | 3da13f39
d1e5db10 | 4997a3ed
d36ae697 | 13dfe1a5
d48a3cfd | f2064e90
de160fc5 | 72768e37
e046803d | eba21481
e1ff915f | aa0558c1
e4849f07 | 72768e37
ebd26779 | 9f47bb94
f6603b37 | 7bfad039 93aed46a
f6ee400a | 290b8716
f8d230ef | 9f47bb94
```

These commands print nothing: the block equals section 3 of the output, the output regenerates, every listed id resolves in the tracker, and every issue that `progress.json` records with the reason "planned work removed by" a unit is listed.

```sh
ids() { awk '/^# 3/{f=1;next} f{print $1}' "$D/030496bd-planned-work.txt"; }
block 2 | diff - <(sed -n '/^# 3/,$p' "$D/030496bd-planned-work.txt")
units | python3 "$D/030496bd-planned-work.py" "$M" | diff - "$D/030496bd-planned-work.txt"
ids | while read -r i; do jit issue show "$i" > /dev/null 2>&1 || echo "$i"; done
jq -r '.created_during_execution[] | select(.reason | startswith("planned work removed by")) | .id' "$D/progress.json" | sort | comm -23 - <(ids)
```

Section 2 of the output lists the `planned work:` entries that name no issue. Four of them record a removed statement without a match:

```sh
awk '/^# 2/{f=1;next} /^# 3/{f=0} f' "$D/030496bd-planned-work.txt" | grep -E 'unmatched|none matched'
```

Units 57436474, 8f451167 and 9dda3958 (each `jit issue show <id>`) name the issue in another commit (rows of section 1). Unit 66858db0 (`jit issue show 66858db0`) has no row in section 1: its commit `0376b472a` records "none matched in the tracker" and its commit `5ab246d0d` records "none". The unit's commit `52b4a73d4` removes the sentence "Divide-and-conquer batch GCD is a future algorithmic upgrade." from the Rustdoc of `FieldPoly::batch_gcd` in `crates/gf2-core/src/field/poly.rs`; the list above holds no issue for it.

```sh
for u in 57436474 8f451167 9dda3958 66858db0; do echo "$u $(sed '/^# 2/,$d' "$D/030496bd-planned-work.txt" | grep -c "^$u ")"; done
git log --format='%h %s' -S'Divide-and-conquer batch GCD is a future' "$M" -- crates/gf2-core/src/field/poly.rs
```

## Non-comment hunks of the sweep

`030496bd-token-diff.py` compares, for every file changed by a commit whose subject scope is a sweep unit, the Rust token sequences of the commit and of its first parent after comments and whitespace are dropped; `030496bd-token-diff.txt` is its output for the measured commit and regenerates with (empty output):

```sh
units | python3 "$D/030496bd-token-diff.py" "$M" | diff - "$D/030496bd-token-diff.txt"
```

The output holds the removed and added tokens of each hunk. Each file it reports is listed here with the commit body or owner decision that names the hunk; decisions DEC-n are in the Decisions section of the named issue.

| Commit | File | Hunk | Named by |
|---|---|---|---|
| `445b6169c` | `crates/gf2-coding/tests/ldpc_validation.rs` | nested `if` collapsed, forced by `clippy::collapsible_if` | body of `445b6169c`; DEC-02 of 13dfe1a5 (`jit issue show 13dfe1a5`) |
| `938ff374c` | `crates/gf2-core/tests/sparse.rs` | two trailing commas dropped by `rustfmt` | body of `938ff374c` |
| `938ff374c` | `crates/gf2-core/tests/sparse_dual.rs` | trailing comma dropped by `rustfmt` | body of `938ff374c` |
| `105fff2f0` | `crates/gf2-sim/tests/campaign_byte_identity.rs` | trailing comma dropped by `rustfmt` | body of `29b22cf7d` |
| `680938c07` | `crates/gf2-coding/examples/llr_operations.rs` | trailing comma dropped by `rustfmt` | body of `680938c07` |
| `f525ad59e` | `crates/gf2-algebra/src/packed/bipedal3.rs` | three trailing commas dropped by `rustfmt` | body of `f525ad59e`; DEC-01 of 3aef9e33 (`jit issue show 3aef9e33`) |
| `fe3a4fb88` | `crates/gf2-kernels-hip/build.rs` | string literal written into the generated probe source | body of `fe3a4fb88`; DEC-01 of 93aed46a (`jit issue show 93aed46a`) |
| `42dc2f197` | `crates/gf2-kernels-simd/src/x86/asm/_lto_opacity_callsites.asm.txt` | annotation text, not Rust source | not named in a commit body |
| `42dc2f197` | `crates/gf2-kernels-simd/src/x86/asm/bipedal_avx512.asm.txt` | annotation text, not Rust source | body of `42dc2f197` |
| `42dc2f197` | `crates/gf2-kernels-simd/src/x86/asm/transpose.asm.txt` | annotation text, not Rust source | body of `42dc2f197` |

The table lists the files of the output in the output's order, and every changed line of the three `.asm.txt` files is blank or a `;` or `#` comment line; both commands print nothing:

```sh
diff <(grep -o '^| `[0-9a-f]\{9\}` | `[^`]*`' "$D/030496bd-sweep-completion.md" | tr -d '|`' | awk '{print $1, $2}') <(grep -E '^[0-9a-f]{8} \|' "$D/030496bd-token-diff.txt" | awk -F' [|] ' '{print $2, $3}')
git show --format= 42dc2f197 -- crates/gf2-kernels-simd/src/x86/asm | grep -E '^[-+]' | grep -vE '^(\+\+\+|---)' | grep -vE '^[-+][[:space:]]*(;|#|$)'
```

The combined diff of every merge commit among the sweep commits is empty, so no merge adds a hunk of its own (empty output):

```sh
git log --merges --format='%h %s' "$M" | grep -E "jit:($(units | paste -sd'|'))\)" | while read -r c _; do git show --format= --name-only -c "$c"; done
```
