# f79bcba3 zen3 survey-harness entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.
`S` is `E/2037941f-profile-and-optimize-mid-range-buffer-operations`.

## Entries

| Entry | Owning issue state (tracker, 2026-10-06) | Destination | Result |
|---|---|---|---|
| `04b85d10` | done | `E/04b85d10` | moved |
| `07ca8585` | done | `E/07ca8585` | moved |
| `12fdeb5b` | done | `E/12fdeb5b` | moved |
| `1c602857` | done | `E/1c602857` | moved |
| `1d0da41f` | done | `E/1d0da41f` | moved |
| `1d4fd63d` | done | `E/1d4fd63d` | moved |
| `2037941f-profile-and-optimize-mid-range-buffer-operations` | done | `S` | moved |
| `3be770d5` | done | `E/3be770d5` | moved |

## Commits

| Commit | Content |
|---|---|
| `089dc4a40` | baseline outputs |
| `82ea39747` | closure generators, harness contract tests and twelve scripts resolve the root and entry paths at run time |
| `aad1fdc5a` | `repo_artifacts.addendum` and `freeze-confirmation.test.sh` locate a recorded file by identity |
| `05f7559c0` | `git mv`, path text in manifests and code, link targets, three closures; its message lists each edit at file:line |
| `b6bddd40e` | path citations in Markdown, `mid-range-tables.md` |
| `264a453d2` | manifest rows |

## Baseline and repeat

Baseline source: `b05286e53614b62324f985832af766423849465c`. Repeat source:
`264a453d22787531c8e021b38fad061a63592638`. Commands, run from the repository
root:

```
R/f79bcba3-state.sh <state-file>
R/f79bcba3-builds.sh <build-file>
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
R/5a25717c-verdicts.sh <verdict-file>
```

| Output | Holds | Baseline against repeat |
|---|---|---|
| `f79bcba3-state-before.txt`, `f79bcba3-state-after.txt` | `--check` exit status of each live closure generator; each relative Cargo path dependency that names or leaves an entry; each line of entry code matching `parents[n]`, `../..`, `ancestors()` or a quoted `../<name>` | every generator exits 0 in both; each path dependency gains one `../` or the epic directory name, apart from `E/07ca8585/survey/arms/Cargo.toml:34`, which names the `3be770d5` harness beside it and keeps its text |
| `f79bcba3-builds-before.txt` | per harness manifest: exit status of `cargo +1.95.0 metadata`, exit status of `cargo +1.95.0 build`, first error line | the output at the repeat source is the same file: `diff` against it prints nothing |
| `f79bcba3-verdicts-before.txt` | benchmark-acceptance verdict line and findings of each committed `zen3-benchmark-receipt-v1` receipt | the output at the repeat source is the same file: `diff` against it prints nothing; `5a25717c-verdict-baseline.md` states the repeat |

The third section of the state listing does not match an `os.pardir` chain;
`E/1d0da41f/survey/summarize.py:24` holds the one such chain of the eight
entries and gains one level in `05f7559c0`.

## Harness crates (REQ-03)

`f79bcba3-builds-before.txt` holds one line per manifest the
`harness-path-dependencies` listing prints for the eight entries.

| Manifest | Dependency resolution | Build on Rust 1.95 |
|---|---|---|
| `E/04b85d10/survey/gf2-side/Cargo.toml` | exit 0 | exit 101 at the baseline and at the repeat, first error `error[E0432]: unresolved import gf2_coding::bch::BchEncoder`; the port of the crate -> 7ff06801 (jit issue show 7ff06801) |
| the nine other manifests | exit 0 | exit 0 |
| `dev/active/f63a2464/survey/arms/Cargo.toml`, which names the `3be770d5` harness | not recorded | `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path dev/active/f63a2464/survey/arms/Cargo.toml` exits 0 at `05f7559c0` |

`./scripts/cargo-budget.sh --test cargo +1.95.0 test --manifest-path S/survey/<crate>/Cargo.toml`
passes for `harness` and `dense-harness` at `82ea39747` and at `05f7559c0`;
`b6bddd40e` and `264a453d2` change no Rust or Cargo file.

## Closure generators (REQ-07)

