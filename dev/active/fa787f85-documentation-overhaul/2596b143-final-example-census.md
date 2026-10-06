# Final Rustdoc example census and doctest timing (JIT 2596b143)

The baseline is the [pre-removal audit](fa787f85-rustdoc-example-audit.md); the census method is its script, [`fa787f85-example-census.py`](fa787f85-example-census.py), unchanged. The intermediate records are [`a0a29512-example-census-after.md`](a0a29512-example-census-after.md) and [`153297cf-example-decisions.md`](153297cf-example-decisions.md). Every command below runs from the root of a checkout holding this record, with:

```sh
D=dev/active/fa787f85-documentation-overhaul
B=ca0d918a6c92c467fb46171d439763183647df4c
M=$(sed -n 's/^commit: //p;q' "$D/2596b143-doctest.txt")
V=$D/fa787f85-rustdoc-example-verdicts.tsv
```

`M` is the measured commit. `B` is the commit that adds the baseline record and `V` (`git log --diff-filter=A --format=%H -- "$V"`). The census script reads the working tree, so its reproduction needs `git diff --quiet "$M" -- crates` to succeed; the comparison script reads both commits through `git show`.

## Committed outputs

| File | Content | Reproduction (empty output) |
|---|---|---|
| [`2596b143-census.txt`](2596b143-census.txt) | Census script output at `M` | `diff <(python3 -B $D/fa787f85-example-census.py . \| sed '/^densest files:/q') <(sed '/^densest files:/q' $D/2596b143-census.txt)` |
| [`2596b143-changed-examples.tsv`](2596b143-changed-examples.tsv) | Every example section at `M` with its status against `B` | `diff <(python3 -B $D/2596b143-changed-examples.py $B $M $V) $D/2596b143-changed-examples.tsv` |
| [`2596b143-baseline-obvious.tsv`](2596b143-baseline-obvious.tsv) | Baseline `OBVIOUS` rows that pair with a section at `M`, and the counts | `diff <(python3 -B $D/2596b143-changed-examples.py $B $M $V --obvious) $D/2596b143-baseline-obvious.tsv` |
| [`2596b143-example-verdicts.tsv`](2596b143-example-verdicts.tsv) | Verdict and reason for each `added` or `changed` section, in the columns of `V` | Classification by reading; no script |
| [`2596b143-doctest.txt`](2596b143-doctest.txt) | Toolchain, CPU, load, wall-clock and harness summary lines of three runs at `M` | Timing figures are excluded from reproduction |

The census reproduction stops at the `densest files:` line because the order of equal counts in that list follows directory iteration order. [`2596b143-changed-examples.py`](2596b143-changed-examples.py) states its section, item and pairing definitions in its header.

## Census comparison (REQ-01, REQ-02)

Baseline sources are lines of `fa787f85-rustdoc-example-audit.md`; final sources are lines of `2596b143-census.txt`.

| Measure | Baseline | Line | Final | Line |
|---|---:|---:|---:|---:|
| `# Example` / `# Examples` headings | 1,404 | 32 | 61 | 9 |
| Rustdoc code fences | 1,660 | 34 | 107 | 11 |
| Fences executed as doctests | 1,290 | 40 | 59 | 12 |
| Fences compiled only (`no_run`) | 122 | 41 | 4 | 13 |
| Fences never compiled | 248 | 42 | 44 | 14 |
| Headings in gf2-algebra | 181 | 59 | 1 | 2 |
| Headings in gf2-coding | 428 | 60 | 20 | 3 |
| Headings in gf2-core | 543 | 61 | 23 | 4 |
| Headings in gf2-kernels-hip | 81 | 63 | 0 | 5 |
| Headings in gf2-kernels-simd | 36 | 64 | 6 | 6 |
| Headings in gf2-sim | 135 | 62 | 11 | 7 |
| Headings in gf2-stats | no row | | 0 | 8 |

## Doctest count and time (REQ-01, REQ-02)

The times are informational wall-clock figures of a working session (DEC-01 of the issue) and support no performance claim. The final figures come from the `[baseline-form]` block of `2596b143-doctest.txt`, which runs the baseline's command; the `[ci-form]` block holds the same lines for the command of the CI `doctest` step. Each cell is doctests passed / ignored / harness `finished in` time.

