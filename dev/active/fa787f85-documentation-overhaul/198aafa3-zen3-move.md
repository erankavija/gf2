# 198aafa3 zen3 survey-harness entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.
`S` is `E/2037941f-profile-and-optimize-mid-range-buffer-operations`.

## Entries

| Entry | Owning issue state (tracker, 2026-10-06) | Destination |
|---|---|---|
| `4c1e441f` | done | `E/4c1e441f` |
| `53c5a8c0` | done | `E/53c5a8c0` |
| `6c6b09b1` | done | `E/6c6b09b1` |
| `6fb89a3c` | done | `E/6fb89a3c` |
| `ad2a6a58` | done | `E/ad2a6a58` |
| `c077a88b` | done | `E/c077a88b` |
| `c7113c5a` | done | `E/c7113c5a` |
| `f63a2464` | done | `E/f63a2464` |

## Commits

| Commit | Content |
|---|---|
| `c7441463b` | baseline scripts and outputs |
| `2902b8b88` | eight scripts and one Makefile resolve the root at run time; two freeze scripts compare a recorded addendum path by file name |
| `552a0548d` | `git mv`, path text in manifests and code, link targets, one closure, link baseline; its message lists each edit at file:line |
| `95ba9291f` | manifest rows |
| `59f56ce93` | path citations in Markdown and in `dev/scripts/receipt-input-omissions.json`, `S/mid-range-tables.md` |
| `8b7dfdd21`, `b80ae85a8` | link counts and one moved path in `f79bcba3-zen3-move.md` and `602652bf-protocol-inputs-move.md` |

## Baseline and repeat

Baseline source: `3f1433616`. Repeat source: `59f56ce93`. Commands, run from
the repository root:

```
R/198aafa3-state.sh <state-file>
R/198aafa3-builds.sh <build-file>
R/198aafa3-selfchecks.sh <selfcheck-file>
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
R/5a25717c-verdicts.sh <verdict-file>
```

| Output | Holds | Baseline against repeat | Does not establish |
|---|---|---|---|
| `198aafa3-state-before.txt`, `198aafa3-state-after.txt` | `--check` exit status of each live closure generator; each relative Cargo path dependency that names or leaves an entry; each line of entry code matching `parents[n]`, `../..`, `ancestors()` or a quoted `../<name>` | every generator exits 0 in both; each path dependency follows the rule of section "Harness crates"; the lines of the third section that take the repository root from `HERE/../../../..` or `parents[n]` are absent from the repeat, and `6c6b09b1/survey/arm-common/src/lib.rs:129` gains one level | the third section does not match an `os.pardir` chain or a Makefile; `2902b8b88` replaces the one chain (`53c5a8c0/survey/summarize.py`) and the one Makefile root (`6c6b09b1/survey/Makefile`) |
| `198aafa3-builds-before.txt` | per harness manifest that lies in an entry or names one: exit status of `cargo +1.95.0 metadata`, exit status of `cargo +1.95.0 build`, first error line | the output at the repeat source is the same file: `diff` against it prints nothing | test targets: `cargo build` does not compile them |
| `198aafa3-selfchecks-before.txt` | exit status of each script test under `dev/active/` and of `freeze-remeasure.py --self-test` | the output at `2902b8b88`, at `552a0548d` and at the repeat source is the same file | `6c6b09b1/survey/make-confirmation-addenda.test.sh` runs a built `survey-analysis`; the runs use an executable built on 2026-09-13 |
| `f79bcba3-verdicts-before.txt` | benchmark-acceptance verdict line and findings of each committed `zen3-benchmark-receipt-v1` receipt | the output at the baseline and at the repeat source is the same file; `5a25717c-verdict-baseline.md` states the repeat | |

## Harness crates (REQ-03)

`198aafa3-builds-before.txt` holds one line per manifest of the eight entries
and per manifest of `19513245`, `07ca8585` and `3be770d5` that names a moved
crate; each reads `resolve=0 build=0` at the baseline and at the repeat. The
build scripts of `53c5a8c0/survey/arms`, `c7113c5a/survey/gf2-side` and
`6c6b09b1/survey/ext-side` link staged external libraries, whose locations
`198aafa3-builds.sh` exports.

| Path dependency of a manifest | Text in `552a0548d` |
|---|---|
| in a moved entry, to a workspace crate or `tuning-campaign-support` | gains one `../` |
| in a moved entry, to a crate of another moved entry | unchanged |
| `f63a2464/survey/arms/Cargo.toml:26`, to the `3be770d5` harness | loses the epic directory name |
| `19513245/survey/consumer/Cargo.toml:25,26`, `19513245/survey/field-laws/Cargo.toml:18`, to `6c6b09b1` crates | gains the epic directory name |
| `07ca8585/survey/arms/Cargo.toml:33`, `3be770d5/survey/harness/Cargo.toml:26`, to the `c077a88b` harness | loses one `../` |

