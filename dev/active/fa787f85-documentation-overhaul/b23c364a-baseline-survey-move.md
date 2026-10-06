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
| Root resolved at run time | commit `refactor(jit:b23c364a): resolve the survey root at run time` | `dev/active/4e732b56` | empty, both |

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
