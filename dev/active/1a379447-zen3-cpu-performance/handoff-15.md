# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (`1a379447`) — session 18

**Date:** 2026-09-16T19:12Z
**Session number:** 18
**Prior handoffs:** `handoff.md`, `handoff-2.md` through `handoff-14.md`

## Current state

- Epic: `1a379447` — state: in_progress
- Wave in progress: wave 3 of 7
- Children summary: this session closed `96c94b81`, `77c21ecd`, `8275a6f8`,
  `7cdc28e9`; `9fb40c83` and `85fc5ff4` wait for tonight's window;
  new issues `8275a6f8` (bug, done), `b9302771` (m = 5 amendment),
  `0a357f94` (closure freshness), `7cc591a0` (test scratch leak, outside the
  epic). Run `jit graph tree 1a379447` for the full picture.
- Active claims: `9fb40c83` (agent:work-9fb40c83), `85fc5ff4`
  (agent:work_85fc5ff4), `2c487595` (agent:worker, gates not all passed), `bb769456` (agent:worker, gates not all passed); all in_progress.
- Open escalations: bb769456 holistic F1 (see the questions section). The three from handoff-14 were ruled on (see
  `progress.json` escalations) and a standing approval covers the
  `Source references: [Key].` citation repair for any epic issue whose
  `cites:` label lacks its token.
- Benchmark window: transient user timer `gf2-bench-window-20260917` armed for
  2026-09-17 04:00 EEST over `bench-window/queue.tsv` (85fc5ff4 and 9fb40c83
  lines, about 24 estimated minutes). State under `.agents/bench-window/`;
  follow with `GF2_WINDOW_UNIT=gf2-bench-window-20260917 bash
  dev/active/1a379447-zen3-cpu-performance/bench-window/follow-window.sh`.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Interviewed the invoker: m = 6 gap closed by amending the closed addendum to
  m = 5 (task `b9302771`, after `bb769456`, before `65c0e13d` and the three
  logical baselines); shared-runner diagnostics fix authorized and landed
  (`8275a6f8`, done); ISA-L compiler provenance folded into `65c0e13d` as
  REQ-04; window armed for tonight.
- Dispatched four opus workers in worktrees (bb769456 rework 1, 77c21ecd,
  96c94b81, 8275a6f8), later two sonnet workers (7cdc28e9, 2c487595) and one
  fresh opus agent for bb769456 rework 2.
- Host incident: `/tmp` tmpfs ran out of inodes (about 29,700 leaked test
  fixture directories from seven test helpers, bug `7cc591a0`), freezing every
  shell and build for about an hour; the invoker cleared the directories. The
  invoker then ruled: do not wake idle workers (prompt caching); the lead
  finished their remaining steps itself.
- Merges on main, each gated by `./scripts/cargo-ci.sh` on the merged tree:
  96c94b81 + 77c21ecd (`f03b8c7c`), 8275a6f8 (`63f72897`) plus a lead fix for
  a suite-load flake in its new test, bb769456 rework 1 (`2689d021`), 7cdc28e9
  (`cf25c7b5`), 2c487595 (`5e6d0ee7`), bb769456 rework 2 (`b39cfd65`).
- Gate rounds surfaced only prose defects after the code reviews passed
  (uncited attributions, a stale design bound, a hard-coded receipt path, a
  rustdoc precedence sentence, an index-reading CI checker); the lead fixed
  each in place and re-ran only the failed gate.
- Lead reviews: `reviews/96c94b81-r1.md`, `77c21ecd-r1.md`, `8275a6f8-r1.md`,
  `7cdc28e9-r1.md`, `2c487595-r1.md`, `bb769456-r2.md`.
- Verified Rust 1.95 for 77c21ecd (gf2-core builds, 2712 tests pass).
- Filed `0a357f94` (closure freshness before a timed logical run) and wired
  the three logical baselines onto it and onto `b9302771`.

## What to do next

- [ ] Collect tonight's window first: for 85fc5ff4 and 9fb40c83 judge each
  campaign from its own execution log and receipt under the worktrees
  `agent-85fc5ff4` and `agent-9fb40c83`, never the job rc; then finish their
  findings, gates and closure (their workers are idle; a fresh agent or the
  lead does the remainder).
- [ ] bb769456: not all gates passed at handoff; read `jit gate status-all bb769456` and `reviews/bb769456-r2.md`; rework attempts are exhausted (2 of 2), so escalate per policy item 5 before any further rework. 2c487595: code-review rerun did not pass; see its findings and decide rework 1.
- [ ] Dispatch when slots allow: `b64dc9c4` (L2 dense product; warm worktree
  `agent-613574db`, detached at main), `b9302771` and `0a357f94` (after
  bb769456 closes), `706a8f93` and `94bbe5d7` (sonnet-sized; worktrees
  `agent-96c94b81` and `agent-8275a6f8` are detached at main and free).
