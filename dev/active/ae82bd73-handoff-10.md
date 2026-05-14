# ae82bd73 (Permanents F_3/F_5/F_7) — session 8 handoff

**Date:** 2026-05-14
**Branch:** main (HEAD `f1867c9a`)
**Session focus:** close W4 sub-4b — three sibling tasks dispatched in parallel via worktrees, multi-round code-review cycles converged.

## What landed this session

- **684a6715 (permanent_bipedal5)** — DONE after 5 review rounds. Worker (Sonnet) initially used `Packed5Vec` accumulator with O(n) conversions per Gray step instead of mirroring the F_3 reference's single-word `Packed5` accumulator with O(1) `Packed5::add` / `Packed5::sub`. Lead-direct rewrite fixed the hot loop and surfaced cascading documentation mismatches (module doc, function doc, packed5.rs `Packed7Matrix` doc) all corrected via narrow `fix(...)` commits. User-approved amendment (option C) documents the infeasibility of literal n=63/64 word-boundary cross-checks (2^63 Gray steps is decades of CPU); the boundary coverage is provided by n=0/1/65-panic plus slow-tier n=15/16/20/24/32.

- **063f49bb (permanent_bipedal7)** — DONE after 1 review round. Worker (Sonnet) correctly used `Packed7::add`/`Packed7::sub` directly. Review found two issues: (a) `permanent/mod.rs` module docstring said F_5/F_7 single-word bound is n≤64, but `Packed7::LANES = 16`; (b) n=13/n=14 cross-checks were in slow tier (`#[ignore]`) while criterion requires them. Both fixed: module doc spells out F_5=64, F_7=16; n=13/n=14 moved to fast tier (measured 0.64s and 1.57s respectively, well under 5s budget).

- **1f769232 (SIMD F_5+F_7 kernels)** — DONE after 4 review rounds. Worker (Sonnet) implemented `Config5: BipedalLikeConfig` and `Config7: BipedalLikeConfig` impls to satisfy criterion 1 ("BipedalLikeConfig impls"). The Config5 impl SILENTLY CORRUPTED F_5 value 4 because the framework's 2-stream `(MagLane, SgnLane)` shape cannot hold F_5's R1 Candidate D 3-plane bit-sliced encoding (`b0, b1, b2` — value 4 needs `b2=1`). User-approved amendment (option A) dropped the BipedalLikeConfig criterion for F_5/F_7; F_5/F_7 now ship exclusively via dedicated AVX2 batch entry points in `x86/bipedal_avx2_packed{5,7}.rs` + runtime-detection bundles + scalar fallbacks. Removing the impls required cascading narrative sweeps across `bipedal/mod.rs`, `bipedal/framework.rs`, `bipedal/bipedal3.rs`, `bipedal/lanes.rs`, and `x86/bipedal_avx2.rs`. AVX-512 criterion recorded as deferred (natural home: `f8d230ef` in epic `7f809931`).

- **Worktree dispatch protocol followed end-to-end.** Pre-flight on clean main, snapshot, 3 worktrees anchored at `5c35cf9f`. Post-completion leak check ran and flagged my own pre-dispatch JIT state changes (expected — committed as `140d936a`). No worker file leakage into main.

## Status

| Wave | State | Notes |
|---|---|---|
| W0 (Decisions/sketches) | DONE | 11 issues |
| W1 (Foundation) | DONE | 7 issues |
| W2 (Reference + repro harness) | DONE | 6 issues |
| W3 (SIMD + multi-word + parallel) | DONE | 8 issues |
| **W4 (F_5/F_7 packed + SIMD)** | **DONE 2026-05-14** | 5 issues, 2 amendments |
| W5 (GPU HIP/ROCm) | PENDING | 6 issues; ROCm gfx1030 confirmed on dev host |
| W6 (Lean verification) | BLOCKED on 8c902184 user sign-off | 4 issues |
| W7 (Reporting + sims) | PENDING | 8 issues including 9480f8a6 S1g and 333028c1 CAS sim |

