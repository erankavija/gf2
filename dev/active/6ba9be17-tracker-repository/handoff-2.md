# Handoff: Move the tracker data into its own repository (6ba9be17), session 2

**Date:** 2026-11-15
**Prior handoffs:** [handoff.md](handoff.md) (session 1, lead `gf2-e3`).

## Current state

- Lead `gf2-bf` under steward `gf2-a2` hands the epic to two leads (owner order, about 23:15): `gf2-5c` takes the cutover line (`e610f5c4`, `6b9e6b14`, `6889e6e6`); `gf2-f6` takes `7854cfad`, `d1eba151`, `2a4c2684` and the quiet-window inventory that `gf2-24` produces. One cargo-ci slot each. Progress: [progress.json](progress.json).
- Done and landed on `main`: `6d5ba9fb`, `28aa2a9d`, `3fb4accb`, `6310679b`, `daa33be8`. Nothing is pushed.
- Durable copies of every brief, design note, Codex output and review verdict of this session: `~/.local/share/gf2/6ba9be17-handoff-2/` (`review-verdicts.md` summarises the review rounds that live only in subagent reports).
- Claims: every open child is assigned to `agent:gf2-bf`; reassign with `jit issue assign <id> agent:<lead>` on `main`.

| Issue | New lead | Branch, head, worktree | Gate state | Running worker at handoff |
|---|---|---|---|---|
| `e610f5c4` cutover tool | `gf2-5c` | `worktree-agent-e610f5c4` at `18a0e0609`, checkpoint of the Codex final-round phase 2 that gf2-bf stopped at 69 min without a report; `--self-test` passes there in 587 s | cargo-ci passed and code-review FAILED (data loss) at `59106f4ad`; nothing run since | Claude Opus 5.5 (gf2-5c): finish the final round as Step 0, then the charter D-12 round (REQ-01, REQ-07, REQ-10, REQ-11) on top; then a blocking Claude Opus review replaying the audit and a Codex review, then gates once; brief in `~/.local/share/gf2/6ba9be17-handoff-3/` |
| `7854cfad` `landed` gate | `gf2-f6` | `worktree-agent-7854cfad` at `b1c997544`, clean | cargo-ci passed at `908423595` (merged head); code-review failed at `897ea4ed0` (semantic-types), fixed in `ebf4cdb61`, Claude Sonnet blocking review PASS; code-review not yet rerun | none |
| `6b9e6b14` runbook and rehearsal | `gf2-5c` | `worktree-agent-6b9e6b14` at `4809b47c9`, worktree `.agents/worktrees/agent-6b9e6b14` | none | Claude Opus, phase 1 runbook draft (`dev/active/6ba9be17-tracker-repository/cutover-runbook.md` on that branch, marked `TBD-rehearsal:` where rehearsal results go); brief `brief-6b9e6b14.md` |
| `2a4c2684` rewrite side | `gf2-f6` | none | none | none; depends on `e610f5c4` |
| `d1eba151` extensionless scripts | `gf2-f6` | none | none | none; after `7854cfad` (both touch the landing scripts) |
| `6889e6e6` cutover execution | `gf2-5c` | none | none | waits for the owner's go, which the steward requests when `6b9e6b14` lands with a passing rehearsal |

## Item details

### `e610f5c4`

- Steward's final round (do not open another): (1) a state file under the git directory of each changed repository records the operation, starting HEAD, planned edits, step reached and each tool-made commit; no restart path infers an interruption from paths or contents; without a matching marker the tool refuses and lists the changes; (2) destructive-command audit: every reset, checkout, clean, update-ref, branch move, worktree removal and file deletion in `cutover.py` with the precondition that the content is the tool's own or archived first; (3) self-tests: an ordinary unstaged edit to each of `.gitignore`, `.gitattributes`, `scripts/tracked-budget.toml`, `scripts/reference-exceptions.toml` refused with nothing discarded; a marked interruption resumes; a stale marker from another HEAD refuses.
- The design note was approved with corrections A to E in the phase 2 brief: tool-made commits recorded in the marker (rollback stays restartable), atomic marker writes with fsync of the marker only, SHA-256 and mode identities with directory manifests, unchanged interfaces, every audit row marked "to fix" fixed.
- When phase 2 returns: commit its edits on the branch, then a blocking Claude Opus review that replays the audit against the code (other vendor), then cargo-ci and code-review at a merged head. Steward: another path that discards or overwrites content the tool did not write or archive parks the branch and goes to the owner as redesign.
- File name `cutover.py` (budget forbids extensionless files under `dev/active`); the issue text records it.

### `7854cfad`

- Next: code-review gate at the branch head (its declared inputs differ from `908423595` only by tracker records), then the steward before the merge (gate-set change).
- Open question for the steward: jit's preset format embeds a full copy of each gate definition (a key-only preset fails to parse), so `.jit/config/gate-presets/code-task.json` can drift from `.jit/gates.toml`. Recommendation: accept and regenerate with `jit gate preset create` whenever a bundled gate changes; ask the jit project for key-only presets.

### `2a4c2684`

