# Handoff: Separate experiment code from the library crates (45ecf975), session 41

**Date:** 2026-11-13
**Prior handoffs:** retired by @/issue/d1c0e062. Their live traps are carried below.
**Lead:** gf2-a6, successor of gf2-69. Steward: gf2-f3.

Artifacts outside the tree are in `<gf2-share>/45ecf975-handoff-38/`, where
`<gf2-share>` is the directory the history-rewrite runbook backs up to.

## Current state

| Issue | State | Where it stands |
|---|---|---|
| `@/issue/595e2fe5` | done | Both gates passed. |
| `@/issue/93df507a` | done | hipcc gets `--rocm-path`. Both gates passed, merged. Window line 67194d70 runs `jit-67194d70-run` at `54c297e5c`. |
| `@/issue/66fefd9e` | in progress, on main | cargo-ci passed; code-review failed twice. Rework 2 is approved, and a third failure goes to the owner. |
| `@/issue/da6200b8` | in progress, on main | cargo-ci passed; code-review failed once (F1–F4 blocking). F3 and F4 are restored on main (`fix(jit:da6200b8): restore a runnable command block ...`). Rework 1 is approved, not dispatched. |
| `@/issue/506f76d4` | in progress, branch only | Head `e60912a40`, unmerged and ungated. Both owning leads approved their files. |
| `@/issue/0fe85425` | in progress, branch only | Head `04ab2400b`, unmerged, ungated, needs rework. |
| `@/issue/5f33fb28` | ready | Blocked by 506f76d4. A read-only inventory (possibly partial) is `5f33fb28-inventory.md` in the handoff artifacts. |
| `@/issue/95077f84` | ready | Merge the two Rust root mechanisms. Depends on 0fe85425. |
| `@/issue/411e7267` | ready | Smoke expected vectors written under `target/`. |
| `@/issue/67194d70` | ready | The validation runs in tonight's window. Then record location and digest. |

## Rulings that bind the next lead

- da6200b8 design (steward): C1a, C2a, C3a, C4a.
  - Bare names resolve from their own directory; issue prose uses repository paths.
  - Every `/tmp` path in prose is host-local unless its line writes or locks it.
  - TOML is read with `tomllib`, YAML with the line grammar, and no dependency is added.
  - A name counts as written only when it reaches a write in the same function. A short `writer_helpers` list in `scripts/reference-exceptions.toml` counts calls to listed helpers. Each listed helper must exist. A writer that fits no listed helper is a single reasoned exception.
  - No version, ledger or schema. The checkers catch mistakes.
- 66fefd9e: the owner approved a one-time exception. In the owner's words: "Only this time to prevent harmful 'evidence' accumulation in the future. Otherwise we don't edit old plan files etc." The August exclusion of `dev/active/b8206228-permanent-statistics/breakdown.json` in `@/issue/240b7618` and `@/issue/679cf170` stays the standing rule. The scope guard covers only passages that tell a worker to commit a receipt, measurement, log or dataset.
- Measured tuning profiles (owner): tests that read them are removed by default. A kept test needs a reason the owner accepts.
- The owner calls a library test that asserts the `dev/` layout "a seriously defective thing in unit tests". 0fe85425 removes `test_phase1_canonical_campaign_surface` and gf2-coding's dev-dependency on tuning-campaign-support. No crate under `crates/` depends on `dev/tools` or `experiments/`, dev-dependencies included.
- cargo-ci.sh resolves `dev/` scripts by name through `repository_files.py`. Literal paths are allowed only where a build system requires them. gf2-73 reviews those steps.
- Gate before merge. A branch passes every gate in its worktree before it merges, with the gate records copied to main. Nothing lands against a failing or pending review. Merges of main into a branch also take a conforming subject.
- Max-throughput gating (owner):
  - There is no lead test lock any more. Independent cargo-ci runs go in parallel under `BUSY_HOST_OVERRIDE=1`, and the gate record says so.
  - Reviews start as soon as cargo-ci passes.
  - A test killed under the busy profile is reported to the steward with `ps -eo pcpu,comm,args --sort=-pcpu | head -30`.
  - The hold on main stays for merges and for gates on the main checkout.
- Relay rule: owner decisions go through the steward, and a relayed answer is the owner's answer. Ask the owner directly only when the relay cannot work.
- Commit subjects: check `git log main..<branch> --format=%s` before every merge: closed type list, scope, at most 71 characters counted with `${#s}`. A commit-msg hook is approved and gf2-73 builds it.

## History rewrite plan (45ecf975 REQ-04)

- [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md) states the preconditions and steps.
  - Step 2 deletes stale branches after the bundle is verified.
  - The rewrite step rewords subjects through [reword_subjects.py](45ecf975/reword_subjects.py).
  - Step 9 deletes `copilot/add-error-correcting-codes-module` and `formal-verification` on origin.