| Crate | Baseline | Line | Final | Line |
|---|---:|---:|---:|---:|
| gf2-core | 559 / 2 / 13.93 s | 100 | 24 / 0 / 0.78 s | 45 |
| gf2-coding | 434 / 7 / 15.27 s | 101 | 20 / 0 / 0.97 s | 43 |
| gf2-sim | 137 / 0 / 14.07 s | 102 | 11 / 0 / 9.70 s | 49 |
| gf2-algebra | 181 / 0 / 4.82 s | 103 | 1 / 0 / 0.19 s | 41 |
| gf2-kernels-simd | 32 / 0 / 0.17 s | 104 | 6 / 0 / 0.06 s | 47 |
| gf2-stats | no row | | 1 / 0 / 0.15 s | 51 |
| tuning-campaign-support | no row | | 0 / 0 / 0.00 s | 53 |
| Total | 1,343 / 9 / 48.26 s | 105 | 63 / 0 / 11.85 s | sum |
| Invocation wall time | 49 s | 107 | 13.67 s | 37 |

The final total is the output of:

```sh
awk '/^\[baseline-form\]/{f=1} f&&/^test result/{n+=$4; i+=$8; sub("s","",$NF); t+=$NF} END{print n, i, t}' $D/2596b143-doctest.txt
```

Measurement protocol:

| Fact | Baseline (audit line 96) | Final (`2596b143-doctest.txt`) |
|---|---|---|
| Command | `cargo test --workspace --all-features --doc` | The same command through `./scripts/cargo-budget.sh --test` (line 35) |
| Features and profile | `--all-features`, `test` profile | `--all-features`, `test` profile (line 39) |
| Toolchain | Not stated | Lines 2 and 3 |
| Build-cache state | Warm target directory | Warm: an earlier run of the same command at `M` populates the same target directory, and line 39 reports the build step of the timed run |
| Host load | Not stated | Load averages before and after the run, lines 36 and 38 |
| CPU | Not stated | Line 4 |
| Date | 2026-08-07 (audit line 8) | Timestamps of lines 36 and 38 |

The timed interval includes the compilation of the doctests by rustdoc in addition to their execution. `gf2-kernels-hip` is outside the default Cargo workspace, so no run in `2596b143-doctest.txt` compiles its rustdoc code fences.

## Classification of added and changed examples (REQ-03)

| Check | Command | Establishes |
|---|---|---|
| Section count equals the census heading count | `diff <(tail -n +2 $D/2596b143-changed-examples.tsv \| wc -l) <(awk '$1=="TOTAL"{print $NF}' $D/2596b143-census.txt)` (empty) | The comparison script lists every heading the census counts at `M` |
| Status distribution | `cut -f1 $D/2596b143-changed-examples.tsv \| tail -n +2 \| sort \| uniq -c` | The number of `added`, `changed` and `unchanged` sections |
| Verdict rows are complete | `diff <(awk -F'\t' '$1!="unchanged"' $D/2596b143-changed-examples.tsv \| cut -f2,3 \| tail -n +2) <(cut -f2,3 $D/2596b143-example-verdicts.tsv \| tail -n +2)` (empty) | Every `added` or `changed` section has one verdict row and no other row exists |
| Verdict classes | `awk -F'\t' 'NR>1 && $1!="EARNED"' $D/2596b143-example-verdicts.tsv` (empty) | Every `added` or `changed` section is classed `EARNED`, each with the contract or workflow it teaches |
| Unchanged sections | `awk -F'\t' '$1=="unchanged" && $5!="EARNED"' $D/2596b143-changed-examples.tsv` (empty) | Every `unchanged` section has the text of a baseline section that `V` classes `EARNED` |
| Baseline `OBVIOUS` rows | `tail -n 3 $D/2596b143-baseline-obvious.tsv` | No baseline `OBVIOUS` row pairs with a section at `M`, by path and item or by identical fence content, and every such row resolves to a section at `B` |

Limits of these checks:

- A section whose file or item name differs between `B` and `M` has status `added` and is classified here; the pairing never marks such a section `unchanged`.
- An `unchanged` section keeps its baseline verdict and is not judged again here. The baseline's soft boundary in the earned direction (its section "Limits of this baseline") applies to those rows.
- The verdicts in `2596b143-example-verdicts.tsv` are judgments against `@/inv/teaching-rustdoc-examples` from reading each section at `M`; no script derives them.
- The census and the comparison cover example sections under a heading. Rustdoc code fences outside such a section are counted by the census fence figures and are not classified.

## CI steps (REQ-04)

`CARGO_CI_STEPS='^(doc|doctest)$' ./scripts/cargo-ci.sh` at `M` reports `✓ doc: ok` and `✓ doctest: ok` (`2596b143-doctest.txt` lines 10 and 11). The run is filtered, so it is the verdict of those two steps and not of the CI contract (line 13). The `doctest` step's command is the `[ci-form]` block, whose harness lines report 63 passed, 0 failed and 0 ignored across the workspace (lines 21 to 33).
