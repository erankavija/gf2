# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 9

**Date:** 2026-09-02T17:32+00:00
**Session number:** 9 (session 8 died at the Claude session limit at 12:08Z with five workers mid-flight and no final reports; this session ends on the owner's request)
**Prior handoffs:** handoff.md, handoff-2.md, handoff-3.md, handoff-4.md, handoff-5.md — their Traps sections remain in force in full.

## Current state

- Epic `ae03bcd0` — state: backlog (container); claimed by agent:jit-execution-lead.
- Wave in progress: wave 11 of 15 (`current_wave` = 11), not yet dispatched; wave-10 straggler `3f7edef1` open on an escalation; wave-12 early start `b1bd75ca` in gates.
- Children: 47 done (this session closed `d7749931`, `2b6968d3`, `88ca7d2f`, `444c06bc`, `bd0edfa2`). Open: `3f7edef1` (in_progress, rework-2 branch `worktree-agent-3f7edef1` at 115ce492 unmerged), `b1bd75ca` (in_progress, merged 531a2039, lake-build passed, code-review + doc-review running detached), `e1e0e7ff` / `5ee83cd3` / `203ee826` (ready, unassigned), then `97410c80` (waits on 203ee826) and waves 12–15.
- Active claims: epic, 3f7edef1, b1bd75ca by agent:jit-execution-lead.
- Open escalation: 3f7edef1 B4 (below).
- Progress file: `progress.json` here (rulings R-40..R-45, session-9 notes, pitfalls, traps).
- Detached job still running at handoff: `gates-wave10-d.sh` (b1bd75ca code-review, then doc-review); its log is `gates-wave10-d.log` in the session-9 scratchpad and its results land in `.jit/` **uncommitted** on main. First action of the next session: `git status --porcelain .jit`, commit those records, read the verdicts with `jit gate status-all b1bd75ca`.

## What just happened

- Reconstructed session 8's end from worker transcripts and CI logs; lead-preserved 3f7edef1's uncommitted draft (d6f03947).
- `88ca7d2f`: first measurement opened at load 7.96; re-measured on the idle host with the worker's committed runner (R-41, 51f11e22), merged 0b408133; code-review R1 F1 (runner copied every `bch_*` artifact) closed lead-direct 6b8577fb (R-44); done; receipt linked.
- `2b6968d3`: rework 1 (clmul-fold family, combined `avx2 && pclmulqdq && sse4.1` predicate) reviewed, one lead-direct survey-doc fix (R-42), merged f5efb7cc; cargo-ci + code-review passed; done; 4e732b56 doc links refreshed.
- `d7749931`: lake-build + code-review passed; doc-review failed twice on the stale O-3 proof sketch — fixed lead-direct (7a380646, 3c744093; R-43 standing rule for lean workers); passed R3; done.
- `444c06bc`/`bd0edfa2`: rework 1 merged 8754afc8; 444c06bc code-review R1 F1 (private `MatrixFill` bound, REQ-06) → R-45 → rework 2 (public trait with provided bodies, opting-in test representation, design amended) merged 0cb1025a; doc-review R3 asked for panic/shape docs → lead-direct 3b839a80; bd0edfa2 code-review R3 failed only on the unlinked receipt → linked; all gates passed; both done.
- `3f7edef1`: B4 GAP retry at -o 50g / 52G scope FAILED (9m17s, 54.4 GB peak RSS, 15.6 GB swap); part-2 rework merged 93f08af2; cargo-ci passed on rerun (first run: transient GPU smoke SIGABRT under load 14); code-review R2 + research-review R2 F1 = B4 root cause ×3 → escalated; doc-review R2 F1 (bare `cargo run` in the example) closed lead-direct e899f8c8; research R2 F2–F5 → rework 2 complete on the branch at 115ce492 (attempt artifacts committed under `oracle/attempts/2026-09-02-b4-heap-50g/`, run.sh stage 4 runs the stream test and the receipt records 808/808, "predeclared" wording fixed, two peak-RSS members) — **not yet lead-reviewed or merged**.
- `b1bd75ca` (Lean O-4): 84 declarations, 0 sorry, four recorded Mathlib-route deviations, load-bearing anchor test, O-4 sketch section remapped; lead review PASS; merged 531a2039 + lead-direct O-3 citation fix 6f94277e; lake-build passed in 33 s after hardlinking the worker's `proofs/.lake` into main's.
- Host: tmpfs /tmp held ~17 GB of stale Luna cargo target dirs (deleted); an sccache daemon spawned under the CCX1 flock inherited the lock (killed); 99 stale gf2-kernels-simd artifacts purged from the cache pool.
- Codex weekly quota 94% → 96% over ~20 review runs (≈0.13%/run); resets Mon 2026-09-07 17:56 EEST.

## What to do next

- [ ] Commit the detached chain's `.jit` records; if b1bd75ca's code-review or doc-review failed, read the findings (`.jit/gate-runs/*/result.json`) — the likely doc subject is the proof sketch — and rework or fix lead-direct; then `jit issue update b1bd75ca --state done`; refresh nothing (doc links are unpinned).
- [ ] Dispatch wave 11 — `e1e0e7ff` (hard → Opus), `203ee826` (hard → Opus), `5ee83cd3` (easy → Sonnet while codex quota is reserved for gates; Luna after the reset): `.agents/skills/jit-execution-lead/scripts/dispatch-worker-worktree.sh e1e0e7ff 203ee826 5ee83cd3` from a clean main, claim each as agent:worker, prefix the script's header to `session-9-prompts/dispatch-<id>.md`, dispatch as general-purpose background agents that report via SendMessage to "team-lead". `97410c80` after 203ee826 is done.
- [ ] `3f7edef1` first: the lead's attempt package on the rework-2 branch (`oracle/attempts/2026-09-02-b4-heap-50g/`) has two defects the worker reported and left verbatim — `command.sh` is a lead-written reconstruction that says "48.6 GiB was available" (timeline.txt says `avail=48651` MiB = 47.5 GiB) and omits the `peak_rss_kib` print that produced the stray empty fourth line of `gap.out`. Replace `command.sh` with the verbatim probe script (`session-9-prompts/gap-b4-probe.sh`, the script that actually ran, with a one-line header naming the date and host) and drop the 48.6 figure, before any research-review reads the package.
- [ ] `3f7edef1`: lead-review the rework-2 branch (Tier 1.5 over all rounds; the worker's resolution table is in its transcript `~/.claude/projects/-home-vkaskivuo-Projects-gf2/b6ff6d78-3691-40ce-9546-069e618e637c/subagents/agent-aworker-3f7edef1-4-*.jsonl` and in this session's transcript), merge, run cargo-ci on the merged tree, and hold the three AI reviews until the owner resolves the B4 question; then apply the approved issue-text change and run code-review, doc-review, research-review in one round.
- [ ] Reclaim merged worktrees agent-{2b6968d3,88ca7d2f,d7749931,bd0edfa2,b1bd75ca} with the reclaim script when no build runs (never agent-4e732b56; keep agent-3f7edef1 until merged).
- [ ] Standing pitfalls to reconcile at close (progress.json `surfaced_pitfalls`): verify-lean.sh header count; FpField.lean link gap for 94597a51; cargo-ci has no doctest step (shared infra, owner); tuning-profile-compose coding-owner mode for fd9d5416/d1b4f85e.

## Traps — do not repeat these

- **Do NOT let cargo spawn sccache under the CCX1 flock.** `ccx1-bench-flock.sh --full-host … cargo bench` on an idle host starts an sccache server that inherits the lock fd and holds `/tmp/gf2-ccx1.lock` after the bench; the next `flock -x` (and every `cargo-budget.sh` shared lock) blocks forever. Check `lslocks | grep ccx1` after every locked run; `sccache --stop-server` frees it; or set `CARGO_CI_NO_SCCACHE=1` for locked runs.
- **Do NOT trust `used` memory before a memory-heavy run.** /tmp is a 32 GB tmpfs; Luna sandbox workers left ~17 GB of `gf2-*-target` dirs there. `du -sh /tmp/gf2-*` and delete caches of done issues first.
- **Do NOT expect a plan amendment to satisfy a reviewer enforcing the issue's own text.** The owner's "amend the evidence protocol" decision was executed in `plan.md`; code-review and research-review still fail on the issue's Background sentence and one reviewer said outright that a plan amendment "is not a binding `## Decisions` item". Binding places are the issue description/criteria or a `## Decisions` item on the issue — an owner-approved issue-text change.
- **Do NOT leave a done issue's linked design doc stale when its anchor test lands.** The proof sketch cost d7749931 two doc-review rounds; lean dispatch prompts now carry R-43.
- **Do NOT make a representation-specialization trait private.** bd0edfa2's private `MatrixFill` failed 444c06bc's code-review against epic REQ-06; the accepted shape is a public trait with provided generic bodies and in-tree overrides (R-45), and a newly public caller-trusted method needs a `# Panics` section stating the exact shapes (doc-review R3).
- **Do NOT run cargo-ci on main while three workers build.** One transient GPU smoke-test SIGABRT (`gf2-sim::dvb_t2_regression test_dvb_t2_regression_smoke_gpu_r12_16qam`, passes isolated in 0.9 s) cost a gate rerun.
- **Do NOT state a measured number in prose without a committed run artifact.** Research-review rejected the 808/808 stream result and the 50 GiB attempt's figures until the runner executed the check and the artifacts were committed; runners must run and record every claimed check, and a lead-side probe's outputs must be committed, not summarized.
- **Do NOT forget to link receipts before the review.** bd0edfa2's code-review R3 failed solely on the unlinked receipt (lead-owned); link every durable artifact right after merge, before gates.
- **Do NOT seed a fresh worktree from the pool without checking gf2-kernels-simd.** Stale pooled rlibs produced `unresolved import gf2_kernels_simd::bch_encode`; purged this session, but re-purge after each harvest (the trap from handoff-4 is still live).
- **Do NOT hand a worker a reconstructed command file as a run artifact.** The lead's `command.sh` for the 50 GiB attempt was rewritten from memory and disagreed with `gap.out` and `timeline.txt` in two places; commit the script that actually ran, verbatim.
- A zsh `IFS='|' read -r a b c <<< "$x"` loop inside a Bash tool call broke PATH (`command not found: jit`); use separate commands.
- All traps in handoff.md through handoff-5.md remain in force.

## Open questions needing invoker input

- Question: how should 3f7edef1's B4 rule be made binding for the reviewers?
  - Context: GUAVA `BCHCode(65535,1,25,GF(2))` cannot be built on this host (50 GiB heap failed; 52 GB peak, 15.6 GB swap; artifacts on the rework-2 branch under `oracle/attempts/2026-09-02-b4-heap-50g/`). The owner's amend branch is in `plan.md` Amendment 2, but code-review and research-review enforce the issue's Background text ("a row where an oracle fails to produce a result is a blocking finding, not a recorded gap"). Same root cause ×3.
  - Options: (1) amend the issue's Background and REQ-01 (or add a `## Decisions` D-item) to state the B4 rule; the lead applies the approved wording and re-runs the gates; (2) a ~90 GB GAP retry through swap (hours, may still fail); (3) reject the issue.
  - Recommendation: option 1. Proposed Background addition: "On corpus row B4 the GAP/GUAVA result is the `BCHCode` generator derivation and GUAVA's cyclic-code polynomial encoding map, with the run's bounded `BCHCode` attempt recorded, as Amendment 2 of the evidence protocol fixes; every other row requires the full code object." and REQ-01: "… across the predeclared corpus (B4 per the amended protocol)".

## Reference artefacts

- Epic: `jit issue show ae03bcd0`; progress.json, plan.md (Amendments 1–2), bch-api-design.md (MatrixFill contract), extension-design.md, oracle-provenance.md, oracle-receipt.md (this directory).
- Session-9 prompts (committed): `session-9-prompts/` in this directory — rework and dispatch briefs, including the three wave-11 dispatch prompts.
- Session-9 scratchpad (may not survive): `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/b6ff6d78-3691-40ce-9546-069e618e637c/scratchpad/` — gate logs (`gates-wave10-{a,b,c,d}.log`, `gates-3f7edef1-r2.log`), `b4-attempt-50g/`, `88ca7d2f-run1/` (the discarded contended measurement).
- Proof sketch: dev/active/64fd3afd/proof-sketch.md (O-5 for 94597a51 is the remaining obligation).
- ETSI DVB-T2 streams: ~/dvb_test_vectors → ~/Projects/dvb_test_vectors/VV001-CR35_CSP (host symlink).