- Delete list: 69 branches in `<gf2-share>/rewrite-plan/branches-delete.tsv`, with `branches-exclude.tsv` beside it. The 12 former "ask" branches are deleted by owner decision. Regenerate the list from the then-current refs before the rewrite.
- Reword (owner): every non-conforming subject in `origin/main..main` is reworded, by original commit ID, keeping its meaning.
  - 36 candidates as of 21:00 UTC, in `reword-candidates.tsv`, drafted by Claude Sonnet.
  - A Codex luna meaning review (`codex-reword-review.txt`) passed 25 and proposed changes to 11. Apply its proposals or settle each one first.
  - Commits after the candidates were drafted also need entries. Two are known: the tracker commit `chore(jit:45ecf975): file 95077f84, record ...` (74 characters) and any later ones. Regenerate the candidates by script, then merge them into `reword_subjects.py`.
- Rehearsal 3 ran steps 1 and 3 to 6 on main `54c297e5c`.
  - The mirror shrank from 404 MB to 60 MB (pack 56.8 MiB). 2,713 blobs were stripped and all 639 refs rewritten.
  - Main went from 9,784 to 9,757 commits, 14 pins mapped, and the map is 488 KB.
  - `jit validate` passed and the bundle verified.
  - Step 7 (cargo-ci in the rewritten checkout) did not run: the run was stopped at wind-down while still building.
  - A fresh rehearsal from the then-current main must pass step 7 before the owner's go. It must also check that no non-conforming subject remains in `origin/main..main`. The script is `rehearse.sh`. Its bundle verify must run with `git -C backup.git`.
- The owner gives the go. Nothing is pushed before it.

## Scope checks from the steward

- `crates/gf2-coding/tests/ebch_128_64_reference.rs:132` compares a recorded fixture string with `dev/reference_data/osd_ebch_128_64_fossorier1994.md` and opens no file. The file exists. This is outside 0fe85425's reader scope; no change.
- `crates/gf2-coding/examples/gf2pow32_smoke_emit_expected.rs:86` and `crates/gf2-coding/examples/sparse_smoke_emit_expected.rs:98` write gitignored, untracked files under `benchmarks/expected/`, so nothing is committed. Their default paths depend on the working directory and land in the working tree. Filed as `@/issue/411e7267`.
- The 5f33fb28 inventory must cover these four test files:
  - `crates/gf2-algebra/tests/tuning_repository_envelopes.rs`
  - `crates/gf2-core/tests/tuning_profile_committed.rs`
  - `crates/gf2-core/tests/tuning_envelope_v2.rs`
  - `crates/gf2-core/tests/support/measured_format2.rs`

## Next

1. 66fefd9e rework 2, the last one allowed.
   - Rewrite `dev/active/b8206228-permanent-statistics/45ecf975/plan.md:50`. The dispatch file `dispatch-7a816262-phase1.md` (retired by @/issue/d1c0e062) is no longer a rewrite target.
   - Widen the sweep to any wording that puts a measurement, pilot, evidence, run output, result or record into the tree, whatever the verb.
   - A worker of the other vendor builds an independent inventory, sliced by directory. Reconcile the two inventories and list every hit with its decision in the issue.
   - Gate on the branch, then merge.
2. da6200b8 rework 1. The review verdict is in `da6200b8-review1.txt`.
   - F1: syntactic, one function. Cover `.with_file_name`, shadowed constants, `.write(false)` and `printf` arguments.
   - F2: track real YAML key indentation and scalar indicators.
   - Check the whole citation pass: every edited fenced code block still runs, and every edited `## Success Criteria` line keeps its meaning. List the checked items in the issue.
   - Replace the 7cc591a0 exception with a checker rule if bare `/tmp` naming a directory should pass.
   - Gate on the branch, then merge.
3. 506f76d4 lands, then 0fe85425 merges main and is reworked.
   - Before merging 506f76d4: apply `506f76d4-desc/` to the tracker by regenerating with `506f76d4-tracker-pass.py` on the current main, and remove 2c487595's document link to `addendum_schema_versions.rs` (`jit doc remove`).
   - Record as-run commit `be4ed399e71827600218f24a32d5307248e932e9` and the edit reason in a83583e0 and dbd8787d.
   - Gate on the branch, then merge.
   - Reword the branch's `wip(...)` subjects before merging: squash with `git reset --soft` onto the merge base, or reword.
   - 0fe85425 then drops its validator edits, removes the layout test and the dev-dependency, gets gf2-73's review of its four cargo-ci steps, and is gated.
4. After the window, record 67194d70's validation record location and digest.

## Traps