Two path literals of Rust sources follow the same rule:
`E/6c6b09b1/survey/arm-common/src/lib.rs:129` (`include_str!`) and
`SURVEY_SHIM_DIR` of `E/3be770d5/survey/harness/build.rs:24`.

`./scripts/cargo-budget.sh --test cargo +1.95.0 test --manifest-path <manifest>`
at `95ba9291f` passes for `E/4c1e441f/survey/gemm-arm` and
`E/6c6b09b1/survey/arm-common`, the two crates of the entries whose tests read
a path outside their crate; the run has no baseline counterpart.

Each build rewrites the tracked lockfiles
`dev/active/19513245/survey/field-laws/Cargo.lock` and
`E/6c6b09b1/survey/field-laws/Cargo.lock` (one `tempfile` line each) at the
baseline and at the repeat, and `cargo +1.95.0 metadata --locked` fails on
both -> @/issue/0d1e9536. The committed files keep their bytes.

## Scripts (REQ-04)

The message of `552a0548d` lists each path-text edit per entry at file:line.
`4c1e441f` and `ad2a6a58` resolve their paths in `survey/locate.py` and hold
no literal of their own directory.

| Literal that keeps the flat path | Record it reproduces or compares | Pin |
|---|---|---|
| `E/6c6b09b1/survey/make-addenda.py:42` | line 11 of each `E/6c6b09b1/addendum-v*.json` | SHA-256 of each addendum in lines 2 to 4 of `dev/bench_results/6c6b09b1/v3-region-axpy-family-ledger.jsonl`, `v3-matrix-product-family-ledger.jsonl` and `v3-pairwise-control-family-ledger.jsonl` |
| `V3_MANIFEST`, `E/c7113c5a/survey/summarize-v3.py:29` | `source.producing.manifest_path` of the receipts it selects | `dev/bench_results/c7113c5a/v3-r1-baselines-pilot/receipt.json:2343`, `v3-r1-baselines-confirmation/receipt.json:3106`, `v3-r1-host-targeting-pilot/receipt.json:817`, `v3-r1-host-targeting-confirmation/receipt.json:1580` |

| Check | Result | Does not establish |
|---|---|---|
| `bash -n` and `python3 -m py_compile` of the scripts `2902b8b88` edits | pass | |
| `python3 -B E/53c5a8c0/survey/summarize.py` at `2902b8b88` | writes the committed `dev/bench_results/53c5a8c0/tables.md` | |
| `make -n -C E/6c6b09b1/survey identities` before and after `2902b8b88` | same lines | |
| self-checks of the table above | same exit statuses | launchers, generators and `verify-*.py` scripts of the entries are not run: each needs a benchmark window, staged externals or built arms |
| `E/53c5a8c0/survey/summarize-profile.py:520,608` against `dev/bench_results/53c5a8c0/profile/profile.md:5,125` | the script names the path under `E`, the record the flat path -> @/issue/8163dacd | |

## Generated files and frozen documents (REQ-05)

| File | Generator | Diff |
|---|---|---|
| `E/1d4fd63d/external-producing-inputs.json` | `make-external-producing-inputs.py` | `552a0548d`: the lines that name files of the `6fb89a3c` survey |
| `S/mid-range-tables.md` | `make-mid-range-tables.py` | `552a0548d`, `59f56ce93`: SHA-256 of the Markdown files whose link targets or citations change |

`python3 dev/scripts/check-family-producing-closures.py` prints
`4 family closures enumerate their trees` at each commit of the table
"Commits". `make-publication-tables.py` and `make-candidate-tables.py` write
their committed tables at `59f56ce93`.

`git diff -M --name-status 2902b8b88 552a0548d` lists each tracked file of the
eight flat directories as a rename; restricted to `*.json`, `*.jsonl`, `*.txt`,
`*.tsv` and `*.log` it holds one line other than `R100`, the closure above. No
Markdown file of the moved entries has its SHA-256 or blob id in a tracked
file outside the migration manifest.

The link `../6fb89a3c/survey/fetch-build.sh` of the frozen
`S/m4ri-operation-match.md` resolves, and `552a0548d` removes its row from
the second table of `contrib/gates/docs-mechanical-baseline.md`; the move adds no row.
`python3 contrib/gates/docs-mechanical.py` prints `PASS` with 0 findings at
`552a0548d`; it checks each inline and reference link of `dev/active/**/*.md`
and no relative path inside a JSON string.

