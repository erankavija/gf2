# 7039d680 addendum fixture entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.
`C` is `E/c04dd4ac-zen3-shifts-and-permutations`.

## Entries

| Entry | Owning issue state (tracker, 2026-10-06) | Destination |
|---|---|---|
| `26465e6c` | done | `E/26465e6c` |
| `eda07788` | done | `E/eda07788` |
| `5cbb6545` | done | `E/5cbb6545` |

## Commits

| Commit | Content |
|---|---|
| `7a1b4ea07` | baseline script and outputs |
| `b52d4eaf7` | `tests/profile_addendum.rs` and `tests/remeasure_addendum.rs` of the `eda07788` gf2-side harness take the root and their fixtures at run time |
| `4ee6815a9` | three scripts separate a path that committed bytes record from the location of live files |
| `a01f6ad78` | `git mv`, path text in manifests and code, link targets, two digest tables, link baseline, manifest rows; its message lists each edit at file:line |
| `7ca6f2ba1` | path citations in Markdown; two generated receipt tables |
| `fbdbecbf2`, `14d3ab443` | link counts and moved paths in `f79bcba3-zen3-move.md` and `602652bf-protocol-inputs-move.md` |

## Fixture test decoupling (REQ-08)

| Located | Form in `E/eda07788/survey/gf2-side/tests/profile_addendum.rs` |
|---|---|
| repository root | `repo()`, line 23: `tuning_campaign_support::repository::repository_root` |
| addendum schema | line 39: `SharedInput::AddendumSchema.locate(&repo(), ADDENDUM_SCHEMA_ID)`, by declared identity |
| addendum, producing manifest, cases, plan tool | `live(name)`, line 28: `repository::locate_live` over the file names of lines 18 to 21 |

`git grep -n -e CARGO_MANIFEST_DIR -e dev/active` on the file prints nothing
at `b52d4eaf7` and after. `remeasure_addendum.rs` takes the same root and
locates its addendum by file name and by the SHA-256 its receipt pins; its
receipt and schema snapshot paths lie under `dev/bench_results/` and keep
their literals.

| Script edit of `4ee6815a9` | Path that committed bytes record |
|---|---|
| `E/eda07788/survey/freeze-remeasure.py:144` compares by file name | addendum path of each execution log |
| `freeze-remeasure.py:174` reads the ledger through the repository-file helper | `ledger_path` of the pilot addendum |
| `freeze-remeasure.py:202,203` state the validation record as text | description of the frozen addendum |
| `V3_SURVEY`, `E/eda07788/survey/inspect-sources.py:43,44,232` | survey path inside the v3 producing snapshot and its launcher logs |
| `MEASURED`, `E/5cbb6545/survey/summarize.py:32,168` | entry path inside the producing snapshots |

The message of `4ee6815a9` gives lines 42 and 43 for the two constants of
`inspect-sources.py`; they are lines 43 and 44.

## Baseline and repeat

Baseline source: `47f407583`. Repeat source: `14d3ab443`. Commands, run from
the repository root:

```
R/7039d680-checks.sh state|builds|tests|selfchecks <file>
./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --cargo-profile ci-test --profile ci -E 'binary(addendum_schema_versions)'
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
R/5a25717c-verdicts.sh <verdict-file>
```

| Output | Holds | Baseline against repeat | Does not establish |
|---|---|---|---|
| `d53cba10-baseline.md` (REQ-03) | result of the addendum schema test | passes at both commits; the record states the repeat | |
| `7039d680-state-before.txt`, `7039d680-state-after.txt` | `--check` exit status of each live closure generator; each relative Cargo or build-script path of the entries; each line of entry code matching `parents[n]`, `../..`, `ancestors()` or `CARGO_MANIFEST_DIR` | every generator exits 0 in both; each Cargo path dependency gains one `../`; the two `REPO` lines of the harness tests are absent from the repeat; every other line differs by the epic directory in its path alone | the level-counting lines that remain resolve inside their entry or inside `E`, except `superseded/v1/survey/fetch-build.sh:12` and `observe-instructions.sh:51` of `26465e6c`, which count from the location `arm-provenance.json` records and are not run |
| `7039d680-builds-before.txt` (REQ-04) | per harness manifest of the entries: exit status of `cargo +1.95.0 metadata`, exit status of `cargo +1.95.0 build`, first error line | the output at `a01f6ad78` and at the repeat source is the same file: each manifest `resolve=0 build=0` | the `aff3ct` feature of `eda07788/survey/nr-derate` is off; `cargo build` compiles no test target |
| `7039d680-tests-before.txt` | `cargo +1.95.0 test --no-fail-fast` result lines and failed test names of the two harness crates that have tests | the output at `b52d4eaf7`, at `a01f6ad78` and at the repeat source is the same file; `producing_input_closure_is_valid_and_complete_on_disk` fails at each with `NotFound` -> 94befdc3 (jit issue show 94befdc3) | |
| `7039d680-selfchecks-before.txt` | exit status of each script test under `dev/active/` | the output at the repeat source is the same file | the entries hold no script test and no `--self-test` |
| `f79bcba3-verdicts-before.txt` (REQ-04) | benchmark-acceptance verdict line and findings of each committed `zen3-benchmark-receipt-v1` receipt | the output at the baseline and at the repeat source is the same file; `5a25717c-verdict-baseline.md` states the repeat | |