- I merged 66fefd9e, da6200b8 and 93df507a to main before their reviews passed, and both reviews then found defects on main. Gate on the branch first.
- A code-review run before cargo-ci has passed fails on "REQ-03 unverified" (93df507a). Chain each review after its cargo-ci, conditionally.
- A worker sweep for "commit" phrasings misses other forms ("collected and committed", "committed evidence"). Use a second-vendor inventory sliced by directory.
- The citation pass replaced `/tmp` inside a runnable command and inside a criterion. Check code blocks and criteria after any mechanical prose pass.
- `crates/*/data/...` in prose tripped the host-local rule. Fixed in da6200b8 (`13ae12fec`).
- Commit subjects from `wip(...)` worker commits and `git merge --no-edit` reach main. Squash or reword on the branch. A subject that looks short can still exceed 71: count it.
- A codex worker given `flock -w` gives up on a busy lock. Give no timeout, or now no lock at all.
- `git stash` is shared across worktrees and sessions; never use it in a brief.
- `jit gate evaluate` and `jit issue update` in a worktree record on that branch's `.jit`. After merging a gated branch, copy its `.jit/gate-runs/<issue-uuid>/` records into main's ignored `.jit/gate-runs/`.

## Traps carried from earlier handoffs

- `breakdown.json` stays excluded from audit by recorded owner decision (`240b7618`, `679cf170`). Do not argue its properties, edit it, or resync it. Do not amend an issue description without checking that issue's copy in it.
- A `jit gate evaluate` success line is not evidence. Confirm with `jit gate status <id> <gate>`. A stale `claims.lock` wedges evaluate; `jit recover` clears it once no jit process runs.
- A cold `cargo-ci` exceeds the 900 s gate timeout, and an `error` at that limit is not a verdict. Warm the tree with a direct run, then rerun the gate.
- Concurrent builds in other worktrees make fast-tier tests miss their 15 s kill. Check the gate record and rerun solo before reading a failure as a regression.
- GPU-device tests fail under host contention. Rerun the single test alone before reading it as a regression.
- Stage explicit paths and check `git diff --cached --stat`. `jit` materializes shared `.git/jit` state into the checkout that runs it, so a worker commit can carry other issues' `.jit/` changes.
- Run `jit` from a checkout's root, not a subdirectory. A tracker write in a worktree lands on that branch and conflicts on `updated_at` when merged.
- `jit doc add` is idempotent per path. Run `jit doc remove` on the old link first. An asset entry in a document link names the file too, so remove it with the link.
- Before deleting a file, remove its `jit doc` links and asset entries, then run `jit validate`. Do not close an issue whose text names a deleted path; rewrite the text first.
- Reference checks read tracked files only. A pass on an untracked file proves nothing.
- Do not rewrite a removed path to a bare base name. Point the citation at an existing file or follow the name with "(removed by @/issue/<id>)", and audit the whole class, not only the cited file.
- The history rewrite needs `map_pins.py`, because tracker documents pin commit IDs. Nothing is pushed before the owner's go; the pre-push hook refuses purged history.
- `codex exec` workers cannot write `.git`. They leave files in the tree, and the lead commits. Start a background `codex exec` with `< /dev/null`.
- Single-agent audits sample rather than read. Slice the tree by directory for full coverage.
- Do not `git checkout --` or `git restore` a worker's uncommitted edits. Commit them as WIP first.
- A commit that changes public rustdoc needs `cargo doc`. Verify a commit's check results before committing; do not commit in the same command as a check whose result is only printed.
- Passing gates are stale once shared code changes, such as `experiments/common`. Re-gate.
- The AFF3CT adapter has its own `Cargo.lock`, which `cargo-ci` does not build. After a dependency change, build it with `--locked`.
- A reviewer re-run can surface new findings. Verify each one; read findings with `jit gate status <id> <gate> --findings`, not from reviewer rollouts.
- Dispatch and rework prompts are ephemeral. Never commit them.
- Do not edit `scripts/cargo-ci.sh` while an instance runs.
- A falsification must reinstate the real pre-fix code byte-for-byte. Exemption tests must check that the fixture reaches the exempted path.
- Host-wide process kills by command-line pattern can kill peer builds. Kill only PIDs from your own process tree.
- The shell is zsh, where `status` is read-only. Do not use it as a variable name in scripts.
- `git commit -F -` after an `&&` chain hangs. Write the message to a file.
- `/usr/bin/time` is not installed. Time commands with shell `date` arithmetic.
- Agent-tool sandboxed worktrees have vanished. Read the leak-check output and check peer sessions before reverting anything.

## Open questions for the invoker

- None new. The rewrite go, its timing, and the reword list's final review are with the steward and owner.

## References

- [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md), [reword_subjects.py](45ecf975/reword_subjects.py), handoff-37.md (retired by @/issue/d1c0e062)
