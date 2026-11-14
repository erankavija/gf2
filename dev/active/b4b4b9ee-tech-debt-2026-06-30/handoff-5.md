# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 5

**Date:** 2026-11-14
**Prior handoffs:** retired; superseded history is in Git history.

## Current state

- Lead `gf2-5b` under steward `gf2-42`; the owner wound all work down at the end of the session. The epic and every open claim are assigned to `agent:gf2-5b`. Peer lead on the shared `main`: `gf2-d7` (d77176e5, BCH); `gf2-e9` (45ecf975 stream) wound down at its clean point.
- Landed this session, all gates passed, closed: `5a3abee1` (merge `8a22f0b7d`), `1b7228b9` (`6c861ab61`), `32a8bc0e` (`6892ecd54`), `5c1c87b7` (`afa6ba4d7`), `51f299d2` (`6d36576d4`); history retirement of this directory and of `dev/active/86b9c719-quality-documentation-tech-debt/` (`ebe4bc0be`, 105557 bytes freed).
- Open branches, each `worktree-agent-<id>` in `.agents/worktrees/agent-<id>`, worktree clean, main merged in at session start (`96fc9b751`) unless stated. Gate records older than the branch's last rework are stale: every gate below reruns on the current head.

| Issue | Head | Gate state | Next |
|---|---|---|---|
| `574ed420` | `9c155472f` (on main `dab7098ba`) | none run | evidence record in the issue lists what exists and what is missing: REQ-04 after side (its run is ENOSPC-suspect), before side and repetitions, REQ-03, the gates, the one-time three-concurrent REQ-02 run |
| `9c0568ee` | `2f3946001` | cargo-ci timed out twice (load); Claude review PASS recorded | cargo-ci, code-review (Codex) |
| `c34e8203` | `5c7dc8f0a` (on main `f9d7aa0d9`) | cargo-ci timed out (cold worktree) | cargo-ci, code-review (Codex); lead review recorded |
| `22bb2dcd` | `81568d0f6` | stale; Claude review PASS recorded, advisories in `90cd64a9` | cargo-ci, code-review (Codex) |
| `697b9178` | `a0a6319a5` | stale; Kani run of the three `bitvec` harnesses passed and is recorded; gf2-d7 approved the `bitvec.rs` hunks | cargo-ci, code-review; lands before `243d1160` rebases |
| `903dc74c` | `4a158a5a1` | none | after `697b9178` lands: merge main, `cargo-kani`, code-review, in a quiet slot |
| `d73ac633` | `a6515556b` | stale | cargo-ci, code-review |
| `e29fd776` | `0abf96190` | stale | cargo-ci, code-review, doc-review |
| `9ff05f12` | `056bc8faf` | none | cargo-ci, code-review, doc-review; gf2-e9 approved the `experiments/permanent-statistics` hunks |

- Filed this session, undispatched: `b5782224` (intermittent Rician runner test; likely an unseeded `run_coded`, see its lead note), `90cd64a9` (runner follow-ups of `22bb2dcd`, after it lands), `6eb12747` (ROCm detection ignores `ROCM_PATH`), `85e7e4c0` (supported-configurations Lean row). Ready and undispatched: `2e6e3557` (its prerequisite `51f299d2` is on main), `41517289` (owner action), the epic's ready set. `a33fda32` depends on gf2-d7's `9a1b24fc`; leave it undispatched.
- Nothing pushed. No build, gate or worker of this lead runs.

## Dispatch record

| Dispatch | Issue | Vendor | Model / tier | Reviewer |
|---|---|---|---|---|
| implement | `5c1c87b7` | Claude | Sonnet 5.5 (mid tier, owner ruling) | Codex gate, gpt-6.1-sol |
| implement | `c34e8203` | Codex | gpt-6-luna, xhigh | lead (Claude) beside the Codex gate |
| design, implement | `574ed420` | Claude | Opus 5.5 (high tier, owner) | Codex gate, pending |
| review | `22bb2dcd` (Codex rework) | Claude | Opus 5.5 | — |
| review | `9c0568ee` (Codex rename) | Claude | lead, Opus 5.5 | — |
| implement | history retirement | Codex | gpt-6-luna, xhigh (low tier, steward) | lead trap review |

## Decisions that bind the next lead

