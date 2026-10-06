# Decoupling findings for the eight flat entries

Read-only investigation of the main checkout at commit `b68598fc6`.

## 0. Facts that hold for all eight entries

### 0.1 No CI step recomputes a digest against a live `dev/active/` file

- `dev/scripts/check-receipt-input-snapshots.py:166-247`: every `Pin.path` is built
  by `under()` (`:179-180`) as `<receipt_dir>/inputs/...`; `check()` (`:250-273`)
  reads those paths from the git index. It reads the snapshot copy inside the
  receipt, never the live source file.
- `benchmark-acceptance` takes one receipt directory
  (`dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs:27`,
  `receipt.rs:993`, snapshot root `receipt.rs:1142`); the baseline runner copies each
  receipt directory to a temporary directory first
  (`dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts.sh:12-14`). Its
  verdicts cannot depend on where a `dev/active/` entry lives.
- `git grep -nF -f <all sha256 and blob ids of the eight entries> -- ':!.jit'` finds
  the digests only in: `dev/bench_results/**` (receipt.json, execution.log,
  launcher.log, checkpoints/manifest.json, host.txt, ledgers, provenance records,
  `inputs/` snapshots), generated digest tables
  (`c04dd4ac-.../publication-tables.md`,
  `2037941f-.../mid-range-tables.md`), smoke records inside the entries,
  `1a379447-.../b2e09d41/plan-checks.tsv` and `reference-observations.json`,
  `fa787f85-.../7bac1303-harness-path-trials.txt`, `f29a9225-identities.tsv`, and the
  migration manifest. No hit in `scripts/`, `dev/scripts/`, `dev/tools/`, `crates/`,
  `contrib/`.

Consequence: column "pinned-and-CI-checked files that block a move" is 0 for every
entry. Every receipt pin is satisfied by the snapshot copy the receipt carries.

### 0.2 What CI does check that a move touches

`scripts/cargo-ci.sh:273-276` runs `check-family-producing-closures.py`, which runs
`--check` of every live `make-*-producing-inputs.py` that offers it
(`dev/scripts/check-family-producing-closures.py:32-58`). Live generators with
`--check` (`git ls-files ':(glob)**/make-*-producing-inputs.py'`):

| Generator | Entry | Moved by | Coupling |
|---|---|---|---|
| `dev/active/c04dd4ac-.../survey/make-shift-producing-inputs.py` | c04dd4ac | new task | Location-independent (`:16-22,49-51`). Its committed closure `shift-profile-producing-inputs.json` names 11 own paths; `--check` compares text, so the closure is regenerated at the move. The closure is not digest-pinned at HEAD. |
| `dev/active/2037941f-.../survey/make-logical-producing-inputs.py` | 2037941f | f79bcba3 | `:12` `parents[4]`, `:13` own literal, `:42,83` `dev/active/bc091474/...`, `:84-86` `dev/active/f547c394/...`; `:95-101` exits when a listed path is missing. |
| `dev/active/2037941f-.../survey/make-dense-producing-inputs.py` | 2037941f | f79bcba3 | `:19` own literal, `:100-102` `dev/active/f547c394/...`. |
| `dev/active/1d4fd63d/make-external-producing-inputs.py` | 1d4fd63d | f79bcba3 | No literal `dev/active/` path (`git grep` prints none). |

Both checks pass at HEAD (`python3 -B <generator> --check` exits 0 for the shift and
logical generators, run 01:14 local, tree unchanged).

`contrib/gates/docs-mechanical.py` (gate `docs-mechanical`, footprint
`dev/active/**/*.md`) checks relative Markdown links, so a dangling relative link in
a moved or sibling Markdown file is a gate finding.

### 0.3 Nothing else invokes a script of the eight entries

`git grep -lF <script name> -- scripts contrib .config .github .jit/config.toml dev/scripts dev/tools Cargo.toml crates`
prints a file only for `make-shift-producing-inputs` (the checker above). The root
workspace (`Cargo.toml:2-23`) holds no harness crate of these entries.

Bench-window queue: `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv`
lines 19, 20, 21, 31, 33-40 name scripts of 19513245, c04dd4ac, 00dd43c3 and
bc091474. Each runs in a worker worktree (`run-window.sh:76-78`); the named worktrees
`agent-19513245`, `agent-85fc5ff4`, `agent-9fb40c83`, `agent-9fb40c83-r4`,
`agent-00dd43c3`, `agent-bc091474` are absent from `.agents/worktrees/`, so the runner
logs `missing` and skips them. The job key hashes the command text
(`run-window.sh:71`), so the lines keep their bytes.

