# Checker retirement (`@/issue/af84619e`)

Commit checked: `6c36ef882055d47a859f19cee817f578a1d411c7`. It holds the
migration checker of `@/issue/a24b2af7`, its self-test and
[af84619e-final-check.txt](af84619e-final-check.txt), the checker's standard
output at that commit.

Commit without the checker: `e2f5d4d71ed740b1b07431cb9e2a4010f3cd9e3a`.
[migration/manifest.toml](migration/manifest.toml) is the
completed manifest; the checker source at the commit checked defines its row
schema (`ARTIFACT_FIELDS`, `POLICY_FIELDS`) and the row assertions.

## Commands

At the commit checked, from the repository root of a checkout of it:

```sh
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py | diff - dev/active/fa787f85-documentation-overhaul/af84619e-final-check.txt
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py > /dev/null; echo $?
python3 -B -m unittest discover -s dev/active/fa787f85-documentation-overhaul/migration -p '*_test.py'
```

At `e2f5d4d71ed740b1b07431cb9e2a4010f3cd9e3a` or a descendant, from the
repository root:

```sh
git ls-files dev/active/fa787f85-documentation-overhaul/migration
git grep -n -E -e 'migration/check' -e 'check_test' -e '(^|[^[:alnum:]_-])check\.py' -- scripts dev/scripts contrib .config .github .jit/gates.toml .jit/config.toml '*.sh' '*.py' '*.toml' '*.yml' '*.yaml' ':!dev/active/fa787f85-documentation-overhaul/migration/manifest.toml'; echo $?
CARGO_CI_STEPS='^$' ./scripts/cargo-ci.sh | grep -c -i -e migration -e 'check\.py'
git grep -l -E -e 'migration/check' -e 'check_test' -e '(^|[^[:alnum:]_-])check\.py'
python3 - <<'EOF'
import collections, tomllib
m = tomllib.load(open("dev/active/fa787f85-documentation-overhaul/migration/manifest.toml", "rb"))
for k, n in sorted(collections.Counter((r["disposition"], r["status"]) for r in m["artifacts"]).items()):
    print(*k, n)
for k, n in sorted(collections.Counter(r["status"] for r in m["policy"]).items()):
    print("policy", k, n)
EOF
t=$(mktemp -d); git show 6c36ef882055d47a859f19cee817f578a1d411c7:dev/active/fa787f85-documentation-overhaul/migration/check.py > "$t/check.py"; python3 -B "$t/check.py" --manifest dev/active/fa787f85-documentation-overhaul/migration/manifest.toml --require-complete; echo $?; rm -r "$t"
python3 contrib/gates/docs-mechanical.py
```

## Criteria

| Criterion | Command | Result | Establishes | Does not establish |
|---|---|---|---|---|
| REQ-01 | the `diff` and `echo $?` commands at the commit checked | `diff` prints nothing, in the working tree and in a clone holding tracked files only; exit status 0 | The committed report is the checker's output. It holds no finding line, so each row passes the schema, carries one of the dispositions the report names (the checker's `DISPOSITIONS`) and passes its source and destination assertion, and each policy row agrees with `.jit/config.toml`. Each count line reads all rows complete except `deletion`: the two rows short of complete name the checker and its self-test, whose sources exist at that commit and whose status there is `pending`. | That a row's disposition is the right one for its artifact. The output for a working directory other than the repository root (`--root` defaults to it) or for a tree where an untracked file sits at a path a row asserts absent. |
| REQ-01 | the `unittest` command at the commit checked | `OK` | The checker passes its self-test at the commit checked. | — |
| REQ-01 | the `git show` command line | one line per disposition and one for `policy`, each with equal counts; exit status 0 under `--require-complete` | The checker of the commit checked, run on the tree without it, reports each row complete with no finding: the two `deletion` rows that name it pass the absence assertion. | As for the first row. The output is not a committed file. |
| REQ-02 | `git ls-files` | one path, the manifest | The tree holds no checker and no self-test beside the manifest. | Untracked files of a working tree. |
| REQ-02 | the first `git grep`; the `cargo-ci.sh` step list | no line, exit status 1; count 0 | No file under `scripts`, `dev/scripts`, `contrib`, `.config`, `.github`, no gate or documentation policy file of `.jit`, and no tracked `*.sh`, `*.py`, `*.toml`, `*.yml` or `*.yaml` file other than the manifest names the checker or its self-test. No `cargo-ci` step name does. | Files of other types; the table below lists them. A reference that spells neither file name. |
| REQ-03 | the `python3 -` heredoc | each printed line carries status `complete` | The manifest parses with `tomllib`; the line set gives the row count per disposition and status and the policy row count per status. | The assertions against the tree; the `git show` command line covers them. |
| — | `docs-mechanical.py` | `PASS`, `0 finding(s)`, the baselined count of [the baseline](../../../contrib/gates/docs-mechanical-baseline.md) | Each link and item address of this record and of the files this unit edits resolves. | — |

## Files that name the checker

The second `git grep` lists them.

| Files | Ground |
|---|---|
| `migration/manifest.toml` | Manifest data: the `path` field of the two `deletion` rows and two `inbound` entries of the `README.md` row. |
| Unit records `<short-id>-*.md` of this directory and the outputs `67048b47-linked-pairs.txt`, `a0ec34f2-rows.txt` | Each states an invocation or a row at the commit its record names. |
| This record | The commands above. |
| `handoff-*.md` and `progress.json` of this directory | Session records of the epic lead; each describes its own session. |
| `handoff-23.md` and `handoff-25.md` under `dev/active/1a379447-zen3-cpu-performance/` | Session records of `@/issue/1a379447`. |
| `.jit/issues/*.json` | Tracker data. The description of `@/issue/96cea1b9` quotes the checker by name; document references are tracker state, which this record does not establish. |

`plan.md` and, in [the planning brief](fa787f85-planning-brief.md), decision
D-41, implementation step 12, one testing step and one non-goal name the
checker without its path, as plan text.
