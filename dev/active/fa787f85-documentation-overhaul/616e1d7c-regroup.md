# Field-dispatch entry regroup record

Issue 616e1d7c covers the Appendix A rows of [investigation.md](investigation.md)
whose top epic includes 6dc81018, less the code-pinned entries a83583e0 and
dbd8787d. Epic 6dc81018 is in tracker state `archived` and its archive root is
`dev/archive/6dc81018-field-capability-dispatch/`; epic 86b9c719 is in state
`backlog`. This issue moves no file and changes no manifest row.

## Entry state

Source: [616e1d7c-entry-state.txt](616e1d7c-entry-state.txt), printed by
[616e1d7c-entry-state.py](616e1d7c-entry-state.py). "Copy" counts flat files
with a byte-identical file at the same relative path under the archive root's
`active/` tree. Manifest rows are `complete` / `pending`.

| Entry | Owner issues | Flat files | Copy | No copy | Archive files | Rows | Disposition |
|---|---|---|---|---|---|---|---|
| 1ac74567 | 1ac74567 | 0 | 0 | 0 | 5 | 5 / 0 | archived |
| 220cab0b | 220cab0b, 2a85f728, 676f55a2, f35daec0 | 1 | 1 | 0 | 1 | 0 / 1 | left: D |
| 2a85f728 | 2a85f728 | 0 | 0 | 0 | 1 | 1 / 0 | archived |
| 34d85cb9 | 34d85cb9 | 180 | 0 | 180 | 1 | 1 / 180 | left: U |
| 389aa4de | 389aa4de | 1 | 0 | 1 | 0 | 0 / 1 | left: U |
| 3fa7c9d0 | 3fa7c9d0 | 1 | 1 | 0 | 1 | 0 / 1 | left: D |
| 50b47eae | 50b47eae | 63 | 0 | 63 | 0 | 0 / 63 | left: U |
| 6dc81018-field-capability-dispatch | 265997f9, 663965f6, 6dc81018 | 6 | 4 | 2 | 21 | 15 / 6 | in the canonical directory; W |
| 7d7c647c | 7d7c647c | 88 | 1 | 87 | 1 | 0 / 88 | left: U, W |
| 7d824b2f | 7d824b2f, a83583e0, eaae1b56 | 1 | 1 | 0 | 1 | 0 / 1 | left: D |
| 9162956b | 9162956b | 14 | 14 | 0 | 14 | 0 / 14 | left: D |
| 972e2b88 | 972e2b88 | 1 | 1 | 0 | 1 | 0 / 1 | left: D |
| e6ea0dde | e6ea0dde | 66 | 0 | 66 | 1 | 1 / 66 | left: U |
| eaae1b56 | eaae1b56 | 1 | 1 | 0 | 1 | 0 / 1 | left: D |
| eb9b324c | eb9b324c | 5 | 0 | 5 | 13 | 13 / 5 | left: U |
| fc976a80 | fc976a80 | 1 | 1 | 0 | 1 | 0 / 1 | left: D |

| Code | Reason |
|---|---|
| D | The owning epic 6dc81018 is archived. Each flat file duplicates an archived file. |
| U | The owning epic 6dc81018 is archived. The flat files have no archive copy, and the tracker references of the owner issue name the flat paths. |
| W | Branch `worktree-agent-103a792a` carries an unmerged commit editing `dev/active/7d7c647c/design.md` and `dev/active/6dc81018-field-capability-dispatch/investigation.md`. |

The `pending` rows of every left entry name a destination under
`dev/active/6dc81018-field-capability-dispatch/<entry>/`, the directory
`jit doc dir 6dc81018 dev/active` resolves. Contract `active-layout` of
[plan.md](plan.md) assigns that directory to open-epic material.

## Owning epic

Each owner issue of an entry outside the table below carries the single
membership label `epic:field-capability-dispatch` and lies in the dependency
closure of exactly one epic, 6dc81018.

| Entry | Closure of | Deciding step of `active-layout` | Owning epic |
|---|---|---|---|
| 220cab0b | 6dc81018, 86b9c719 | single membership label of the entry issue | 6dc81018 |
| 389aa4de | 6dc81018, 86b9c719 | owner decision, row "Active-layout ties" of [plan.md](plan.md) | 6dc81018 |
| 3fa7c9d0 | 6dc81018, 86b9c719 | single membership label of the entry issue | 6dc81018 |
| 6dc81018-field-capability-dispatch | 6dc81018, 86b9c719 | the epic issue ID names the entry | 6dc81018 |

## Checks

Entry state, from the repository root:

```sh
G=$(git ls-files ':(glob)**/616e1d7c-entry-state.py')
python3 "$G" | diff - "${G%.py}.txt"
```

It establishes file locations, byte identity of copies, tracker references in
the checkout's `.jit/issues`, and manifest row status. It does not establish
why the archive run left a source in place.

Consumer check, per entry `<e>` of the left set, with one `':!dev/active/<x>'`
exclusion per left entry `<x>`:

```sh
git grep -nF "dev/active/<e>" -- ':!dev/archive' ':!**/inputs/**' ':!*.md' \
  ':!.jit' ':!dev/active/fa787f85-documentation-overhaul' ':!dev/active/<x>'...
```

| Match | Kind |
|---|---|
| `scripts/cargo-ci.sh:268` (7d824b2f) | shell comment |
| `crates/gf2-core/benches/tuning_calibration.rs:607` (eaae1b56) | Rustdoc comment |
| `dev/active/f547c394/research-r3-sweeps.json` (220cab0b, 7d824b2f, 6dc81018-field-capability-dispatch) | provenance strings in a data file |

No match is a non-comment code, test, script or config consumer. The check
excludes references between the listed entries: scripts in eb9b324c read
50b47eae, and one script each in e6ea0dde and 7d7c647c names 34d85cb9 and
e6ea0dde. Scripts in 34d85cb9, 50b47eae, 7d7c647c, e6ea0dde and eb9b324c name
their own directory by path literal.

Worker-branch check, per entry and per branch of `git worktree list`:
`git log --oneline main..<branch> -- dev/active/<e>`. Code W lists every
non-empty result.

Link scan: `python3 contrib/gates/docs-mechanical.py` passes.
