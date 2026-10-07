# 2e18b37c zen3 entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.

## Entries

| Entry | Owning issue state (tracker, 2026-10-07) | Destination |
|---|---|---|
| `f8dd4dde` | done | `E/f8dd4dde` |
| `a203a23c` | done | `E/a203a23c` |
| `19513245` | done | `E/19513245` |

## Commits

| Commit | Content |
|---|---|
| `af9265484` | baseline script and outputs |
| `06fe749d3` | `19513245/survey/run-profile.sh:30` takes the root from git; `make-addenda.py:37` states the path text of the frozen addenda; `msrv-intrinsics.rs:24` names its file as `<this file>` |
| `df7b52dd6` | `git mv`, path text in manifests and code, link targets, one digest table, manifest rows; its message lists each edit at file:line |
| `2f81ba52e` | path citations in two Markdown files of the entries and in `dev/scripts/` |

## Baseline and repeat

Baseline source: `6e05ccb79`. Repeat source: `2f81ba52e`. Command, run from
the repository root:

```
R/2e18b37c-checks.sh state|builds|root|selfchecks|ci-step|regen <file>
```

| Output | Holds | Baseline against repeat | Does not establish |
|---|---|---|---|
| `2e18b37c-state-before.txt`, `2e18b37c-state-after.txt` | each relative Cargo path of the entries; each line of entry code matching `parents[n]`, `../..`, `ancestors()`, `pardir` or `CARGO_MANIFEST_DIR` | each path dependency follows the table of "Harness crates"; the one level-counting line, `run-profile.sh:30`, is absent from the repeat | |
| `2e18b37c-root-before.txt` (REQ-02) | the root that the `HERE` and `REPO` assignments of `19513245/survey/run-profile.sh` resolve, relative to the checkout root | the output at `06fe749d3` and at the repeat source is the same file: `root=.` | the profile: no other line of the script runs, and a session needs a benchmark window |
| `2e18b37c-builds-before.txt` (REQ-03) | per harness manifest of the entries: exit status of `cargo +1.95.0 metadata`, exit status of `cargo +1.95.0 build`, first error line | the output at the repeat source is the same file: each manifest `resolve=0 build=0` | `cargo build` compiles no test target; `--locked` resolution, see below |
| `2e18b37c-selfchecks-before.txt` (REQ-06) | exit status of each script test under `dev/active/` and of `a203a23c/select-receipts.py --self-test` | the output at the repeat source is the same file; the self-test exits 0 | |
| `2e18b37c-ci-step-before.txt` (REQ-06) | verdict line of the `receipt-input-snapshots` CI step without its duration; summary line of `dev/scripts/check-receipt-input-snapshots.py` | the output at the repeat source is the same file | the CI verdict: the filtered run omits every other step |
| `2e18b37c-regen-before.txt`, `2e18b37c-regen-after.txt` | per committed record that an entry script writes without a build or a benchmark window: `same` or the count of differing lines | table "Regenerated records" | the other scripts of the entries are not run; `make-producing-inputs.py` lists the tree it runs on, and D3 covers `make-source-evidence.py` |

Each build rewrites the tracked `E/19513245/survey/field-laws/Cargo.lock` (one
`tempfile` line) at the baseline and at the repeat, and
`cargo +1.95.0 metadata --locked` fails on that manifest
-> 0d1e9536 (jit issue show 0d1e9536); it exits 0 on the other two. The
committed lockfile keeps its bytes.

## Regenerated records

`2e18b37c-regen-after.txt` is the listing on the tree of `78d74ddb3`.

| Committed record | Writer | Baseline | Tree of `78d74ddb3` |
|---|---|---|---|
| `dev/bench_results/19513245/r1-consumer-profile/profile-summary.md` | `19513245/survey/summarize-profile.py` | `same` | `same` |
| `a203a23c/timing/timing.json` | `a203a23c/timing/measure.py --assess` | `same` | `same` |
| `a203a23c/timing/timing-tmpfs-superseded.json` | `measure.py --assess` | differs: D4 | differs: D2, D4 |
| `19513245/addendum-v4-*.json` | `make-addenda.py`, `freeze-confirmations.sh`, at the frozen times the addenda state | each differs: line 37 of `make-addenda.py` in the form of `552a0548d` | `same` |
| `19513245/confirmation-derivation-*.txt` | `freeze-confirmations.sh` | `same` | `same` |

