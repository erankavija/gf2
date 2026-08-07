# Handoff — Continue gf2-core SOTA catch-up — close A8 + §6.3 follow-ups (026fc832) — session 2

**Date:** 2026-05-24
**Session number:** 2
**Prior handoffs:** `dev/active/026fc832-handoff.md` (session 1, 2026-05-24)

## Current state

- Epic: `026fc832` — state: `backlog` (cannot transition to in_progress because 4 deps remain incomplete; assignee still `agent:project-lead` from session 1)
- Wave completed this session: wave 2 of 5 (per `dev/active/026fc832-progress.json`)
- Children summary (direct epic deps): 5 done (615db3b9, 27bb2f75, 5ce13bae, aaa847cf, 52cce970), 1 ready (bd9c6e13 — new this session), 3 backlog (873cbec1, e8a0c47a, b0fa00af — all chained via 41096af5)
- Active claims: epic itself claimed by `agent:project-lead`. No worker claims on children.
- Open escalations: None unresolved. Three resolved this session (5ce13bae 3 cells → [aspirational]; bd9c6e13 filed under epic per user approval; 52cce970 R1 worker AVX-512 framing → "fflas has no AVX-512 either, gap is AVX2-closable")
- Progress file: `dev/active/026fc832-progress.json` (still reflects session-1's wave plan; wave 2 statuses now all `done`, needs update to advance to wave 3)

## What just happened

Wave 2 dispatch and close. All 5 wave-2 issues closed PASS with hard numbers in CCX1-pinned 5-trial evidence docs.

- **a70b1c70** (Phase 0 controls): R0 PASS — GF(241) FAIL aspirational (float-modular structural gap, expected), GF(127) PASS, GF(251) drift +3.47%/+2.19% within ±5%. Sonnet. Commit `b1705490`.
- **27bb2f75** (small-n GEMM dispatch): 4 review rounds (R0+R1+R2+R3). R0 perf PASS but evidence gaps (GF(31)/n=4096 proxy, 3-trial n=4096). R1 worker fixed evidence; lead-direct R2/R3 fixed stale doc-line, `SAFETY:` comment claiming `get_unchecked` while code used safe indexing, GF(251)/n=256+1024 "not re-measured" proxy → direct 5-trial. Headline: GF(7)/n=64 +73% (34.40 Gop/s vs 24.40 target), GF(31)/n=64 +55% (31.15 vs 24.10), GF(251)/n=64 +87% (32.66 vs 20.10 aspirational). Closed `[aspirational]` markers on these cells via post-amendment sweep of 7a106fe4 + 615db3b9 plan. Final commit: `889cff98`.
- **5ce13bae** (Markowitz sparse RREF): 2 review rounds (R0+R1). R0 implementation correct (1.35x-1.76x speedup all 10 cells, all 31 tests PASS) but 3/10 cells failed 1.5× LinBox at n=256 dense regime. Escalated; user picked Option 1 (amend 3 cells to `[aspirational]`). R0 also discovered pre-existing FieldMatrix::rref non-canonical bug; user picked Option C (file under 026fc832 as type:bug). R1 doc rework (false equivalence claim, col_nnz drift, false `# Panics`). Final commit: `196a7aec`.
- **aaa847cf** (M4RM Gray-table invert): 1 review round (R0 SSOT refactor). R0 perf passed 3/3 (ratios 0.635×/1.043×/1.293× vs M4RI; speedups 7-17× over pre-rework Gauss-Jordan) but R0 had SSOT duplication (inline Gray-table reimpl instead of calling existing `m4rm::build_gray_table_flat`; `default_block_size_invert` duplicated `rref.rs` policy) and missing public-API doc sections. R1 worker refactored to shared helpers + added required doc sections; perf held. Final commit: `a1f887b8`.
- **52cce970** (GF(251) charpoly+minpoly residuals): 5 review rounds (R0+R1+R2+R3+R4). R0 closed minpoly PASS via 27bb2f75 inheritance but left charpoly at 2.001× the 1.5× ceiling. R0 evidence § 6 framed remaining 33.5% as AVX-512 territory; user corrected ("AVX-512 does not play a role here as the baseline does not have it either"). R1 dispatched with corrected framing → closed via 2 AVX2 codegen-quality fixes (Barrett μ hoist + inline-asm vpmulhuw bypassing LLVM 19's harmful widen-then-pack codegen for `_mm256_mulhi_epu16`). R1 review surfaced 2 hard violations (BasisReducer::push_col signature changed; boundary tests were #[test] not proptest!). R2 fixed via Option A trait extension. R3 review: barrett_mu_u16 doctest type-overflow. R4 review: SmallPrimeFns struct doc stale + design-doc still showing R0 4-arg signature. Each lead-direct fix. Final commit: `022fa251`. Headline: minpoly 1.263×, charpoly 1.418× PASS with 5.5% headroom; 16/16 non-regression cells PASS.

- **Newly filed: `bd9c6e13`** (type:bug) — Fix non-canonical RREF in FieldMatrix::rref dense PLE path. Filed under epic 026fc832 per user approval 2026-05-24 (Option C in the 5ce13bae escalation). Reproducer: 15×17 GF(7)/seed=1/density=0.05. Pivots `{0,1,2,3,5,6,7,13,15,16}` observed vs canonical `{0,1,2,3,5,6,7,10,15,16}`. Ready state. Wired as a 026fc832 dep.

## What to do next

In order of priority:

- [ ] **Update `dev/active/026fc832-progress.json`** to mark wave 2 issues `done` and advance `current_wave` to 3. Add `bd9c6e13` to the progress file (out-of-wave dep, can run in parallel with wave 3 since different code areas).
- [ ] **Dispatch wave 3 prototypes** (per session-1 plan): 68cdf4c8 (route A: in-Rust GF(251) f32/FMA cascade), 91429c1c (route B: optional BLAS-backed cascade), fc182ed5 (route C: pure integer panelized micro-kernel). All depend on `a70b1c70` (done). Session-1 plan says these need worktree isolation (touch `crates/gf2-kernels-simd`). **Recommended**: serialize anyway — these are perf prototypes that need 5-trial CCX1-pinned single-occupancy bench measurements, and parallel runs would pollute each other. Even with worktrees, criterion bench results would suffer.
- [ ] **Dispatch bd9c6e13** (independent, ready) in parallel with one of the wave-3 prototypes. The dense RREF bug is correctness-only — no benchmarks — so it can safely run alongside a bench-heavy prototype worker without pollution. Use worktree isolation (touches `crates/gf2-core/src/field/ple.rs` which is unrelated to kernels-simd).
- [ ] **After wave 3 closes, dispatch wave 4 (41096af5 route selection)**. Single task. May escalate per its SC#7 if no route clears the 1.5× threshold — that's a legitimate escalation, not a soft amendment.
- [ ] **After wave 4 closes, dispatch wave 5** parallel: e8a0c47a (Phase 2 GF(p) generalization), 873cbec1 (Phase 4 ext-field GEMM design — design-only), b0fa00af (Phase 5 terminal scorecard — superseding doc).
- [ ] **Epic close (Section 10)** after all deps done, including bd9c6e13.

## Traps — do not repeat these

**This section is mandatory and carries forward all session-1 traps + new ones from session-2.**

Carry forward unresolved session-1 traps (link, don't copy): `dev/active/026fc832-handoff.md` § Traps — AVX-512 routing to 7f809931; 615db3b9 design-only; code-review "one round all findings" trap; 5.7s cargo-ci as no-op signal; worktree isolation requirement for kernels-simd parallel work. **All still in force.**

**New session-2 traps:**

- **Do NOT trust worker-supplied "AVX-512 closes the gap" framing without verifying the baseline has AVX-512.** In 52cce970 R0, the worker correctly measured the perf gap but incorrectly framed the closure as needing AVX-512. The user corrected: "AVX-512 does not play a role here as the baseline does not have it either." The reference host (Zen 3) has no AVX-512; fflas-ffpack on the same host has no AVX-512 either. If the next lead sees a worker reach for `_mm512_*` to close a fflas comparison gap, push back and ask the worker to find an AVX2 lever first. Evidence: `dev/bench_results/2026-05-24-52cce970-charpoly-minpoly-closure.md` § 11 amendment + `/proc/cpuinfo` on this host.

- **Do NOT accept "same code path" proxy arguments for `[hard]` non-regression criteria.** Every (prime × n) combination in a non-regression criterion needs a direct 5-trial measurement. 27bb2f75 was burned 3 rework rounds on this (R0: GF(31)/n=4096 + 3-trial n=4096; R3: GF(251)/n=256+1024 "not re-measured"). The reviewer is strict-literal about benchmark criteria. Pre-emptively measure EVERY cell named in the criterion — even if the code path is identical.

- **Do NOT extend `pub` trait signatures in `gf2-core` as part of a `gf2-kernels-simd` perf optimization.** The criterion "safe `gf2-core` API surface stays unchanged" is read literally by the reviewer. 52cce970 R1 was burned on extending `BasisReducer::push_col(&mut self, col: &[F])` to take an extra `pivot_row: usize` argument. **Pattern that works**: add an additive default trait method (e.g., `push_col_with_pivot_row(...)` with a default body that delegates to `push_col`) and override on the optimized impl. Preserves API; lets hot paths use the optimized variant.

- **Do NOT add `#[test]` boundary tests when the criterion says "proptests".** 52cce970 R1 worker added deterministic unit tests for boundary lengths `{0, 1, 15, 16, 17, 63, 64, 65, 255, 256}`. The criterion explicitly says "proptests". The reviewer flags this literally. Use the `proptest!` macro with `prop_oneof![Just(n) for n in …]` over the boundary set + a random seed.

- **Do NOT write `SAFETY:` comments without `unsafe` blocks.** 27bb2f75 R0 had a `SAFETY:` comment claiming `get_unchecked` "shaves a branch per element" but the actual code used safe `from_mont[raw]` indexing. R2 lead-direct fix. The mismatch is review-failing technical debt. Either implement what `SAFETY:` documents OR remove the comment.

- **Do NOT use `lateout(ymm_reg)` for inline asm output where `out(ymm_reg)` would do.** `lateout` allows the register allocator to alias the output with an input, which can corrupt source operands for non-destructive instructions like `vpmulhuw`. 52cce970 R1 review flagged this; R2 fix changed to `out(ymm_reg)`. For 3-operand VEX-encoded AVX2 instructions, prefer `out` for clarity + safety.

- **Do NOT write doc-comment claims you haven't verified against the code.** The 5ce13bae R0+R1 cycle was burned twice on doc-claim drift (false dense-equivalence promise to a known-buggy reference; false `# Panics` documentation; `col_nnz` maintenance claim that didn't match the impl). The 52cce970 R0-R4 cycle was burned three more times on doc-claim drift (false `SAFETY:` claim; stale `SmallPrimeFns` struct doc; stale design-note signature). **Process rule**: after every code change, grep the touched module's `///` comments and the linked design notes for any narrative that the change invalidates. Fix in the same commit.

- **Do NOT trust criterion's auto-compare percentage in place of a 5-trial median for `[hard]` non-regression.** 27bb2f75 R0 evidence used criterion's `[-0.13% +0.07% +0.13%]` for Mersenne31/Fp<65537> non-regression. Reviewer didn't flag that one (the path-equivalence argument was accepted there) — but this could surface in a future review. Where the criterion says "5-trial median", do 5 trials.

- **Do NOT trust `--lib` test count alone before committing.** 52cce970 R2's `3815 passed` came from `cargo nextest run --workspace --release --profile ci` (which excludes doctests). The broken `barrett_mu_u16` doctest only surfaced in R3 review. Workers should run `cargo test --doc` separately for any new `///` example.

- **Worker scope-creep on `.github/workflows/ci-slow.yml` happened twice.** Once in 27bb2f75 R1 (added DVB-T2 test exclusions while running cargo nextest during the perf work). The R1 worker's session was unrelated to CI/DVB-T2. Lead reverted via `git restore`. **Pre-dispatch instruction worth carrying forward**: "If you find an unrelated test that times out, log it as an open question — do not fix or skip it. Do NOT touch unrelated files."

## Open questions needing user input

None. All escalations this session were resolved.

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (still session-1 schema; needs wave-2-closed update)
- Session-1 handoff: `dev/active/026fc832-handoff.md` (all session-1 traps still in force)
- Wave-2 evidence docs:
  - `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md`
  - `dev/bench_results/2026-05-24-27bb2f75-small-n-dispatch.md`
  - `dev/bench_results/2026-05-24-5ce13bae-markowitz-sparse-rref.md`
  - `dev/bench_results/2026-05-24-aaa847cf-m4rm-invert.md`
  - `dev/bench_results/2026-05-24-52cce970-charpoly-minpoly-closure.md`
- Wave-2 design notes:
  - `dev/active/5ce13bae-markowitz-design.md`
  - `dev/active/aaa847cf-m4rm-invert-design.md`
  - `dev/active/52cce970-bespoke-avx2-design.md`
- Predecessor scorecard (still authoritative until b0fa00af supersedes): `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md`
- Newly-filed bug: `jit issue show bd9c6e13`
- AVX-512 routing memory: `~/.claude/projects/-home-vkaskivuo-Projects-gf2/memory/feedback_avx512_scope_to_7f809931.md`
- Companion AVX-512 epic (where AVX-512 work belongs): `jit issue show 7f809931`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Lead review protocol (6 tiers): `.claude/skills/project-lead/references/lead-review-protocol.md`
- Reference host: AMD Ryzen 9 5900X (Zen 3), 12c/24t — verified via `/proc/cpuinfo`. AVX2+BMI2+VAES+VPCLMULQDQ, no AVX-512.
- M4RI 20260122, LinBox 1.7.1, fflas-ffpack 2.5.0 — all verified via `pkg-config --modversion` this session.
