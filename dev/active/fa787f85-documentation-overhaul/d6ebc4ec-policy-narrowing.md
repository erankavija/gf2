# Documentation policy (`@/issue/d6ebc4ec`)

State of the `[documentation]` and `[docs-mechanical]` tables of
`.jit/config.toml`. Commands run from the repository root.

## REQ-01: policy values

`jit config get documentation` prints the three keys. Per value,
`test -e V && git ls-files -- V | head -1` establishes that the path exists and
holds a tracked file; it does not establish what the material is. The material
column cites the row of [`dev/index.md`](../../index.md) that describes the
path.

| Key | Value | Material |
|---|---|---|
| `managed_paths`, `issue_scoped_areas` | `dev/active` | epic documents, one directory per epic |
| `managed_paths`, `issue_scoped_areas` | `dev/studies` | study inputs and measurement logs, one directory per issue |
| `managed_paths`, `issue_scoped_areas` | `dev/bench_results` | benchmark receipts, one directory per issue |
| `managed_paths` | `dev/benchmarks` | benchmark output tables and plans |
| `permanent_paths` | `docs` | permanent documentation |
| `permanent_paths` | `dev/archive` | container and legacy archives |
| `permanent_paths` | `dev/campaigns` | `sim_runner` campaign definitions |
| `permanent_paths` | `dev/reference_data` | reference curves and tuning profiles |
| `permanent_paths` | `dev/research` | standalone prototype crates |
| `permanent_paths` | `dev/scripts` | CI checks and campaign runners |
| `permanent_paths` | `dev/simulation_results` | campaign output and datasets |
| `permanent_paths` | `dev/index.md` (file) | development index |
| `permanent_paths` | `dev/authoring-conventions.md` (file) | authoring conventions |

Each value exists and holds a tracked file. The three keys hold no `dev/plans`,
`dev/presentations`, `dev/sessions` or `dev/docs` value, and each policy row of
`migration/manifest.toml` is `complete`.

## Archived mirrors of absent areas

`jit archive container ID --json` maps each unpinned document reference under
`dev/archive/<container>/<area>/` to the source path `dev/<area>/` and
classifies that path through the policy. An area that is neither managed nor
permanent yields the blocker `unmanaged-selected-root`; a commit-pinned
reference is retained without classification.

| Area | Policy | Preview of an artifact mirrored from the area |
|---|---|---|
| `dev/research`, `dev/simulation_results` | permanent | `action` `copy`, `already_archived` true |
| `dev/plans`, `dev/presentations`, `dev/docs` | none; the directories are absent | `action` `retain`, evidence `pinned-historical` |

Each tracker reference to an archived mirror of the three absent areas is
commit-pinned except the one this command prints:

```zsh
python3 - <<'EOF'
import glob, json
for f in sorted(glob.glob(".jit/issues/*.json")):
    for d in json.load(open(f))["documents"]:
        p = d["path"].split("/")
        if p[:2] == ["dev", "archive"] and p[3:4] and p[3] in ("plans", "presentations", "docs") and not d.get("commit"):
            print(d["path"])
EOF
```

It prints
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/a9e461de/s5_gpu_crossover.md`,
which the ae82bd73 preview blocks with `repository-escape`.

## Archive previews

```zsh
for m in dev/archive/*/.jit-container; do e=${${m:h:t}%%-*}
  jit archive container $e --json | python3 -c '
import json,sys
d=json.load(sys.stdin); a=d["artifacts"]
print(sys.argv[1], d["eligible"], d["action_counts"]["block"], d["action_counts"]["pending_deletions"],
      all(x["already_archived"] for x in a if x["action"] in ("move","copy")))' $e
done
```

Each container except ae82bd73 prints `True 0 0 True`: the preview is
eligible, blocks nothing, deletes nothing, and each artifact it moves or copies
is already archived. ae82bd73 prints `False 2 0 False`; its two blocked
artifacts are `dev/benchmarks/gf2_algebra_permanent/README.md`
(`destination-conflict`) and the reference of the preceding section. The
previews do not establish what `--execute` does.

## REQ-02: mechanical check footprint

`contrib/gates/docs-mechanical.py` reads `markdown`, `citations`, `rustdoc`,
`exclude` and `baseline` of `[docs-mechanical]`. `baseline` names
[`contrib/gates/docs-mechanical-baseline.md`](../../../contrib/gates/docs-mechanical-baseline.md),
which lies outside `dev/` and outside the footprint.

| Criterion term | Key and value |
|---|---|
| root README | `markdown`: `README.md` |
| `AGENTS.md` | `markdown`: `AGENTS.md` |
| `CLAUDE.md` | `markdown`: `CLAUDE.md` |
| `docs/` | `markdown`: `docs/**/*.md` |
| crate READMEs | `markdown`: `crates/*/README.md` |
| `dev/active/` Markdown | `markdown`: `dev/active/**/*.md` |
| the two permanent pages of `dev/` | `markdown`: `dev/index.md`, `dev/authoring-conventions.md` |
| repository-relative links in Rustdoc | `rustdoc`: `crates/**/*.rs`, `dev/tools/tuning-campaign-support/**/*.rs` |
| excludes `dev/archive/` | `exclude`: `dev/archive` |

The `members` and `exclude` lists of the root `Cargo.toml` establish that each
workspace crate and `crates/gf2-kernels-hip` lies under `crates/` or under the
second `rustdoc` glob. Rust sources under `dev/research`, `dev/benchmarks`,
`dev/studies` and `dev/tools/tuning-profile-compose` lie outside the footprint.

## REQ-03: checks

| Command | Result | Establishes |
|---|---|---|
| `python3 contrib/gates/docs-mechanical.py` | `PASS`, 0 findings | links, anchors, citations, item addresses and projections of the footprint |
| `python3 contrib/gates/tests/docs-mechanical-test.py` | `OK` | the check's self-tests |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0, `policy` line complete | each policy row agrees with `.jit/config.toml`; the baseline row's destination exists |
| `python3 -m unittest discover -s dev/active/fa787f85-documentation-overhaul/migration -p '*_test.py'` | `OK` | the checker's self-tests |
| `jit validate` | `Repository validation passed` | the policy, the applied `packages/jit-default` profile and each document reference agree |
