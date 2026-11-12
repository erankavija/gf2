# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5), session 3 close

**Date:** 2026-11-12
**Prior handoffs:** `dev/active/d77176e5-competitive-benchmarking/handoff.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-2.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-3.md`

## Current state

- Main is `28a40f5e4` plus the commit that adds this file. Nothing is pushed, no worker of this lead is running, and no claim is held except `ebfaff31`.
- `scripts/cargo-budget.sh` is gone (4f21da96, merged by gf2-79). Commands run directly, as `AGENTS.md` states. Briefs drop the prefix.
- Every branch of this lead is merged into main. That includes `jit-fd9d5416-cutover` (merge `06ba1503d`; `git log main..jit-fd9d5416-cutover` is empty) and `worktree-agent-ebfaff31`. Checking the cutover worktree out at main runs the merged cutover harness.

Open issues of this lead:

| issue | state | what remains |
|---|---|---|
| `fd9d5416` (ae03bcd0) | in progress | Code merged. Read tonight's cutover table and record it with the REQ-03 falsification. Replace the indication in the three gap issues. Then run doc-review and research-review. |
| `259acf37` | in progress | REQ-05 reworded to the one-sided bound (owner). Read tonight's FER curve and record the DVB-T2 crossings. Rerun code-review; its last finding was the missing flooding crossing. |
| `ebfaff31` | ready, claimed | Merged in `fdf587ee1`. Doc-review failed at `28a40f5e4` with five findings on `docs/reference/performance-evidence.md` (below). Not yet reworked. |
| `a6039c72` | ready | GF(2^8) 4 KiB axpy and n = 1024 byte product regressions; not started. |
| `60c82c4c`, `5e71e993`, `92ec2e2b` | backlog, depend on `fd9d5416` | REQ-03 gaps on the 2026-11-11 indication; the window table replaces it. |
| `9d1c580c` | ready, low | BCH reflected-domain fold; not started by owner decision. |
| `ae03bcd0` | backlog | Closes after `fd9d5416`: epic gates and criteria mapping. |
| `d77176e5` | backlog | Open children: `259acf37`, `ebfaff31`, `12a3c312` and `f768c925` (both ready; they wait by owner order). |

ebfaff31 doc-review findings at `28a40f5e4`:

1. F1 (line 131): the gf2x command does not measure the four- or nine-word wide-field products or Barrett reduction the entry describes.
2. F2 (line 175): the F3 permanent reproduction runs one execution and emits raw CSV, without the dispersion calculation or executable digest the entry describes.
3. F3 (line 6): the page promises that removed values stay retrievable through the cited issues, but some removed comparisons have no citation left.
4. F4 (line 36): the LDPC entry conflates the flooding batch-decoder routes with the separate i8 layered decoder and its cap of 25.
5. F5 (line 147): the GF(2^8) entry names an install prefix without the reference libraries' fastest build configurations. The configurations: ISA-L and gf-complete as built under `.agents/ext/6c6b09b1`; M4RI and M4RIE at `-O3 -march=native`, SSE2, no OpenMP.

## Tonight's window (2026-11-13 02:00 local)

- The owner arms it; this session's permission classifier refuses `systemd-run --user`. The command sent to the steward detaches `agent-de311215`, `agent-fd9d5416` and `jit-259acf37-run` at main, then runs `dev/scripts/bench-window/run-window.sh` with `GF2_WINDOW_MAX_EST_MIN=0`. `agent-67194d70` stays at `259366a0b` (gf2-79).
- Queue order, `dev/scripts/bench-window/queue.tsv`:
  1. `67194d70` validation (gf2-79's).
  2. `de311215` decode table and route profiles, with new keys after the barrier fix and the digest change.
  3. `fd9d5416` cutover (`CUTOVER_CALL_BUDGET_S=2000`, `CUTOVER_ROUNDS=3`).
  4. `259acf37` DVB-T2 FER curves on 1.0 to 2.0 dB.
- The be6d6f45, 040bc2b8, befccab3 and c9cb0dd2 lines skip on their done markers.
- What to read after it:
  - `.agents/bench-window/window.log` from the last `window start` line.
  - Each job's `.agents/bench-window/<key>.out`, the last table in each.
  - The cutover table under `target/encode-compare/cutover/<UTC>/` in `agent-fd9d5416`.
  - The FER curve, whose per-point lines are in the 259acf37 job's `.out` (libtest `--nocapture`).
  - Record each with host, commit and digests, in the issue that owns it.
  - de311215 is done; its new decode table goes in as a falsification check of the multi-worker cells measured before `a02010986`.

## Next

1. After the window: record the tables above.
2. Close `fd9d5416` through its gates, then `ae03bcd0`.
3. Gate `259acf37` with code-review.
4. Rework `ebfaff31` against its five findings, then rerun doc-review.
5. Dispatch `a6039c72`.
6. Before the d77176e5 container gate: reconcile `surfaced_pitfalls` in `progress.json`.

## Traps

- The traps of handoff-2 and handoff-3 stay in force, except the `cargo-budget.sh` command forms, which 4f21da96 removed.
- The FER curve runs under libtest (`cargo test --release`), not nextest: the slow profile kills any test at 600 s.
- `GF2_LAYERED_FER_GRID` replaces the grid of both decoders, and the comparator fails unless each crossing is bracketed. A grid that starts above the layered crossing near 1.13 dB fails by construction.
- After gf2-79's coming daef2dc3 landing, `scripts/check-tracked-budget.py` refuses result tables and run records under `experiments/`, `dev/`, `benchmarks/` and `crates/`, and `scripts/check-references.py` checks `@/inv` citations and bare deleted file names.
- Gate runs on main need a hold agreed with gf2-79 both ways.

## Open questions for the invoker

None. The owner answered 259acf37 REQ-05, fd9d5416 REQ-02 and REQ-03, the docs ruling (charter D-6) and the windows. Arming tonight's timer is the owner's action.

## References

- Progress: `dev/active/d77176e5-competitive-benchmarking/progress.json`.
- Charter: `dev/research/2caf738d-charter.md`.
- Dashboard: `https://claude.ai/artifact/4ZoceRZh2aYohYxaumogP7`.