| Generator | Root | Entry directory | Files of other directories | Closure |
|---|---|---|---|---|
| `S/survey/make-logical-producing-inputs.py` | `git rev-parse --show-toplevel` from the script directory | script location | `run-candidate.sh`, `portfolio.md`, `addendum.schema.json`, `amendment-v4.md`, `measurement-contract.md` through `repository_files.live_file`; the protocol document through `repository_files.document` | `S/survey/logical-producing-inputs.json` |
| `S/survey/make-dense-producing-inputs.py` | same | same | the last four of the row above | `S/survey/dense-producing-inputs.json` |

Each generator imports the one live `repository_files.py`, and each closure
lists that file as a build input: the closure-freshness test of each harness
copies the build inputs alone into a scratch checkout and runs the generator
there. The line enters both closures in `82ea39747`; in `05f7559c0` each
closure differs from its parent in the moved paths alone.

`check-family-producing-closures.py` checks three closures of the moved
entries: the two above and `E/1d4fd63d/external-producing-inputs.json`, whose
generator `E/1d4fd63d/make-external-producing-inputs.py` resolves its paths
at run time. Each is the output of its generator at `05f7559c0`, and its diff
in that commit is the moved paths.

`python3 dev/scripts/check-family-producing-closures.py` prints
`4 family closures enumerate their trees` at each commit of the table "Commits".

## Frozen documents

| Document of `S` | SHA-256 at | Links that leave the entry |
|---|---|---|
| `dense-parity-addendum.md` | `S/survey/dense-harness/src/cells.rs:22`; `dense-campaign` refuses a timed run on another digest | rows of the second table of `contrib/gates/docs-mechanical-baseline.md` -> faac6007 (jit issue show faac6007) |
| `dense-parity-addendum-v1.md` | `S/survey/dense-baseline-validation.txt:6` | as above |
| `m4ri-operation-match.md` | `dev/bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/receipt.json:9383` | as above |
| `logical-buffer-addendum.md` | `dev/bench_results/2037941f/isal-dispatch-availability.md:7`, `S/mid-range-tables.md:470` | resolve; `80a18d4c9` repoints lines 13 and 341 |
| `isal-comparator.md` | `dev/bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/receipt.json:4825` | resolve |

`ADDENDUM_SHA256` of `S/survey/harness/src/cells.rs:17` is the SHA-256 of
`logical-buffer-addendum.md` at `5cbbd7ced` and differs from that of the live
file -> d0aaf09b (jit issue show d0aaf09b).

## Flat paths that stay

`R/f79bcba3-remaining.sh <outfile>` classifies each tracked file in the
scope its header states that holds `dev/active/<entry>` for one of the eight
entries; `f79bcba3-remaining.txt` is its output and holds no line of the class
`unclassified`. `git log -1 --follow --diff-filter=AM --format=%h -- <file>`
names the commit of a file whose ground is tool output or a named commit.

| Class | Ground |
|---|---|
| `snapshot` | receipt `inputs/` snapshots |
| `receipt-directory` | receipts, logs and generated tables under `dev/bench_results/` |
| `data-record` | JSON, JSON Lines, TSV, text and log files: addenda, producing manifests, derivations, ledgers, smoke and validation records; tool output at the commit the command above names |
| `epic-record` | records, scripts and raw outputs of other units of this epic: text bound to the commit the command above names |
| `frozen-document` | link labels and command text of the documents of the table above |
| `manifest` | `path`, `consumers` and `inbound` fields of the migration manifest |
| `bench-window` | `E/bench-window/queue.tsv`; the job key hashes the command text |
| `session-record` | `E/reviews/53b74fc2-r1.md:6`: a dated review record that quotes the flat directory `dev/active/1d4fd63d`, present nowhere in the tree -> 5406d020 (jit issue show 5406d020) |
| `generated-table` | `E/63bad95d/route-inventory.md`, output of `survey/make-route-inventory.py` beside `route-inventory.json` |
| `fixture-layout` | `E/c7113c5a/survey/freeze-confirmation.test.sh:30-32`: the layout that `E/1c602857/confirmation-derivation.txt:18` states; the test compares that record and the confirmation addendum, whose digest `dev/bench_results/1c602857/2026-09-13-1c602857-public-clmul-confirmation/receipt.json:2339` holds, byte for byte |
| `snapshot-path` | `E/04b85d10/survey/summarize-receipts.py:34`, a path inside a receipt snapshot |

Tracker files under `.jit/` are outside the scan: they are tracker state.

