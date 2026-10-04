# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (`1a379447`) — session 17

**Date:** 2026-09-16T13:05Z
**Session number:** 17
**Prior handoffs:** `handoff.md`, `handoff-2.md` through `handoff-13.md`

## Current state

- Epic: `1a379447` — state: in_progress
- Wave in progress: wave 3 of 7
- Children summary: this session closed `3ea122df`, `92385645`, `c5e01de3`,
  `613574db`; `9fb40c83` and `85fc5ff4` are repaired and merged, waiting for a
  benchmark window; `bb769456` is merged but in rework; four GF(2^8) leaves
  (77c21ecd b64dc9c4 ad2a6a58 4c1e441f) and two bugs (`598d8057` outside the epic, `7cdc28e9` inside)
  were created. Run `jit graph tree 1a379447` for the full picture.
- Active claims: `bb769456` (agent:worker), `9fb40c83` (agent:work-9fb40c83),
  `85fc5ff4` (agent:work_85fc5ff4); all three issues stay in_progress.
- Open escalations: the ISA-L family's frozen `m = 6` cannot be reserved by
  the ledger mechanism (protocol gap); the shared-runner diagnostics loss in
  `process.rs:341-343`; the ISA-L probe's missing compiler provenance (task
  candidate). See `progress.json` escalations and the section below.
- Benchmark window: none armed (invoker hold). Queue lines for `85fc5ff4` and
  `9fb40c83` are committed and repaired.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Interviewed the invoker on the three open escalations: 92385645 focused
  correction authorized (counter reset); 3ea122df citation repair approved;
  1d4fd63d question closed (lane unchanged, 63bad95d calibrates). No window
  armed.
- Rulings during the session: subagents may not spawn codex agents; separate
  independent reviews beside the issue gates are void (gates alone decide);
  no new work after ~12:00Z; stall checks every 20 minutes.
- `3ea122df`: citation appended, one review finding fixed by the lead
  (cite the preserved 64-word no-win), seven gates pass, closed at `dd0a781a`.
- `92385645` round 3: MAKEFLAGS/GNUMAKEFLAGS/MFLAGS/MAKEFILES cleared, build-time
  provenance from `make V=1` and ELF `.comment`, fresh-cache regressions on both
  channels; seven gates pass; closed at `61923ebb`.
- `c5e01de3`: three alpha fields, canonical freezer fixed, acceptance schema
  versioned to v2 (gate F1); gates pass; closed at `770d1d90`.
- `613574db`: design and manifest merged; holistic failed until the four
  leaves existed in the tracker; closed at `ed7c67bc`.
- `9fb40c83` and `85fc5ff4`: window failures root-caused to the arms (request
  mirror byte order and null-spelled optionals; window-variable demand before
  stdin); arms fixed, runner smoke of every arm, v4-r1 attempts voided per
  protocol, replacement queue lines committed; merged at `59e4d818` and
  `34376022`; merged-tree cargo-ci green.
- `bb769456`: harness merged at `f2bd8f1c`; code-review, research-review and
  holistic-review fail on two converging blockers (see reviews/bb769456-r1.md);
  rework not dispatched on the invoker's instruction.
- Disk: removed two closed worktrees carrying over 300 GB of build caches and
  three stub directories; reclaimed `agent-92385645` and `agent-c5e01de3`
  without harvest.

## What to do next

- [ ] Dispatch the `bb769456` rework (attempt 1 of 2) from
  `reviews/bb769456-r1.md`: zero-sample smoke (REQ-03) and a window guard over
  the whole producing manifest (REQ-04); re-run code-review, research-review,
  holistic-review last.
- [ ] Put the three open escalations to the invoker (section below), then act:
  the `m = 6` gap blocks `65c0e13d`'s confirmation; the ISA-L provenance task
  should precede `65c0e13d`; the `process.rs` fix is a one-line shared change.
- [ ] Dispatch `96c94b81` (dense-parity addendum; `92385645` is done) and the
  GF(2^8) leaf 77c21ecd when slots allow; `agent-613574db` is a warm worktree
  to reuse for the leaf.
- [ ] When the invoker arms a window: run it, audit `85fc5ff4` and `9fb40c83`
  from their execution logs and receipts, then re-engage their workers for
  findings, gates and closure.
- [ ] Continue wave 3 per `progress.json`; `63bad95d` (wave 4) stays blocked
  on `2037941f` and `c04dd4ac`.

## Traps — do not repeat these

- **Do NOT let a worker wait on a background CI notification or a
  process-name search.** Three workers idled for over 40 minutes waiting for a
  notification that never woke them, and one shell loop gated on a
  process-name search matched its own shell and spun forever. Tell workers to
  wait with the shell's `wait` builtin or on the log's final line, and to
  never idle before the final report exists.
