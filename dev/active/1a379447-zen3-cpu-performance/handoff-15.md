# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (`1a379447`) — session 18

**Date:** 2026-09-16T19:12Z
**Session number:** 18
**Prior handoffs:** `handoff.md`, `handoff-2.md` through `handoff-14.md`

## Current state

- Epic: `1a379447` — state: in_progress
- Wave in progress: wave 3 of 7
- Children summary: this session closed `96c94b81`, `77c21ecd`, `8275a6f8`,
  `7cdc28e9`, `bb769456`, `2c487595`; `9fb40c83` and `85fc5ff4` wait for tonight's window;
  new issues `8275a6f8` (bug, done), `b9302771` (m = 5 amendment),
  `0a357f94` (closure freshness), `7cc591a0` (test scratch leak, outside the
  epic). Run `jit graph tree 1a379447` for the full picture.
- Active claims: `9fb40c83` (agent:work-9fb40c83), `85fc5ff4`
  (agent:work_85fc5ff4); all in_progress.
- Open escalations: none. The three from handoff-14 were ruled on (see
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
  96c94b81 + 77c21ecd (`4522673a`), 8275a6f8 (`7997c767`) plus a lead fix for
  a suite-load flake in its new test, bb769456 rework 1 (`2c4d8c42`), 7cdc28e9
  (`d74b84a4`), 2c487595 (`d1825118`), bb769456 rework 2 (`29c75271`).
- Gate rounds surfaced only prose defects after the code reviews passed
  (uncited attributions, a stale design bound, a hard-coded receipt path, a
  rustdoc precedence sentence, an index-reading CI checker); the lead fixed
  each in place and re-ran only the failed gate.
- Lead reviews: `reviews/96c94b81-r1.md`, `77c21ecd-r1.md`, `8275a6f8-r1.md`,
  `7cdc28e9-r1.md`, `2c487595-r1.md`, `bb769456-r2.md`.
- Verified Rust 1.95 for 77c21ecd (gf2-core builds, 2712 tests pass).
- Filed `0a357f94` (closure freshness before a timed logical run) and wired
  the three logical baselines onto it and onto `b9302771`.
- Invoker ruling at the end of the session: holistic-review is a container
  gate; the lead removed it from every open leaf under the epic (containers
  keep it). bb769456's leaf holistic failure on the closure-freshness gap is
  therefore not a required gate; the gap stays tracked as `0a357f94`.
- Second gate ruling: tdd-reminder is removed from every open leaf;
  research-review from leaves that produce no measurement, addendum or
  findings (b64dc9c4, 0a357f94, e1f9a78f, 23a08297, 1956017f, 7d3ced35,
  c71becc5); asm-artefact-present from leaves that cannot touch SIMD kernel
  sources, where it is a no-op that only confuses. Containers and the
  measurement leaves keep their sets; `jit issue show` is authoritative.

## What to do next

- [ ] Collect tonight's window first: for 85fc5ff4 and 9fb40c83 judge each
  campaign from its own execution log and receipt under the worktrees
  `agent-85fc5ff4` and `agent-9fb40c83`, never the job rc; then finish their
  findings, gates and closure (their workers are idle; a fresh agent or the
  lead does the remainder).
- [ ] bb769456 is done; dispatch `b9302771` and `0a357f94` next (both depend on it).
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
- **Do NOT put holistic-review on a leaf.** It is a container review; on
  leaves it re-reported the other gates' findings and blocked bb769456 on a
  successor task's scope. The invoker removed it from every open leaf.
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

None.

## Reference artefacts

- Epic: `jit issue show 1a379447`
- Design docs: `dev/active/1a379447-zen3-cpu-performance/613574db/design.md` (aligned with the L1 code this
  session), `dev/active/2037941f-.../dense-parity-addendum.md`,
  `.../logical-harness.md`
- Planning docs: `progress.json`, `handoff-14.md`
- Benchmark/result artefacts: `bench-window/queue.tsv`,
  `dev/active/2037941f-.../survey/logical-runner-smoke.txt`
- Reviews: `reviews/96c94b81-r1.md`, `reviews/77c21ecd-r1.md`,
  `reviews/8275a6f8-r1.md`, `reviews/7cdc28e9-r1.md`, `2c487595-r1.md`, `bb769456-r2.md`
- Lead scripts (scratchpad, not committed): `gate-driver.sh`, `lead-wave*.sh`
