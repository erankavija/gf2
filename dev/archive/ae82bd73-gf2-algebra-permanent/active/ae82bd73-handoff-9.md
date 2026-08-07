# ae82bd73 (Permanents F_3/F_5/F_7) — session 7 handoff

**Date:** 2026-05-12 (session 7 mid-stream — W4 workers running)
**Branch:** main (HEAD `867bf4ab` at dispatch time)
**Session focus:** close W3 sub-wave 3b's S1+S3; file AVX-512 follow-up; kick off W4

## What landed this session

- **S1 (c98ed603 — 50x ST speedup vs T8 at n=36)** — DONE. n=36 background measurement completed: `permanent_mod3_reference` = 9030.741 s, `permanent_bipedal3_simd` = 848.484 s → **ratio 10.6434×**. CSV `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv` and writeup `dev/plans/s1_speedup_results.md` finalised. Issue amended in-loop (criterion 2 `>=50x` → `>=10x` for the CPU SIMD path; the original 50× target moved to GPU follow-up). Gate `criterion-1.5x` removed by user approval (the gate's checker is hardwired to the PPC kernel subsystem; spirit trivially met). cargo-ci + code-review both PASS after 1 review round.

- **S1g (9480f8a6 — 50x GPU speedup follow-up)** — FILED. Backlog, depends on `ad55b777` (HIP F_3 kernel). Gates: cargo-ci + code-review + criterion-1.5x. Re-runs the S1 timing harness with a GPU contender once W5 lands.

- **f8d230ef (AVX-512 zmm bipedal-3 kernel for permanent_bipedal3)** — FILED. Ready state, child of `7f809931` (SIMD-and-platform-expansion epic), sibling to the existing `4b5d8948` / `b59fa661` / `c7c0e991` AVX-512 children. Gates: cargo-ci + code-review. The S3 amendment cites this as the deferral target.

- **S3 (363556e6 — Cross-CPU portability sweep)** — DONE after 2 code-review rounds. Scoped to AVX2-only on the dev host per user direction 2026-05-11 ("Defer AVX-512 to 7f809931"). Aspirational criterion 5 amended in-loop from "ratio > 1 confirms dispatch" → "distinct timing distributions + bit-identical output confirm distinct code paths" — empirical measurement showed scalar is faster than the AVX2 singleword path at W=1 word (~3.13×) because the SIMD path zero-pads to a 4-element lane. This is documented W=1 behaviour; the dispatch-confirmation goal still holds via timing-difference + Fp<3> equality. Artefacts:
  - `crates/gf2-algebra/examples/s3_scalar_vs_avx2_sanity.rs` (new)
  - `dev/benchmarks/gf2_algebra_permanent/s3_cross_cpu-2026-05-12.csv`
  - `dev/plans/s3_cross_cpu_portability.md`
  - `dev/active/363556e6-amendments-2026-05-12.md`

- **W3 fully closed.** Sub-wave 3a (T12+T13+T14+T15) + sub-wave 3b (S1+S2+S3) all done.

- **W4 (F_5 / F_7 packed types) — sub-wave 4a dispatched.** Two Sonnet workers running in parallel via worktrees:
  - `6917eb85` (F_5 Packed5) — Candidate D bit-sliced 3-plane per R1 outcome. Worktree `.claude/worktrees/agent-6917eb85` on branch `worktree-agent-6917eb85`, anchored to main 867bf4ab.
  - `56c5dabc` (F_7 Packed7) — Candidate A 3-bit + 2^16 LUT per R2 outcome. Worktree `.claude/worktrees/agent-56c5dabc` on branch `worktree-agent-56c5dabc`, anchored to main 867bf4ab. LUTs as `static const` (not OnceLock) per Kani-friendliness.
  - Both workers were briefed with the R1/R2 decision docs, the dev/research/f{5,7}_packing prototype sources to transliterate from, and the PackedField + PackedFieldVec trait surfaces in `crates/gf2-algebra/src/packed/mod.rs`.

## Status

**W2** — DONE
**W3 sub-wave 3a** — DONE (T12, T13, T14, T15)
**W3 sub-wave 3b** — DONE (S1, S2, S3) this session
**W4 sub-wave 4a** — IN PROGRESS (6917eb85 + 56c5dabc workers running in worktrees)
**W4 sub-wave 4b** — pending: `1f769232` (SIMD F_5+F_7 kernels) — waits on 6917eb85 + 56c5dabc.
**W5 (GPU HIP)** — pending; `ad55b777` (F_3 HIP) is the natural home for S1g's 50× contender.
**W6 (Lean)** — pending; 4 issues, 2 with approved sketches.
**W7 (Reporting)** — pending; 5 issues for final epic artefacts.

## Open escalations / decisions for next session

None — all session-6 escalations resolved (S1 amendment, S3 amendment+scope, AVX-512 follow-up filed).

## What worked / what to repeat

- **`/schedule`-style workflow on a ~2.5 hr background timing.** S1's n=36 ref run was kicked off in session 5 and completed cleanly into session 7 with the writeup + criterion alignment landed across two short orchestration windows. The lead-direct review-fix-amend loop on S1 took only 1 review round to converge — keeping the threshold-line fix and writeup edit in a single commit avoided the round-tripping seen in earlier waves.

- **AskUserQuestion for criterion-1.5x gate-removal.** When a gate's mechanism doesn't fit the issue type (PPC label vs permanent-benchmark), surfacing it as a structured option set instead of unilaterally removing it kept the project rule "gates are non-negotiable / scope changes require escalation" intact while still moving the work forward quickly.

- **In-loop `[aspirational]` amendment on S3.** CLAUDE.md's `[aspirational]` semantic explicitly permits in-loop amendment when empirical data falsifies a target's phrasing but the aggregate contract still holds. S3's "ratio > 1" → "distinct paths" amendment is a textbook example: writing the amendment narrative into both the issue description (visible in `jit issue show`) and a separate `dev/active/<id>-amendments-*.md` doc (visible via `jit doc list`) kept the contract auditable.

- **Worktree dispatch with Sonnet for mechanical transliterations.** F_5 + F_7 packed types are direct transliterations from the existing dev/research/ prototypes plus the documented trait surface; Sonnet is the right model. Detailed prompts named the exact source files (`cand_d.rs`, `cand_a.rs`), the target file paths (`packed/packed5.rs`, `packed/packed7.rs`), the trait method list, and the test grid (exhaustive × proptest × word-boundary).

## Traps — do not repeat these

(Carrying forward all traps from handoffs 1–8. New traps from this session:)

- **Trap S1-G**: A gate whose automated checker is hardwired to a different subsystem (e.g. `criterion-1.5x` → `ppc-kernel:<id>` label + `dev/benchmarks/ppc-baselines.json`) is not the same as a gate that legitimately fails. Removing such a gate requires user approval per the project rules; the spirit can be trivially met but the mechanism is misconfigured. **Surface this kind of gate-vs-issue-type mismatch via AskUserQuestion early in the close sequence, not as a last-ditch escalation. The user's "remove" answer takes 30 seconds; manually re-classifying the gate without approval would have violated the "gates are non-negotiable" project rule.**

- **Trap S3-asp**: An aspirational criterion phrased as "X is faster than Y" can be empirically falsified at one regime (small n) while the aggregate contract (X dispatch is happening + correct) still holds. **When the gate-fail review cites "the data contradicts the aspirational criterion", check first whether the criterion's wording is the problem rather than the implementation. CLAUDE.md's `[aspirational]` semantic explicitly permits in-loop amendment, and amending the wording is often the correct response — but the amendment text must be visible in the issue description AND the writeup AND the relevant code comments and the inline `// > 1 confirms dispatch` comment in the example must also be updated.** S3 took an extra review round on the last point — one stale inline comment remained after the doc-block was fixed.

- **Trap W4 module-naming**: The W1 skeleton files at `crates/gf2-algebra/src/packed/bipedal5.rs` and `bipedal7.rs` predate the R1/R2 decisions and use the old naming convention. The R1/R2 outcomes were "F_5 = bit-sliced 3-plane (not bipedal-shaped)" and "F_7 = LUT-based (not bipedal-shaped)" — so the W4 module names `packed5` and `packed7` are intentional and the workers are instructed to delete the empty `bipedal{5,7}.rs` stubs. **When following older skeleton conventions to land newer-decision implementations, the worker prompt should explicitly authorise deletion of the obsolete skeleton — otherwise the worker may try to preserve both names and create confusion.**

## Active worktrees

- `.claude/worktrees/agent-6917eb85` — F_5 Packed5 worker (Sonnet, in progress)
- `.claude/worktrees/agent-56c5dabc` — F_7 Packed7 worker (Sonnet, in progress)

## Active background processes

None — both W4 workers running via the Agent harness, not bash background jobs.

## Session-7 metrics

- **Issues closed:** S1 (c98ed603) — 1 review round, 1 user-approved gate change (criterion-1.5x removed). S3 (363556e6) — 2 review rounds, 1 in-loop aspirational amendment.
- **Issues filed:** S1g (9480f8a6) GPU follow-up; f8d230ef AVX-512 bipedal-3 kernel.
- **Issues in progress:** 6917eb85 + 56c5dabc (W4 sub-wave 4a, workers running).
- **User escalations resolved:** 2 (S1 amendment shape + S1g state; criterion-1.5x gate removal).
- **User escalations open:** 0.
- **Tests passing on HEAD (867bf4ab):** 3614 (gf2-algebra + workspace, release tier; per worker's reported nextest count for S3 pre-commit).

## Next-session priorities

1. Wait for + review 6917eb85 (F_5 Packed5) and 56c5dabc (F_7 Packed7) worker output. Lead-review tiers 1, 1.5, 2, 2.5, 2.75, 3 per `lead-review-protocol.md`. Merge worker branches into main on PASS; rework on FAIL.
2. Post-merge, dispatch sub-wave 4b: `1f769232` (SIMD F_5+F_7 kernels in gf2-kernels-simd).
3. After W4 closes, W5 (GPU HIP/ROCm): `ad55b777` (F_3 HIP kernel) is the priority — unblocks S1g (9480f8a6).
4. W6 (Lean) sketches: `0606186a`, `30e98ef1`, `f05ffbe1` — confirm sketches before dispatch per CLAUDE.md "Verification work" section.
5. W7 (Reporting) — `7cd9afdb`, `16f03734`, `8808b051`, `424aa94f`, `c90db5a4`.
6. Final: epic completion report + transition ae82bd73 to done.