No build or test run changes a tracked lockfile of the entries.

## Harness crates and scripts (REQ-04, REQ-05)

The message of `a01f6ad78` lists each path-text edit per entry at file:line.

| Check | Result | Does not establish |
|---|---|---|
| `cargo +1.95.0 fmt --check` in the five harness crates at `a01f6ad78` | exit 0 in four; rustfmt wraps `import-v1-ledger.rs:27` in that commit | `E/5cbb6545/survey/gf2-side` exits 1 at `47f407583` and at `a01f6ad78` in `src/bin/count-verify.rs`, a pure rename -> f63ad8dd (jit issue show f63ad8dd) |
| `freeze-remeasure.py <frozen_utc of the addendum>` at `47f407583`, `4ee6815a9` and `a01f6ad78` | writes the committed `addendum-dvb-t2-bit-interleave-v3-remeasure.json` | |
| `inspect-sources.py --ext <staged externals> --output <file>` at `47f407583`, `4ee6815a9` and `a01f6ad78` | the three registers differ in the gf2 commit and, at the third, in the epic directory of the survey paths | each run reports `source needle not found` for the claims `gf2-rate-matched-doc-filler` and `gf2-rref-column-mapping` -> e59f207c (jit issue show e59f207c) |
| `5cbb6545/survey/summarize.py`, `26465e6c/survey/summarize-receipts.py` | write the committed `dev/bench_results/<entry>/tables.md` at `47f407583` and at `7ca6f2ba1` | launchers, the other generators and the `reevaluate.sh` script are not run: each needs a benchmark window, staged externals or a release build |
| Python CI steps: `CARGO_CI_STEPS='^(repository-files-self-test\|campaign-producing-closure-self-test\|campaign-producing-closure\|family-producing-closures-self-test\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots-self-test\|receipt-input-snapshots\|addendum-schema-versions-self-test\|addendum-schema-versions\|docs-mechanical-self-test\|tuning-campaign-launcher-syntax\|tuning-campaign-validator)$' ./scripts/cargo-ci.sh` | each step `ok` on the trees of `a01f6ad78` and `14d3ab443` | the CI verdict: the filtered run omits every cargo step |

## Generated files and byte-fixed files (REQ-06)

| File | Generator | Diff |
|---|---|---|
| `E/2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-tables.md` | `make-mid-range-tables.py` | `a01f6ad78`: SHA-256 of three outcome records whose link targets change |
| `C/publication-tables.md` | `make-publication-tables.py` | `a01f6ad78`: path and SHA-256 of `E/eda07788/findings.md`, SHA-256 of `C/investigation.md` |
| `dev/bench_results/26465e6c/tables.md`, `dev/bench_results/5cbb6545/tables.md` | `summarize-receipts.py`, `summarize.py` | `7ca6f2ba1`: the lines that cite files of the entries |

`python3 dev/scripts/check-family-producing-closures.py` prints
`4 family closures enumerate their trees` at each commit of the table
"Commits"; no CI-checked closure lists a path of the three entries.

`git diff -M --name-status 4ee6815a9 a01f6ad78` lists each tracked file of
the three flat directories as a rename. The same listing restricted to
`*.json`, `*.jsonl`, `*.txt`, `*.tsv`, `*.log` and `*.lock` holds `R100`
lines alone: each addendum, producing manifest, ledger, validation record
and lockfile of the entries is a pure rename. The renames below `R100` are
the five `Cargo.toml`, the three `findings.md` and the 18 code files the
message of `a01f6ad78` names.

`python3 contrib/gates/docs-mechanical.py` prints `PASS` with 0 findings at
`a01f6ad78`; it checks each inline and reference link of `dev/active/**/*.md`
and no relative path inside a JSON string. `a01f6ad78` removes the rows of
the second table of `contrib/gates/docs-mechanical-baseline.md` that link to
`../5cbb6545/findings.md` and adds none.

## Flat paths that stay