- Owner decisions reach the lead through the steward, and a relayed answer is the owner's answer. Ask the owner directly only when a session refuses the relayed action, then report the outcome to the steward.
- Vendors: alternate Claude and Codex per implementation dispatch, tier by difficulty; the reviewer is the other vendor. The `code-review` gate is fixed to Codex in `.jit/gates.toml` and stays unchanged; a Codex-implemented change gets a blocking Claude review beside it, recorded in the issue (steward).
- Load: at most two `cargo-ci` runs host-wide, one per lead slot, lent by message for a named run, until the `574ed420` three-concurrent REQ-02 run passes (steward); that run is approved once as a named exception, with the other lead's agreement and no other cargo-ci during it. All runs under `BUSY_HOST_OVERRIDE=1`, recorded. A gate that times out at 900 s under load is rerun and is not a failure against the issue; a warm `./scripts/cargo-ci.sh` pre-run is allowed only as the retry after a timeout. Indications and Kani runs need a quiet slot agreed with the other lead.
- Disk (steward): when an issue closes, remove its worktree's `target/` in the same wave and state `df -h /` in each landing report; below 40G free, stop builds and tell the steward. Results between about 14:15 and 14:40 local on 2026-11-14 are ENOSPC-suspect and are rerun.
- Tree budget: compute each merge's net byte change against `scripts/tracked-budget.toml` before merging; a refused merge is aborted and reported; the limit is not raised. Batch tracker commits on main, one per wave. The owner decided the budget stops counting `.jit/`.
- History (owner): superseded history files are retired, not edited; this directory keeps one current handoff, the assessment report and `progress.json`.
- `574ed420` (steward): option (d) approved; a CI-contract change goes to the steward before it merges.
- `2e6e3557` option A (owner, 2026-11-13): workflow clippy, build, test and coverage take `$(./scripts/cargo-features.sh)`; doc and doctest in the workflow and `scripts/cargo-ci.sh` read one variable; the workflow states its non-ROCm coverage; Kani, Lean and profile steps stay; workflow clippy adds `--features tuning-profile`. Record the owner approval in the issue.
- `c34e8203` (owner): versioned commit-msg hook, one source for the type list. Until it lands, check every subject (at most 71 characters, closed type list) before a merge.
- `f21b25ac` REQ-04 (owner, 2026-11-13): a drafted upstream report meets the reporting branch; filing is `41517289`, an owner action.
- File boundary: gf2-d7 owns `experiments/`, `dev/scripts/bench-window/`, `docs/reference/performance-evidence.md`, the BCH encode paths (`crates/gf2-coding/src/bch/encode.rs`, `crates/gf2-core/src/bitvec.rs`, `Shortened` in `crates/gf2-coding/src/transform/mod.rs`) and `crates/gf2-coding/src/ldpc/layered.rs`. Overlaps go to the steward. Main takes a hold from the other active lead before a merge, a main-checkout gate or a tracker commit.

## What happened

- Merged main into every open branch; resolved issue-JSON conflicts field-wise and one `benchmarks/README.md` conflict.
- `32a8bc0e` indication (branch against main `96fc9b751`, 22 cells of rows B2 and T2S, batches 256 and 4096, W1 and W6): ratio 0.973 to 1.008, inside run-to-run spread; recorded with gf2-af's approval.
- `1b7228b9`: merged-tree cargo-ci failed only on `reference-check`, fixed by citing the lock path with its `flock` writer; HIP-crate nextest passes under `--release`.
- `574ed420` filed on the owner's instruction: sccache gives every `rustc` a private jobserver, so codegen threads escape the cargo pool; designed, implemented, partly measured.
- Escalated and resolved through the steward: two file-boundary overlaps, the `grand_phase1_smoke.rs` overlap with `0fe85425`, gate timeouts under load, the tree ceiling, the full disk.

## Next

The benchmark window is armed for 2026-11-15 02:00 local; the host is idle by then.


1. `574ed420`: REQ-04 after and before sides and repetitions, the gates, REQ-03, then the three-concurrent REQ-02 run; report the runnable-thread ceiling to the steward, who restores the cap.
2. Gate `9c0568ee`, `c34e8203`, `22bb2dcd`, `697b9178`, `d73ac633`, `e29fd776`, `9ff05f12` in one slot; land each on a pass; then `903dc74c`.
3. Dispatch `2e6e3557`, `b5782224`, then `90cd64a9` after `22bb2dcd`, then the ready set.

## Traps

