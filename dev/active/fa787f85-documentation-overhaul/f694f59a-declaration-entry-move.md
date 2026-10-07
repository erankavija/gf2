# f694f59a a83583e0 declaration entry move record

Move, path strings and citations in `a0ebe2884`; the repeat validator run
enters with this record. `E` is
`dev/active/6dc81018-field-capability-dispatch/a83583e0`, `F` the flat entry
path `dev/active/a83583e0`.

## Criteria

| Criterion | Evidence | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | `jit issue show a83583e0 --json` on main prints state `done`; the message of `a0ebe2884` records it | The owning issue is terminal when the move starts | — |
| REQ-02 | `jit doc dir 6dc81018 dev/active` prints the directory above `E`; `git ls-files F` is empty | Each file of the entry is under the epic directory | — |
| REQ-03 | `git show --stat a0ebe2884 -- '*.rs' '*.py' '*.sh'` lists `crates/gf2-algebra/benches/tuning_calibration.rs` alone, one doc-comment line | The commit changes no line of the extent driver, validator or launcher | — |
| REQ-04 | Section "With entry a83583e0 under the epic directory" of [the verdict baseline](../../archive/6dc81018-field-capability-dispatch/active/fa787f85-documentation-overhaul/tuning-extent-verdict-baseline.md) | Both campaigns keep the publication verdict and entry count of the earlier sections, with the dbd8787d declaration at its flat path | The verdict of `--stage` without `--publication`: it exits 2 for the a83583e0 stage with the validator of the main checkout and with that of `a0ebe2884`, with different first messages |
| REQ-05 | `git ls-files E` | The entry holds no script | — |
| REQ-06 | `git diff -M --stat 3f6b03a99 a0ebe2884 -- F E`: `campaign-declaration.json:4,5` (`protocol`, `producing_manifest`) are the changed lines, each other file is a rename without a changed line. `git grep -F` of the SHA-256 and the blob id of the declaration at `3f6b03a99` finds no line in the tree or `.jit/` | Path text only; no digest pins the edited file | Resolution of the links in the fifth table of [8f61d6de-baseline-findings.md](8f61d6de-baseline-findings.md): the three files keep their bytes and `python3 contrib/gates/docs-mechanical.py` passes with those rows suppressed |
| REQ-07 | `jit doc list a83583e0` on main: each reference names an archive path or carries a commit pin. `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` exits 0 with each row of the entry `complete` | No tracker reference changes | The Rust CI verdict; `cargo-ci` runs it |

## Checks

| Check | Command | Result |
|---|---|---|
| Unmerged work | `git diff --stat main...<branch> -- F` and the repointed files, for each branch of `git worktree list` | no diff line |
| Campaign pins | `grep -n a83583e0` over the declaration and producing manifest of 63bad95d and over `bench-window/queue.tsv` | the 63bad95d declaration names the two published a83583e0 profiles, which stay; the queue line of a83583e0 runs in its own worktree |
| CI steps | `CARGO_CI_STEPS='^(tuning-campaign-validator\|campaign-producing-closure\|family-producing-closures\|tuning-campaign-records\|receipt-input-snapshots\|tuning-campaign-launcher-syntax\|tuning-campaign-support-nextest\|tuning-algebra-calibration-nextest\|fmt)$' ./scripts/cargo-ci.sh` | each step ok |
| Archive preview | `jit archive container 6dc81018 --json` in the worktree, before and after the append to the verdict baseline | eligible, no blocker, equal action counts; the worktree tracker is a snapshot |

## Files that keep the flat path

`git grep -lF F` outside `.jit/` hits these files only.

| File | Ground |
|---|---|
| `dev/bench_results/**/inputs/**` | `inputs/` snapshots |
| `dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md`, `-evidence.md` | receipt and its evidence index |
| `dev/archive/**` | marker-backed archive |
| `E/failed-attempts/gf2-a83583e0-20260928T130755Z-3719910/execution.log` | failed-attempt log |
| `616e1d7c-preexec.json` | digest pin: content hash in `.jit/issues/616e1d7c-1e65-45f0-91d4-2eb194746882.json` |
| `616e1d7c-verify.txt`, `67048b47-linked-pairs.txt`, `96cea1b9-archive-rows.txt`, `fa4939c3-comment-read.txt` | tool output at `fc571c890`, `d5b3d8b4b`, `bedcd932f`, `d70c2e883` |
| `616e1d7c-regroup.md:44,61` | quotes the `excluded` section of `616e1d7c-verify.txt` and the commit-pinned references of a83583e0 |
| `investigation.md:119` | investigation of the tree at `fca6df53d`, the commit its header names |
| `migration/manifest.toml` | `path`, `bundle`, `consumers` and `inbound` fields name the source by schema |
| `breakdown.json`, `progress.json` | tracker state: an issue description, a touched-path list and escalation questions |
| `../6dc81018-field-capability-dispatch/handoff-12.md:41` | tracker state: the reference of a83583e0 to its producing manifest is pinned at `ecd8531` |
| `findings.md`, `research-r3-sweep-resolutions.md`, `research-r3-sweeps.json`, `rework-validation.md` under `../1a379447-zen3-cpu-performance/f547c394/`; `handoff.md` and `reviews/f547c394-r2.md` of that epic directory | no ground: another session edits that epic directory |
