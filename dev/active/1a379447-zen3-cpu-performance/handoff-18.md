# Handoff — Zen 3 CPU performance (1a379447) — session 23

**Date:** 2026-09-26
**Session number:** 23
**Prior handoffs:** See the ordered list in [progress.json](progress.json), through [handoff-17.md](handoff-17.md).

## Current state

- Epic in progress; wave 3 of 7. Direct wave entries: 12 done, 2 rejected, 2 backlog stories (2037941f and c04dd4ac). Twelve unfinished descendants are listed in progress.json session_scope; some are stored in original later wave buckets but are prerequisites of these stories.
- Sol high workers prepared 00dd43c3 and 65c0e13d; Sol xhigh prepared bc091474. Workers finished; no Astra used. Lead lease on bc091474 renewed for one hour before integration; other leases may expire. Query live claims before mutations.
- Rework decisions on e1f9a78f and 9fb40c83 and the next window time remain pending. No timer is armed. The observed window log ends September 19; the historical September 20 scheduling record does not prove execution.
- Progress: [progress.json](progress.json). The current wave is NOT complete.

## What just happened

- Closed 5ac79460: pilot-pair defaults in the shared execution-log checker now follow the receipt-local addendum; preserved receipts pass. Merged CI and code review passed; lead review [5ac79460-r1](reviews/5ac79460-r1.md).
- Closed 18a87159: documented observed top-level window evidence and explicitly reconstructed inner invocations. User accepted this historical reconstruction in decision D-01 and prioritized results over excessive provenance bookkeeping. Both code and research review passed with zero findings after that decision; lead review [18a87159-r3](reviews/18a87159-r3.md). Existing measured snapshots remain unchanged; future launches record argv.
- Merged dense staged smoke at becc06505; merged CI passed. Code review failed on lost original timed argv and old-reference commit subjects. All 15 cited historical commits are outside main ancestry. [e1f9a78f-r6](reviews/e1f9a78f-r6.md) records the audit. Additional rework requires the pending decision.
- Merged residual-shift preparation at c87d90dba; CI passed. Worker agent-00dd43c3 is clean and pinned at 3c08c3494. Seven cells, both routes, shared untimed smoke, zero timing windows. One 20-minute pilot queued.
- Merged ISA-L preparation in two stages; both merged trees passed CI. Worker agent-65c0e13d is clean and pinned at 8f2875d8c. Twelve-cell pilot, compiler/object evidence, shared smoke with 24 validation dispatches and zero timing windows. One 20-minute pilot queued.
- Merged logical candidate preparation at 59bff36c8; cargo-ci a35c8d88 passed (6101 tests, zero failures). Its inputs_moved annotation reflects only lead handoff/progress/doc-link writes during the gate; code stayed fixed. Worker agent-bc091474 is clean and pinned at 2d33ac5ca. AVX2 factors 2 and 4 only; no NR constructor candidate. Both release harnesses passed 31 tests; four staged smokes each completed 17 cells and zero timing windows. Four pilots and two profiles queued, estimated 76 minutes. Ordinary production builds retain their existing kernel.
- Candidate wrapper may commit only verified receipt/profile artifacts and its own family ledger in its worker branch during the window. This is required for the next pilot to read a committed ledger. No source, tracker, main, or push writes are authorized by that wrapper.

## What to do next

- [ ] Read any responses to the three pending questions below; user approval of reconstruction alone does not answer them.
- [ ] If authorized, dispatch the dense argv repair and historical-ref audit, then rerun its code/doc gates. Do not rewrite main or delete evidence refs.
- [ ] If authorized, dispatch DVB profile rework collecting counters and hot symbols in all nine repetitions and reporting intervals; queue a fresh v4-r3 profile. Preserve the accepted A/B evidence.
- [ ] Before scheduling the agreed overnight time, hold the five live out-of-wave confirmation lines (f63a2464 x3, ad2a6a58, 4c1e441f). Leave their pinned worktrees and evidence intact. Current-wave prepared jobs total an estimated 116 minutes, plus any newly authorized profile.
- [ ] Run only through the canonical window runner with GF2_BENCH_WINDOW=1. Read each campaign execution log, preserve receipt inputs including ignored Cargo.lock, regenerate the receipt schema index after receipt commits, merge sequentially and run merged-tree CI after every merge.
- [ ] Interpret pilots and freeze confirmations under existing rules; do not infer a performance win from preparation. Finish gates and lead review only after each issue's timed criteria are met.
- [ ] Follow dependencies: 65c + bc → fcb; bc → a1ad; e1 → c73; c73 + fcb → 50f → 6e → 2ad (also a1ad) → 2037941f. 00dd + 9fb → a081 → c04dd4ac. Keep wave 3 until both stories finish.

## Traps — do not repeat these

- Earlier traps remain in [handoff-17.md](handoff-17.md) and its predecessors. Do not merge old-history pinned confirmation branches directly into rewritten main; replay measured changes through scratch branches after their window.
- Historical commit IDs outside main are not main subject violations. The ancestry audit is in the dense lead review; do not rewrite immutable main history or discard source refs to appease a review.
- Do not apply the accepted historical reconstruction decision as a blanket gate waiver. It resolves 18a87159 D-01 only. Conversely, do not invent more provenance frameworks; the user explicitly prioritizes results over excessive bookkeeping.
- The shared staged smoke can pause with exit 3. Resume the same stage until completion; clearing it on each retry prevents progress. Smoke remains untimed and finalize must refuse it.
- ISA-L build helpers must propagate failures inside command substitution. Retained C build evidence can survive a Rust-only relink; refresh the Rust arm digest after validating those C inputs.
- Candidate family ledgers must be committed between pilots. The narrow issue wrapper owns that evidence commit, not shared campaign infrastructure.
- No timing outside the overnight window, no host locks during this working session, no implicit timer approval. No worker builds during lead CI. Preserve pinned trees until their queued evidence is collected.

## Open questions needing invoker input

- Authorize one further e1f9a78f rework round to preserve original timed argv and document the historical-reference findings? Recommended: yes, scoped to those findings, with no history rewrite. Six rounds have already occurred; session 21 requires a decision for every round beyond the limit.
- Authorize 9fb40c83 round 4 with a fresh overnight profile collecting counters/hot symbols in all nine repetitions and interval tables? Recommended: yes; preserve existing A/B measurements.
- Schedule the prepared current-wave jobs for September 27 at 04:00 Europe/Helsinki (01:00 UTC), holding unrelated confirmations? Recommended: yes. The invoker sets the time; silence is not approval.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [progress](progress.json), [worker brief](worker-brief.md), [measurement contract](measurement-contract.md).
- [Candidate readiness](../bc091474/readiness.md), [portfolio](../bc091474/portfolio.md), [candidate launcher](../bc091474/run-candidate.sh).
- [ISA-L pilot](../2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-isal-base-gap.json), [residual-shift pilot](../00dd43c3/pilot-addendum.json).
- [Queue](bench-window/queue.tsv), [window runner](bench-window/run-window.sh); runtime log `.agents/bench-window/window.log`.
- Pinned out-of-wave trees: agent-0a357f94 at a00540a8f (ad2a6a58), agent-9fb40c83 at 706450eed (f63a2464), agent-c04dd4ac at 897ca69ab (4c1e441f). Foreign agent-02b8137c-run must not be touched.
