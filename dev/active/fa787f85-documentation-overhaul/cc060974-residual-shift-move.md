# cc060974 residual-shift entry move

`E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints. `R` is this record's directory.
`Z` is `E/00dd43c3` and `C` is `E/c04dd4ac-zen3-shifts-and-permutations`.

## Entries

| Entry | Owning issue state (tracker, 2026-10-07) | Destination |
|---|---|---|
| `00dd43c3` | done | `Z` |
| `c04dd4ac-zen3-shifts-and-permutations` | done | `C` |

## Commits

| Commit | Content |
|---|---|
| `ef15dadc9`, `88227cc19` | baseline script and output |
| `b47e9ebfa` | 23 scripts resolve the root and their entry at run time; its message lists each edit per entry |
| `b911fe67c` | `git mv`, link targets, three generated files, manifest rows |
| `37391ec5c` | path text in eight Markdown files |
| the commit that adds this record | the baseline script with its commit argument and order-free digest, its two outputs, the listing of flat paths, the verdict repeat |

## Baseline and repeat (REQ-02)

Command, run from the repository root:
`R/cc060974-baseline.sh <outfile> [<commit>]`. Its header states each line
form. It runs in a scratch clone of the commit: the two script tests,
`make-shift-producing-inputs.py --check`, and the generators of the table
"Generators".

| Source commit | Output |
|---|---|
| `88227cc19`, the parent of the decoupling | `cc060974-baseline-before.txt` |
| `b47e9ebfa`, after the decoupling | `diff` against `cc060974-baseline-before.txt` prints nothing |
| `37391ec5c`, after the move | `cc060974-baseline-after.txt` |

This command prints the three differences of the table below and nothing
else:

```
F='s# raw=[0-9a-f]*##; s#dev/active/1a379447-zen3-cpu-performance/\(00dd43c3\|c04dd4ac-zen3-shifts-and-permutations\)#dev/active/\1#g'
diff <(sed "$F" R/cc060974-baseline-before.txt) <(sed "$F" R/cc060974-baseline-after.txt)
```

| Line of the listing | Before | After | Cause |
|---|---|---|---|
| `Z/survey/producing-inputs.json` rewritten in place | 10 added, 0 deleted | 22 added, 12 deleted; same `flat` digest | the committed manifest is receipt-pinned and states 12 flat paths; the generator lists the tree it runs on |
| `C/survey/dvb-producing-inputs.json` rewritten in place | 18 added, 577 deleted | 38 added, 597 deleted; same `flat` digest | as above, 20 flat paths |
| `make-publication-tables.py` against `publication-tables.md` | `same` | `same`, other `flat` digest | the table states the SHA-256 of five Markdown files whose link targets `b911fe67c` changes |

| Does not establish | Reason |
|---|---|
| line order of a generated file | the `flat` digest sorts lines and drops trailing commas, since a moved path sorts at another place |
| the shell launchers and `find-shift-executable.py` | table "Generators", column "Run" |
| `run-campaign.sh prepare`, `smoke-shift-arms.sh --check` and `smoke-dvb-arms.sh --check` against the committed smoke records | each compares a smoke of a plan that states the addendum path; the plan of the moved tree states the path under `E`, and the comparison needs a build |

## Generators

| Script | Run | Reason when excluded |
|---|---|---|
| `Z/survey`: `freeze-pilot.py`, `make-plan.py`, `make-producing-inputs.py` | yes | |
| `Z/survey/freeze-confirmation.py` | yes; exits 1 with `the family's one-candidate confirmation attempt is already reserved` at each commit, before it writes | |
| `C/survey`: `freeze-shift-addendum.py`, `inspect-shift-consumers.py`, `make-dvb-addendum.py`, `make-dvb-plan.py`, `make-dvb-tables.py`, `make-producing-inputs.py`, `make-profile-provenance.py`, `make-publication-tables.py`, `make-shift-plan.py`, `make-shift-producing-inputs.py`, `make-source-evidence.py`, `make-voided-attempt.py`, `make-voided-attempt-record.py`, `summarize-profile.py`, `summarize-shift-receipt.py` | yes | |
| `C/survey/make-shift-source-evidence.py` | yes; exits 1 at each commit (V1) | |
| `C/survey/find-shift-executable.py` | no | reads the message stream of a `cargo build` |
| `C/survey/summarize-arm-smoke.py`, `Z/survey/verify-smoke.py` | no | read the smoke record and plan of a built arm under `target/` |
| each `.sh` of `Z/survey` and `C/survey` | no | builds, or runs in a benchmark window; `bash -n` passes for each at `b47e9ebfa` |

## Scripts (REQ-01)

