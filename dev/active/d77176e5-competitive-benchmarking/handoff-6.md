# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5), session 5

**Date:** 2026-11-13
**Prior handoffs:** none in the tree; the live traps of the retired handoffs are condensed under Traps.

## Current state

- Lead `gf2-af` succeeded `gf2-3d`; steward `gf2-f3`. Nothing is pushed. No worker of this lead is running and the host is idle from this lead.
- Stream 2 (45ecf975, 67194d70's record, the history-rewrite rehearsal) moved to lead `gf2-a6`, together with `scripts/check-references.py` and `scripts/check-tracked-budget.py`. This lead keeps `experiments/`, `dev/scripts/bench-window/`, `docs/reference/performance-evidence.md` and the BCH encode paths.
- Every tracker change of this lead is committed on main (see the final commit of this handoff).

| issue | state | what remains |
|---|---|---|
| `c6b20bec` | done | Merged `ff3223f20`, closed. |
| `fd9d5416` | in progress | Window table recorded (REQ-02 met, REQ-03 falsified; gap owners named). The doc and provenance rework is on branch `fd9d5416-docs` (`47c26bb6c`, rebased on main), see "Gate runs at stop" below. Close when doc-review and research-review pass; merge the branch first. |
| `259acf37` | in progress | Window curve recorded with both contradictions. Branch `worktree-agent-259acf37` (rebased on main): comparator `47c4a0bcc` and per-code flooding reference `9fbfc1852`; cargo-ci passed on an earlier base, code-review failed twice (F1: REQ-05 needs the NMS 0.875 curve; F2: `GUARD_MEAN_MULTIPLE`). F2 rework outcome below. Depends on `d5cd53e0`. |
| `d5cd53e0` | in progress | Findings recorded (provisional). Harness `c5babbc43` on `worktree-agent-d5cd53e0`; its gate records and the findings text sit in that worktree's `.jit`. Code-review and research-review failed once; rework 1 outcome below. |
| `a6039c72` | in progress | Gates passed. Window line queued; record the table and close. |
| `6379e04e` | in progress | Changes 1 to 4 on `worktree-agent-6379e04e` (`7524c5679`, `25b095902`, rebased on main `ca5cfd6a9`); indication recorded in that worktree's `.jit`. Gate outcome below. |
| `7fbd1042` | backlog | WIP `c97b4c3d0` on `worktree-agent-7fbd1042` (stacked on `25b095902`): Codex stopped mid-task, untested. |
| `9a1b24fc` | backlog | New: DVB-T2 batch and workspace entry points (owner: library first). Design worker stopped before writing; branch `worktree-agent-9a1b24fc` at main has nothing. |
| `4443de83` | in progress | Two reworks on `worktree-agent-4443de83` (`afba20572`, `5766a2bb4`); code-review failed once (F2 is REQ-01 on DVB-T2 rows, which waits for `9a1b24fc`). Rerun code-review after `9a1b24fc` lands and the DVB-T2 rows time its `encode_batch_into`. |
| `243d1160` | in progress | Owner decision (b1) recorded. Worker on `worktree-agent-243d1160`, stacked on the pre-rebase 6379e04e head `b3224b9e6`; outcome below. |
| `3461a52a` | in progress | WIP planning record `cb96b9810` on `worktree-agent-3461a52a` (Codex stopped). Review and finish, then plan-review. |
| `b8157e14` | ready | Blocks 1.0 (owner). Diagnosis not dispatched: it waits for 259acf37's F2 change to `crates/gf2-coding/src/ldpc/layered.rs`. |
| `60c82c4c`, `5e71e993`, `92ec2e2b`, `2bca2b28` | backlog | Designs merged (`75ea2c792`, crate paths fixed `72cfd2547`) and linked. |

Chain order (one file at a time along `crates/gf2-coding/src/bch/encode.rs`, `crates/gf2-core/src/bitvec.rs` and `Shortened` in `crates/gf2-coding/src/transform/mod.rs`):

`6379e04e` → `7fbd1042` → `9a1b24fc` → `92ec2e2b` → `2bca2b28`; `243d1160` after `6379e04e`; `4443de83` after `9a1b24fc`; `60c82c4c` after `4443de83`; `5e71e993` after `60c82c4c`.

## Gate runs and worker outcomes at stop

- `fd9d5416` (branch `fd9d5416-docs`, `47c26bb6c` plus gate records `4c94c6686`): research-review PASSED; doc-review FAILED (second doc-review failure on this issue).
  - F1: the harness rustdoc, and the earlier fd9d5416 timing note, call the first-call minimum the reported statistic; it feeds only the call-budget projection. The issue note is corrected on main; the rustdoc on the branch still needs the fix.
  - F2: `docs/reference/performance-evidence.md`'s "Both:" bullet now applies the cutover digest rule to `compare.sh bch`, which validates before timing (`experiments/encode-compare/external/src/main.rs`, `reproduces`); the bullet needs one rule per harness.
  - F3 (advisory): the "gf2 path" bullet's entry points; `4443de83` changes that bullet.
  - Per the steward's rule, a second review failure goes to the steward before rework.
- `259acf37` F2 (branch at `4dde2f74e`, Claude Opus, partial): `DEFAULT_GUARD_MULTIPLE = 2.0` with `with_guard_multiple` and `guard_multiple()`; a bit-exact test against the old formula passes on DVB-T2 short R1/2 and NR BG1 frames. The basis is the consistent-Gaussian LLR symmetry: the share above twice the mean equals the share below zero. No i8-alphabet ratio or degree gives 2 for both configurations. Not yet run: the full `crates/gf2-coding/tests/ldpc_layered_contract.rs` binary (the `test(/layered/)` filter matched 4 tests) and clippy. The 00:00 deadline was overtaken by the wind-down. The rerun lines stay commented out in the queue.
- `d5cd53e0` rework 1 (branch `b966634b3`, then records `7cdf5e172`): all eight findings fixed.
  - Wilson intervals on every FER; a typed recheck enum; a run header with the executable digest; a diagnosis-steps script that writes logs; citations.
  - The REQ-03 "total dynamic range" claim is withdrawn. The `layered-range` step shows that the message bound, the v→c bound and the resolution all matter: scale 3 with message limit 40 lost 4 frames at 1.3 dB against 36 for the default.
  - REQ-02's verdict (no flooding defect) stands.
  - The corrected tables are recorded in the issue on the branch (commit after `7cdf5e172`); the step logs stayed outside the tree and are not preserved.
  - Reviews are not rerun. The issue Background's "near 1 dB" sentence has no source: drop it or replace it with the measured 1.0 to 1.2 dB transition at NMS 0.875.
- `6379e04e`: cargo-ci was stopped mid-run at the wind-down (load about 60); it has no verdict. Code-review has not run.
- `7fbd1042` (`c97b4c3d0`), `243d1160` (`55ca8205c`, stacked on `b3224b9e6`, the pre-rebase 6379e04e head) and `3461a52a` (`cb96b9810`, a 272-line plan): WIP from stopped workers, untested and unreviewed. Not for main.
- Load: 59.7 at 23:12, with no lead lock. The ps capture went to the steward. At that moment rustc builds of gf2-core in several worktrees, a HIP clang compile, a Kani cargo run, check-references and other leads' dispatch scripts dominated.

## Rulings that bind the next lead

- `243d1160`: owner chose (b1), a safe `enum { Inline([u64; 2]), Heap(Vec<u64>) }` in gf2-core. Admission gate: indications before merge on matvec and transpose-matvec, LDPC batch decoding (reused outputs, lane/QC/scalar routes), OSD row materialisation, long XOR/popcount kernels, and encode at batches 1/16/256/4096. Any regression outside run-to-run spread returns to the owner; do not tune around it. Indications need a ten-minute quiet slot agreed with the other leads. Implementer strongest Claude tier, reviewer Codex.
- `259acf37` ruling (a): the flooding reference uses each code's layered check rule (DVB-T2 NMS 0.875, NR NMS 0.75). REQ-05's text is unchanged. Nothing lands against a failing review: the window rerun runs from `jit-259acf37-run` detached at the branch head after cargo-ci passes on the branch, and the record names the test binary's digest. F2 is fixed in 259acf37 before the curve is measured; if no structural form preserves today's frame scale bit-exactly, record that, move F2 to `b8157e14` as a hard criterion, and run the curve without it. A third code-review failure on new grounds goes to the steward before rework.
- `d5cd53e0`: its conclusions are provisional until both reviews pass; `b8157e14`, `3461a52a` and `259acf37` say so. A second review failure goes to the steward.
- `b8157e14` blocks 1.0 (owner); it is reachable from milestone `2caf738d` through `d77176e5`. Its first step is the diagnosis (a scale between 2 and 4 or a wider belief type, no waterfall cost).
- `4443de83`: library first (owner). REQ-01 stands; the proposed DVB-T2 amendment was declined. `9a1b24fc` provides the entry points.
- W2 bar (owner): both arms count, the faster per row sets the bar, and the AFF3CT W2 arm is labelled harness-composed.
- Gating (owner, "max throughput"): no test lock; independent branches run cargo-ci concurrently in their own worktrees under `BUSY_HOST_OVERRIDE=1`, recorded as such; reviews start as soon as cargo-ci passes; holds on main stay for merges and main-checkout cargo-ci; a killed test is reported with a `ps` capture.
- Stacking: start a chained issue on its base branch once the base's changes are committed and its tests pass; rebase on landing; gate in chain order.
- Vendor alternation: this session dispatched Claude 7 (6379e04e, 259acf37 reference and F2, d5cd53e0, fd9d5416-docs, 243d1160) and Codex 7 (4443de83 three rounds, 259acf37 comparator, 3461a52a, 9a1b24fc, 7fbd1042). Continue alternating per dispatch.
- Relay rule: owner decisions come through the steward; a relayed answer is the owner's. Ask the owner directly only when the relay cannot work, then tell the steward.

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

## Next

1. Run `6379e04e`'s cargo-ci and code-review on its branch, land it, then rebase `worktree-agent-243d1160` and `worktree-agent-7fbd1042` onto it and finish both (WIP is untested).
2. Rerun d5cd53e0's reviews on its branch (r1 tables are in its issue there), and bring fd9d5416's second doc-review failure to the steward. Record the `a6039c72` window table after the 2026-11-14 window and close it.
3. Run the full layered contract binary and clippy on `259acf37`'s F2 commit, get the reviewer's verdict on the LLR-symmetry basis, gate the branch, and enable the two `#PENDING-GATE 259acf37` queue lines. Then dispatch `b8157e14`'s diagnosis and the `9a1b24fc` design (Codex next by alternation).

## Open questions for the invoker

None open from this lead; the owner answered every question raised tonight.

## References

- Progress: `dev/active/d77176e5-competitive-benchmarking/progress.json`.
- Designs: `dev/active/d77176e5-competitive-benchmarking/design-60c82c4c.md`, `design-5e71e993.md`, `design-92ec2e2b.md` in the same directory.
- Charter: `dev/research/2caf738d-charter.md`.
- Queue: `dev/scripts/bench-window/queue.tsv`.