### 0.4 Precedent for editing a receipt-pinned script

Commit `18d31dec9` (jit:b0b1faf5, done, owner DEC-01) edited
`dev/active/c077a88b/survey/snapshot-inputs.py` and
`dev/active/f63a2464/survey/record-preparation.py`; the SHA-256 of their parent
versions occur in 16 and 34 tracked files (receipts, checkpoints, logs, snapshots).
The plan's byte-preservation list (`plan.md:70-72`: receipt directories,
preregistrations, addenda, plans, journals, schemas, continuation JSON, snapshot
copies) and the sibling criterion (f79bcba3 REQ-05: declarations, producing
manifests, addenda, receipts, preregistrations, snapshot copies) name no script.
`7bac1303-harness-path-exception.md:86-93` rejects one form because it changes the
pinned identity of scripts. The two readings conflict; see decision D2.

### 0.5 Manifest flag versus grep

`migration/manifest.toml` marks rows `digest_pinned = true` for files that grep finds
unpinned at HEAD, for example `dev/active/bc091474/run-candidate.sh` and
`dev/active/c04dd4ac-.../survey/make-shift-plan.py`. Those files are listed by path
in a producing closure and their earlier versions are in receipt snapshots; no tracked
file holds the digest of their HEAD bytes.

## 1. 00dd43c3 (15 files)

**Consumers**

| Consumer | Where | State at HEAD |
|---|---|---|
| `dev/active/c04dd4ac-.../survey/make-publication-tables.py:326,341,356,357` | outside | confirmed |
| `survey/freeze-confirmation.py:11`, `freeze-pilot.py:10`, `make-plan.py:7-8`, `make-producing-inputs.py:14`, `run-campaign.sh:22` | inside | confirmed |
| `survey/test_pilot_freeze.py:11,13,42` | inside | confirmed |
| `survey/freeze-confirmation.py:16` names `dev/active/c7113c5a/survey/freeze-confirmation.py` | inside, outbound | missing from the table; c7113c5a moves under 198aafa3 |
| `survey/make-producing-inputs.py:56`, `run-campaign.sh:48` name `c04dd4ac-.../survey/find-shift-executable.py` | inside, outbound | confirmed (listed under c04dd4ac) |
| `survey/producing-inputs.json` (11 own-path lines), `pilot-smoke.json`, `confirmation-smoke.json`, `confirmation-derivation.txt` | inside, data | provenance strings |
| `queue.tsv:31,40`, `1a379447-.../63bad95d/route-inventory.json`, 27 files under `dev/bench_results/00dd43c3` | outside, records | strings; keep bytes |

**Pins** `freeze-confirmation.py`, `freeze-pilot.py`, `make-plan.py`,
`make-producing-inputs.py`, `run-campaign.sh`, `verify-smoke.py`,
`producing-inputs.json`, both addenda and `profile-context.json`: receipt pins in
`dev/bench_results/00dd43c3/v4-r1-{pilot,confirmation}/receipt.json` plus snapshot
copies under their `inputs/`; satisfied by snapshots (0.1). `confirmation-outcome.md`,
`confirmation-derivation.txt`: SHA-256 in `c04dd4ac-.../publication-tables.md`
(generated table, no checker). `test_pilot_freeze.py`, both smoke JSON: unpinned.

**Runs** No invoker (0.3).

**Minimal change** Class (b) with D2: 6 files inside (5 receipt-pinned scripts and the
test), about 12 lines, to derive the root from `git rev-parse --show-toplevel` and the
entry directory from the script location (form of
`c04dd4ac-.../survey/make-shift-producing-inputs.py:16-22,49-50`), and to locate
`find-shift-executable.py` by name (`repository_files.live_file`; one live file of
that name). `freeze-confirmation.py:16` needs a lookup that tells three live
`freeze-confirmation.py` files apart (00dd43c3, 6fb89a3c, c7113c5a), or a path text
edit after 198aafa3. Mutual references with c04dd4ac: move both in one task. If the
entry moves with no edit, nothing that runs fails; the five launchers and generators
and the test stop working.