Root: `git rev-parse --show-toplevel`. Entry directory: the script location.
The message of `b47e9ebfa` names the lookup of each file of another entry:
`repository_files.live_file` by name, `repository_files.document` by opening
line, in shell `dev/scripts/repository_files.py file <name>`.

This command lists each line of an entry script that names `dev/active`,
counts levels or holds `../`:

```
git grep -nE 'dev/active|parents\[|\.\./|pardir' -- 'Z/survey/*.py' 'Z/survey/*.sh' 'C/survey/*.py' 'C/survey/*.sh'
```

At `37391ec5c` it prints the lines of this table and no line of `Z`. REQ-01
is unmet for each line of rows one to four.

| Lines of `C/survey` | Ground: text a script reproduces from a frozen document or committed record |
|---|---|
| `freeze-shift-addendum.py:19`, `make-dvb-addendum.py:28` | `family_wise.ledger_path` of the frozen addenda `C/shift-profile-addendum.json:35` and `C/dvb-profile-addendum.json:35`; `--check` and the baseline compare the bytes |
| `make-profile-provenance.py:39-47,218` | paths of the tree the profile session names in `host.txt`, read with `git show <revision>:<path>` and printed into the two committed provenance records, which the baseline reproduces as `same` |
| `make-voided-attempt.py:30,102,103`, `make-voided-attempt-record.py:18,43,49,50` | text of `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-voided-profile-attempt.json` and `dev/bench_results/85fc5ff4/v4-voided-launch-attempt.json`, which the baseline leaves unchanged |
| `summarize-shift-receipt.py:32` | entry path in `dev/bench_results/c04dd4ac/residual-shift-tables.md`, which the baseline leaves unchanged |
| `summarize-profile.py:49` | a relative link in the text of `profile-summary.md`; it locates no file |

## Move (REQ-03, REQ-04, REQ-06)

`git diff -M --name-status b47e9ebfa b911fe67c` lists each `.py`, `.sh`,
`.rs`, `.toml`, `.s`, `.txt`, `.jsonl` and `.json` file of the two entries as
`R100`, except `shift-profile-producing-inputs.json`. `git ls-files
dev/active/00dd43c3 dev/active/c04dd4ac-zen3-shifts-and-permutations` prints
nothing at `b911fe67c`.

| Generated file | Generator | State |
|---|---|---|
| `C/survey/shift-profile-producing-inputs.json` | `make-shift-producing-inputs.py` | written at `b911fe67c`; `--check` prints `enumerates this tree` at `37391ec5c` |
| `C/publication-tables.md` | `make-publication-tables.py`, standard output; no build | written at `b911fe67c`; `same` at `37391ec5c` |
| `C/dvb-interleave-profile-tables.md` | `make-dvb-tables.py`, standard output | written at `b911fe67c`; `same` at `37391ec5c` |

`python3 dev/scripts/check-family-producing-closures.py` prints
`4 family closures enumerate their trees` at each commit of the table
"Commits".

## Citations (REQ-05)

| Change | Files | Commit |
|---|---|---|
| relative link targets | `Z/confirmation-outcome.md`; eight Markdown files of `C`; `E/f8dd4dde/retention-rule.md`, `E/handoff-18.md` to `E/handoff-22.md`, four files of `E/reviews/` | `b911fe67c` |
| path text | `C/publication-findings.md`, `E/handoff-10.md`, `E/handoff-11.md`, `E/handoff-16.md`, `E/handoff-17.md`, `E/handoff-19.md`, `E/reviews/9fb40c83-r8.md`, `E/reviews/c04dd4ac-r2.md` | `37391ec5c` |

`python3 contrib/gates/docs-mechanical.py` prints
`PASS: 0 finding(s), 15 baselined` at each commit of the table "Commits"; it
checks each inline and reference link of `dev/active/**/*.md` and no path
inside a JSON string. Each Markdown file the unit changes lies under
`dev/active/`. `8f61d6de-baseline-findings.md` names no file of the two
entries as a source: the unit removes 0 rows and adds 0.

`R/cc060974-remaining.sh <outfile>` classifies each line of a tracked file
outside `.jit/` and outside the `cc060974-` files of `R` that holds
`dev/active/<entry>` for one of the two entries; `cc060974-remaining.txt` is
its output at `37391ec5c`, with the file and line count of each class, and
holds no line of the class `unclassified`.