## Flat paths that stay

`R/198aafa3-remaining.sh <outfile>` classifies each tracked file in the
scope its header states that holds `dev/active/<entry>` for one of the eight
entries; `198aafa3-remaining.txt` is its output and holds no line of the class
`unclassified`.

| Class | Ground |
|---|---|
| `snapshot` | receipt `inputs/` snapshots |
| `receipt-directory` | receipts, logs, launch scripts and generated tables under `dev/bench_results/` |
| `data-record` | JSON, JSON Lines, TSV, text and log files: addenda, producing manifests, plans, derivation and smoke records, which keep their bytes; tool output at the commit `git log -1 --follow --diff-filter=AM --format=%h -- <file>` names |
| `epic-record` | records, scripts and raw outputs of other units of this epic: text bound to the commit the same command names |
| `manifest` | `path`, `consumers` and `inbound` fields of the migration manifest |
| `manifest-quote` | `E/handoff-23.md:53` quotes a `path` field of the migration manifest |
| `tracker-state` | `breakdown.json` of `R` and `ed3d490e-zen3-ldpc-frame-simd` |
| `bench-window` | `E/bench-window/queue.tsv`; the job key hashes the command text |
| `frozen-document` | `S/dense-parity-addendum.md` and `S/dense-parity-addendum-v1.md`, with the digest pins `f79bcba3-zen3-move.md` lists |
| `generated-table` | `E/63bad95d/route-inventory.md`, output of `survey/make-route-inventory.py` beside `route-inventory.json` |
| `snapshot-path` | `path` fields of `dev/scripts/receipt-input-omissions.json`: files inside receipt `inputs/` snapshots |
| `recorded-value`, `addendum-text` | table of section "Scripts" |

Tracker files under `.jit/` are outside the scan: they are tracker state.

## Manifest and tracker (REQ-06)

| Check | Command | Result | Does not establish |
|---|---|---|---|
| Manifest | `python3 R/migration/check.py` at `95ba9291f` and after | no finding, exit 0: each row whose path is one of the eight flat directories or lies below one is `complete`, with its source at its destination under `E` or, for the deletion rows of `264a453d2`, absent | a tracked file of the entries without a row of its own is covered by the row of its directory. The command exits 1 at `552a0548d` |
| Tracker | the two commands below | the first prints 0; the second counts the commit-pinned references, which keep the path of their commit | |
| Python CI steps | `CARGO_CI_STEPS='^(repository-files-self-test\|campaign-producing-closure-self-test\|campaign-producing-closure\|family-producing-closures-self-test\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots-self-test\|receipt-input-snapshots\|addendum-schema-versions-self-test\|addendum-schema-versions\|docs-mechanical-self-test\|tuning-campaign-launcher-syntax\|tuning-campaign-validator)$' ./scripts/cargo-ci.sh` | each step `ok` on the tree of `59f56ce93` | the CI verdict: the filtered run omits every cargo step |
| Formatting | `cargo +1.95.0 fmt --check` in each harness crate whose Rust files `552a0548d` edits | exit 0 in six crates; `E/6c6b09b1/survey/ext-side/build.rs` is formatted in that commit | `S/survey/dense-harness` exits 1 at `3f1433616` and at `552a0548d`, in lines the commit leaves alone -> @/issue/3d864278 |

```
P='^dev/active/(4c1e441f|53c5a8c0|6c6b09b1|6fb89a3c|ad2a6a58|c077a88b|c7113c5a|f63a2464)(/|$)'
jq -r '.documents[]? | select(.commit == null) | .path' .jit/issues/*.json | grep -cE "$P"
jq -r '.documents[]? | select(.commit != null) | .path' .jit/issues/*.json | grep -cE "$P"
```

`git -C <main checkout> status --short --ignored -- dev/active/<entry>` prints
nothing for the eight entries: the flat directories hold no ignored file.

## Criteria

| Criterion | Evidence |
|---|---|
| REQ-01 | table "Entries"; message of `552a0548d` |
| REQ-02 | `git ls-files dev/active/<entry>` prints nothing for each of the eight entries; row "Manifest" |
| REQ-03 | sections "Baseline and repeat" and "Harness crates"; message of `552a0548d` |
| REQ-04 | section "Scripts"; message of `552a0548d`, lists per entry |
| REQ-05 | sections "Generated files and frozen documents" and "Flat paths that stay" |
| REQ-06 | section "Manifest and tracker"; gate `cargo-ci` of @/issue/198aafa3 |