- [ ] Then `e1f9a78f` (dense-parity harness; depends on 96c94b81 and bb769456),
  the three logical baselines, `ad2a6a58` after b64dc9c4.
- [ ] Reclaim worktrees with `LEAD_CACHE_DIRS=none` once their issues close;
  the cache pool holds only release and ci-test seeds now.

## Traps — do not repeat these

- **Do NOT start concurrent `jit gate evaluate` calls without staggering.**
  They collide on `.jit-bootstrap.lock` (5 s timeout) and a failed start
  records nothing, so a chain silently skips gates; codex reviewers' own jit
  reads add contention. Use `scratchpad/gate-driver.sh`'s pattern: staggered
  starts, retry on `Lock timeout`, and `jit gate status` afterwards to confirm
  a verdict exists.
- **Do NOT read a manual gate with `jit gate status`.** It reports
  "has not been run yet" for a recorded attestation; `jit gate status-all`
  shows it. A wrong check aborted wave 4 once.
- **Do NOT trust `du` on seeded worktrees.** The pool seeds by hardlink, so
  three 171 GB "debug" trees were one tree; deleting them freed 14 GB. Space
  came back only when the last hardlinked copies (stale test binaries baked
  with other worktrees' manifest paths) went.
- **Do NOT let a full `cargo-ci.sh` run refill `/tmp`.** Until `7cc591a0`
  lands, each run leaks a few thousand fixture directories
  (`gf2-f547c394-*`, `gf2sim-*`, `gf2-coordinator-fixture-*`, `gf2-driver-*`,
  `tuning-campaign-support-*`); remove them between runs with `setopt
  nullglob` (a zsh glob with no match aborts the whole `rm`).
- **Do NOT assign to a variable named `path` in zsh.** It aliases `PATH`; a
  `while read path` loop lost every command until the shell was replaced.
- **Do NOT use `set -- $pair` or unquoted word splitting in zsh.** It does not
  split; loops silently ran with empty arguments.
- **Do NOT re-run passed gates for a lead docstring touch-up.** The invoker
  ruled it waste; record the touch-up in the verdict instead.
- **Do NOT write "smoke through the real runner" as a requirement for a
  non-timed smoke.** The runner has no zero-window mode; the brief now admits a
  harness smoke that uses the runner's own `transport` encoder with a pinned
  request mirror. The proposed runner change is in `progress.json`
  `surfaced_pitfalls`.
- Prior handoff traps remain in force; read `handoff-14.md` (stall patterns,
  gate ordering with holistic last, seeded-cache `cargo clean -p`, request
  mirror and `GF2_BENCH_WINDOW` traps) and its predecessors rather than copies
  here.

## Open questions needing invoker input

- Question: how to close bb769456's holistic-review F1 (blocking): the window
  guard verifies a static producing-input manifest, so a measured source
  committed after the last regeneration can change rebuilt executable bytes
  without appearing in the receipt closure (REQ-02/REQ-04). Both rework
  attempts are spent (policy item 5), and the same defect is already filed as
  task `0a357f94` (depends on bb769456; the three logical baselines depend on
  it), which the reviewer cites.
  - Options: (A) accept `0a357f94` as the fix, close bb769456 with the
    holistic finding recorded as resolved-by-successor, and let `0a357f94` land
    before any timed logical run; (B) authorize a third rework of bb769456 that
    folds `0a357f94`'s REQ-01/REQ-02 in and close `0a357f94` as duplicate;
    (C) reject bb769456 and re-scope.
  - Recommendation: A. The guard already refuses every listed dirty or
    untracked path; the gap is regeneration freshness, which `0a357f94` states
    exactly and which no timed run can reach before it lands.
- Question: the remaining bb769456 gates after the doc fix: doc-review rerun
  is in wave 5; holistic was not re-run because F1 stands until the ruling.

## Reference artefacts

- Epic: `jit issue show 1a379447`
- Design docs: `dev/active/613574db/design.md` (aligned with the L1 code this
  session), `dev/active/2037941f-.../dense-parity-addendum.md`,
  `.../logical-harness.md`
- Planning docs: `progress.json`, `handoff-14.md`
- Benchmark/result artefacts: `bench-window/queue.tsv`,
  `dev/active/2037941f-.../survey/logical-runner-smoke.txt`
- Reviews: `reviews/96c94b81-r1.md`, `reviews/77c21ecd-r1.md`,
  `reviews/8275a6f8-r1.md`, `reviews/7cdc28e9-r1.md`, `2c487595-r1.md`, `bb769456-r2.md`
- Lead scripts (scratchpad, not committed): `gate-driver.sh`, `lead-wave*.sh`
