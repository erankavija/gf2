# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 10

**Date:** 2026-08-23T19:15Z
**Session number:** 10
**Prior handoffs:** `handoff.md` (s4), `handoff-2.md` (s6), `handoff-3.md` (s7), `handoff-4.md` (s8), `handoff-5.md` (s9)

## Current state

- Epic: `6dc81018` — state: in_progress, claimed by `agent:jit-execution-lead`
- Wave in progress: 24 of 25 (all selector-cutover and seam waves COMPLETE; only the measured/proof endgame remains)
- Children summary: every t-task (t1–t11, t15) and seam task (U1–U7, incl. U3 schema) done. Remaining: `389aa4de` harness-merged/measured-run-pending (in_progress, claimed agent:worker); `5bdc9552` (U8) ready; `a83583e0` (t13) ready (held for shared bench file); `eaae1b56` (t12), `dbd8787d` (U10) blocked on 389aa4de; `06ba0418` (U9) blocked on U8
- Active claims: `6dc81018` (lead); `389aa4de` (agent:worker — harness landed, keep for the measured phase)
- Open escalations: one owner question (Block E shape — see Open questions)
- Progress file: `progress.json` beside this file (waves through 23.5; escalations through DEC-B17-as-amended)
- UNCOMMITTED at handoff: `.jit` gate-run state from 389aa4de's chain (committed with this handoff)

## What just happened

