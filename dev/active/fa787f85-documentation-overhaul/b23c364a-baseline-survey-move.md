# Baseline survey entry: root resolution and move (b23c364a)

`E` is the directory of the entry of issue 4e732b56 and `S` is
`E/baseline-survey`.

## Captures (REQ-02)

Both commands run from the repository root.

| Capture | Command | Output |
| --- | --- | --- |
| Generator check | `python3 -B S/verify-generators.py dev/bench_results/4e732b56/generators.txt` | [`b23c364a-verify-generators.txt`](b23c364a-verify-generators.txt) |
| Makefile variable `EXT` | `realpath -m "$(make --no-print-directory -C S --eval='b23c364a-ext: ; @echo $(EXT)' b23c364a-ext)" \| sed "s#$(git rev-parse --show-toplevel)#<root>#"` | [`b23c364a-ext.txt`](b23c364a-ext.txt) |

The `--eval` goal echoes the variable and runs no target of the Makefile.
`realpath -m` reduces `..` segments without reading the file system, and `sed`
replaces the checkout root, so the capture holds the location `EXT` names and
not the spelling of the assignment.

| State | Commit | `E` | `diff` of each capture against its committed file |
| --- | --- | --- | --- |
| Baseline | `4dfbab9ce` | `dev/active/4e732b56` | the committed files are these outputs |
| Root resolved at run time | `d99814285` | `dev/active/4e732b56` | empty, both |
| Entry under the epic directory | `fa33fdf2b` | `dev/active/ae03bcd0-general-bch/4e732b56` | empty, both |

The generator check exercises `REPO` of `verify-generators.py` only, and the
`EXT` capture exercises the Makefile only.

## Root resolution (REQ-01)

| File of `S` | Root |
| --- | --- |
| `Makefile` | `$(shell git -C $(CURDIR) rev-parse --show-toplevel)` |
| `fetch-build.sh`, `run-survey.sh` | `git -C "${HERE}" rev-parse --show-toplevel`, `HERE` being the script's directory |
| `verify-generators.py`, `verify-generator-matrices.py` | the same git command through `subprocess`, run in the script's directory |

`dev/scripts/repository_files.py` holds the shared `repository_root`. A script
of the entry reaches that module only through a path relative to its own
directory or through the root the function returns, so the two Python files
call git directly.

| Check | Result | Establishes | Does not establish |
| --- | --- | --- | --- |
| `( HERE="$PWD/S"; eval "$(grep '^REPO=' S/<script>)"; [ "$REPO" = "$PWD" ] )` for the two shell scripts, from the root | exit 0, both | `REPO` is the checkout root | Any fetch, build or timed run of the scripts |
| `python3 -B -c "import runpy,sys; print(runpy.run_path(sys.argv[1], run_name='x')['REPO'])" S/verify-generator-matrices.py` | the checkout root | `REPO` of that script | Its comparison, which needs the two built binaries |
| `git grep -nE '\.\./\.\.\|parents\[' -- S` | the two path dependencies of `gf2-side/Cargo.toml` only | No other depth-counted path in `S` | |

No digest of the five files appears in the tree outside `.jit/` as a pin
(`git grep -nF` of each SHA-256 and each blob id).
`7bac1303-harness-path-trials.txt` lists the SHA-256 of `fetch-build.sh` at the
commit of that output with `pinned_in=0`.

## Move (REQ-03)

`jit doc dir ae03bcd0 dev/active` prints `dev/active/ae03bcd0-general-bch`.
Commit `fa33fdf2b` renames every file of the entry into
`dev/active/ae03bcd0-general-bch/4e732b56/`; its message lists the five edited
lines. `git show -M --stat fa33fdf2b` shows changed lines in
`gf2-side/Cargo.toml` and `make-receipt.py` only.

## Build on Rust 1.95 (REQ-04): not met

| Command | Result | Establishes | Does not establish |
| --- | --- | --- | --- |
| `./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path S/gf2-side/Cargo.toml` at `fa33fdf2b` (`rustc 1.95.0 (59807616e 2026-04-14)`) | ``error: could not compile `survey-gf2-side` (bin "survey-gf2-side") due to 4 previous errors`` | `gf2-core` and `gf2-coding` compile as path dependencies; `src/main.rs` does not compile against `gf2-coding` | |
| The same build on `git archive fd09f908b` of `Cargo.toml`, `Cargo.lock`, `.cargo`, `crates` and the `gf2-side` directory, extracted under `target/` | the same four errors | The failure is independent of the root resolution and of the move | |
| `cargo +1.95.0 metadata --locked --offline --no-deps --format-version 1 --manifest-path S/gf2-side/Cargo.toml` at `fa33fdf2b` | the two path dependencies resolve to `<root>/crates/gf2-coding` and `<root>/crates/gf2-core` | The two edited paths name the workspace crates | That the binary builds |