`R/7039d680-remaining.sh <outfile>` classifies each tracked file in the
scope its header states that holds `dev/active/<entry>` for one of the three
entries; `7039d680-remaining.txt` is its output and holds no line of the class
`unclassified`.

| Class | Ground |
|---|---|
| `snapshot` | receipt `inputs/` snapshots |
| `receipt-directory` | receipts, logs, launch records, provenance records and compiled-tool tables under `dev/bench_results/` |
| `data-record` | JSON, JSON Lines, text and log files: addenda, producing manifests, ledgers, source-evidence and validation records, which keep their bytes; tool output at the commit `git log -1 --follow --diff-filter=AM --format=%h -- <file>` names |
| `epic-record` | records, scripts and raw outputs of other units of this epic: text bound to the commit the same command names |
| `manifest` | `path`, `consumers` and `inbound` fields of the migration manifest |
| `tracker-state` | `R/breakdown.json` |
| `frozen-document` | `dense-parity-addendum.md` and `dense-parity-addendum-v1.md` of the mid-range entry, with the digest pins `f79bcba3-zen3-move.md` lists |
| `generated-table` | `E/63bad95d/route-inventory.md`, output of `survey/make-route-inventory.py` beside `route-inventory.json` |
| `session-record` | `E/handoff-3.md:36`: a dated session record that cites `survey/drafts/` of the shifts entry, a directory absent from the tree -> 5406d020 (jit issue show 5406d020) |
| `pinned-script` | scripts of `E/26465e6c/superseded/v1/survey/`; `arm-provenance.json:43-50` beside them holds the SHA-256 of each |
| `addendum-text` | `E/eda07788/survey/freeze-remeasure.py:199,202,203,219`: text of line 11 of `E/eda07788/addendum-dvb-t2-bit-interleave-v3-remeasure.json`, which the script reproduces byte for byte; pins: `dev/bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/receipt.json:3865` and `E/eda07788/dvb-t2-bit-interleave-baselines-trial-ledger.jsonl:5` -> dfff0e05 (jit issue show dfff0e05) |
| `snapshot-path` | `V3_SURVEY` of `E/eda07788/survey/inspect-sources.py:43`: a path inside a receipt `inputs/` snapshot |
| `revision-path` | `C/survey/make-profile-provenance.py:46,47,218`: paths the script reads from the tree of the revision a session's `host.txt` records |

`MEASURED` of `E/5cbb6545/survey/summarize.py:32` joins `dev/active` and the
entry id and belongs to the class `snapshot-path`; the scan pattern does not
match it. Tracker files under `.jit/` are outside the scan: they are tracker
state. `E/bench-window/queue.tsv` holds no path of the three entries.

## Manifest and tracker (REQ-07)

| Check | Command | Result | Does not establish |
|---|---|---|---|
| Manifest | `python3 R/migration/check.py` at `a01f6ad78` and after | no finding, exit 0: each row whose path is one of the three flat directories or lies below one is `complete`, and each source lies at its destination under `E` | |
| Tracker | the two commands below | each prints 0: no unpinned and no commit-pinned reference under the three flat paths | |

```
P='^dev/active/(26465e6c|eda07788|5cbb6545)(/|$)'
jq -r '.documents[]? | select(.commit == null) | .path' .jit/issues/*.json | grep -cE "$P"
jq -r '.documents[]? | select(.commit != null) | .path' .jit/issues/*.json | grep -cE "$P"
```

`git -C <main checkout> status --short --ignored -- dev/active/<entry>` prints
nothing for the three entries: the flat directories hold no ignored file.

## Criteria

| Criterion | Evidence |
|---|---|
| REQ-01 | table "Entries"; message of `a01f6ad78` |
| REQ-02 | `git ls-files dev/active/<entry>` prints nothing for each of the three entries; `a01f6ad78` changes no line of `profile_addendum.rs`, `remeasure_addendum.rs` or `dev/tools/tuning-campaign-support/tests/addendum_schema_versions.rs`, and its code edits are path text |
| REQ-03 | row `d53cba10-baseline.md` of "Baseline and repeat" |
| REQ-04 | rows `7039d680-builds-before.txt` and `f79bcba3-verdicts-before.txt`; message of `a01f6ad78` |
| REQ-05 | section "Harness crates and scripts"; message of `a01f6ad78`, lists per entry; row `addendum-text` of "Flat paths that stay" |
| REQ-06 | sections "Generated files and byte-fixed files" and "Flat paths that stay" |
| REQ-07 | section "Manifest and tracker"; row "Python CI steps"; gate `cargo-ci` of `jit issue show 7039d680` |
| REQ-08 | section "Fixture test decoupling"; `b52d4eaf7` precedes `a01f6ad78` |