- sccache 0.17.0 strips the cargo jobserver from every compile it runs, in the server and in the client's local compiles of non-cacheable crates, so three concurrent cargo-ci runs drove the load above 60 and gates hit the 900 s timeout. Until `574ed420` lands, keep the cap of two.
- The disk filled at about 570 G of worktree `target/` directories, about 40 G each; remove `target/` when an issue closes.
- The `code-review` gate reads the branch's copy of the issue JSON. An issue edit made only on main fails the review: `5c1c87b7` failed REQ-02 because its inventory sat on main. Copy description edits to the branch before its review.
- `scripts/check-references.py` refuses a `/tmp` path in tracked text, issue JSON included, unless its own line writes or locks it, and a repository path written without its root; check after every issue edit.
- The versioned commit-msg hook is active in a worktree that contains it: it rejected a 72-character subject in `agent-c34e8203`.
- `/usr/bin/time` does not exist on this host; time with `date +%s`.
- A fresh worktree without seeded caches runs cargo-ci past the 900 s timeout; `git worktree add` instead of the dispatch script skips cache seeding.
- `.agents/skills/jit-execution-lead/scripts/land.sh` commits every pending tracker file on main under a nonconforming subject; merge by hand. Before a merge, save the other sessions' uncommitted lines of `.jit/events.jsonl`, check the file out, merge, and append them back. Stage only your own issue files and event lines.
- Two tracker commits made in sequence duplicated one event line (`a1f08b3d`); `jit validate` reports it. Build the staged `events.jsonl` from HEAD plus lines absent from HEAD.
- Kani harnesses that allocate need `#[kani::unwind]`; without it `mask_tail_zeros_padding` unwound `Vec::extend_with` past 6000 iterations in 27 minutes.
- The HIP crate's standalone nextest config applies a 15 s kill; run it with `--release`, or the debug CPU BCJR reference in its benchmark-shaped tests times out (`3ea744cf`).
- Stopping a background `jit gate evaluate` leaves its cargo children running; kill them by their worktree path.
- `dispatch-worker-worktree.sh` refuses when main has any uncommitted tracked change; a copy whose check ignores `.jit/` works.
- Rebasing or merging a worker branch over main conflicts on its issue JSON; resolve field-wise and validate the JSON after every resolution.
- zsh parses `$VAR:c...` as a history modifier: write `${B}:crates/...`.
- The permission classifier refused an acceptance-criterion edit relayed from another session; the steward made it in their session.
- A test that only pins a prose string is a literal inventory under `@/inv/shared-test-contracts`.
- Gate runs live in each worktree's ignored `.jit/gate-runs/`; copy them into the main checkout before `jit issue update --state done`.
- Run cargo-ci before code-review; read `gates_required` first, since `jit gate evaluate` on a gate the issue lacks exits 2.
- Do not stage under `.agents/upstream/`: it is not ignored; use `.agents/scratch/`.
- Retain a cold-build test failure's output and confirm on a warm run before reworking; a test killed even under the busy profile goes to the steward with a `ps` capture.
- Do not change main while a main-checkout cargo-ci gate runs.
- Do not seed the cache pool from existing worktrees; delete each worktree's `target/` before reclaim.
- Do not delete committed artifacts without a pin search (`git grep -n <filename> -- dev`).
- Verify an issue's premise against current code before dispatch; reject with evidence when it no longer holds.
- Reuse canonical test mechanisms; test the named public behavior directly; cite the API that owns an inventory instead of copying it into docs.
- Preserve hot-kernel code shape unless an indication supports the change.
- Keep follow-ups below the number of issues closed; fold small findings into the next issue touching that file (invoker's rule).
- Do not add or remove gates without the invoker's approval.
- Stop finished workers after their branch commit.
- Do not use `pgrep -f` in a wait loop; the harness blocks it. Use `ps -eo comm,etimes,args`.
- Keep merge subjects at or below 71 characters.
- In zsh, build optional `--gate` arguments conditionally, not through `${6:+--gate $6}`.
- Do not edit a benchmark-window run worktree while its queue line is active; keep builds out of benchmark windows.
- Confirm standard tables against the PDF page, not `pdftotext -layout`.
- After merging a branch that regenerates a shared generated file, rebase the next branch before merging.
- `git merge` runs no pre-commit hook; run `python3 scripts/check-tracked-budget.py` after each merge.
- Workers do not run `jit issue claim`; the lead claims on main.
- Do not kill unrelated processes by PID; stop only tasks this session started.

## Open questions for the invoker

None.

## References

- Epic `b4b4b9ee`; steward handoff `dev/active/2caf738d-1-0/handoff-3.md`; charter `dev/research/2caf738d-charter.md`; progress `dev/active/b4b4b9ee-tech-debt-2026-06-30/progress.json`; assessment `dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md`.