## Open escalations / decisions for next session

- **8c902184 (API freeze checkpoint)** — Criterion 4: *"User has signed off on the freeze checkpoint (recorded as an ## Approval section in the doc and in this issue's description)."* Cannot proceed autonomously per project-lead escalation policy item 4 (issue scope changes / explicit user attestation). The next session must:
  1. Draft `dev/plans/gf_api_freeze_w6.md` listing every public symbol in `gf2-algebra::packed::*`, `gf2-algebra::permanent::*`, and the GPU dispatch surface that needs to be locked.
  2. Reference the W6 issues (`f05ffbe1`, `0606186a`, `30e98ef1`) that consume the locked symbols.
  3. Establish change-control protocol per criterion 3.
  4. Present to user for explicit sign-off via `AskUserQuestion` (or whatever mechanism the user prefers — note in the doc + a yes/no question on intent).

## What worked / what to repeat

- **Worktree-isolated parallel dispatch.** Three Sonnet workers ran cleanly in parallel via the scripts in `~/.claude/skills/project-lead/scripts/`. Build + test + fmt + clippy all clean on each worker branch before merge. Merging into main needed no manual conflict resolution because each worker touched different modules.

- **User-approved amendment for infeasible criteria.** Two amendments this session, both via AskUserQuestion with concrete option sets. Option-C-on-684a6715 (document infeasibility) and option-A-on-1f769232 (drop BipedalLikeConfig requirement) both let the work proceed without softening any contract that could actually be met. The amendments are recorded in the issue descriptions AND mirrored in the implementation doc-comments + sweep commits.

- **Lead-direct rewrite for trivial worker mistakes.** The bipedal5 hot-loop fix was a one-shot 30-line rewrite. Dispatching a rework agent for that would have spent 30k tokens for an obvious mechanical change. The lead-direct fix kept the loop tight (one commit per finding) and converged in 5 rounds.

## Traps — do not repeat these

(Carrying forward all traps from handoffs 1–9. New traps from this session:)

- **Trap W4b-1 (CRITICAL — silent correctness bug)**: The `gf2-kernels-simd::bipedal::framework::BipedalLikeConfig` trait's 2-stream `(MagLane, SgnLane)` shape was designed assuming F_5 / F_7 could fit it. The post-R1 decision (F_5 = 3-plane bit-sliced) and post-R2 decision (F_7 = 1-plane LUT) BROKE that assumption. A worker faking F_5 through 2 planes WILL silently corrupt value 4. **For any future issue whose criterion text was written before R1/R2 outcomes were known, audit the criterion against the actual encodings BEFORE dispatch — do not let a worker discover the mismatch and improvise an unsound workaround.**

- **Trap W4b-2 (test budget vs criterion-stated n range)**: bipedal7's worker marked n=13/n=14 as slow-tier `#[ignore]` thinking they'd exceed 5s budget. Actual measurement was 0.64s and 1.57s. Criterion required n=1..14 to be cross-checked; #[ignore] excluded them from default `cargo nextest --profile ci`. **Measure first when in doubt — don't assume slow tier based on n alone; F_7 LUT is much faster per step than F_5 bit-sliced.**

- **Trap W4b-3 (CLAUDE.md word-boundary contract vs computational feasibility)**: CLAUDE.md:149 says "Always cover word-boundary edge cases: 0, 1, 63, 64, 65 bits". For permanent functions of an n×n matrix, n=63/64 means walking 2^63 ≈ 9.2e18 Gray steps — physically infeasible. Reviewer interpreted the bit-boundary rule for permanent's n dimension. **If you face a similar literal-contract-vs-infeasibility conflict, escalate via AskUserQuestion early — do not silently add tests that satisfy the letter of the rule (e.g. `n=20/24/32` slow-tier) and hope the reviewer accepts it. Reviewers don't, and the round count keeps climbing.**