- t1 `856f1b48`: owner-approved DEC-B11 (REQ-01 permanent-family exception), gates re-run PASS, CLOSED.
- Seam recoveries: U1 merged + KC_U16 doc fix, CLOSED (rework 1). U2 lead-preserved from dead worker, CI 5/5, merged (ple.rs conflict vs t1 hoist), CLOSED. U4 reviewed, merged, CLOSED.
- Wave 21a (7 parallel workers, reused worktrees): t2 CLOSED (gf2-sim contention flake on R1, clean re-run); t4 CLOSED after 6 gate rounds (witness placement → parallel-arm observation → pool tolerance reversed → single-read threading (lead takeover) → panic doc → stale comment); t5 CLOSED (DEC-B13 call-site deviation, DEC-B14 stride-gate reporter, Amendment A2); t6 CLOSED (DEC-B12 second reporter `inv_route`, Amendment A1); t8 CLOSED first-run; t10 CLOSED (codex-luna-assisted worker); t11 CLOSED after observed-forwarding rework (test-support `last_effective_chunk` pattern established).
- t7 `fbe66a7a`: first DIRECT `codex exec` task (owner directive: no wrapper agents); luna implemented, lead fixed one SIMD-unconditional witness, CLOSED.
- Wave 22: t3 CLOSED (codex luna, first-run gates); t9 CLOSED (codex sol, code-review 0 findings; 3 doc rounds of stale/unconditional AXPY narration lead-swept); t15 CLOSED (native opus; reporter-authority arc ending in the single-authority refactor whose executed==reported witness caught a real variant-overlap bug); U3 CLOSED (codex luna; asserted-bounds caps, non-x86 portability fix — kernel constants re-homed arch-independently, seam Amendment A1).
- Wave 23 seam cutovers: U5 CLOSED (codex luna; shared `winograd_takes_base_case` predicate; 7 doc rounds converging every WINOGRAD_THRESHOLD narration to record-time/host-level framing); U6 CLOSED (codex sol; structural read audit); U7 CLOSED (native opus stalled post-commit, lead ran CI + review; code-review caught a REAL lane-ceiling bypass in sub-panel recursion — fixed with min(split,lane) cap + observed max-dispatch-width witness).
- `389aa4de` harness phase MERGED `28f674d6` (native opus, exemplary): omission set as schema complement (33/38, 7 drift tests), forced-arm child-process sweep (route/operand/product verified, 14 tests), DEC-B16 grid-point forcing (worker's escalation, ratified — crossover direction now physical, between degree 16 and 31), DEC-B17-as-amended (v2 token sequenced with the v2 profile commit), receipt-notes.md Blocks A–E drafted and doc-linked. cargo-ci PASS; code-review FAIL F1/F2/F3 = exactly the measured-phase deliverables — phase boundary, not rework.
- Amendments landed: 7d824b2f A1 (DEC-B12), A2 (DEC-B13/B14 + errata), A3 (DEC-B15 naming + D5 blind spot), A4 (DEC-B16/B17-as-amended); 7d7c647c A1 (lane caps at PANEL_SCRATCH_COLS).
- Infra: worktree REUSE protocol active (scratchpad `reuse-worker-worktree.sh` — move + re-anchor, target/ kept); ~6G reclaimed; rustc ICE on stale incremental cache cleared (freed 3.8G); 20-min watchdog monitor pattern established after the whole wave stalled silently on sleeping worker monitors.

## What to do next

- [ ] FIRST, SOLO: `389aa4de` measured phase (lead-run, benchmark-mode, never codex). Read `dev/active/389aa4de/receipt-notes.md` IN FULL first — it is the checklist. One change must contain: the measured calibration run's v2 profile commit, the harness token bump to `tuning-calibration-v2` (SUPPORTED + parse + :202 rustdoc), the committed-profile decision per the owner's Block E answer (see Open questions), both f35daec0 fixture tokens, both tuning_profile_committed tests, baked.rs include_str! sites + :15 rustdoc + SIMD_MIN_WORDS if the measurement moves it (DEC-G re-pin evidence obligation then applies), the three baked test binaries, receipt Blocks A–E applied. Bench discipline: host quiet, flock, no commits on main during timed phases, toolchain pinned deliberately (host is 1.97.0; 5ecc9bf8's receipt was 1.95.0).
- [ ] Then re-run `389aa4de` full gate chain (its code-review F1/F2/F3 close with the measured phase), close it.
- [ ] Then t12 `eaae1b56` + t13 `a83583e0` (both touch tuning_calibration.rs — SERIALIZE them, t13 after t12 or vice versa; they extend the sweep to the follow-on/seam threshold fields).
- [ ] U8 `5bdc9552` SOLO (native opus or lead-direct; verify-lean + lake-build gates; the one surface-moving task — removes the trait selectors, regenerates the proof surface; deps all done).
- [ ] U10 `dbd8787d` benchmark-mode solo after 389aa4de.
- [ ] U9 `06ba0418` docs last (fold in the fp_small_ple.rs:184 informal 'KC = 256' prose noted at U1).
- [ ] Section 10: reconcile surfaced_pitfalls (now incl. the D5 blind spot and the U5 per-field-carrier gap) against the epic criteria, `jit gate evaluate-all 6dc81018` (holistic re-run must see the session-9 F1/F2/F3 resolved), completion report, close, archive.
- [ ] Worktree reclaim: `agent-389aa4de` after its measured phase; `agent-ef18c60b`, `agent-fbe66a7a` reusable or removable now; `control-0c072d73` LAST after epic close; `/tmp/gf2-ens5`, `/tmp/gf2-ens6` keep until close. `.claude/worktrees/agent-a7c37e…` holds an unmerged commit for out-of-epic issue `82dd7384` — not ours, leave.

## Traps — do not repeat these

- **Do NOT trust worker file-monitors to wake workers, and do NOT let the session sleep on them.** The entire wave-21a CI queue drained while every worker slept; the lock sat free 40+ min. Run a 20-min watchdog (Monitor emitting lock state + per-log digests) and probe/nudge on every firing. Workers' "armed monitors" repeatedly failed to fire; artifacts + direct log reads are authoritative.
- **Do NOT queue worker worktree CI while a main gate chain needs the lock.** The gate runner's budget (900s) is SHORTER than cargo-ci's flock patience (1800s): a queued worker CI starves the gate into an `error` round (t4's chain, 15 min wasted). Main gate chains claim the lock first; workers queue after.
- **Do NOT run compound git/jit commands without leading `cd /home/vkaskivuo/Projects/gf2 &&`, and remember BACKGROUND compounds inherit the session cwd AT LAUNCH.** Standing trap, re-confirmed twice this session: one background compound committed a fix's `.jit` files under a message claiming a code change (the code deletion had landed in a different checkout), costing two junk gate rounds; another merged from inside a worktree. Also `jit gate status` from a worktree reads the WORKTREE's stale `.jit` ("has not been run yet" lies).
- **Do NOT accept equivalence-only or gate-tolerant witnesses; also do NOT over-tolerate.** The reviewer failed, across four issues, every witness that could pass without the mechanism executing: chunk-invariant equality (t11), unverified rayon dispatch (t4 R2), single-thread-pool tolerance (t4 R3 — my own fix, reversed), reporter-as-copy (t15 R1/R2). The converged contract: the reporter IS the code dispatch runs (shared function); witnesses OBSERVE the value at the production site (cfg(any(test, feature = "test-support")) atomics — `last_effective_*` pattern); where the gate is forcible (thread pools) FORCE it (dedicated pool helper); where it is hardware (SIMD hooks) BRANCH on the dispatcher's own gate.
- **Do NOT write doc prose that names a retired constant as live authority, states profile-dependent outcomes unconditionally, or claims per-field capability for host-level fields — and SWEEP EXHAUSTIVELY in one pass.** Reviewers drip-feed one site per round (t9: 3 rounds; U5: 7 rounds). `git grep` the constant name over crates/ AND benches/ AND committed receipts; converge on: trait constant = record-time carrier retained for the extraction surface; profile field = live host-level authority; conservative default framing; criterion state-cells in receipts get "Pass at record time … Superseded:" treatment.
- **Do NOT force the karatsuba asymptotic arm with the minimal threshold** (issue Background + design §5.1 cond 2 described it; DEC-B16 falsified it — it times degree-0 recursion production never runs, losing 2.9–12.3× everywhere and freezing the conservative default). Force the grid point; evidence in receipt-notes Block D and design A4.
- **Do NOT bump the harness schema token in isolation.** `gf2-5ecc9bf8-calibration-e202c080.json` (committed, v1) is the drift anchor of the ENTIRE baked mechanism — nine readers incl. the cargo-ci baked step. Token bump alone breaks 4 tests; file replacement reaches all nine and may re-pin DEC-G (`simd_min_words`: smoke says 2 vs baked 4 — a performance re-pin with `@/inv/benchmark-backed-performance` obligations). Sequenced change only, per DEC-B17-as-amended and receipt-notes Block E.
- **Do NOT read a one-red worker CI as a defect before an identical-tree re-run.** Three occurrences this session, all the gf2-sim contention class (bug 76665001): load hit 71/24 cores with unthrottled parallel builds. Wave discipline that fixed it: workers use `CARGO_BUILD_JOBS=8 nice -n 10` for targeted runs, run NOTHING cargo-related once their CI is queued.
- **Do NOT assume a rustc ICE is a code defect: `check_mod_privacy` panics with incremental caches are environmental.** Clear `target/*/incremental` and re-run (U3's gate; identical content had passed in the worktree).
- **Do NOT let codex-written module docs and receipts go to review unswept** — codex (luna and sol) reliably leaves cross-file narrations stale (t3 clean, t9 2 sites + 2 more rounds, U5 crate doc). Sweep the retired-authority greps BEFORE the first doc-review round.
- **Do NOT idle-nudge Opus workers repeatedly for progress reports — probe artifacts and take over when the work is committed.** Native opus workers this session repeatedly idle-bounced after committing good work without reporting (U7, t15 earlier, 389aa4de's report arrived only on explicit demand). The work is in the commits; the lead can run CI and review from artifacts (U7 done exactly so).
- All traps from `handoff-5.md` and its chain remain in force, notably: worker idle ≠ death (probe artifacts, SendMessage nudge); no merges/commits on main while a gate chain evaluates; `jit doc add` worker deliverables BEFORE doc-review; no stash with lead state in tree; benchmark-mode never to codex; append-only closed records with state-cell exceptions; no bare `=`-leading echo separators in zsh; process-liveness polling never in heredocs (and `pkill -f "cargo-ci.sh"` matches YOUR OWN compound — use `ps -eo pid,etimes,args` inspection instead); transitive-reduction-aware dep wiring; solo slots for benchmark-mode issues.

## Open questions needing invoker input

- Question: Block E shape for the committed calibrated profile when the measured run lands — replace or add-beside?
  - Context: replacing `gf2-5ecc9bf8-calibration-e202c080.json` with the v2 profile leaves ONE committed profile but re-pins DEC-G's baked `SIMD_MIN_WORDS` if the new measurement moves `simd_min_words` (smoke suggests 2 vs the baked 4 — a ratified performance value backed by receipt-3 and the frozen non-regression bracket). Adding the v2 beside it keeps DEC-G untouched but requires regenerating the v1 file with a v2 token from its own recorded numbers to avoid the orphaned-artifact problem DEC-B17 was amended to prevent.
  - Options: (1) replace + DEC-G re-pin with fresh evidence; (2) add-beside + regenerate v1's token from recorded numbers.
  - Recommendation: (1) if the measured run is taken under full bench discipline anyway — one profile, one anchor, evidence fresh; the DEC-G re-pin then rides the same receipt. (2) only if the owner wants DEC-G's pin insulated from this run.

## Reference artefacts

- Epic: `jit issue show 6dc81018`; wave plan + escalations: `progress.json` beside this file
- Measured-phase checklist: `dev/active/389aa4de/receipt-notes.md` (Blocks A–E; doc-linked)
- Governing designs with amendments: `dev/active/7d824b2f/design.md` (A1–A4), `dev/active/7d7c647c/design.md` (A1), `dev/active/220cab0b/design.md`
- Calibration chain: `dev/benchmarks/tuning_profiles/` (plan v1 → receipts 1–6, 2026-08-20-host-calibration.md), committed profile `crates/gf2-core/data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json`
- Worktree reuse script (session-local): `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/f70cb36b-1741-4039-8c61-eda00377598b/scratchpad/reuse-worker-worktree.sh` — copy into the next session's scratchpad or recreate from the dispatch script + `git worktree move`
- Holistic round-1 verdict (session 9, F1/F2/F3 now addressed by DEC-O/P/Q work): `jit gate status 6dc81018 holistic-review --findings`
- Out-of-epic: `4dd5372a` (upstream defects), `76665001` (gf2-sim flake), `82dd7384` (stray worktree salvage, not ours)