- **Do NOT run `jit gate evaluate-all` on a 2037941f child and trust the
  result.** It evaluates in gate-list order and stops at the first failure;
  holistic-review sits third, so it runs before code-review and
  research-review and fails on their staleness (92385645) or halts the chain
  (c5e01de3). Evaluate the gates individually, holistic-review last.
- **Do NOT commit on main while a gate evaluation is running**, and do not
  run the reclaim script with harvesting on a large worktree: the harvest is an
  rsync copy, not a hardlink, and the disk was at 98%. Use
  `LEAD_CACHE_DIRS=none` for closed worktrees whose caches the pool already
  seeded.
- **Do NOT seed a worker from the pool and assume its test binaries run.**
  Pool-seeded `target/` trees carried test binaries whose baked
  `CARGO_MANIFEST_DIR` named a deleted worktree; 84 tests aborted (92385645,
  85fc5ff4). The remedy is `cargo clean -p` for the affected crates, which the
  worker brief names; say so in the dispatch prompt.
- **Do NOT write an arm's request mirror as a typed struct or with
  `#[serde(default)]` alone.** The runner forwards the case as a
  `serde_json::Value` (sorted keys) and omits absent optionals;
  `decode_canonical` compares bytes. Both window failures this morning were this
  class. Mirror `ab-smoke-workload.rs`; smoke the exact queued executable
  through the real runner before queuing.
- **Do NOT let an arm demand `GF2_BENCH_WINDOW` before reading stdin.** The
  runner clears the child environment (PATH, HOME, RAYON_NUM_THREADS,
  RUSTUP_TOOLCHAIN, per-arm variables, the sentinel only); the 9fb40c83 arm
  died before the handshake and the runner's write hit EPIPE, losing the
  child's stderr (shared-runner defect escalated).
- **Do NOT run a separate independent review beside the gates.** The invoker
  voided that convention this session; the gates decide.
- **Do NOT write "the smoke runs through the real runner" and "the smoke emits
  zero receipt samples" as if they were compatible without checking.** The
  worker brief's runner-smoke rule and bb769456 REQ-03 collided; the rework
  must satisfy both or report the contradiction.
- Prior handoff traps remain in force; read `handoff-13.md` and `handoff-12.md`
  for the sccache, MAKEFLAGS and window-policy traps rather than copies here.

## Open questions needing invoker input

- Question: how to close the ISA-L family's reservation gap (`m = 6` frozen in
  `logical-buffer-addendum.md`, five reservable by `trial_ledger::reserve`)?
  - Context: the declared unavailable `isal-dispatched-xor-gen` row spends a
    comparison in the frozen rule but has no representation in the addendum
    schema or ledger mechanism (bb769456 worker finding).
  - Options: (A) versioned protocol change making a declared unavailable row a
    reservation-bearing cell kind; (B) amend the closed addendum 3ea122df to
    `m = 5`; (C) record a decision on 65c0e13d to reserve five and cite the
    discrepancy.
  - Recommendation: A; it keeps the frozen, more conservative corrected alpha
    and fixes the mechanism once.
- Question: may the lead make the one-line shared-runner change so a request
  write error keeps the captured child diagnostics
  (`dev/tools/tuning-campaign-support/src/process.rs:341-343`, deliver the
  error through `callback_error`)?
  - Context: the 2026-09-16 window recorded no stderr for either failed job.
  - Options: authorize with a regression test; file as a bug for later.
  - Recommendation: authorize.
- Question: create an epic task giving the ISA-L comparator compiler
  provenance (`survey/run-isal-xor-probe.sh` uses bare `cc` and records no
  compiler identity), wired before `65c0e13d`?
  - Options: create and wire; defer to 65c0e13d's own scope.
  - Recommendation: create and wire.
- Question: when to arm the next benchmark window (jobs for 85fc5ff4 and
  9fb40c83 are queued and repaired)?
  - Recommendation: the next 04:00 EEST slot.

## Reference artefacts

- Epic: `jit issue show 1a379447`
- Design docs: `dev/active/1a379447-zen3-cpu-performance/613574db/design.md`, `dev/active/1a379447-zen3-cpu-performance/613574db/breakdown.md`,
  `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md`,
  `.../logical-harness.md`, `.../m4ri-operation-match.md`
- Planning docs: `progress.json`, `handoff-13.md`
- Benchmark/result artefacts: `bench-window/queue.tsv`,
  `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-voided-profile-attempt.json`,
  `dev/bench_results/85fc5ff4/v4-voided-launch-attempt.json`
- Reviews: `reviews/3ea122df-r1.md`, `reviews/92385645-r3.md`,
  `reviews/c5e01de3-r1.md`, `reviews/c5e01de3-r2.md`, `reviews/613574db-r1.md`,
  `reviews/bb769456-r1.md`
