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
| Merge of main and of the BCH entries branch | `c0784d1d5` | the same | empty, both |

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

## Path dependencies and build on Rust 1.95 (REQ-04)

`M` is the path of `gf2-side/Cargo.toml` at the stated commit; the toolchain is
`rustc 1.95.0 (59807616e 2026-04-14)`. The baseline rows run in an extraction
of `git archive 4dfbab9ce Cargo.toml Cargo.lock .cargo crates scripts <gf2-side>`.

| Command | Commit | Result |
| --- | --- | --- |
| `./scripts/cargo-budget.sh cargo +1.95.0 metadata --manifest-path M --format-version 1` | `4dfbab9ce` | exit 0; `gf2-coding` and `gf2-core` resolve to `<extraction>/crates/` |
| the same | `c0784d1d5` | exit 0; `gf2-coding` and `gf2-core` resolve to `<root>/crates/` |
| `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path M` | `4dfbab9ce` | exit 101; first error ``error[E0432]: unresolved import `gf2_coding::bch::BchEncoder` `` |
| the same | `c0784d1d5` | exit 101; the same first error |

Each build compiles `gf2-core` and `gf2-coding` as path dependencies and stops
in `src/main.rs` with the same four errors at lines 23, 107, 109 and 110:
`E0432`, `E0107` (`BchCode` without generics) and two `E0599`
(`BchCode::dvb_t2`, `BchCode::new`). `diff` of the error blocks of the two
build logs, with the directory of the crate removed, is empty. Commit
`9591b0674` removes that code surface from `gf2-coding`. `src/main.rs` keeps
its bytes in this unit: it is the gf2 arm of the survey receipts under
`dev/bench_results/4e732b56/`. Both commands add a `blake3` line to
`gf2-side/Cargo.lock` in the working tree; the committed lock file keeps its
bytes. The port of the crate is tracked -> c73aa593
(`jit issue show c73aa593`).

## Citations (REQ-05)

Repointed, path text only, in commit `docs(jit:b23c364a): repoint citations
of the baseline survey`:

| File | Edit |
| --- | --- |
| `findings.md` and `workload-selection.md` of the entry | own-path citations; relative link targets to the epic plan, `dev/bench_results/` and `dev/archive/` at the entry's depth |
| `dev/active/04b85d10/findings.md` | citation of the workload-selection contract |
| `dev/active/ae03bcd0-general-bch/` `handoff.md`, `handoff-2.md` to `handoff-5.md`, `handoff-8.md`, `session-10-prompts/dispatch-591a1c5e.md`, `session-10-prompts/dispatch-97410c80.md` | citations of the entry |
| `dev/active/ae03bcd0-general-bch/d1b4f85e/run.sh:15` | comment citing the workload-selection contract |
| `dev/active/ae03bcd0-general-bch/fd9d5416/receipt-protocol.md:7,154` | relative link targets to the entry beside it |
| `dev/active/f547c394/research-r3-sweep-resolutions.md`, `research-r3-sweeps.json` (JSON string; the file parses) | location `workload-selection.md:142`, which holds the cited sentence |
| `ec655592-dev-plans-removal.md` | location `findings.md:34` |
| `eace5009-regroup.md:46` | the consumer table names the entry at the move commit of that record |
| `7dbcde0c-bch-entries-move.md` | location `workload-selection.md:312` |

Hits of `git grep -nF 'dev/active/4e732b56'` at the tip of this branch; each
lies in a file of this table:

| Location | Ground |
| --- | --- |
| `dev/active/04b85d10/survey/code-rows.json` | SHA-256 pinned at `dev/bench_results/1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-confirmation/inputs/producing/producing-snapshot.json:1` |
| `dev/active/04b85d10/survey/gf2-side/src/lib.rs` | SHA-256 pinned at `dev/bench_results/04b85d10/2026-09-13-04b85d10-count-v4-pilot/checkpoints/manifest.json:1` |
| `dev/active/04b85d10/survey/profile-cases.py` | SHA-256 pinned at `dev/bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/host.txt:152` |
| `dev/active/04b85d10/survey/summarize-profile.py` | SHA-256 pinned at `dev/bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/host.txt:164` |
| `dev/active/6fb89a3c/survey/verify-bit-mapping.py` | SHA-256 pinned at `dev/bench_results/6fb89a3c/v3-r1-6fb89a3c-bch-confirmation/checkpoints/manifest.json:1` |
| Files under `dev/bench_results/` | Receipt directories and their `inputs/` snapshots |
| `67048b47-linked-pairs.txt` | Committed tool output at `d5b3d8b4b` |
| `7bac1303-harness-path-trials.txt` | Committed tool output at `3e3fadcdd` |
| `migration/manifest.toml`, `path` and `consumers` fields | The fields name the source by schema |
| `plan-review-r1.md:171` | Review report of plan revision 1, committed at `724c2c011`; the finding names the entry as cited from `crates/gf2-coding`, and no Rust source under `crates/` cites it |
| This record | It names the source of the move |
| `.jit/issues/` | Tracker state |

No file of the entry is digest-pinned: `git grep -nF` of the SHA-256 and of the
blob id of each edited file, outside `.jit/`, finds the `pinned_in=0` line of
`7bac1303-harness-path-trials.txt` only.

## Tracker and manifest (REQ-06)

Issue 4e732b56 references each file of the entry without commit pin. Each
manifest row whose `path` lies under `dev/active/4e732b56/` has
`status = "complete"`; its `destination` is the path under the epic directory.

## Verification

| Command | Result | Establishes | Does not establish |
| --- | --- | --- | --- |
| `python3 contrib/gates/docs-mechanical.py` | PASS, 0 findings | Every inline link of the Markdown files under `dev/active/`, the moved and the touched ones included, resolves | Code-span paths |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding names a row of this unit | Each row of the entry is `complete`, its destination exists and its source is absent | Rows of other units |
| `bash -n` on the two shell scripts, `python3 -m py_compile` on the three edited Python files, byte code written under `target/` | exit 0 | Syntax | Behavior beyond the captures above |
| `python3 -B -c "import json,sys; json.load(open(sys.argv[1]))" dev/active/f547c394/research-r3-sweeps.json` | exit 0 | The repointed JSON parses | |