| Class | Ground |
|---|---|
| `snapshot` | receipt `inputs/` snapshots |
| `receipt-directory` | receipts, logs, plans, checkpoints, ledgers and tables under `dev/bench_results/` |
| `manifest` | `path`, `consumers` and `inbound` fields of the migration manifest |
| `tracker-state` | `E/progress.json` |
| `bench-window` | `E/bench-window/queue.tsv`; the job key hashes the command text |
| `data-record` in `Z` and `C` | receipt-pinned inputs (the two producing manifests, two addenda) and output of the freezer and of built tools at the commit that adds each file (the derivation record, three smoke records, `harness-validation.txt`, `runner-smoke.txt`) |
| `data-record` and `generated-table` in `E/63bad95d` | tool output at `2d9fdf471`: `calibration-budget.json`, `route-inventory.json` and the tables their generators write beside them (V2) |
| `breakdown-manifest` | `C/breakdown.json`, the manifest the tracker instantiated; four of its paths name files the tree never held |
| `record-text` | table "Scripts" |
| `epic-record` | records, scripts and raw outputs of other units in `R`; each describes the tree of the commit `git log -1 --follow --diff-filter=AM --format=%h -- <file>` names |

The classes `breakdown-manifest` and `epic-record` are outside the grounds
the unit's assignment lists; the return of the unit raises both.

## Divergences

| Id | Divergence | Tracking |
|---|---|---|
| V1 | `C/survey/make-shift-source-evidence.py` exits 1 at the baseline and at the repeat: `crates/gf2-core/src/bitvec.rs` lacks the fragment `self.data[i] = (self.data[i - word_shift] << bit_shift)` | tracking issue: none |
| V2 | `E/63bad95d/survey/make-calibration-budget.py` and `make-route-inventory.py` list tracked paths; on a clone of `37391ec5c` they rewrite `calibration-budget.json`, `calibration-budget.md`, `route-inventory.json` and `route-inventory.md` with other bytes, and the committed lines `:151`, `:104`, `:2421,2422` and `:386` of those files state the flat path of the two entries | tracking issue: none |

## Verdicts (REQ-07)

Section "Repeat at the cc060974 entry move" of `5a25717c-verdict-baseline.md`:
the listing at `d322f4f97` and at `37391ec5c` equals
`f79bcba3-verdicts-before.txt`. `git diff --stat d322f4f97 37391ec5c --
dev/bench_results dev/tools` prints nothing.

## Manifest and tracker (REQ-08)

| Check | Command | Result | Does not establish |
|---|---|---|---|
| Manifest | `python3 R/migration/check.py` at `b911fe67c` and after | no finding, exit 0: the 88 rows whose path is one of the two flat directories or lies below one are `complete`, and each source lies at its destination under `E` | |
| Python CI steps | `CARGO_CI_STEPS='^(repository-files-self-test\|campaign-producing-closure-self-test\|campaign-producing-closure\|family-producing-closures-self-test\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots-self-test\|receipt-input-snapshots\|addendum-schema-versions-self-test\|addendum-schema-versions\|docs-mechanical-self-test\|tuning-campaign-launcher-syntax\|tuning-campaign-validator)$' ./scripts/cargo-ci.sh` | each of the 13 steps `ok` on the tree of `37391ec5c` | the CI verdict: the filtered run omits every cargo step, and the `cargo-ci` gate runs on the merged tree |
| Tracker | the two commands below, on the main checkout | 84 unpinned references under the two flat paths and 4 commit-pinned ones, which keep the path of their commit | the tracker changes themselves: the relink script of the unit's return makes them on the main checkout |

```
P='^dev/active/(00dd43c3|c04dd4ac-zen3-shifts-and-permutations)(/|$)'
jq -r '.documents[]? | select(.commit == null) | .path' .jit/issues/*.json | grep -cE "$P"
jq -r '.documents[]? | select(.commit != null) | .path' .jit/issues/*.json | grep -cE "$P"
```

`git -C <main checkout> status --short --ignored` prints one ignored path
under the two flat directories:
`dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/__pycache__/`.

## Criteria

| Criterion | Evidence | Does not establish |
|---|---|---|
| REQ-01 | section "Scripts"; message of `b47e9ebfa` | unmet for each line of rows one to four of its table, which keeps a `dev/active/` literal as record text |
| REQ-02 | section "Baseline and repeat" | the launchers and the smoke comparisons |
| REQ-03 | section "Move"; table "Entries" | |
| REQ-04 | row 1 of the generated files; closure check | the closure-freshness of other families, which the same check covers |
| REQ-05 | section "Citations"; `git diff -M --name-status b47e9ebfa 37391ec5c` shows each addendum, ledger, smoke record and pinned producing manifest as `R100` | `C/breakdown.json` and the records of `E/63bad95d` keep their citations |
| REQ-06 | row 2 of the generated files | |
| REQ-07 | section "Verdicts" | |
| REQ-08 | section "Manifest and tracker" | the tracker references and the `cargo-ci` gate, which belong to the main checkout |