- **Trap W4b-4 (post-amendment doc sweep)**: Removing `Config5` / `Config7` / `Bipedal7x4` from one file leaves stale references in EVERY file that previously documented "F_5/F_7 will use the framework". Found and fixed in: `bipedal/mod.rs`, `bipedal/framework.rs`, `bipedal/bipedal3.rs`, `bipedal/lanes.rs`, `x86/bipedal_avx2.rs`. **After a major architecture amendment, sweep the entire affected module hierarchy in ONE pass — do not let the reviewer surface them one at a time. `git grep` on the removed symbol names is the right tool.**

- **Trap W4b-5 (reviewer cycle count vs MAX_REWORK_ATTEMPTS=2)**: The bipedal5 path hit 5 review rounds via lead-direct fixes (one round of worker dispatch + 4 lead-direct fix commits). Per `references/escalation-policy.md` the rework counter applies to worker dispatches; lead-direct fix commits are not "rework attempts" in the strict sense. **But each lead-direct fix that misses something the reviewer flags later is functionally equivalent to a worker round — they cost the same review cycle. When the reviewer surfaces a NEW finding each round (not a regression), assume the next round will surface yet another and audit the entire artifact for similar issues BEFORE running the gate again. Don't run the gate to find the next finding.**

## Active worktrees

None — all W4 sub-4b worktrees cleaned up post-merge.

## Active background processes

None.

## Session-8 metrics

- **Issues closed:** 3 (684a6715 — 5 review rounds; 063f49bb — 1 round; 1f769232 — 4 rounds).
- **User escalations resolved:** 2 (684a6715 boundary infeasibility; 1f769232 BipedalLikeConfig criterion drop).
- **User escalations open:** 0 in-progress. 1 future escalation queued for next session: 8c902184 API-freeze sign-off.
- **Commits on main:** 20 (3 worker `feat`, 3 `merge`, 4 doc/sweep commits, 1 stale-narrative cleanup, 1 amendment commit, 4 `fix` rounds on 684a6715, 1 round on 063f49bb, 4 `fix` rounds on 1f769232, 2 JIT-state chores).
- **Tests passing on HEAD `f1867c9a`:** 3783 fast-tier (gf2-algebra: 431; gf2-kernels-simd: 222; gf2-core: 3074; gf2-coding: 56 — workspace --all-features --release --profile ci).
- **Lines of code added/changed (net, post-amendment removals):** roughly +2500 / -345 = +2155 net.

## Next-session priorities (verbatim from progress.json)

1. **ESCALATE 8c902184 (API freeze) to user** — criterion 4 requires explicit user sign-off; cannot proceed autonomously. Lead should prepare the `dev/plans/gf_api_freeze_w6.md` draft and present for user approval before W6 Lean work starts.

2. **W5 GPU sub-wave**: dispatch `b62c86d8` (hip scaffold) first; once landed, dispatch `ad55b777` + `b43cdf33` + `5c0505b2` in parallel via worktrees; then `2fbbdfa5` (host dispatcher) and `a9e461de` (GPU vs CPU sim).

3. **W6 Lean (post-freeze)**: `f05ffbe1` → `0606186a` (parallel after `f05ffbe1`) → `30e98ef1` (after `f05ffbe1`). All three need approved proof sketches per CLAUDE.md verification-work convention; sketches `a0c0a45f` and `4aaa6e4d` already done; new sketch may be needed for F_5/F_7 (issue `30e98ef1` is `[aspirational]` so amendment is permitted if encodings prove intractable).

4. **W7 Reporting**: `7cd9afdb`, `16f03734`, `8808b051`, `424aa94f`, `c90db5a4` — all simple-ish, can run in parallel.

5. **S1g (`9480f8a6`)** — dep on `ad55b777`; runs after W5 GPU.

6. **Final**: epic `ae82bd73` completion report + transition to done.