- Assigned to this epic by the steward (former stream lead `gf2-00` has no session). Design: `~/.local/share/gf2/rewrite-plan/2026-11-14b/2a4c2684-design.md` (present). Scope (steward): one tracked change to the rewrite runbook, `rewrite_paths.py`, `map_pins.py` and the precondition; pins remapped in a tracker clone mounted in the rewritten checkout, validated there, then the primary tracker fast-forwards; the commit map stays in the code repository; the rehearsal check "lease directory under `.jit/.git`, `jit claim list` unchanged across the reseat". Files belong to the paused `45ecf975` stream, owned for this issue only; everything on scratch clones and the rehearsal mirror. Its REQ-06 is the rewrite rehearsal moved from `6b9e6b14`.

## Owner decisions and rulings of 2026-11-15

- D-10 (charter and plan): the cutover runs alone, in a quiet window of its own, before the history rewrite; accepted cost "branches settle twice".
- Parked branches, option (a): the cutover cleans every parked branch (main merged in, tracker conflicts resolved by deletion, tracked `.jit/` removed); a branch conflicting outside `.jit` is listed for the steward, never forced. The parked list includes the branches `gf2-24` parked tonight. The 17 no-issue branches are cleaned the same way (lead's answer to the steward: the criterion requires it; no exception asked).
- `6b9e6b14` REQ-05 moved to `2a4c2684` REQ-06; `2a4c2684` REQ-05 reworded; `2a4c2684` depends on `e610f5c4`, the epic on `2a4c2684` directly (`4b78d4e7b`).
- Commit scope `jit:none` for work without an issue.
- File-name alignments allowed without asking when an enforced rule requires them, noted in the issue (`cutover.py`, the runbook's `.md` suffix).
- The rehearsal check "the cutover's step-6 snapshot precedes the rewrite's step-1 bundle" is moot: the two run in separate windows.
- All pushes stay held until the rewrite.

## Path to the cutover

1. `e610f5c4` final round, review, gates, landing (about two hours if it passes).
2. `7854cfad` code-review, steward, landing (about one hour, parallel). `6889e6e6` REQ-10 needs it.
3. `6b9e6b14` phase 2: the rehearsal on scratch clones after both tools are on `main`, with the batch clean over the full parked list plus the 17 branches; fills the runbook's `TBD-rehearsal:` markers; gates.
4. The steward asks the owner for the go when `6b9e6b14` lands; then `6889e6e6` in its quiet window (about 30 to 60 minutes).

## Vendor alternation

- The last implementation dispatches: `e610f5c4` final round to Codex (running), `6b9e6b14` draft to Claude (running). The next implementation dispatch goes to Codex unless the steward's tier rule names Claude (`3fb4accb`, `daa33be8`, `7854cfad`, `e610f5c4` take the strongest non-Fable Claude tier); the reviewer is the other vendor; a Codex change gets a blocking Claude review. Every brief states that cargo runs only through the host cargo shim on PATH.

## Traps

- Cargo-ci gates on a fresh or rebuilt worktree hit the 900 s gate timeout under load. Run `./scripts/cargo-ci.sh` directly first, then `jit gate evaluate <id> cargo-ci --force`; without `--force` jit reuses the previous verdict, a failed one included.
- Fast-tier tests time out at 15 s under load until `18276ff1` lands; force a rerun. Nextest exit 100 with tests "not run" and no failing test is tracked in `959ab809`; failed steps keep their log under `target/cargo-ci-logs/`.
- A main change under `crates/`, `scripts/`, `dev/` or `AGENTS.md` invalidates cargo-ci at branch tips; tracker-only drift does not, and is merged without re-gating. `code-review` declares no inputs and reruns at every new head.
- The worktree dispatch script refuses while another session holds uncommitted `dev/` files in `main`; use `git worktree add -b worktree-agent-<id> .agents/worktrees/agent-<id> main`.
- Main is shared: announce every main commit to the other leads, check `git diff --cached` is empty, stage only your own event lines (blob from `HEAD` plus your lines). A peer's `update-ref` can move `HEAD` under a staged index; re-stage.
- Hand landing (the script is in the durable directory as `land.py`): merge commit via `git commit-tree` over the branch tree plus carried main-side files, `update-ref`, `read-tree HEAD`, check out the changed paths, rebuild `events.jsonl` keeping other sessions' pending lines and order; copy `gate-runs/<issue>-*` from the worktree into the primary store.
- Under the landed reference checker a bare file name in issue prose resolves from the store directory: cite a repository path (resolves through a branch head while unmerged). Tracked files outside the store get no branch lookup.
- A chained background command cannot pause for a hold; kill the wrapper shell only.
- Codex workers cannot commit; the lead commits their edits.
- Earlier traps in [handoff.md](handoff.md) still bind unless resolved above.

## Open questions for the invoker

- `7854cfad` preset staleness, above.

## References

- [plan.md](plan.md), [breakdown.json](breakdown.json), [investigation.md](investigation.md), [progress.json](progress.json).
- Durable artifacts: `~/.local/share/gf2/6ba9be17-handoff-2/`.
- Steward handoff `dev/active/2caf738d-1-0/handoff-5.md`; charter `dev/research/2caf738d-charter.md`.
