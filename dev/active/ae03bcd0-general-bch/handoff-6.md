# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 9

**Date:** 2026-09-02T17:05+00:00 (interim; rewritten at session end if the session survives)
**Session number:** 9 (session 8 died at the Claude session limit at 12:08Z with five workers mid-flight and no final reports)
**Prior handoffs:** handoff.md, handoff-2.md, handoff-3.md, handoff-4.md, handoff-5.md — their Traps sections remain in force in full.

## Current state

- Epic `ae03bcd0` — state: backlog (container); claimed by agent:jit-execution-lead.
- Wave in progress: wave 10 of 15, plus early starts (wave-12 `b1bd75ca`).
- Children: 45 done (this session closed `d7749931`, `2b6968d3`, `88ca7d2f`); open in wave 10: `444c06bc` (rework 2 in flight, `worker-444c06bc-r2`), `bd0edfa2` (gates passed; done blocked by 444c06bc), `3f7edef1` (rework 2 in flight for research F2–F5, `worker-3f7edef1-4`; B4 escalated); early: `b1bd75ca` (`worker-b1bd75ca`, wave 12 Lean O-4).
- Active claims: epic, 3f7edef1, bd0edfa2 by agent:jit-execution-lead; 444c06bc, b1bd75ca by agent:worker.
- Open escalation: 3f7edef1 B4 (below).
- Progress file: `progress.json` here (current through rework-2 dispatches; rulings R-40..R-45).

## What just happened

- Reconstructed session 8's end from worker transcripts and CI logs; lead-preserved 3f7edef1's uncommitted draft (d6f03947).
- `88ca7d2f`: first measurement opened at load 7.96; re-measured on the idle host with the worker's committed runner (R-41, 51f11e22), merged 0b408133; code-review R1 F1 (runner copied every `bch_*` artifact) closed lead-direct 6b8577fb (R-44); all gates passed; done; receipt linked.
- `2b6968d3`: rework 1 (clmul-fold family, combined predicate) reviewed, one lead-direct survey-doc fix (R-42), merged f5efb7cc; cargo-ci + code-review passed; done. 4e732b56 doc links refreshed.
- `d7749931`: lake-build + code-review passed; doc-review failed twice on the stale O-3 proof sketch (anchor test "must add", shifted citations, L3.7 row) — fixed lead-direct (7a380646, 3c744093; R-43 standing rule for lean workers); passed R3; done. `b1bd75ca` unblocked and dispatched early.
- `444c06bc`/`bd0edfa2`: rework 1 merged 8754afc8; cargo-ci ×2, 444c06bc doc-review R2, bd0edfa2 code-review R2 passed; 444c06bc code-review R1 F1: private `MatrixFill` bound blocks external `SymbolMatrix` representations (REQ-06) → R-45 public trait with provided bodies → rework 2 in flight.
- `3f7edef1`: B4 GAP retry at -o 50g / 52G scope FAILED (9m17s, 54.4 GB peak RSS, 15.6 GB swap); part-2 rework merged 93f08af2; cargo-ci passed on rerun (first run: transient GPU smoke SIGABRT under load 14); code-review R2 + research-review R2 F1 = B4 root cause ×3 → escalated; doc-review R2 F1 (bare `cargo run` in the example) closed lead-direct e899f8c8; research R2 F2–F5 → rework 2 in flight with the lead's 50 GiB attempt artifacts packaged for committing.
- Host: tmpfs /tmp held ~17 GB of stale Luna cargo target dirs (deleted); an sccache daemon spawned under the CCX1 flock inherited the lock (killed).

## What to do next