The four errors are `E0432` (`gf2_coding::bch::BchEncoder`), `E0107`
(`BchCode` without generics) and two `E0599` (`BchCode::dvb_t2`,
`BchCode::new`), at `src/main.rs:23,107,109,110`. Commit `9591b0674` (issue
4a2baa12) deletes that code surface from `gf2-coding`. `src/main.rs` keeps its
bytes in this unit: it is the gf2 arm of the survey receipts under
`dev/bench_results/4e732b56/`. The build also adds a `blake3` line to
`gf2-side/Cargo.lock`; the committed lock file keeps its bytes.

## Citations (REQ-05)

Repointed, path text only, in commit `docs(jit:b23c364a): repoint citations
of the baseline survey`:

| File | Edit |
| --- | --- |
| `findings.md` and `workload-selection.md` of the entry | own-path citations; relative link targets to the epic plan, `dev/bench_results/` and `dev/archive/` at the entry's depth |
| `dev/active/04b85d10/findings.md` | citation of the workload-selection contract |
| `dev/active/ae03bcd0-general-bch/` `handoff.md`, `handoff-2.md` to `handoff-5.md`, `session-10-prompts/dispatch-591a1c5e.md`, `session-10-prompts/dispatch-97410c80.md` | citations of the entry |
| `dev/active/f547c394/research-r3-sweep-resolutions.md`, `research-r3-sweeps.json` (JSON string; the file parses) | location `workload-selection.md:142`, which holds the cited sentence |
| `ec655592-dev-plans-removal.md` | location `findings.md:34` |

Hits of `dev/active/4e732b56` that keep their bytes:

| Location | Ground |
| --- | --- |
| `dev/active/04b85d10/survey/code-rows.json` | SHA-256 pinned at `dev/bench_results/1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-confirmation/inputs/producing/producing-snapshot.json:1` |
| `dev/active/04b85d10/survey/gf2-side/src/lib.rs` | SHA-256 pinned at `dev/bench_results/04b85d10/2026-09-13-04b85d10-count-v4-pilot/checkpoints/manifest.json:1` |
| `dev/active/04b85d10/survey/profile-cases.py` | SHA-256 pinned at `dev/bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/host.txt:152` |
| `dev/active/04b85d10/survey/summarize-profile.py` | SHA-256 pinned at `dev/bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/host.txt:164` |
| `dev/active/6fb89a3c/survey/verify-bit-mapping.py` | SHA-256 pinned at `dev/bench_results/6fb89a3c/v3-r1-6fb89a3c-bch-confirmation/checkpoints/manifest.json:1` |
| Files under `dev/bench_results/` | Receipts and their `inputs/` snapshots |
| `67048b47-linked-pairs.txt` | Committed tool output at `d5b3d8b4b` |
| `7bac1303-harness-path-trials.txt` | Committed tool output at `3e3fadcdd` |
| `migration/manifest.toml`, `path` and `consumers` fields | The fields name the source by schema |
| `eace5009-regroup.md` | Its consumer table describes the entry at the commit of that record |
| `plan-review-r1.md:171` | Review finding on Rustdoc citations in `gf2-coding`; no Rust source under `crates/` cites the entry |
| `dev/active/ae03bcd0-general-bch/handoff-8.md:49,50`, `dev/active/d1b4f85e/run.sh:15` (comment) | Branch `worktree-agent-7dbcde0c` holds unmerged commits on both files |
| This record | It names the pre-move path |
| `.jit/issues/` | Tracker state; `target/b23c364a-relink.sh` repoints the references |

`dev/active/fd9d5416/receipt-protocol.md:7,154` links the entry as
`../4e732b56/`. The file is outside this unit's write scope, and branch
`worktree-agent-7dbcde0c` moves it to `dev/active/ae03bcd0-general-bch/fd9d5416/`
and rewrites the two targets to `../../4e732b56/`. At that location the target
that resolves to the entry is `../4e732b56/`.

No file of the entry is digest-pinned: `git grep -nF` of the SHA-256 and of the
blob id of each edited file, outside `.jit/`, finds the `pinned_in=0` line of
`7bac1303-harness-path-trials.txt` only.

## Tracker and manifest (REQ-06)

Issue 4e732b56 references each file of the entry without commit pin;
`target/b23c364a-relink.sh` adds each reference at its path under the epic
directory with its type and label, removes the pre-move reference, and rescans
the tracked documents of the first table above. Each manifest row whose `path`
lies under `dev/active/4e732b56/` has `status = "complete"`; its `destination`
is the path under the epic directory.

## Verification

| Command | Result | Establishes | Does not establish |
| --- | --- | --- | --- |
| `python3 contrib/gates/docs-mechanical.py` | FAIL on `dev/active/fd9d5416/receipt-protocol.md:7,154` only | Every other inline link of the Markdown files under `dev/active/`, the moved and the touched ones included, resolves | Code-span paths, anchors |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding names a row of this unit | Each row of the entry is `complete`, its destination exists and its source is absent | Rows of other units |
| `bash -n` on the two shell scripts, `python3 -m py_compile` on the three edited Python files, byte code written under `target/` | exit 0 | Syntax | Behavior beyond the captures above |
| `python3 -B -c "import json,sys; json.load(open(sys.argv[1]))" dev/active/f547c394/research-r3-sweeps.json` | exit 0 | The repointed JSON parses | |
