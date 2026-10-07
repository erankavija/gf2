# 602652bf protocol entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.
`S` is `E/2037941f-profile-and-optimize-mid-range-buffer-operations`.

| Entry | Owning issue state (tracker, 2026-10-06) | Destination |
|---|---|---|
| `f547c394` | done | `E/f547c394`; `23219502-inventory-notes.md` applies the multi-owner rule |
| `bdc507a3` | done | `E/bdc507a3` |

## Commits

| Commit | Content |
|---|---|
| `8f50e32a4` | merge of main into the lane |
| `deb8854e5` | four readers outside the entry locate the addendum schema by name |
| `6fdb776f7` | `git mv`, own-path text of one script, link targets, five generated files, link baseline |
| `49672ffd6` | path citations in 19 Markdown files, two digest tables |
| `8ec2a4abe` | manifest rows |
| `08eaa462e` | link counts of `f79bcba3-zen3-move.md` |

## Readers (REQ-03)

`git grep -nE 'dev/active/(f547c394|bdc507a3)' -- dev/tools dev/scripts crates scripts contrib`
prints nothing at `8fd000a70`, the parent of the unit.

| Reader | Locates the inputs through | Result at `08eaa462e` |
|---|---|---|
| `dev/scripts/check-campaign-producing-closure.py` | `repository_files.shared_producing_manifest` | names `E/f547c394/producing-inputs.json` |
| `dev/scripts/check-addendum-schema-versions.py` | file names and `$id` among the files git lists | passes; `--write` gives the committed `schema-versions.json` |
| `protocol.rs`, `receipt.rs`, `benchmark-ab-runner.rs` of `tuning-campaign-support` | `SharedInput::locate`, `repository::locate_live` | covered by the two rows below |
| `tests/protocol_contracts.rs`, `tests/addendum_schema_versions.rs` | declared identity or SHA-256 | `./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --cargo-profile ci-test --profile ci`: 213 passed |
| `benchmark-acceptance` | receipt directory alone | section "Verdicts" |
| `S/survey/make-logical-producing-inputs.py`, `make-dense-producing-inputs.py` | `repository_files.live_file`, `repository_files.document` | `--check` exits 0 |

Readers decoupled in `deb8854e5`:

| Reader | Form | Check |
|---|---|---|
| `E/6fb89a3c/survey/freeze-remeasure.py`, `E/eda07788/survey/freeze-remeasure.py` | `located("addendum.schema.json")` runs the `file` command of the one `repository_files.py` | `located(SCHEMA)` prints `E/f547c394/addendum.schema.json` at `08eaa462e`; `--self-test` of the first passes; neither freeze path is run |
| `E/eda07788/survey/gf2-side/tests/profile_addendum.rs` | `SharedInput::AddendumSchema.locate(REPO, ADDENDUM_SCHEMA_ID)` | at `deb8854e5`, with `GF2_XDSOPL_INCLUDE` set, seven of eight tests pass, among them the schema reader `profile_addendum_is_frozen_valid_and_fully_exploratory`; not rerun after the move |
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-producing-inputs.py` | `repository_files.live_file`, `repository_files.document` | output at `deb8854e5` equals the output of its parent version; at `08eaa462e` it differs from that output in the two moved paths |

`producing_input_closure_is_valid_and_complete_on_disk`, the eighth test of
`profile_addendum.rs`, fails at `deb8854e5`: the frozen
`dvb-producing-inputs.json` lists `crates/gf2-coding/src/bch/extended.rs`,
which the tree lacks -> 94befdc3 (jit issue show 94befdc3). A receipt pins
that manifest, so it keeps its bytes.

`6fdb776f7` changes one script, `E/bdc507a3/compare-acceptance-verdicts.sh`,
at lines 4, 6, 112 and 134 (own path text, REQ-08), and no `.py`, `.rs` or
`.toml` file.

## Move and generated files (REQ-01, REQ-09)

`git diff -M --name-status deb8854e5 6fdb776f7`: 93 `R100` renames, six
renames below `R100`, 36 files modified in place. `git ls-files
dev/active/f547c394 dev/active/bdc507a3` prints nothing.

| Generated file | Generator | Diff in `6fdb776f7` |
|---|---|---|
| `S/survey/logical-producing-inputs.json`, `S/survey/dense-producing-inputs.json` | `make-logical-producing-inputs.py`, `make-dense-producing-inputs.py` | the three moved paths of each closure |
| `E/f547c394/schema-versions.json` | `check-addendum-schema-versions.py --write` | five `path` fields |
| `S/mid-range-tables.md` | `make-mid-range-tables.py` | SHA-256 of changed files and link targets |
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/publication-tables.md` | `make-publication-tables.py`, standard output | SHA-256 of three changed files |

`python3 dev/scripts/check-family-producing-closures.py` prints
`4 family closures enumerate their trees` at `8f50e32a4`, `6fdb776f7` and
`08eaa462e`. The two digest tables are generator output at `49672ffd6` as well.

## Verdicts (REQ-04)

`5a25717c-verdict-baseline.md`, section "Repeat at the 602652bf entry move",
states the command and the two commits: empty `diff`. Section
"Repeat at the link and literal edits" states the repeat at `78d74ddb3`.

## Frozen documents (REQ-06)

| Document | SHA-256 at | Links that leave the entry |
|---|---|---|
| `E/f547c394/protocol.md` | `dev/bench_results/00dd43c3/v4-r1-confirmation/receipt.json:2324` | row of the third table of `8f61d6de-baseline-findings.md` -> faac6007 (jit issue show faac6007) |
| `E/f547c394/amendment-v4.md` | `dev/bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/receipt.json:9659` | as above; `../bdc507a3/verdict-preservation.json` resolves |

