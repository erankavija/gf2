# Documentation policy at `ba8f988fa` (`@/issue/d6ebc4ec`)

State of the `[documentation]` and `[docs-mechanical]` tables of
`.jit/config.toml` at commit `ba8f988fa`. Commands run from the repository
root.

## REQ-01: policy values

`jit config get documentation` prints the three keys. Per value,
`test -e V && git ls-files -- V | head -1` establishes that the path exists and
holds a tracked file; it does not establish what the material is. The material
column cites the row of [`dev/index.md`](../../index.md) that describes the
directory.

| Key | Value | Exists with tracked files | Material |
|---|---|---|---|
| `managed_paths` | `dev/active` | yes | epic documents |
| `managed_paths` | `dev/studies` | yes | study inputs and measurement logs |
| `managed_paths` | `dev/benchmarks` | yes | benchmark output tables and plans |
| `managed_paths` | `dev/bench_results` | yes | benchmark receipts |
| `managed_paths` | `dev/research` | yes | standalone prototype crates |
| `managed_paths` | `dev/simulation_results` | yes | campaign output and datasets |
| `managed_paths` | `dev/plans` | **no** | none |
| `managed_paths` | `dev/presentations` | **no** | none |
| `managed_paths` | `dev/docs` | **no** | none |
| `permanent_paths` | `docs` | yes | permanent documentation |
| `permanent_paths` | `dev/archive` | yes | container and legacy archives |
| `permanent_paths` | `dev/campaigns` | yes | `sim_runner` campaign definitions |
| `permanent_paths` | `dev/reference_data` | yes | reference curves and tuning profiles |
| `permanent_paths` | `dev/scripts` | yes | CI checks and campaign runners |
| `permanent_paths` | `dev/index.md` | yes (file) | development index |
| `permanent_paths` | `dev/authoring-conventions.md` | yes (file) | authoring conventions |
| `issue_scoped_areas` | `dev/active` | yes | one directory per epic |
| `issue_scoped_areas` | `dev/studies` | yes | one directory per issue |
| `issue_scoped_areas` | `dev/bench_results` | yes | one directory per issue |

REQ-01 is unmet for the three `managed_paths` values marked **no**, and the
temporary entries `dev/docs`, `dev/research` and `dev/simulation_results`
remain. The section "Entries the archive planner reads" gives the reason.

`dev/sessions` is absent from the table, and no `dev/plans` or
`dev/presentations` value sits under `issue_scoped_areas`.

## Entries the archive planner reads

`jit archive container ID --json` plans from the document references of the
container's issues. Those references name the source path of each published
artifact, and the planner classifies that path through `managed_paths`.

Each row states what the preview of the named executed container prints when
the value is taken out of `managed_paths` in a scratch edit of the config
(`dev/research` and `dev/simulation_results` placed under `permanent_paths`
instead) and every other value stays as committed.

| Value | Container | Artifact | Preview with the value removed |
|---|---|---|---|
| `dev/plans` | 806eb14e | `dev/plans/hip_gpu_prototype_wave.md` | `eligible: false`, blocker `unmanaged-selected-root` |
| `dev/presentations` | f9717e7e | `dev/presentations/talk.html` | `eligible: false`, blocker `unmanaged-selected-root` |
| `dev/docs` | 6efb756b | `dev/docs/presentations/6efb756b-grand-sogrand/talk.html` | `eligible: false`, blocker `unmanaged-selected-root` |
| `dev/research` | e095a100 | `dev/research/rns_representation.md` | `eligible: true`; `action` is `copy` in place of `move`, source absent |
| `dev/simulation_results` | 6efb756b | `dev/simulation_results/phase4_comparison_report.md` | `eligible: true`; `action` is `copy` in place of `move`, source absent |

The table names one artifact per value; it does not enumerate the affected
artifacts. The policy rows of `migration/manifest.toml` for `dev/docs`,
`dev/research` and `dev/simulation_results` keep their open status, and the
rows for `dev/archive` and `dev/reference_data` are `complete`.

## Archive previews at the committed policy

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
is already archived. ae82bd73 prints `False 2 2 False`, and prints the same at
`338275ff2`; its archive is not executed.

Relative to the policy at `338275ff2`, `action`, `destination`,
`already_archived`, `eligible` and `action_counts` of each preview are equal.
The `evidence` list of each artifact whose source lies under `dev/archive` or
`dev/reference_data` carries the additional tag `permanent-path`. The previews
do not establish what `--execute` does.

## REQ-02: mechanical check footprint

`contrib/gates/docs-mechanical.py` reads `markdown`, `citations`, `rustdoc`,
`exclude` and `baseline` of `[docs-mechanical]`.

| Criterion term | Key and value |
|---|---|
| root README | `markdown`: `README.md` |
| `AGENTS.md` | `markdown`: `AGENTS.md` |
| `CLAUDE.md` | `markdown`: `CLAUDE.md` |
| `docs/` | `markdown`: `docs/**/*.md` |
| crate READMEs | `markdown`: `crates/*/README.md` |
| `dev/active/` Markdown | `markdown`: `dev/active/**/*.md` |
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
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0, no finding line | each policy row agrees with `.jit/config.toml` |
| `python3 -m unittest discover -s dev/active/fa787f85-documentation-overhaul/migration -p '*_test.py'` | `OK` | the checker's self-tests |
| `jit doc check-links --scope all` | output byte-equal to the output at `338275ff2` | the policy edit changes no link finding |
| `jit validate` | exit 4, `applied profile provenance for 'jit-default@1.0.2' does not match the package its record resolves to` | the package manifest differs from the applied profile record |
| `jit validate` with `packages/jit-default/manifest.toml` at `338275ff2` | exit 1, three `profile-ownership` errors and no other error | the removed contributions (`dev/sessions` managed; `dev/plans`, `dev/presentations` issue-scoped) are the only rule errors |

`jit validate` passes after `jit profile capture` and `jit profile upgrade`
re-mint `packages/jit-default`; this record does not establish that run.
