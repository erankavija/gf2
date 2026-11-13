# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5), session 5

**Date:** 2026-11-13
**Prior handoffs:** `dev/active/d77176e5-competitive-benchmarking/handoff.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-2.md` to `dev/active/d77176e5-competitive-benchmarking/handoff-5.md`

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

Filled at the stop; see the section appended below.

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

- The traps of handoff-2 to handoff-5 stay in force.
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

1. Read the outcomes section below. Rebase `worktree-agent-243d1160` onto the rebased `worktree-agent-6379e04e`, then land `6379e04e` once both its gates pass; rebase `7fbd1042` and finish it (WIP is untested).
2. Close `fd9d5416` after its reviews pass and `fd9d5416-docs` is merged; record the `a6039c72` window table and close it.
3. Finish the `259acf37` F2 rework (or apply the fallback ruling), gate the branch, enable the two `#PENDING-GATE 259acf37` queue lines; then dispatch `b8157e14`'s diagnosis and the `9a1b24fc` design (Codex next by alternation).

## Open questions for the invoker

None open from this lead; the owner answered every question raised tonight.

## References

- Progress: `dev/active/d77176e5-competitive-benchmarking/progress.json`.
- Designs: `dev/active/d77176e5-competitive-benchmarking/design-60c82c4c.md`, `design-5e71e993.md`, `design-92ec2e2b.md` in the same directory.
- Charter: `dev/research/2caf738d-charter.md`.
- Queue: `dev/scripts/bench-window/queue.tsv`.
