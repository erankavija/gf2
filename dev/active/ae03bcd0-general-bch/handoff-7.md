# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 10

**Date:** 2026-09-03T04:59+00:00
**Session number:** 10
**Prior handoffs:** handoff.md, handoff-2.md, handoff-3.md, handoff-4.md, handoff-5.md, handoff-6.md — their Traps sections remain in force in full.

## Current state

- Epic `ae03bcd0` — state: backlog (container); claimed by agent:jit-execution-lead.
- Wave in progress: wave 11 of 15 (`current_wave` = 11). 203ee826 done; e1e0e7ff and 3f7edef1 and b1bd75ca merged with one or two AI gates each awaiting a re-run; 5ee83cd3 and 97410c80 merged with code-review awaiting a run (cargo-ci passed on both).
- Children: 44 of 59 done (5 in_progress, 0 ready, 10 backlog). Open:
  - `997f0ab9` (Create the tracked follow-up issues against the canonical in): work done (follow-ups 1642af1c, 1a8f6acd, b4d7a25d); dep 7a3a6738 now done -> claim, repo-validate gate, state done pending
  - `19fe9394` (Generic irreducibility validation with reusable certificates): merged b9311d2b; cargo-ci gate passed; code-review gate NEXT in queue
  - `3f7edef1` (External oracle and standards-vector agreement): session 10: merged dddd121f; cargo-ci, code-review, doc-review passed; research-review R3 findings closed lead-direct 01003c2d; doc-review and researc
  - `e1e0e7ff` (Shared property and conformance suites across field classes): session 10: merged 9672d944; cargo-ci passed; code-review PENDING (last run recorded "Agent produced no output", not a verdict)
  - `5ee83cd3` (Migrate the component-code BCH consumers to the canonical mo): session 10: native Sonnet; work complete on worktree-agent-5ee83cd3 at 7c6904cc (four commits: GLDPC, BCJR, OSD adapter evidence, crate-root re-export
  - `97410c80` (Migrate the DVB-T2 BCH consumers to the canonical model): session 10: dispatched to native Opus at 941f81db (reclassified hard: three lead rulings R-50 legacy-constructor relocation to core.rs, R-51 decode th
  - `591a1c5e` (Migrate gf2-coding benchmarks): pending
  - `ef8ff9c4` (Migrate gf2-coding binaries and examples): pending
  - `0c21cb1e` (Migrate gf2-sim BCH consumers): pending
  - `227ac5c8` (Migrate gf2-coding integration tests): pending
  - `b1bd75ca` (Lean proofs: generator base-field membership and root correc): session 10: rework 1 merged f6f11755; lake-build + code-review passed; doc-review R2 F1 closed by linking the O-4 sketch; doc-review re-run PENDING (l
  - `d1b4f85e` (Extend Criterion benchmarks to the selected workloads): pending
  - `4a2baa12` (Delete the superseded BCH code surface): pending
  - `94597a51` (Lean proofs: systematic encoding correctness): pending
  - `f759d724` (Sweep stale prose references to the removed BCH surface): pending
  - `fd9d5416` (Committed performance receipts: non-regression, determinism,): pending
  - `4ad869d6` (Researcher-oriented rustdoc and runnable examples): pending
- Active claims: epic, 3f7edef1, b1bd75ca, e1e0e7ff, 5ee83cd3, 97410c80 by agent:jit-execution-lead.
- Open escalations: none (the 3f7edef1 B4 question is resolved by owner ruling R-48; the issue text carries D-01).
- Progress file: `progress.json` here (rulings R-46..R-53, session-10 notes, traps, pitfalls).
- No detached job and no worker is running at handoff; every gate record is committed.

## What just happened

- 3f7edef1: attempt package corrected on the branch (verbatim `gap-b4-probe.sh` replaces the reconstructed `command.sh`, dbc9de19); lead review of rework 2 (Tier 1.5 from the worker's transcript, Tier 2.5/2.75 clean); merged dddd121f; cargo-ci passed; owner ruling applied to the issue text (Background sentence, REQ-01 parenthetical, D-01; 832038a7); code-review and doc-review passed; research-review R3 F1 (rss.log peak not attributable) + F2 (probe identity) closed lead-direct 01003c2d; both docs re-linked.
- b1bd75ca: rework 1 (native Opus) modelled production's three `witness_longest_run` branches (`witnessedRun`, `witnessedStart`, `reportedBound`, `correctionRadius`; L4.7/L4.8 over every T; base-case anchors spec.rs:2659/2673) — lead review PASS, merged f6f11755; lake-build + code-review passed; doc-review R2 failed only on the unlinked sketch → `jit doc add b1bd75ca dev/active/64fd3afd/proof-sketch.md`.
- Wave 11 dispatched at dddd121f: 203ee826 (Opus) → merged ecbdb8e1, all three gates passed, done 941f81db (R-46 `has_canonical_message_order`; findings filed c2663ce9, ccaed736 outside the epic); e1e0e7ff (Opus) → integration with 203ee826 done by the same worker (data-driven canonical-layout case; both `Shortened` derivations asserted), merged 9672d944, cargo-ci passed; 5ee83cd3 (Sonnet, owner choice R-47) → four commits on its branch at 7c6904cc, report pending.
- 97410c80 dispatched to native Opus at 941f81db with rulings R-50 (legacy `BchCode::dvb_t2` relocates to core.rs), R-51 (decode through the mother, citing 1a8f6acd), R-52 (bit-identical to the legacy encoder on all twelve configurations); brief in `session-10-prompts/dispatch-97410c80.md`.
- Four AI gate runs at ~04:52–04:55Z recorded `ERROR: Agent produced no output` (b1bd75ca doc-review, e1e0e7ff code-review, 3f7edef1 doc-review, 3f7edef1 research-review); they are checker failures, not verdicts.

## What to do next

- [ ] Run, serially, with the lead's lease renewed on each issue first: `jit gate evaluate b1bd75ca doc-review`, `jit gate evaluate e1e0e7ff code-review`, `jit gate evaluate 5ee83cd3 code-review`, `jit gate evaluate 97410c80 code-review`, `jit gate evaluate 3f7edef1 doc-review`, `jit gate evaluate 3f7edef1 research-review`. Check the first result before the next.
- [ ] Done-transitions once gates pass: b1bd75ca (frees 94597a51), e1e0e7ff (frees fd9d5416's second dep), 3f7edef1 (closes wave 10). Commit `.jit` per transition.
- [ ] 5ee83cd3: merged e54ad63c, cargo-ci passed, lead review PASS; run `jit gate evaluate 5ee83cd3 code-review` (lease held by the lead), then done. Footprint note from its worker: `bch/mod.rs` module doc still headlines `BchCode::new` (4a2baa12's cleanup).
- [ ] 97410c80: merged 17d511af (+ lead-direct survey wording e78aebcf, 4e732b56 link refreshed), lead review PASS (R-54), cargo-ci passed; run `jit gate evaluate 97410c80 code-review` and transition done. Its footprint findings are filed as a33fda32 (Shortened<C> allocation-free encode path; a REQ-13 risk for the DVB-T2 W1 cells, measure first in d1b4f85e) and 113ae672 (generic layout-declaring view), both outside the epic; docs/DVB_T2.md goes to f759d724; the bare `#[ignore]` tier fix is in the 227ac5c8 brief.
- [ ] Wave 12 once 5ee83cd3 and 97410c80 are done (both merged; only code-review outstanding): create worktrees with the dispatch script for 591a1c5e, ef8ff9c4, 0c21cb1e (native Sonnet) and 227ac5c8 (native Opus); prompt = script header + `session-10-prompts/wave12-common.md` + `session-10-prompts/dispatch-<id>.md`, with the DVB-T2 names filled in: `gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchCode, DvbT2MotherCode, DvbT2BchDecoder, DVB_T2_LAYOUT}` (see the module doc's example); the legacy `BchCode::dvb_t2` now lives in `bch/core.rs`. Disjoint file groups; one parallel wave.
- [ ] Then d1b4f85e (after 591a1c5e), 94597a51 (Lean O-5, Opus, R-43 sketch rule, after b1bd75ca), 4a2baa12 (after wave 12), f759d724, fd9d5416, 4ad869d6 per the wave plan.
- [ ] Reclaim merged worktrees agent-{2b6968d3,88ca7d2f,d7749931,bd0edfa2,b1bd75ca,203ee826,e1e0e7ff,3f7edef1} with the reclaim script when no build runs (never agent-4e732b56); re-purge stale gf2-kernels-simd artifacts from the pool after the harvest.
- [ ] Standing pitfalls to reconcile at close (progress.json `surfaced_pitfalls`): verify-lean.sh header count; FpField.lean link gap for 94597a51; cargo-ci has no doctest step (owner); tuning-profile-compose coding-owner mode for fd9d5416/d1b4f85e; the transform test module's private rank helpers duplicating test_support's (fold into 4a2baa12).

## Traps — do not repeat these

- **Do NOT read a fast AI-gate failure that says `ERROR: Agent produced no output` as a verdict.** It is a checker failure; the gate record carries no findings. Re-run the gate. A detached chain of gates fails every link the same way once the first does, so check the first result before letting a chain continue.
- **Do NOT publish a probe figure the sampler cannot attribute.** The B4 attempt's `rss.log` sampled `pgrep -x gap | head -1` without a PID; research-review rejected its peak. Read memory figures from the scope's own accounting (`scope.txt`) and say what the sampler is (01003c2d).
- **Do NOT leave the proof sketch unlinked on the Lean issue that amends it.** b1bd75ca's doc-review R2 failed only on that; `jit doc add <lean-issue> dev/active/64fd3afd/proof-sketch.md` alongside the 64fd3afd refresh.
- **Do NOT dispatch a trait-widening task and a suite that consumes the trait in parallel without a merge plan.** 203ee826 and e1e0e7ff conflicted in `test_support.rs`; the working resolution was routing the suite worker through `git merge main` after the first landed and making its layout choice data-driven (`is_systematic() && has_canonical_message_order()`).
- **Do NOT wait silently on a Sonnet worker that backgrounded cargo-ci.** It ends its turn until notified; when the host goes idle without a report, nudge it with SendMessage.
- **Do NOT read a gate launcher's exit code as the verdict.** A `jit gate evaluate` can exit 1 with `Error: Failed to restore recovery serialization after external process` while other jit commands run concurrently (doc add, issue create); the checker never ran and the tracker shows no run. Run `jit recover` and evaluate again; never read the launcher's exit code as the gate's verdict without `jit gate status`.
- **`jit issue create --priority medium` is rejected.** Omit the flag or use a value the command accepts.
- **Do NOT reconstruct a run artifact** (handoff-6 trap, confirmed): the verbatim probe script with a one-line header is what the reviewers accepted.
- All traps in handoff.md through handoff-6.md remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show ae03bcd0`; progress.json, plan.md (Amendments 1–2), bch-api-design.md (derivation paragraph, `has_canonical_message_order`), extension-design.md, oracle-provenance.md, oracle-receipt.md (this directory).
- Session-10 prompts (committed): `session-10-prompts/` — 97410c80 brief, the four wave-12 briefs plus `wave12-common.md`, the b1bd75ca rework brief, the 3f7edef1 description as applied.
- Session-10 scratchpad (may not survive): `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/1c68c678-c2ed-424a-b30e-978558266cc5/scratchpad/` — gate logs `gate-*.log`, `gates-chain-{a..e}.log`.
- Proof sketch: dev/active/64fd3afd/proof-sketch.md (O-5 for 94597a51 is the remaining obligation).
- ETSI DVB-T2 streams: ~/dvb_test_vectors → ~/Projects/dvb_test_vectors/VV001-CR35_CSP (host symlink).