- [ ] Collect `worker-444c06bc-r2`: review per lead-review-protocol (Tier 1.5: cargo-ci timeout, doc-review R1 F1, code-review R1 F1 for 444c06bc; bd0edfa2 R1 F1–F3), merge, run `444c06bc cargo-ci --force`, `444c06bc code-review`, `bd0edfa2 code-review --force`, `444c06bc doc-review --force`; then done 444c06bc, done bd0edfa2 (link bd0edfa2's receipt `dev/bench_results/bd0edfa2/2026-09-02-bd0edfa2-genmatrix-receipt.md`; refresh 4e732b56's workload-selection link and 7a3a6738's bch-api-design link).
- [ ] Then dispatch wave 11: `e1e0e7ff`, `5ee83cd3`, `203ee826` (prompts in the session-8 scratchpad copy `s8/dispatch-*.md`, session-9 scratchpad; re-derive from `bch-api-design.md` "Consumer migration files" if gone). Native workers only (R-40). `97410c80` after 203ee826.
- [ ] Collect `worker-3f7edef1-4` (F2–F5); merge; re-run doc-review + research-review (+ code-review); F1 stays open until the owner answers the escalation.
- [ ] Collect `worker-b1bd75ca` (Lean O-4): lake-build, code-review, doc-review; refresh 64fd3afd's sketch link after merge.
- [ ] Reclaim merged worktrees agent-{2b6968d3,88ca7d2f,d7749931} when no build runs (script; never agent-4e732b56).

## Traps — do not repeat these

- **Do NOT let cargo spawn sccache under the CCX1 flock.** `ccx1-bench-flock.sh --full-host … cargo bench` on an idle host starts an sccache server that inherits the lock fd and holds `/tmp/gf2-ccx1.lock` after the bench; the next `flock -x` (and every `cargo-budget.sh` shared lock) blocks forever. Check `lslocks | grep ccx1` after every locked run; `sccache --stop-server` frees it; or set `CARGO_CI_NO_SCCACHE=1` for locked runs.
- **Do NOT trust `used` memory before a memory-heavy run.** /tmp is a 32 GB tmpfs; Luna sandbox workers left ~17 GB of `gf2-*-target` dirs there (counted as RAM/swap). `du -sh /tmp/gf2-*` and delete caches of done issues first.
- **Do NOT expect a plan amendment to satisfy a reviewer enforcing the issue's own text.** The owner's "amend the evidence protocol" decision was executed in `plan.md`; both code-review and research-review still fail on the issue's Background sentence. Binding places are the issue description/criteria or a `## Decisions` item on the issue — an owner-approved issue-text change (policy category 3/4).
- **Do NOT leave a done issue's linked design doc stale when its anchor test lands.** The proof sketch cost d7749931 two doc-review rounds; lean dispatch prompts now carry R-43 (worker updates the sketch's own obligation section and remaps shifted citations).
- **Do NOT run cargo-ci on main while three workers build.** One transient GPU smoke-test SIGABRT (gf2-sim `test_dvb_t2_regression_smoke_gpu_r12_16qam`, passes isolated in 0.9 s) cost a gate rerun.
- **Do NOT run the full-corpus stream comparison's claims without a receipt.** Research-review treats a number stated in prose (808/808, the 50 GiB attempt's figures) as unsupported unless a committed run artifact carries it; runners must execute and record every claimed check.
- A zsh `IFS='|' read -r a b c <<< "$x"` loop inside a Bash tool call broke PATH (`command not found: jit`); use separate commands.
- All traps in handoff.md through handoff-5.md remain in force.

## Open questions needing invoker input

- Question: how should 3f7edef1's B4 rule be made binding for the reviewers?
  - Context: GUAVA `BCHCode(65535,1,25,GF(2))` cannot be built on this host (50 GiB heap failed; 52 GB peak, 15.6 GB swap). The owner's amend branch is in `plan.md` Amendment 2, but code-review and research-review enforce the issue's Background text ("a row where an oracle fails to produce a result is a blocking finding, not a recorded gap"). Same root cause ×3.
  - Options: (1) amend the issue's Background and REQ-01 to state the B4 rule (lead applies the approved wording, re-runs gates); (2) a ~90 GB GAP retry through swap (hours, may still fail); (3) reject the issue.
  - Recommendation: option 1; proposed wording is in the session-9 transcript's escalation message.

## Reference artefacts

- Epic: `jit issue show ae03bcd0`; progress.json, plan.md (Amendments 1–2), bch-api-design.md, extension-design.md, oracle-provenance.md, oracle-receipt.md (this directory).
- Session-9 scratchpad (`/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/b6ff6d78-…/scratchpad/`): `rework-444c06bc-bd0edfa2-r2.md`, `rework-3f7edef1-r2.md`, `rework-3f7edef1-r1-part2.md`, `dispatch-b1bd75ca.md`, `b4-attempt-50g/`, `s8/dispatch-{203ee826,5ee83cd3,e1e0e7ff}.md`.
- Proof sketch: dev/active/64fd3afd/proof-sketch.md (O-4 for b1bd75ca).
- ETSI DVB-T2 streams: ~/dvb_test_vectors → ~/Projects/dvb_test_vectors/VV001-CR35_CSP (host symlink).
