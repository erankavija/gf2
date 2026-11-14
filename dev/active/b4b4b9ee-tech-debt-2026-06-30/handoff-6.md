# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 6

**Date:** 2026-11-15
**Prior handoffs:** `handoff-5.md` is no longer cited (the `d77176e5` lead dropped the citation in `d75b874e9`) and is retired in the next tracker commit. Its traps that still bind are repeated below.

## Current state

- Wound down by the owner's order of 2026-11-15 01:22, through the steward: everything except the carve-out `6ba9be17` stops. This epic takes no dispatch, gate or landing. Every open branch is parked as it stands and is carried through the history rewrite as a rewritten ref (owner). A parked branch resumes only on a new owner word.
- Lead `gf2-7d` under steward `gf2-96`. `6ba9be17` was handed to `gf2-e3` at 01:18 with both wave-1 worktrees, heads, claims, gate state and the Claude review of `6d5ba9fb`. Its `6d5ba9fb` cargo-ci gate, run 65dc8131, timed out at 900 s on a cold worktree: an environment error, no verdict.
- No build, gate, worker or Codex process of this lead runs. No `cargo-pool-run` process of this lead remains.
- On main this session: `2e6e3557` (merge `9e5cbf7c8`, closed) and `c34e8203` (merge `100e5380a`, closed under the owner's scope ruling).
- Filed this session: `d8aee1f4` (protocol tests with a fixed 6 s stall deadline) and `18276ff1` (nextest test processes draw from the host cargo pool).
- Parked branches, each `worktree-agent-<id>` in `.agents/worktrees/agent-<id>`, with `target/` removed. Every gate reruns on the head that lands.

| Issue | Head | Done | Missing |
|---|---|---|---|
| `574ed420` | `4d3829b01` (main merged at 01:20) | REQ-01, REQ-02 met; design and implementation reviewed | REQ-04 pair back to back on a quiet host (before at main, after at the branch, cold and warm, all rc=0, start load stated); REQ-03 rerun; cargo-ci; code-review. Harness kept outside the tree. |
| `18276ff1` | `945da678c` | design reviewed; owner decisions and REQ-01 to REQ-09 recorded; `649530477` (wrapper test) and `4c83737f3` (wrapper) committed by the worker | `945da678c` holds unverified work in progress from the stop: the budget gate script, filter checker, AGENTS.md, invariants, gates.toml and per-epic gate edits, gf2-sim width changes, other leads' handoff wording. Review or drop it before any merge. REQ-08 width-24 decision (A/B/C, in the issue); REQ-06 evidence after `574ed420`; Codex gate; steward approval. |
| `697b9178` | `26a4d4d01` | code-review passed; Kani run of the bitvec harnesses recorded earlier | cargo-ci (killed tests under concurrency; a direct run passed in 517 s) |
| `9c0568ee` | `8b4a79e6b` | Claude review PASS recorded earlier | cargo-ci (GPU-lock timeout; a direct run passed in 584 s), code-review |
| `22bb2dcd` | `ca855d173` | code-review passed at this head | cargo-ci (GPU-lock timeout; a direct run passed in 628 s); afterwards `90cd64a9` |
| `d73ac633` | `0ac0d915b` | code-review passed; blocking Claude review PASS at `593832a42` | cargo-ci |
| `e29fd776` | `5520d7b03` | code-review passed; blocking Claude review PASS at `50b003164` | cargo-ci, doc-review |
| `9ff05f12` | `b9f2f9a42` | — | cargo-ci, code-review, doc-review |
| `b5782224` | `75f70cab1` | REQ-01 to REQ-03 implemented (Claude Sonnet 5.5); the folded interleaver finding is parked in the issue's Notes (owner, option 1) | cargo-ci, code-review |
| `903dc74c` | `22f57141d` | — | after `697b9178`: merge main, `cargo-kani`, code-review |

- Stopped without dispatch: `90cd64a9`, `d8aee1f4`, `6eb12747`, `85e7e4c0`, `41517289` (owner action) and the ready set.
- Nothing is pushed.

## Dispatch record

| Dispatch | Issue | Vendor | Model / tier | Reviewer |
|---|---|---|---|---|
| implement | `b5782224` | Claude | Sonnet 5.5 | Codex gate |
| implement | `2e6e3557` | Codex | gpt-6-luna, xhigh | Claude Sonnet, blocking |
| review | `d73ac633`, `e29fd776` | Claude | Sonnet 5.5, Opus 5.5 | — |
| design | `18276ff1` | Claude | Opus 5.5 | lead |
| rework 1 | `c34e8203` | Codex | gpt-6.1-sol, high | Claude Sonnet, blocking |
| rework 2 | `c34e8203` | Claude | lead, Opus 5.5 | Claude Haiku ×2, Codex gate |
| implement | `28aa2a9d` | Claude | Opus 5.5 | Codex gate |
| implement | `6d5ba9fb` | Codex | gpt-6.1-sol, high | Claude Sonnet, blocking |

## Decisions that bind the next lead

- Owner decisions reach the lead through the steward; a relayed answer is the owner's answer.
- The cargo-ci cap is lifted (owner). The lead keeps its own queue narrow enough that gates pass: on this host a 900 s cargo-ci gate fits with about two concurrent cargo-ci runs, until `18276ff1` lands.
- `574ed420` REQ-04 (steward, 2026-11-15): repetitions are dropped; the evidence is one cold and warm pair before and after, all rc=0, back to back on a quiet host with the start load stated.
- `c34e8203` (owner, 2026-11-15): contrived-byte findings are out of scope; recorded in the issue's Notes. A gate failure of another class goes to the steward.
- `18276ff1` (owner, 2026-11-15): timeout enforcement moves to epic level (`budget` profile, `CARGO_CI_BUDGET=1`, epic gate `test-budgets` that waits on a busy host); the rayon gap is in scope (REQ-08); wording in the issue.
- `2e6e3557` (steward): the `ci-slow.yml` fold was approved as a scope extension.
- Vendors alternate per implementation dispatch; the reviewer is the other vendor; a Codex-implemented change gets a blocking Claude review.
- Before every review or plan gate, merge main into the branch: a verdict on a base older than `49c78483d` reruns (steward).
- After a landing that touches `crates/`, run `CARGO_NET_OFFLINE=true cargo check --locked -p aff3ct-ldpc-gaps --bin dvb_t2_fer_diag` on the merged main tip before releasing main (`gf2-f4`).
- `6ba9be17` (steward): scripts and checkers only; nothing touches the real store or history before `6889e6e6`; `2a4c2684` is `gf2-00`'s.

## What happened

- `574ed420` REQ-02 passed (three concurrent cold runs, every `rustc` on the pool fifo, runnable threads p95 24 for 24 tokens, load max 33 against 63 to 69 before).
- Running about ten cargo-ci runs at once (this lead's gates and measurements, two peers' runs) drove the load to 87; every gate timed out at 900 s and two REQ-04 runs ran for an hour. Narrowing to about four concurrent runs still timed out or failed on test kills.
- The 18276ff1 design falsified the steward's premise: direct runs already take one pool token per test, and the token wait runs on the per-test timer; recorded in the issue.

## Next

Nothing, until the owner resumes this epic. On resumption: alternation continues with a Codex implementation dispatch (the last implementation dispatch was Claude Opus 5.5 on `18276ff1`); `18276ff1` first, since the pool leak and the per-test timer cost tonight's gates; then `574ed420`; then the parked branches two at a time.

## Pool findings (2026-11-15)

- The owner's cargo wrapper sets the target runner to `cargo-pool-run`, so each direct nextest test process holds one pool token. The wait for it runs on the per-test timer, and a killed runner leaks its token.
- The daemon refills only when no client holds the lease, so leaked tokens drained the pool: about 01:04 to 01:10, 75 waiters at load 0.34. It refilled `0 -> 24` at 01:10:54 once the queued runs stopped.
- With about two concurrent cargo-ci runs a gate fits 900 s. With more, GPU-lock waits and pool waits on the per-test timer kill tests at 60 s.

## Traps

- zsh does not split an unquoted variable: `kill $list` with several PIDs kills none. Use an array, `l=(...)`, `kill "${l[@]}"`.
- Killing processes by working directory also kills the Bash tool's own shell when it sits in that worktree; run such a kill from the main checkout.
- `jit gate evaluate` in a worktree leaves the issue JSON and `events.jsonl` modified there; `git merge main` then refuses. Discard with `git checkout -- .jit/` before merging (the run files stay in the ignored `gate-runs/`), or commit them as "record gate runs" when landing.
- A timed-out gate run stores no output; diagnose with a direct `./scripts/cargo-ci.sh`, which prints each step.
- The GPU lock and the cargo pool wait both run on nextest's per-test timer: concurrent cargo-ci runs kill GPU tests and the calibration harness tests at 60 s. Until `18276ff1` lands, gate one or two at a time.
- `cargo-ci.sh` killed with SIGKILL leaves its `mktemp -d` run root behind; such roots from about 23:58 and 00:05 exist under the system temp directory.
- The landing script in the session scratchpad merges by hand: it commits the worktree's gate records on the branch, merges with `events.jsonl` as a by-id union, restores the main checkout's other pending lines and resets the main-side issue file of the landed issue first. Build it again from these steps in a new session.
- A gate's code-review reads gate evidence: a review run before a passing cargo-ci fails on "CI timed out" (`9c0568ee`).
- Earlier traps still bind: copy issue-description edits to the branch before its code-review; check every subject's length and type before a merge; `git merge` runs no pre-commit hook, so run `python3 scripts/check-tracked-budget.py`; stage only your own tracker lines; do not use `pgrep -f` in wait loops; `/usr/bin/time` is absent; Kani harnesses that allocate need `#[kani::unwind]`; the HIP crate's nextest needs `--release`; `.agents/skills/` paths are gone (skill scripts live in the user-level skill directory).

## Open questions for the invoker

None.

## References

- Epic `b4b4b9ee`; epic `6ba9be17` and `dev/active/6ba9be17-tracker-repository/plan.md`; steward handoff in `dev/active/2caf738d-1-0/`; progress `dev/active/b4b4b9ee-tech-debt-2026-06-30/progress.json`; assessment `dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md`.