## Divergences

| Id | Divergence | Tracking |
|---|---|---|
| D1 | `cargo +1.95.0 fmt --check` exits 1 in `E/19513245/survey/consumer` at `df7b52dd6`, in `src/bin/consumer-profile.rs`, a pure rename; it exits 0 in the other two harness crates | -> 3de8d0f3 (jit issue show 3de8d0f3) |
| D2 | `E/a203a23c/timing/measure.py:360,766` write the path under `E`; lines 34, 40 and 330 of `timing-tmpfs-superseded.json` state the flat path | -> 8163dacd (jit issue show 8163dacd) |
| D3 | `19513245/survey/make-source-evidence.py` exits 1 at the baseline and at the repeat: `crates/gf2-core/src/field/vec.rs:663` lacks the fragment of the claim `axpy-signature` | -> d12e9ded (jit issue show d12e9ded) |
| D4 | `measure.py --assess` on `timing-tmpfs-superseded.json` wraps the two withdrawn scheduling statements a second time | -> c9920dc6 (jit issue show c9920dc6) |

## Harness crates (REQ-03)

| Path dependency | Text in `df7b52dd6` |
|---|---|
| `f8dd4dde/msrv-validation/Cargo.toml:23`, `19513245/survey/consumer/Cargo.toml:27`, `19513245/survey/field-laws/Cargo.toml:20`, to `crates/gf2-core` | gains one `../` |
| `19513245/survey/consumer/Cargo.toml:25,26`, `19513245/survey/field-laws/Cargo.toml:18`, to `6c6b09b1` crates | loses the epic directory name |
| `19513245/survey/field-laws/Cargo.toml:19`, to `../consumer` | unchanged |

## Scripts (REQ-04, REQ-05)

The message of `df7b52dd6` lists each path-text edit per entry at file:line
and states that `RESTORE_PATH` keeps its text. The lines of the `19513245`
scripts that name the `6c6b09b1` and `c7113c5a` entries carry the path under
`E`.

| Check | Result | Does not establish |
|---|---|---|
| `bash -n` and `python3 -m py_compile` of each shell and Python script of the three entries at `df7b52dd6` | pass | behaviour of the scripts the regeneration listing omits |

| Literal that keeps the flat path | Class | Ground |
|---|---|---|
| `E/a203a23c/timing/measure.py:99` (`RESTORE_PATH`), `:7`, `:946`; `E/a203a23c/timing/README.md:6` | `revision-path` | path of the restoration tool at commit `76812a4e6` and its parent, which `measure.py` reads with `git show <revision>:<path>` |

## Generated files and byte-fixed files (REQ-07)