The producing manifests of the moved entries other than the three closures
above are `R100` renames in `05f7559c0` (REQ-05); those that list flat paths
are in the `data-record` class, as are the receipt-pinned addenda of
`E/12fdeb5b` that declare `family_wise.ledger_path` at the flat path;
locating the ledger without the path -> 3514d822 (jit issue show 3514d822).

## Checks

| Check | Command | Result | Establishes |
|---|---|---|---|
| Renames | `git diff -M --name-status aad1fdc5a 05f7559c0` | 419 renames, 322 of them `R100`; 27 files changed in place | each moved file that `05f7559c0` does not name keeps its bytes |
| Byte-fixed data | the same listing restricted to `*.json`, `*.jsonl`, `*.txt`, `*.tsv`, `*.log` | three files below `R100`: the three closures | every other data file of the moved entries is a pure rename |
| Links | `python3 contrib/gates/docs-mechanical.py` | `PASS` with 0 findings | each inline and reference link of `dev/active/**/*.md` resolves, apart from the baseline rows |
| Manifest | `python3 R/migration/check.py` | no finding, exit 0 | each complete row has its destination and no source; each deletion row has no source |
| Python CI steps | `CARGO_CI_STEPS='^(repository-files-self-test\|campaign-producing-closure-self-test\|campaign-producing-closure\|family-producing-closures-self-test\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots-self-test\|receipt-input-snapshots\|addendum-schema-versions-self-test\|addendum-schema-versions\|docs-mechanical-self-test\|tuning-campaign-launcher-syntax\|tuning-campaign-validator)$' ./scripts/cargo-ci.sh` | each step `ok` at `264a453d2` | the filtered run is not the CI verdict |
| Digest table | `python3 -B S/survey/make-mid-range-tables.py` | writes the committed `S/mid-range-tables.md` | the table is the generator's output on the tree of `b6bddd40e` |
| Script tests | each tracked `*_test.py`, `test-*.py`, `test_*.py`, `*.test.sh` and `verify-records.py` under `dev/active/` outside `fd9d5416` and `R` | each exits 0 at `aad1fdc5a` and at `264a453d2`, except `S/survey/test-dense-profile-request.py`, which runs a built `dense-campaign` and exits 1 at both | the move changes no exit status of these tests |

## Manifest (REQ-06, REQ-08)

| Rows | State |
|---|---|
| rows of the eight directories | `status = "complete"`, destination under `E` |
| `dev/active/3be770d5/survey/harness/src/bin/ldpc-plan-check.rs` | `disposition = "deletion"`, `evidence = "commit"`, `citation = "0e296481bd71406bc472ce2130d39b393990036b"` |
| deletion rows of `dev/active/4c1e441f`, `dev/active/ad2a6a58` and `dev/active/f63a2464` | the same form, with the removing commits `7eacbd17d`, `30ea6da8f` and `e3efec549` |

`67048b47-linked-pairs.txt` and `b0b1faf5-outputs.sh` name `ldpc-plan-check`:
the first is the pair listing of its unit, tool output at `b6899936f`; the second
builds a fixture of executable names for the source commits its record names.

## Tracker (REQ-06)

Run from the repository root. The first command lists the unpinned document
references under a flat path of the eight entries and prints nothing; the
second counts the commit-pinned references, which keep the path of their
commit.

```
P='^dev/active/(04b85d10|07ca8585|12fdeb5b|1c602857|1d0da41f|1d4fd63d|2037941f-profile-and-optimize-mid-range-buffer-operations|3be770d5)(/|$)'
jq -r '.documents[]? | select(.commit == null) | .path' .jit/issues/*.json | grep -E "$P"
jq -r '.documents[]? | select(.commit != null) | .path' .jit/issues/*.json | grep -cE "$P"
```

## Criteria

| Criterion | Evidence |
|---|---|
| REQ-01 | table "Entries"; message of `05f7559c0` |
| REQ-02 | `git ls-files dev/active/<entry>` prints nothing for each of the eight entries; `R/migration/check.py` |
| REQ-03 | sections "Baseline and repeat" and "Harness crates"; message of `05f7559c0` |
| REQ-04 | message of `05f7559c0`, list per entry |
| REQ-05 | checks "Renames", "Byte-fixed data", "Links"; section "Frozen documents" |
| REQ-06 | sections "Tracker" and "Manifest"; gate `cargo-ci` of `jit issue show f79bcba3` |
| REQ-07 | section "Closure generators" |
| REQ-08 | section "Manifest" |