**Verification** `python3 -B dev/active/00dd43c3/survey/test_pilot_freeze.py`
(2 tests, OK at HEAD, no build). `make-plan.py` output before and after on fixed
arguments (the test's own call, `test_pilot_freeze.py:41-46`). Launchers: `bash -n`
only outside a window.

## 2. 19513245 (68 files)

**Consumers**

| Consumer | Where | State at HEAD |
|---|---|---|
| `survey/consumer/Cargo.toml:25-27`, `survey/field-laws/Cargo.toml:18,20` | inside | confirmed; `:25,26,18` name `../../../6c6b09b1/survey/...` (6c6b09b1 moves under 198aafa3), `:27,20` name `crates/gf2-core` at five levels |
| `survey/make-producing-inputs.py:23`, `make-source-evidence.py:27` | inside | confirmed |
| `survey/run-profile.sh:30` (depth), `:121` (literal through `ISSUE`) | inside | confirmed |
| `survey/make-producing-inputs.py:25` `dev/active/6c6b09b1/survey`, `freeze-confirmations.sh:28` `dev/active/c7113c5a/survey/freeze-confirmation.py` | inside, outbound | missing from the table |
| `survey/smoke-arms.sh:31`, `freeze-confirmations.sh:26`, `make-addenda.py:272`, `summarize-profile.py:67,176`, `disassemble.sh:10`, `msrv-intrinsics.rs:24` | inside | own-path literals built from `ISSUE` or printed text; not listed by the table's rule |
| `queue.tsv:19`, `63bad95d/route-inventory.json`, 90 files under `dev/bench_results/19513245` | outside, records | strings; keep bytes |

**Pins** Both `Cargo.toml`, both `Cargo.lock`, all `src/`, `make-addenda.py`,
`make-plan.py`, `make-producing-inputs.py`, `make-source-evidence.py`,
`smoke-arms.sh`, `producing-inputs.json`, `source-evidence.json`, six addenda and
five conformance records: receipt pins in six
`dev/bench_results/19513245/*/receipt.json` plus 15-18 snapshot copies each;
satisfied by snapshots. `run-profile.sh`, `disassemble.sh`,
`freeze-confirmations.sh`, `summarize-profile.py`, `asm/**`: SHA-256 in one
`host.txt` (profile record, no checker).

**Runs** No invoker.

**Minimal change** Class (a): the two manifests are in the scope of
`harness-path-dependencies` (the listing command of
`7bac1303-harness-path-exception.md:17` prints both), 5 path lines. After both
19513245 and 6c6b09b1 sit under `1a379447-zen3-cpu-performance/`, lines `:25,26,18`
have today's text again and only `:27,20` gain one level. If 198aafa3 moves 6c6b09b1
first, the exception rule (`7bac1303-...:24-27`) obliges that move to rewrite the
three lines; the 19513245 move rewrites them back. Scripts: `run-profile.sh:30` one
line to the runtime root; own-path literals in about 8 scripts, about 12 lines, of
which `make-producing-inputs.py` and `make-source-evidence.py` are receipt-pinned
(D2). If the entry moves with no edit: both harness crates fail to build, nothing in
CI fails.

**Verification** `./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path`
for the two manifests (compiled build). Verdict baseline
`5a25717c-verdicts.sh` (compiled `benchmark-acceptance`); by 0.1 it cannot change.

## 3. a203a23c (10 files)

**Consumers**

| Consumer | Where | State at HEAD |
|---|---|---|
| `timing/measure.py:99` `RESTORE_PATH`, read through `git show <revision>:<path>` at `:314,364,418` (callers at `:363,484,569,821`) and run in a clone at `:558` | inside | confirmed; the revisions are the constant `BATCH_REVISION` (`:96`) and its parent (`:896`) |
| `timing/measure.py:7,33,69,70,75,113,360,766,851,946,950`, `restore-receipt-inputs.py:9,18,20,32,116`, `select-receipts.py:13,14` | inside | own-path text and record strings |
| `dev/scripts/receipt-input-omissions.json:13,23,33,43,53` (`recorded_in`) | outside | missing from the table; the checker reads only `receipt` and `path` (`check-receipt-input-snapshots.py:285-287`) |
| `dev/scripts/check-receipt-input-snapshots.py:14` (docstring) | outside | missing from the table; citation |
| `1a379447-.../428f2f6b/inventory-c077a88b.json`, `timing/timing*.json`, `restored-inputs.json`, `receipt-reevaluation*.json` | records | strings; keep bytes |

**Pins** Every file unpinned (`reevaluate-receipts.sh` earlier version in
`7bac1303-harness-path-trials.txt` only). `receipt-input-omissions.json` and the
checker are unpinned.

**Runs** No invoker. `reevaluate-receipts.sh` already resolves by identity
(`:20-25`, commit `18d31dec9`). `measure.py` refuses outside a `--full-host` lock
(`:887-893`).

**Minimal change** Class (a): `restore-receipt-inputs.py:32,116` (2 lines) plus usage
text; 6 citation lines in `dev/scripts/`. `measure.py:99` stays as written: it is the
path of the tool at two recorded revisions, valid there after any move, and a rewrite
to the new path makes `git show <revision>:<path>` fail (D5). `receipt-input-omissions.json`
is also cited for `dev/active/c077a88b/findings.md` (198aafa3): shared file.

**Verification** `python3 -B dev/active/a203a23c/select-receipts.py --self-test`
(passes at HEAD, no build). `scripts/cargo-ci.sh` steps `receipt-input-snapshots` and
its self-test (Python).

## 4. bc091474 (19 files)

**Consumers**

| Consumer | Where | State at HEAD |
|---|---|---|
| `dev/active/2037941f-.../survey/make-logical-producing-inputs.py:42,83` | outside | confirmed; run by CI (0.2), exits on a missing path (`:95-101`) |
| `dev/active/2037941f-.../survey/logical-producing-inputs.json:217,247,578,579` | outside, data | missing from the table; CI compares it with the generator output |
| `run-candidate.sh:9` (`${HERE}/../../..`) | inside | table says `:8,100`; at HEAD the depth is at `:9` and the file holds no own-path literal |
| `make-candidate-tables.py:10-11` | inside | confirmed; also `:109` and emitted relative links at `:127,128,159,161,180,203-205` |
| `portfolio.md`: 10 relative links leaving the entry | inside | missing from the table |
| `candidate-tables.md` 14, `pilot-outcome.md` 9, `readiness.md` 3, `unroll-variant-corrections.md` 1 relative links leaving the entry | inside | ordinary repoints |
| `2037941f-.../mid-range-tables.md` (42 hits), `1a379447-.../fa1d8733/survey/flag-readers.json`, `b2e09d41/plan-checks.tsv`, `queue.tsv:33-38`, 32 files under `dev/bench_results/2037941f` | outside | citations and records |

**Pins** `portfolio.md`: receipt pin in four `dev/bench_results/2037941f/*/receipt.json`
plus 8 snapshot copies; also SHA-256 in `mid-range-tables.md`. `run-candidate.sh`,
`run-candidate.test.sh`, `make-candidate-tables.py`: unpinned at HEAD (0.5).
`make-logical-producing-inputs.py`: receipt pin in the same four receipts plus 8
snapshot copies; satisfied by snapshots. `logical-producing-inputs.json`: no receipt
pin, SHA-256 cited in `mid-range-tables.md`. `candidate-tables.md`,
`pilot-outcome.md`, `readiness.md`, three `asm/` files: SHA-256 in
`mid-range-tables.md:403,481` and neighbours (generated by
`2037941f-.../survey/make-mid-range-tables.py`, no invoker).

**Runs** `make-logical-producing-inputs.py --check` runs in every
`./scripts/cargo-ci.sh`. If bc091474 moves with no edit there, CI fails with
`producing inputs are missing`.

**Minimal change** Class (b): `make-logical-producing-inputs.py:42,83` locate
`run-candidate.sh` and `portfolio.md` through `repository_files.live_file` (one live
file of each name), 2 to 6 lines; the closure JSON is regenerated at the move
(4 lines). `run-candidate.sh:9` and `make-candidate-tables.py:10-11`: 3 lines to the
runtime root. The generator belongs to an entry f79bcba3 moves, and its `:12,13`
depth and own literal need the same treatment there. Link repoints: 27 links in four
unpinned Markdown files; `portfolio.md` is D3; the digest table is D4.

**Verification** `bash dev/active/bc091474/run-candidate.test.sh` (14 `ok` lines and no `FAILED` line at
HEAD, no build). `python3 -B .../make-logical-producing-inputs.py --check` (CI step,
Python). `docs-mechanical` gate for links.

## 5. bdc507a3 (2 files)

**Consumers** `dev/active/f547c394/amendment-v4.md:27`
(`../bdc507a3/verdict-preservation.json`), outside, confirmed.
`compare-acceptance-verdicts.sh:4,6,112,134`, inside, own-path text (the table lists
`:112,134` only in the unit table). `verdict-preservation.json:5` records the
generator path.

**Pins** `amendment-v4.md`: receipt pin in 12 `receipt.json` plus 25 snapshot copies;
12 snapshot copies hold the link text. Both bdc507a3 files unpinned
(`7bac1303-harness-path-trials.txt:262` records `pinned_in=0`).

**Runs** No invoker. The link is checked by the `docs-mechanical` gate.

**Minimal change** Class (d). f547c394 goes under epic 1a379447
(`23219502-inventory-notes.md:4`), bdc507a3 too, so the link text resolves again once
both have moved; between the two moves the gate reports one unresolved link in a file
that keeps its bytes. Script: 4 path text lines.

**Verification** `docs-mechanical` gate. The script needs `benchmark-acceptance` at
two revisions; `bash -n` otherwise.

## 6. c04dd4ac-zen3-shifts-and-permutations (71 files)

**Consumers**

| Consumer | Where | State at HEAD |
|---|---|---|
| `dev/active/eda07788/survey/gf2-side/tests/profile_addendum.rs:17,19,21,251` | outside | confirmed; `:15` `CARGO_MANIFEST_DIR/../../../../..` and `:16` `dev/active/f547c394/...` break under 7039d680 and 602652bf as well |
| `dev/active/00dd43c3/survey/make-producing-inputs.py:56`, `run-campaign.sh:48` | outside | confirmed |
| `survey/shift-profile-producing-inputs.json` (11 own-path lines), checked by CI | inside, data | missing from the table |
| 8 receipt-pinned scripts: `build-dvb-harness.sh:6,7,43`, `freeze-shift-addendum.py:9`, `inspect-shift-consumers.py:10,14`, `make-dvb-addendum.py:18,21`, `make-producing-inputs.py:7-9,61,66-68`, `make-source-evidence.py:8,10,17,18`, `run-dvb-campaign.sh:12,18-22`, `run-profile.sh:12` | inside | confirmed |
| 10 unpinned scripts: `make-shift-plan.py:12,16`, `run-shift-profile.sh:24`, `make-profile-provenance.py:38-46,151,211`, `make-publication-tables.py:17,18,326,341,356,357,362`, `make-shift-source-evidence.py:15,17`, `make-voided-attempt.py:14,22`, `make-voided-attempt-record.py:18`, `smoke-dvb-arms.sh:10,22`, `smoke-shift-arms.sh:22`, `summarize-shift-receipt.py:27` | inside | the table lists `make-shift-plan.py`, `make-shift-producing-inputs.py:9` and `run-shift-profile.sh` as digest-pinned; grep finds them unpinned, and `make-shift-producing-inputs.py` holds no literal |
| Outbound: eda07788 (`build-dvb-harness.sh:7`, `make-producing-inputs.py:9`, `make-source-evidence.py:17,18`, `make-profile-provenance.py:45,46,211`, `make-publication-tables.py:362`), f547c394 (`make-producing-inputs.py:66,67`), 1a379447 directory (`make-producing-inputs.py:68`, `make-profile-provenance.py:43`) | inside | missing from the table |
| `queue.tsv:20,21,39`, `63bad95d/*.json`, 94 files under four `dev/bench_results/` directories, 12 Markdown files | outside | citations and records |

**Pins** The 8 scripts, `dvb-producing-inputs.json`, `dvb-source-evidence.json`,
`find-shift-executable.py`, `make-dvb-plan.py`, both addenda, both ledgers, three
shift-profile JSON: receipt pins under `dev/bench_results/c04dd4ac`,
`dev/bench_results/85fc5ff4`, `dev/bench_results/00dd43c3` plus snapshot copies;
`run-profile.sh`, `profile-cases.txt`, `harness-validation.txt`,
`render-hot-instructions.sh`: `profile-inputs.sha256` and two provenance records.
All satisfied by committed copies or unchecked. `shift-profile-producing-inputs.json`
and `profile_addendum.rs`: unpinned.

**Runs** `make-shift-producing-inputs.py --check` in every CI run; after a move with
the closure bytes kept, CI fails with a path difference. No other invoker;
`profile_addendum.rs` belongs to a harness crate outside the workspace and
`cargo build` does not compile it.

**Minimal change** Class (b): 18 scripts, about 50 lines, to the runtime root and the
script location (8 of them D2); `profile_addendum.rs` 5 lines to
`tuning_campaign_support::repository::repository_root` and name lookups (shared file
with 7039d680); regenerate `shift-profile-producing-inputs.json` at the move.
`survey/residual-shift-feasibility/Cargo.toml` has no relative path dependency.
Markdown: 70 relative links leaving the entry in 8 unpinned files.

**Verification** `make-shift-producing-inputs.py --check` (Python, CI step);
`python3 -B .../survey/test-profile-intervals.py` (3 tests, OK at HEAD);
generator outputs before and after for the Python generators that need no build;
`cargo +1.95.0 test --manifest-path dev/active/eda07788/survey/gf2-side/Cargo.toml --test profile_addendum`
(compiled; needs the external `xdsopl` checkout the crate links, see
`build-dvb-harness.sh:13-32`).

## 7. f8dd4dde (6 files)

**Consumers** `msrv-validation/Cargo.toml:23` (inside, confirmed);
`validate-msrv.sh:4,11,13` own-path text; `validate-msrv-record.json:51` recorded
command; `retention-rule.md` 5 relative links leaving the entry.

**Pins** All unpinned (`validate-msrv.sh` in `7bac1303-harness-path-trials.txt` only).

**Runs** No invoker.

**Minimal change** Class (a): the manifest is in the scope of
`harness-path-dependencies` (listing command prints it); 1 manifest line, 3 script
lines. No decoupling task is needed.

**Verification** `./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path
<new>/msrv-validation/Cargo.toml` (compiled).

## 8. 4e732b56 (18 files, epic ae03bcd0)

**Consumers** All six rows of the eace5009 table confirmed at the stated lines
(`Makefile:10`, `fetch-build.sh:20`, `run-survey.sh:25`, `gf2-side/Cargo.toml:16-17`,
`verify-generator-matrices.py:29`, `verify-generators.py:27`), inside.
`make-receipt.py:264-266` own-path text. Outside, citations only:
`dev/active/04b85d10/survey/profile-cases.py:12`, `summarize-profile.py:159`,
`gf2-side/src/lib.rs:22,510,534`, `code-rows.json:5`;
`dev/active/6fb89a3c/survey/verify-bit-mapping.py:125`; `dev/active/d1b4f85e/run.sh:15`;
`dev/active/f547c394/research-r3-sweeps.json:4402`; 14 Markdown files.

**Pins** Every file of the entry unpinned. The citing `04b85d10` and `6fb89a3c` files
are receipt-pinned or `host.txt`-pinned and keep their bytes.

**Runs** No invoker.

**Minimal change** Class (b) for 5 files, 5 lines (`git rev-parse --show-toplevel`
in the three shell and Make files, the same through `subprocess` in the two Python
files); class (a) for `gf2-side/Cargo.toml` (in the exception's listing), 2 lines;
3 own-path lines in `make-receipt.py`.

**Verification** `python3 -B .../verify-generators.py dev/bench_results/4e732b56/generators.txt`
(prints two `identical=True` rows at HEAD, no build); `make -n` for the Makefile
variable; `cargo +1.95.0 build --manifest-path .../gf2-side/Cargo.toml` (compiled).

## Task proposal

Four tasks and one amendment cover the eight entries.

### T1: Decouple and move the 4e732b56 baseline survey under the general BCH epic dir

Moves: 4e732b56. Dependencies: none. Shares citation lines with f79bcba3 (04b85d10),
198aafa3 (6fb89a3c) and 602652bf (`f547c394/research-r3-sweeps.json`); those files
are pinned or records and keep their bytes, so no file is shared.

- [hard] REQ-01: `Makefile`, `fetch-build.sh`, `run-survey.sh`, `verify-generators.py` and `verify-generator-matrices.py` resolve the repository root at runtime, with no path that counts directory levels.
- [hard] REQ-02: Before the change a linked record holds the output of `verify-generators.py` on `dev/bench_results/4e732b56/generators.txt` and of `make -n -p` for `EXT`, with command and commit; after the change and after the move the outputs are identical.
- [hard] REQ-03: The entry lives under the canonical directory of epic ae03bcd0, and the move commit changes no consumer code apart from the two Cargo path dependencies of `gf2-side/Cargo.toml` (path text only, exception `harness-path-dependencies`) and the three own-path literals of `make-receipt.py`; the commit message records each edit.
- [hard] REQ-04: `gf2-side` builds on Rust 1.95 after the move.
- [hard] REQ-05: Human-facing citations are repointed, digest-pinned files keep their bytes, and a link scan of the touched files reports no unresolved local link.
- [hard] REQ-06: Each tracker document reference names the new path, the manifest records each moved row as complete, and the Rust CI gate passes.

### T2: Move entries f8dd4dde, a203a23c and 19513245 under the zen3 epic dir

Moves: f8dd4dde, a203a23c, 19513245. Dependency: 198aafa3 (moves 6c6b09b1 and
c7113c5a, which 19513245 names; shares `dev/scripts/receipt-input-omissions.json`
through its c077a88b citation). f8dd4dde and a203a23c have no dependency and can be
split off if 198aafa3 is late.

- [hard] REQ-01: The three entries live under the canonical directory of epic 1a379447.
- [hard] REQ-02: `19513245/survey/run-profile.sh` resolves the repository root at runtime; a linked record shows the resolved root equal before and after the change.
- [hard] REQ-03: Each harness crate of a moved entry (`f8dd4dde/msrv-validation`, `19513245/survey/consumer`, `19513245/survey/field-laws`) builds on Rust 1.95 after the move; the move rewrites its relative Cargo path dependencies (path text only) under `harness-path-dependencies`, and the commit message records each edit.
- [hard] REQ-04: Scripts in each moved entry that name the entry's own directory, or the directory of 6c6b09b1 or c7113c5a, by path literal are updated to the path at the move (path text only), and each edit is recorded per entry in the commit message.
- [hard] REQ-05: `a203a23c/timing/measure.py` keeps `RESTORE_PATH`, the path of the tool at the revisions the script names; the commit message states it. (D5)
- [hard] REQ-06: `python3 -B <entry>/select-receipts.py --self-test` passes after the move, and the `receipt-input-snapshots` CI step prints the same summary line as before.
- [hard] REQ-07: Digest-pinned inputs (declarations, producing manifests, addenda, receipts, preregistrations and snapshot copies) keep their bytes; human-facing citations, the five `recorded_in` strings of `dev/scripts/receipt-input-omissions.json` and the docstring citation of `dev/scripts/check-receipt-input-snapshots.py` are repointed; a link scan of the touched files reports no unresolved local link.
- [hard] REQ-08: Each tracker document reference names the new path, the manifest records each moved row as complete, and the Rust CI gate passes.

### T3: Decouple and move the residual-shift entries 00dd43c3 and c04dd4ac

Moves: 00dd43c3, c04dd4ac-zen3-shifts-and-permutations (mutual references).
Dependencies: 7039d680 (moves eda07788; shared file
`eda07788/survey/gf2-side/tests/profile_addendum.rs`), 602652bf (f547c394 named at
`make-producing-inputs.py:66,67`), 198aafa3 (c7113c5a named at
`00dd43c3/survey/freeze-confirmation.py:16`).

- [hard] REQ-01: Each script of the two entries listed in the linked record resolves the repository root at runtime and its entry directory from its own location, and locates a file of another entry by name or content through `dev/scripts/repository_files.py`; no path counts directory levels and no literal names `dev/active/`.
- [hard] REQ-02: `profile_addendum.rs` locates the repository root, the addendum schema, the addendum, the producing manifest, the cases and the plan tool without a literal `dev/active/` path or a path relative to `CARGO_MANIFEST_DIR`.
- [hard] REQ-03: Before the change a linked record holds, with command and commit: the output of `test_pilot_freeze.py`, of `test-profile-intervals.py`, of `make-shift-producing-inputs.py --check`, and of each Python generator that runs without a build on committed inputs; after the decoupling the outputs are identical, and after the move they differ only in the moved paths.
- [hard] REQ-04: The two entries live under the canonical directory of epic 1a379447, and the move commit changes no consumer code.
- [hard] REQ-05: `shift-profile-producing-inputs.json` is the output of its generator at the new location, and the `family-producing-closures` CI step passes.
- [hard] REQ-06: Digest-pinned inputs (declarations, producing manifests other than the one of REQ-05, addenda, receipts, preregistrations and snapshot copies) keep their bytes; human-facing citations are repointed, and a link scan of the touched files reports no unresolved local link.
- [hard] REQ-07: The benchmark-acceptance run recorded in the receipt verdict baseline, repeated after the move, yields identical verdicts and findings.
- [hard] REQ-08: Each tracker document reference names the new path, the manifest records each moved row as complete, and the Rust CI gate passes.

REQ-01 depends on D2; the digest column of `publication-tables.md` depends on D4.

### T4: Decouple the logical closure generator and move bc091474

Moves: bc091474. Dependency: f79bcba3 (moves 2037941f; shared files
`2037941f-.../survey/make-logical-producing-inputs.py`, `logical-producing-inputs.json`,
`mid-range-tables.md`).

- [hard] REQ-01: `make-logical-producing-inputs.py` locates `run-candidate.sh` and `portfolio.md` by name through the runtime root; `run-candidate.sh` and `make-candidate-tables.py` resolve the repository root at runtime and the entry directory from their own location.
- [hard] REQ-02: Before the change a linked record holds the output of `run-candidate.test.sh` and of `make-logical-producing-inputs.py --check` with command and commit; after the decoupling both are identical and `logical-producing-inputs.json` keeps its bytes.
- [hard] REQ-03: The entry lives under the canonical directory of epic 1a379447, and the move commit changes no consumer code; `logical-producing-inputs.json` is the output of its generator after the move and differs from its parent only in the four bc091474 paths.
- [hard] REQ-04: `run-candidate.test.sh` passes after the move and the `family-producing-closures` CI step passes.
- [hard] REQ-05: Human-facing citations are repointed and a link scan of the touched files reports no unresolved local link. (`portfolio.md`: D3; digest table: D4.)
- [hard] REQ-06: Each tracker document reference names the new path, the manifest records each moved row as complete, and the Rust CI gate passes.

### A1: Amend 602652bf to move bdc507a3 with f547c394

Add bdc507a3 to the entries of 602652bf REQ-01 and state in REQ-06 that
`amendment-v4.md` keeps its bytes and its link `../bdc507a3/verdict-preservation.json`
resolves after the move. REQ-08 already covers
`compare-acceptance-verdicts.sh:4,6,112,134`. (D1)

## Owner decisions

- **D1 (bdc507a3).** Options: (a) amend 602652bf to move both entries in one commit; (b) a separate task that lands in the same commit as 602652bf; (c) move separately and baseline one `docs-mechanical` finding between the two commits. Recommendation: (a). A separate task cannot satisfy the gate on its own commit.
- **D2 (receipt-pinned scripts: 5 in 00dd43c3, 2 in 19513245, 8 in c04dd4ac, plus `make-logical-producing-inputs.py`).** Options: (a) decoupling edits change their bytes, as commit `18d31dec9` did; (b) they keep their bytes and stop working at the new depth; (c) path text edits only at the move, which keeps the depth-dependent form. Recommendation: (a). No check reads the live bytes (0.1), and for `make-logical-producing-inputs.py` CI forces the edit. Under (b) T3 shrinks to the unpinned scripts and the two entries still move.
- **D3 (`bc091474/portfolio.md`).** Receipt-pinned Markdown with 10 relative links that leave the entry. Options: (a) repoint the link targets, bytes change, receipts stay satisfied by snapshots; (b) keep bytes and baseline 10 `docs-mechanical` findings. `plan.md:70-78` supports both. Recommendation: (a).
- **D4 (digest tables).** `c04dd4ac-.../publication-tables.md` and `2037941f-.../mid-range-tables.md` state SHA-256 of Markdown files whose links the moves repoint. Options: (a) regenerate each table with its generator in the moving task; (b) leave the digests and record them as pre-move. No check reads them. Recommendation: (a) where the generator runs without a build, else (b) with the record.
- **D5 (`a203a23c/timing/measure.py:99`).** Options: (a) keep the literal as a revision-qualified path; (b) replace it with blob ids of the tool at each revision, which changes a timing harness that only a `--full-host` window can re-run. Recommendation: (a), stated in T2 REQ-05.

## Hazards in the sibling issues

- f79bcba3 moves 2037941f. `make-logical-producing-inputs.py:12,13` and `make-dense-producing-inputs.py:19` hold depth and own-path forms, and CI runs both with `--check`; the move fails CI unless both generators change and both closures are regenerated. f79bcba3 REQ-05 says producing manifests keep their bytes.
- 602652bf moves f547c394. `make-logical-producing-inputs.py:84-86` and `make-dense-producing-inputs.py:100-102` name `dev/active/f547c394/...`; the move fails the same CI step. 602652bf REQ-03 lists neither generator.
- 198aafa3 moves 6c6b09b1 and c7113c5a; `19513245/survey/consumer/Cargo.toml:25,26`, `field-laws/Cargo.toml:18`, `19513245/survey/make-producing-inputs.py:25`, `freeze-confirmations.sh:28` and `00dd43c3/survey/freeze-confirmation.py:16` name them.
- 7039d680 moves eda07788; `profile_addendum.rs:15` counts levels from `CARGO_MANIFEST_DIR`, and `cargo build` (its REQ-04) does not compile the test.
