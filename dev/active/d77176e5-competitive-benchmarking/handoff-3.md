# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5), session 3

**Date:** 2026-11-12
**Prior handoffs:** `dev/active/d77176e5-competitive-benchmarking/handoff.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-2.md`

## Current state

- Lead session gf2-7b, recruited by steward gf2-94 on the owner's instruction. A second lead, gf2-79, drives `45ecf975`, `daef2dc3` and `4f21da96`, and owns `experiments/` outside the four harness directories (`encode-compare`, `clmul-compare`, `gf256-baselines`, `aff3ct-ldpc-gaps`), plus `dev/` and `scripts/check-*.py`.
- Done this session: `07a1813a`, `069e3c33`, `de311215`, `040bc2b8`, `8a8a3bc9`, `befccab3`, `c9cb0dd2` (epic d77176e5 and the handoff-2 review items), and `1ce7e8a5`, `cbd1eb28` (epic ae03bcd0, by owner order).
- Waiting on owner decisions, relayed through gf2-94:
  - `259acf37`: REQ-05 needs the DVB-T2 normal R1/2 flooding crossing at FER 1e-3, an hours-long simulation (`crates/gf2-coding/tests/ldpc_layered_fer.rs` under `GF2_BENCH=1`, grid narrowed with `GF2_LAYERED_FER_GRID`). It also needs a ruling on whether "within 0.1 dB" is one-sided.
  - `fd9d5416`: code merged (REQ-01 tests, `experiments/encode-compare/cutover.sh`). Its queue line is not yet in `dev/scripts/bench-window/queue.tsv`; it waits on the per-call budget decision. Its gates (doc-review, research-review) run after the window table. Worktree `.agents/worktrees/agent-fd9d5416` is kept for the queue line.
- New issues: `9d1c580c` (BCH reflected-domain fold, low, depends on `040bc2b8`) and `a6039c72` (GF(2^8) 4 KiB axpy and n = 1024 byte product regressions, depends on `c9cb0dd2`).
- Epic d77176e5 children not done: `259acf37` (above), `12a3c312` and `f768c925` (ready, untouched by owner order). Epic ae03bcd0 not done: `fd9d5416`.
- No worker is running. Claims: the epic `d77176e5` and `259acf37`, `fd9d5416` under `lead:d77176e5`.

## What happened

- Recorded the 2026-11-11 night window in five issues; updated the dashboard `https://claude.ai/artifact/4ZoceRZh2aYohYxaumogP7`, now at the 2026-11-12 values.
- `07a1813a` REQ-01 amended by owner decision, with the falsified helper measurement.
- `c9cb0dd2`: the 1 MiB axpy gap was a destination at 16 mod 32 in gf2's AVX2 kernel; an aligning scalar head took the cell from 0.98 to 1.33 of ISA-L (window 2026-11-12).
- One repository-root convention for the bench window and the harnesses: `run-window.sh` exports `GF2_EXT_ROOT`; the harnesses find `ccx1-bench-flock.sh` through `repository_files.py`; every harness prints executable SHA-256 digests via `experiments_common::write_executable_digests`.
- `de311215`: the fast-bench workers now start their timed calls together after warm-up; multi-worker tables recorded before `a02010986` include the skew, noted in the issue.
- 8a8a3bc9's tutorial no longer commits measured rows (`@/inv/committed-evidence-budget`, doc-review verdict).
- A short window ran 01:33Z to 01:36Z on 2026-11-12 from this session (gf2x and GF(2^8) lines with digests).

## Next

1. Act on the owner's answers: queue the `259acf37` simulation and the `fd9d5416` cutover line as decided. Arm the next window only after gf2-79 confirms its workers are idle; one window at a time.
2. After the fd9d5416 table: record it with the REQ-03 falsification, file the gap issues if the owner asks, run its doc-review and research-review, then close epic ae03bcd0 (its gates and criteria mapping).
3. `a6039c72` is ready to dispatch.
4. Before the d77176e5 container gate: reconcile `surfaced_pitfalls` in `progress.json` against the epic's criterion.

## Traps

- Handoff-2's traps stay in force.
- `jit gate evaluate` on an issue this lead has leased needs `JIT_AGENT_ID=lead:d77176e5`; without it the gate refuses to write, and the run looks like a failure.
- A gate that declares its inputs returns a cached verdict when the content matches a prior run, including a cached failure. Use `--force` to rerun.
- The cargo-ci gate has a 900 s timeout, and its fast tier kills tests at 15 s. Run `./scripts/cargo-ci.sh` directly first to warm the build, then evaluate the gate with worker builds idle. A loaded host timed out a tuning-calibration test.
- Gate runs read the main checkout, so tell gf2-79 before each run and wait for its "gates done" before merging. A merge landed 2 s after gf2-79 started a run.
- `systemd-run --user` is refused by the permission classifier. A short window runs as a background command of the session.
- Run worktrees are not refreshed by the runner: detach each one at main (`git -C <worktree> checkout --detach main`) before a window. `jit-be6d6f45-run` still sits at `9734f2118`, so its BCH and NR lines measured old code on 2026-11-12 and were not recorded.
- A shell chain with `;` after a failing check still runs the commit: use `&&` throughout, and `set -- ${=p}` only in zsh.
- Worktree checkouts carry their own `.jit`, so `jit gate status --all` there shows no runs. Give workers the findings in the brief.
- Tracked issue text must not carry host-local or `.agents/` paths; name digest files by basename.

## Open questions for the invoker

- `259acf37` REQ-05: how to run the DVB-T2 flooding curve (night queue line recommended), and whether "within 0.1 dB" is one-sided.
- `fd9d5416` REQ-02: measure the five pre-cutover cells beyond the 10 s budget (`CUTOVER_CALL_BUDGET_S=2000`, `CUTOVER_ROUNDS=3`, about 2.5 to 3 h; recommended) or accept labelled projections. REQ-03 fails on the indication: file the gap issues now or after the window.
- `docs/reference/performance-evidence.md` commits measured ratios: a decision record under `@/inv/committed-evidence-budget`, or move them to issue history.

## References

- Progress: `dev/active/d77176e5-competitive-benchmarking/progress.json`.
- Charter: `dev/research/2caf738d-charter.md` (D-2: every stated cell at 1.0 or above).
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`; state under `.agents/bench-window/`.