| File | Generator | Diff |
|---|---|---|
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/publication-tables.md` | `make-publication-tables.py`, standard output | `df7b52dd6`: SHA-256 of `00dd43c3/confirmation-outcome.md`, `investigation.md` and `shift-feasibility-record.md`, whose link targets change |

`make-mid-range-tables.py` and `make-candidate-tables.py` write their
committed tables at `2f81ba52e`. `python3
dev/scripts/check-family-producing-closures.py` prints `4 family closures
enumerate their trees` at each commit of the table "Commits"; no CI-checked
closure lists a path of the three entries.

`git diff -M --name-status 06fe749d3 df7b52dd6` lists each tracked file of
the three flat directories as a rename. The same listing restricted to
`*.json`, `*.jsonl`, `*.txt`, `*.tsv`, `*.log` and `*.lock` holds `R100`
lines alone: each addendum, producing manifest, derivation record,
conformance record, timing record and lockfile of the entries is a pure
rename. The renames below `R100` are the three `Cargo.toml`,
`19513245/findings.md`, `f8dd4dde/retention-rule.md` and the 11 scripts the
message of `df7b52dd6` names.

`python3 contrib/gates/docs-mechanical.py` prints `PASS` with 0 findings at
each commit of the table "Commits"; it checks each inline and reference link
of `dev/active/**/*.md` and no relative path inside a JSON string. Each
Markdown file that `df7b52dd6` or `2f81ba52e` changes lies under
`dev/active/`. `8f61d6de-baseline-findings.md` names none of the three entries.

## Flat paths that stay

`R/2e18b37c-remaining.sh <outfile>` classifies each line of a tracked file in
the scope its header states that holds `dev/active/<entry>` for one of the
three entries; `2e18b37c-remaining.txt` is its output and holds no line of the
class `unclassified`.

| Class | Ground |
|---|---|
| `snapshot` | receipt `inputs/` snapshots |
| `receipt-directory` | receipts, logs, plans, checkpoints, the profile record directory and the launcher `run-consumer-bytefield.sh` under `dev/bench_results/` |
| `data-record` | JSON and text files: producing manifest, source evidence, derivation, conformance, timing, restoration, re-evaluation and validation records of the entries, `E/428f2f6b/inventory-c077a88b.json` and `E/63bad95d/route-inventory.json`, which keep their bytes |
| `epic-record` | records, scripts and raw outputs of other units in `R`: text bound to the commit `git log -1 --follow --diff-filter=AM --format=%h -- <file>` names |
| `manifest` | `path`, `consumers` and `inbound` fields of the migration manifest |
| `bench-window` | `E/bench-window/queue.tsv:19`; the job key hashes the command text |
| `queue-quote` | `E/19513245/findings.md:570` quotes the command of that queue line |
| `generated-table` | `E/63bad95d/route-inventory.md`, output of `survey/make-route-inventory.py` beside `route-inventory.json` |
| `session-record` | `E/handoff-16.md` and files of `E/reviews/`: dated session and review-round records |
| `revision-path` | table "Scripts" |

Tracker files under `.jit/` are outside the scan: they are tracker state.

## Manifest and tracker (REQ-08)

| Check | Command | Result | Does not establish |
|---|---|---|---|
| Manifest | `python3 R/migration/check.py` at `df7b52dd6` and after | no finding, exit 0: each row whose path is one of the three flat directories or lies below one is `complete`, and each source lies at its destination under `E` | |
| Python CI steps | `CARGO_CI_STEPS='^(repository-files-self-test\|campaign-producing-closure-self-test\|campaign-producing-closure\|family-producing-closures-self-test\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots-self-test\|receipt-input-snapshots\|addendum-schema-versions-self-test\|addendum-schema-versions\|docs-mechanical-self-test\|tuning-campaign-launcher-syntax\|tuning-campaign-validator)$' ./scripts/cargo-ci.sh` | each step `ok` on the tree of `2f81ba52e` | the CI verdict: the filtered run omits every cargo step |
| Tracker | the two commands below | the first prints 0; the second counts the commit-pinned references, which keep the path of their commit | |

```
P='^dev/active/(f8dd4dde|a203a23c|19513245)(/|$)'
jq -r '.documents[]? | select(.commit == null) | .path' .jit/issues/*.json | grep -cE "$P"
jq -r '.documents[]? | select(.commit != null) | .path' .jit/issues/*.json | grep -cE "$P"
```

`git -C <main checkout> status --short --ignored -- dev/active/<entry>` prints
nothing for the three entries: the flat directories hold no ignored file.

## Criteria

| Criterion | Evidence | Does not establish |
|---|---|---|
| REQ-01 | table "Entries"; `git ls-files dev/active/<entry>` prints nothing for each entry at `df7b52dd6` | |
| REQ-02 | row `2e18b37c-root-before.txt`; `06fe749d3` | a profile session |
| REQ-03 | row `2e18b37c-builds-before.txt`; section "Harness crates"; message of `df7b52dd6` | test targets, `--locked` resolution, formatting (D1) |
| REQ-04 | section "Scripts"; message of `df7b52dd6`, lists per entry | |
| REQ-05 | `E/a203a23c/timing/measure.py:99`; message of `df7b52dd6` | |
| REQ-06 | rows `2e18b37c-selfchecks-before.txt` and `2e18b37c-ci-step-before.txt` | |
| REQ-07 | sections "Generated files and byte-fixed files" and "Flat paths that stay"; message of `2f81ba52e` | the citations of the class `session-record` |
| REQ-08 | section "Manifest and tracker"; gate `cargo-ci` of `jit issue show 2e18b37c` | |
