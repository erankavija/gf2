# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5), session 6

**Date:** 2026-11-14
**Prior handoffs:** none in the tree (earlier sessions' handoffs are in Git history). Steward handoff: `dev/active/2caf738d-1-0/handoff-3.md`. Charter: `dev/research/2caf738d-charter.md`.

## Current state

- Lead `gf2-d7` succeeded `gf2-af`; steward `gf2-42`. Other leads: `gf2-5b` (b4b4b9ee tech debt); `gf2-e9` (45ecf975 stream) is winding down, after which holds on main come from `gf2-5b` only and the two cargo-ci slots are this lead's and `gf2-5b`'s. Nothing is pushed. The owner ordered a wind-down; no worker of this lead runs.
- Landed today: `259acf37` (merge `46363757a`), `d5cd53e0` (`4ce5f59e1`), `fd9d5416` (`9464e4ee0`), `bb28cf4f` (`bba4322cb`), the history retirement of `dev/active/d77176e5-competitive-benchmarking/` and `dev/active/ae03bcd0-general-bch/` (`650288545`, 201 179 bytes freed). `a6039c72` closed on its window table.

| issue | state | branch head | what remains |
|---|---|---|---|
| `6379e04e` | in progress | `37c0e556a` | cargo-ci passed on `c60aaf35e` (main merged), 2026-11-14 12:47 to 12:56Z under `BUSY_HOST_OVERRIDE=1`, no write errors in its log; record on the branch. Next: code-review (Codex gate; implementer Claude), land, clean its `target/`. |
| `9a1b24fc` | ready | `62d95b89d` | Design rework 2 was stopped mid-run (Codex) and committed unreviewed. Check every round-2 finding (N1 blocking, N2-N6) and the steward's conditions against it, finish, then round 3 by Claude Opus. A round-3 failure goes to the steward before rework 3, the last attempt. Implementation waits for an approved design. |
| `243d1160` | in progress | `55ca8205c` (WIP, untested) | Rebase onto `9a1b24fc` branch A after A lands; keep the `697b9178` Kani harnesses valid against the inline enum. Admission-gate ruling below. |
| `7fbd1042` | backlog | `c97b4c3d0` (WIP, untested) | Finish on branch A after A lands; implements the lockstep `batch_granularity` override A introduces. |
| `3461a52a` | in progress | `fdd47c14c` | Planning record draft (rework 1 passed lead review findings 1-4). REQ-02 open: rewrite the decision to the owner's accepted form, add the DE estimates, and add the DE tool and table as proposed issues; then plan-review. Validation slice queued (below). |
| `b8157e14` | in progress | `worktree-agent-b8157e14` | Diagnosis worker (Claude Opus) was told at wind-down to stop its simulations and commit a partial `findings-b8157e14.md` (labelled partial, merging main first); verify that commit exists and the worktree is clean. Partial logs stay under `.agents/worktrees/agent-b8157e14/target/b8157e14/`. Then the DE-first order for REQ-03. |
| `4443de83` | in progress | `5766a2bb4` | Waits for `9a1b24fc`; times `encode_batch_into` on the DVB-T2 rows in the trait form. The fd9d5416-F3 docs bullet follows the new form. |
| `47ec8c01` | ready | none | Rides with `b8157e14`'s implementation in `crates/gf2-coding/src/ldpc/layered.rs`. |
| `7f9751af` | ready | none | Wilson-interval convergence; `dev/tools/tuning-campaign-support/` is outside this lead's file boundary. |
| `92ec2e2b`, `2bca2b28`, `60c82c4c`, `5e71e993` | backlog | as before | Chain order below. |

## Next

1. Benchmark window armed by the owner: timer `gf2-bench-window-20261115` at 2026-11-15 02:00 local, `GF2_WINDOW_MAX_EST_MIN=0`. Its command creates or detaches `jit-3461a52a-run` at main and runs `dev/scripts/bench-window/run-window.sh`; only the two `3461a52a` lines lack done markers. Do not create that worktree and leave the queue as it is. Afterwards read `.agents/bench-window/window.log` from the last `window start` and record both curves in `3461a52a` with the executable digest, host and start loads; a curve contradicting the DE estimate is a falsification.
2. `6379e04e`: code-review, land, then `9a1b24fc` design: check rework 2, round 3 review.
3. `9a1b24fc` branch A per the order below; `ae03bcd0` completion after A lands.
4. `b8157e14`: per its row; then `3461a52a` record per its row.

## Rulings that bind the next lead

- `9a1b24fc` (owner): one public batch-encoding trait is the only batch form of `BchCode`, `LayoutView`, the DVB-T2 code and the LDPC and Richardson-Urbanke encoders; no alias. REQ-01 rewritten, REQ-05 and REQ-06 added. REQ-06 has no exemption: `experiments/encode-compare/cutover/pre.rs` is deleted with the cutover and everything citing it is swept to current behaviour; results recorded from it stay in their issues. The inherent `encode_batch_family_into` is removed (lead decision under REQ-01).
- `9a1b24fc` split and order (steward): two stacked branches under the one issue. Order: `6379e04e`; branch A (trait, BCH and `LayoutView` cutover, `Shortened` impl, BCH callers, the pre-cutover sweep) on its own cargo-ci and code-review; then `243d1160` and `7fbd1042` on A; branch B (LDPC and RU convergence) in parallel after A; the issue's gates run on B's final head and it closes after B; then `4443de83`, `92ec2e2b`, `2bca2b28`, `60c82c4c`, `5e71e993`. The tracker edge `9a1b24fc` → `7fbd1042` was removed for this order.
- `3461a52a` (owner): REQ-02 stands. A table of the density-evolution optimum per code, rate and iteration cap, produced by an in-tree offline tool and caller-overridable, is the structural rule if window curves confirm it; a contradicting curve is recorded as a falsification. The same literature-then-measurement order applies to `b8157e14` REQ-03.
- `259acf37` F2 (owner): the LLR-symmetry basis of `DEFAULT_GUARD_MULTIPLE` is structural; its rustdoc follow-up is `47ec8c01`.
- `ae03bcd0` (owner): this lead's after the chain; run its completion after `9a1b24fc` branch A lands; record REQ-13/14 as they are. `a33fda32` depends on `9a1b24fc` (its batch half is delivered there; its narrowing is decided when A lands); `3931ac6f` notes the batch half.
- `243d1160`: owner (b1), a safe `enum { Inline([u64; 2]), Heap(Vec<u64>) }` in gf2-core. Admission gate: indications before merge on matvec and transpose-matvec, LDPC batch decoding, OSD row materialisation, long XOR/popcount kernels, and encode at batches 1/16/256/4096; any regression outside run-to-run spread returns to the owner. Implementer strongest Claude tier, reviewer Codex.
- `b8157e14` blocks 1.0 (owner). `4443de83`: library first; REQ-01 stands. W2 bar: both arms count, the faster per row sets the bar, the AFF3CT W2 arm is labelled harness-composed.
- History files (owner): superseded handoffs and prompt files are retired, not edited; one current handoff per directory.
- Disk (steward): when an issue closes, its lead removes that worktree's `target/` in the same wave and reports free space; below 40 GB free, stop builds and tell the steward.
- cargo-ci: at most two runs host-wide until `574ed420` (sccache jobserver fix) lands; borrow a slot by message for a named run and report its end. Runs under `BUSY_HOST_OVERRIDE=1`, recorded as such. A run timed out or ENOSPC-affected is rerun and is not a gate failure.
- A second review failure on one issue, a criterion change, an owner question and an admission-gate regression on `243d1160` go to the steward. Owner decisions reach the lead through the steward.

## Evidence locations

- Density-evolution research for `3461a52a` and `b8157e14` (estimates, single-seed sample DE; precision about 0.05 dB DVB-T2 and 0.1 dB NR): the findings and scripts were in the session scratchpad and are summarised in `3461a52a`'s "Density-evolution estimate" section. refdb theme `ldpc-min-sum-parameter-selection` holds the seven ingested sources. Estimated optima: DVB-T2 normal R1/2 about 0.90 at cap 50, DVB-T2 short R1/2 at least 0.95, NR BG1/BG2 low and middle rates 0.6 to 0.7, NR at rates 0.77 to 0.88 0.75 to 0.80. Falsification candidate: Zhang et al. report 0.75 for the (16200, 7200) code.
- Queued window lines: `3461a52a` slice in `dev/scripts/bench-window/queue.tsv` (flooding 0.875 and 0.90 at 1.0 to 1.3 dB, 0.75 at 2.0 to 2.6 dB; estimate 155 min), worktree `jit-3461a52a-run`. The owner has not armed a window.
- NR validation needs a campaign TOML for `crates/gf2-coding/src/bin/sim_runner.rs` (verify BPSK and cap 50 first); DVB-T2 short and other rates over BPSK need an extension of `experiments/aff3ct-ldpc-gaps/src/bin/dvb_t2_fer_diag.rs`.

## Dispatch record (session 6)

| issue | task | vendor, tier |
|---|---|---|
| `9a1b24fc` | design | Codex gpt-6.1-sol high |
| `bb28cf4f` | implementation | Claude Sonnet |
| `d5cd53e0` | rework 2 | Codex gpt-6-luna medium |
| `b8157e14` | diagnosis | Claude Opus |
| `fd9d5416` | doc rework | Codex gpt-6-luna high |
| `3461a52a` | condense plan | Codex gpt-6.1-sol high |
| `3461a52a` | rework 1 | Claude Sonnet |
| `3461a52a`, `b8157e14` | DE research (out of turn: research-librarian skill and web are Claude-side) | Claude Sonnet |
| `9a1b24fc` | design revision for the owner's scope | Codex gpt-6.1-sol high |
| retirement | history files | Claude Haiku |
| `9a1b24fc` | design review rounds 1 and 2 | Claude Opus |
| `9a1b24fc` | design rework 1, rework 2 (stopped) | Codex gpt-6.1-sol high |

Claude 7, Codex 7 dispatches; the next is Claude. The reviewer of a change is the other vendor from its implementer; the review gates' configured reviewer is Codex gpt-6.1-sol, so a Codex-implemented change also gets a Claude review.

## Traps


- `grep -c 'window end'` on the window log counts earlier windows too; wait on the last `window start` or `window end` line.
- Run `df` before blaming code: per-worktree `target/debug` directories filled the disk, and two cargo-ci runs failed with "No space left on device".
- A pause request does not stop a build a worker already started; ask workers to confirm idle before a window.
- `evaluate-many` prints verdicts only at the end; read `jit gate status <id> <gate>` for progress.
- A merge that touches a tracker file a gate run modified conflicts; take gate state from main and title and description from the branch.
- `! command` under `set -e` asserts nothing in a shell test.
- A comparison library's default build is not its fastest configuration (bchlib needed `CONFIG_BCH_CONST_PARAMS`); read its build options before recording a ratio.
- A criterion that asks for a table can be met by a test; read the criterion before generating inputs (`de311215` REQ-04).
- The orphan rule blocks `std::ops` impls in `gf2-core` for types defined in a kernel crate; put an element type with operators where its operators are implemented.
- Appending a bullet to a description that ends with criteria lands it inside `## Success Criteria`; add a `## Notes` heading first.
- Count a commit subject before committing; the limit is 71 characters (`aa98b11f2` was 72).
- `cd` into a worktree in a Bash call moves the session's working directory; use `git -C` and absolute paths.
- `jit gate evaluate` on a leased issue needs `JIT_AGENT_ID=lead:<id>`; without it the gate refuses to write and the run looks like a failure.
- A gate that declares its inputs returns a cached verdict, including a cached failure; use `--force` to rerun.
- cargo-ci has a 900 s timeout and the fast tier kills tests at 15 s; on a cold main build after a merge, run `./scripts/cargo-ci.sh` in the main checkout first, with worker builds idle.
- Gate runs read the main checkout; agree a hold on main with the other lead before a run and before a merge.
- `systemd-run --user` is refused by the permission classifier; a short window then runs as a background command of the session.
- Worktree checkouts carry their own `.jit`; `jit gate status --all` there shows no runs.
- Tracked issue text must not carry host-local or `.agents/` paths; name digest files by basename.
- Chain checks with `&&`: `;` runs the next command after a failure. `set -- ${=p}` splits words only in zsh.
- `jit doc add` scans assets by default and copied 22 dead external receipt URLs into each issue; pass `--skip-scan`.
- The execution-lead skill's scripts include no landing script; merge with `git merge --no-ff` and a conforming `chore(jit:<id>): merge ...` subject.
- `progress.json` files hold no measured values (owner ruling C2).
- `scripts/check-tracked-budget.py` refuses result tables and run records under `experiments/`, `dev/`, `benchmarks/` and `crates/`.
- The FER curve runs under libtest (`cargo test --release`), not nextest: the slow profile kills any test at 600 s.
- `GF2_LAYERED_FER_GRID` replaces the grid of both decoders; the comparator fails unless each crossing is bracketed, so a grid starting above the layered crossing near 1.13 dB fails by construction.
- Session 1 traps still in force:
  - Edit nothing in a queue worktree or in `dev/scripts/bench-window/run-window.sh` while a window runs: bash reads the script as it executes.
  - A queue line repeats only after its `.agents/bench-window/<key>.done` marker is removed; the key is sha256 over issue, worktree and command joined by tabs, first 16 hex digits.
  - `systemd-run --user` needs `XDG_RUNTIME_DIR=/run/user/$(id -u)` and `DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus`.
  - A merge committed without a hook run can exceed the tree budget; merge with `--no-commit`, then commit, or run `python3 scripts/check-tracked-budget.py`.
  - `git add -A dev` stages host-local data under `dev/`; add named paths. `cmd | tail && git commit` commits after a failed `cmd`.
  - Frames/s indications at host load above about 20 swing by 2x; use instruction counts or interleaved runs there. Run a control before a cache-capacity explanation.
  - The recorded LDPC bundles carry $y/\sigma^2$, half the natural-log LLR scale.
- `echo =====` in zsh expands `=word` as a command path and fails; quote it.
- The window runs under the systemd user manager: `PATH=/usr/local/bin:/usr/bin`, no `ROCM_PATH`. Without `ROCM_PATH`, hipcc's clang adds `-isystem /include`; `93df507a` makes the build script pass `--rocm-path`.
- The arming command, not `dev/scripts/bench-window/run-window.sh`, detaches run worktrees at main. Never list a worktree pinned to a branch head (`jit-259acf37-run`).
- The reference checker now refuses a bare file name outside its own directory, and any bare name in issue prose. Write repository paths. Run `python3 -B scripts/check-references.py` before every main commit; `&&` after `| tail -1` does not stop on its failure.
- Main's `.jit/events.jsonl` holds several leads' uncommitted lines. Stage only your own lines (hash-object plus update-index); a `git checkout -- .jit/events.jsonl` before a merge drops the others' lines, so save the file first and re-append the missing lines after.
- `jit gate evaluate` inside a worktree records on that branch, and `jit issue update` there edits the branch's `.jit`. Merging such a branch conflicts on `updated_at` in the issue JSON; resolve by keeping HEAD's timestamp.
- Gate-run records live in the ignored `.jit/gate-runs/`; after merging a gated branch, copy `.agents/worktrees/<wt>/.jit/gate-runs/<issue-uuid>/*.json` into main's, or `jit issue status` reports "Gate run not found".
- `jit issue create` with labels passed through one unquoted zsh variable sends them as one argument and fails; pass each `-l` literally.
- The window runner's done markers key on issue, worktree and command: changing a line's command reruns it, and a failed job leaves no marker, so it reruns next window unless commented out.
- `DvbT2BchCode` is `Shortened<DvbT2MotherCode>` and has no batch or workspace entry point; a harness that lifts into the mother inside the timed call times harness work.

- The tracked-tree budget check runs on every commit, branch commits included: a branch whose base predates a freeing merge on main cannot commit until it merges main.
- Main's working tree carries other leads' uncommitted tracker files. Build merges with `git merge-tree --write-tree` plus `git commit-tree`, `git update-ref` and per-path checkout, append the branch's new event lines, and stage your own files and event lines by index; never check out or reset `.jit/events.jsonl` (a concurrent `jit` write lands between checkout and restore).
- `git show "$B:path"` in zsh applies the `:c`-style history modifiers to `$B:c...`; write `"${B}:path"` or a literal revision.
- A Codex worker cannot receive messages mid-run; stopping it with the task tool leaves its partial edits uncommitted.
- `jit issue show` in a worktree fails for an issue whose gate-run records are only in main's ignored `.jit/gate-runs/`; read the issue JSON or copy the records.
- A gate run killed or starved of disk can leave test fixtures in the worktree; check `git status` before the next gate (fixture-leak step).