`git diff -M --name-status deb8854e5 6fdb776f7` restricted to `*.json`,
`*.jsonl`, `*.txt`, `*.tsv` and `*.log` lists three files below `R100`: the
two closures and `schema-versions.json`.

## Citations (REQ-06)

| Change | Files | Commit |
|---|---|---|
| 76 relative link targets | 35 Markdown files, four of them in the entry | `6fdb776f7` |
| path text | `S/dense-parity-harness.md`, `S/logical-harness.md`, ten handoffs, four reviews and `worker-brief.md` of `E`, `dev/active/c04dd4ac-zen3-shifts-and-permutations/plan.md`, `E/f547c394/design.md` | `49672ffd6` |

`python3 contrib/gates/docs-mechanical.py` prints
`PASS` with 0 findings at `6fdb776f7`, `49672ffd6` and `08eaa462e`; it
checks each inline and reference link of
`dev/active/**/*.md`. The gate does not check a relative path inside a JSON
string.

`R/602652bf-remaining.sh <outfile>` classifies each tracked file in the
scope its header states that holds `dev/active/f547c394` or
`dev/active/bdc507a3`; `602652bf-remaining.txt` is its output and holds no
line of the class `unclassified`.

| Class | Ground |
|---|---|
| `snapshot` | receipt `inputs/` snapshots |
| `receipt-directory` | receipts, logs, launch scripts and records under `dev/bench_results/` |
| `data-record` | JSON, TSV, text and log files: plans of the entry, the receipt-pinned `dvb-producing-inputs.json`, and tool output at the commit `git log -1 --follow --diff-filter=AM --format=%h -- <file>` names |
| `line-qualified-record` | `findings.md`, `research-r3-*-resolutions.md` and `rework-validation*.md` of the entry: command transcripts and tables whose header reads "file:line at HEAD" or "committed tree"; the same `git log` command names the commit |
| `manifest` | `path`, `consumers` and `inbound` fields of the migration manifest |
| `epic-record` | records, scripts and raw outputs of other units of this epic: text bound to the commit the same `git log` command names |
| `generated-table` | `S/mid-range-tables.md`, `publication-tables.md`: the source path a receipt records for its protocol pin |
| `tracker-state` | `R/breakdown.json` |
| `frozen-document` | link labels in the addenda of `S` that `f79bcba3-zen3-move.md` lists with their digest pins |
| `pinned-script` | comment at `E/26465e6c/superseded/v1/survey/record-provenance.sh:6`; digest at `E/26465e6c/superseded/v1/survey/arm-provenance.json:45` |
| `commit-permalink` | `docs/reference/performance-evidence.md:19`: URL of the protocol at commit `6776fe30e` |

Tracker files under `.jit/` are outside the scan: they are tracker state.

## Manifest and tracker (REQ-02, REQ-05, REQ-07)

| Check | Command | Result | Does not establish |
|---|---|---|---|
| Manifest | `python3 R/migration/check.py` at `08eaa462e` | no finding, exit 0; each row whose path is one of the two flat directories or lies below one is `complete` with a destination under `E` | `amendment-p02.md` has no row of its own; the row of the directory covers it |
| Tracker | the two commands below | each prints 0: no unpinned and no commit-pinned reference under the two flat paths | |
| Python CI steps | `CARGO_CI_STEPS='^(repository-files-self-test\|campaign-producing-closure-self-test\|campaign-producing-closure\|family-producing-closures-self-test\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots-self-test\|receipt-input-snapshots\|addendum-schema-versions-self-test\|addendum-schema-versions\|docs-mechanical-self-test\|tuning-campaign-launcher-syntax\|tuning-campaign-validator)$' ./scripts/cargo-ci.sh` | each step `ok` at `8f50e32a4` and at `8ec2a4abe` | the CI verdict: the filtered run omits every cargo step |
| Script tests | each tracked `*_test.py`, `test-*.py`, `test_*.py`, `*.test.sh` and `verify-records.py` under `dev/active/` outside `fd9d5416` and `R` | each exits 0 at `8ec2a4abe`, except `S/survey/test-dense-profile-request.py`, which runs a built `dense-campaign` and exits 1 | equality with the parent of the unit: the baseline of these tests is the row "Script tests" of `f79bcba3-zen3-move.md` |

```
P='^dev/active/(f547c394|bdc507a3)(/|$)'
jq -r '.documents[]? | select(.commit == null) | .path' .jit/issues/*.json | grep -cE "$P"
jq -r '.documents[]? | select(.commit != null) | .path' .jit/issues/*.json | grep -cE "$P"
```

`git -C <main checkout> status --short --ignored -- dev/active/f547c394
dev/active/bdc507a3` prints nothing: the flat directories hold no ignored
file.

## Criteria

| Criterion | Evidence |
|---|---|
| REQ-01 | first table; section "Move and generated files" |
| REQ-02 | row "Tracker" |
| REQ-03 | section "Readers"; message of `6fdb776f7` |
| REQ-04 | section "Verdicts" |
| REQ-05 | rows "Python CI steps" and the `tuning-campaign-support` tests; gate `cargo-ci` of `jit issue show 602652bf` |
| REQ-06 | sections "Frozen documents" and "Citations" |
| REQ-07 | row "Manifest" |
| REQ-08 | section "Readers", last paragraph; message of `6fdb776f7` |
| REQ-09 | section "Move and generated files" |
