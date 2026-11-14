# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 6

**Date:** 2026-11-15
**Prior handoffs:** `handoff-5.md` is no longer cited (the `d77176e5` lead dropped the citation in `d75b874e9`) and is retired in the next tracker commit. Its traps that still bind are repeated below.

## Current state

- Wound down (owner order, 2026-11-15 01:15, through the steward): the tech-debt epic takes no new dispatch. Its only remaining work is to land the implemented branches in gate slots that the live work leaves free. A branch whose gate or review fails is parked, not reworked, and reported to the steward. Stopped: `90cd64a9`, `d8aee1f4`, `6eb12747`, `85e7e4c0` and the ready set; nothing new is filed unless a `6ba9be17` task needs it.
- The live container of this lead is `6ba9be17` (tracker repository). The steward classes `18276ff1` and `574ed420` as serving it, since without them gates time out and the pool leaks.
- Lead `gf2-7d` under steward `gf2-96`; peer leads `gf2-f4` (d77176e5, ae03bcd0) and `gf2-00` (45ecf975 stream). The same lead also executes epic `6ba9be17` (tracker repository): its state is in the table below.
- Stopped by 01:20 local for the 2026-11-15 02:00 benchmark window: no build, gate or worker of this lead runs.
- Landed this session: `2e6e3557` (merge `9e5cbf7c8`, closed `a8711524a`), plus every row the table marks landed.
- Filed: `d8aee1f4` (protocol tests with a fixed 6 s stall deadline), `18276ff1` (nextest test processes draw from the host cargo pool; design reviewed, owner decisions and criteria recorded, implementation approved after the window).
- Branches, each `worktree-agent-<id>` in `.agents/worktrees/agent-<id>`, main merged in at about 00:02 (after `49c78483d`). The gate state is per head; every gate reruns on the head that lands.

| Issue | Head | Gate state | Next |
|---|---|---|---|
| `574ed420` | `a1070b26f` | REQ-02 met (in the issue); REQ-03 two attempts failed only on `d8aee1f4`'s tests; REQ-04 attempt is no evidence | quiet-host slot after `window end`: REQ-04 pair back to back (before at main, after at the branch, cold and warm, all rc=0, start load below about 10), REQ-03 rerun; then merge main, cargo-ci, code-review (copy the main issue JSON to the branch first) |
| `c34e8203` | landed | merged `100e5380a`, closed | — |
| `697b9178` | `26a4d4d01` | code-review passed; cargo-ci failed on `tuning-calibration` calibration-harness tests all killed at 60 s under concurrent runs; a direct run passed (517 s) | rerun cargo-ci alone; then land, then `903dc74c` |
| `9c0568ee` | `8b4a79e6b` | cargo-ci failed on one GPU test killed at 60 s (`gf2-sim` OOM fallback), GPU-lock contention; direct run passed (584 s); code-review failed only because cargo-ci had not passed | cargo-ci alone, then code-review |
| `22bb2dcd` | `ca855d173` | code-review passed at this head (00:18); cargo-ci failed on one GPU test killed at 60 s; direct run passed (628 s) | cargo-ci alone; land; then dispatch `90cd64a9` |
| `d73ac633` | `0ac0d915b` | code-review passed; blocking Claude review PASS (recorded below); cargo-ci timed out | cargo-ci alone |
| `e29fd776` | `5520d7b03` | code-review passed; blocking Claude review PASS; cargo-ci timed out | cargo-ci alone, doc-review |
| `9ff05f12` | `b9f2f9a42` | cargo-ci timed out | cargo-ci alone, code-review, doc-review |
| `b5782224` | `75f70cab1` | cargo-ci timed out; implementation by Claude Sonnet 5.5 reviewed by the lead | the folded `test_interleaver_still_composes` finding needs a rework, and reworks are stopped: report it to the steward as parked, or land the branch as implemented if the steward rules the folded part out |
| `903dc74c` | `22f57141d` | none | after `697b9178` lands: merge main, `cargo-kani` in a quiet slot, code-review |
| `6d5ba9fb` (6ba9be17) | `43e85778e` | Codex gpt-6.1-sol; blocking Claude review PASS | merge main, cargo-ci, code-review |
| `28aa2a9d` (6ba9be17) | `17da51131` | Claude Opus 5.5; cargo-ci was running at the stop | cargo-ci, code-review (Codex) |
| `18276ff1` | `worktree-agent-18276ff1` at main | design only | implement after the window by the strongest non-Fable Claude tier, base main; evidence after `574ed420` merges; CI-contract change to the steward before merge; sequence with `7854cfad` (both touch `.jit/gates.toml` and `AGENTS.md`) |

- Blocking Claude reviews beside the Codex gate (copy each into its issue at landing): `d73ac633` PASS at `593832a42` (Sonnet; F1-F3 of round 1 fixed); `e29fd776` PASS at `50b003164` (Opus; F1 fixed); `c34e8203` rework 1 PASS at `f1a36fab5` (Sonnet, real `git commit` matrix), rework 2 PASS at `74600e571` (Haiku); `6d5ba9fb` PASS at `43e85778e` (Sonnet). Only merges of main followed these heads.
- Undispatched: `90cd64a9` (after `22bb2dcd`), `d8aee1f4`, `6eb12747`, `85e7e4c0`, `41517289` (owner action), the epic's ready set; `6ba9be17` waves 2 to 6 (`3fb4accb` after wave 1).
- Nothing pushed.

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

1. After `window end`, `6ba9be17` waves in order. Wave 1: merge main, cargo-ci and code-review for `6d5ba9fb` (Claude review passed; a cargo-ci gate ran at 01:13 on the steward's word, see its run record) and `28aa2a9d`; land; then wave 2 (`3fb4accb`, strongest non-Fable Claude tier, Codex gate).
2. `18276ff1` implementation, at its full owner-decided scope, and `574ed420`'s quiet-slot REQ-04 and REQ-03, then its gates and landing. Agree the slot with `gf2-f4` and `gf2-00`.
3. Only in gate slots left free by 1 and 2, gate two at a time and land: `697b9178`, `9c0568ee`, `22bb2dcd`, `d73ac633`, `e29fd776`, `9ff05f12`, then `903dc74c` after `697b9178`. Park and report any failure. The history rewrite requires every live branch to be merged or abandoned.
4. Retire `handoff-5.md` (`git rm`) in the first tracker commit.

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
