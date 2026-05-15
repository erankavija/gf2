# ae82bd73 (Permanents F_3/F_5/F_7) — session 10 handoff

**Date:** 2026-05-15
**Branch:** main (HEAD `762ce0ac`)
**Session focus:** close W5 host dispatcher (2fbbdfa5); land V1 Lean (f05ffbe1); start V2 Lean (0606186a); start GPU crossover sim (a9e461de).

## What landed since handoff-12

### Closed issues this session

1. **2fbbdfa5 (host-side GPU dispatcher)** — `gf2_algebra::gpu::permanent_batch_bipedal{3,5,7}` behind `--features hip`. Three criterion tests at n=24, M=1000 PASS on gfx1030 (F_3 217s, F_5 1462s, F_7 1695s — last two under GPU contention with each other). Verification doc at `dev/active/ae82bd73-w5-gpu-dispatcher-verification.md`. Closed after 3 review rounds:
   - Round 1: feature-gating leak (F_5/F_7 dispatch imports + GF7_ONCE unconditional → fails under `--no-default-features --features hip`). Fixed in `dcc44a70`.
   - Round 2: `ignore` doctests don't satisfy "examples must compile and pass" CLAUDE.md standard. Fixed in `635d6dfe` (converted to `no_run` with `# #[cfg(...)] {}` guards; 6 new doctests now compile under `cargo test --doc --all-features`). Also added gf2-algebra to root README workspace-layout table.
   - Round 3: missed `gf2-algebra | hip` row in root README feature matrix + stale narrative in `crates/gf2-core/README.md:162` ("used only from gf2-coding"). One-pass sweep in `4ca444ab`.

2. **f05ffbe1 (V1 Lean — bipedal F_3 correctness)** — 20-lemma proof per D2 sketch §10. User-approved div→neg substitution (`PackedField<Fp<3>>` trait exposes `{add, sub, mul, neg}` — no `div` in production; reproduced sketch contract with neg in div's place). Production target via inherent wrappers `Bipedal3::{add,sub,mul,neg}_inherent` (Option A from dispatch prompt). Closed after 1 review round:
   - Round 1: `bipedal3_add_word` / `bipedal3_sub_word` lemmas were dead-code placeholders that didn't bind to the actual production formula expressions. Worker bypassed them in `*_correct` with inline `simp only [BitVec.getLsbD_*]`. Fix in `521da541`: word lemmas now stated against the *exact* composed bitwise expressions used in production; `*_correct` proofs now `obtain` + `rw` through the word lemmas (load-bearing).

### In flight at handoff

3. **a9e461de (GPU vs CPU SIMD crossover sim)** — Sonnet worker running ~80 min as of handoff. Multiple sim re-runs in progress (n=28 sweep + custom n=32/36 sub-run). Worker will return a structured handoff when complete. **CSV path expected:** `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv` (plus a `-n32n36` companion run). Writeup expected at `dev/plans/s5_gpu_crossover.md`.

4. **0606186a (V2 Lean Ryser bounded n ≤ 63)** — Opus session 1, ~22 min wallclock. Landed:
   - `crates/gf2-algebra/src/permanent/ryser_fp3.rs` (commit `ce897278`) — `pub fn permanent_ryser_fp3(matrix: &[Fp<3>], n: usize) -> Fp<3>`, iterator-free `while` loops (Aeneas iterator-adapter library gap workaround), 5 unit tests crossing-check vs generic `permanent_ryser::<Fp<3>>`.
   - `scripts/verify-lean.sh` extended with three `--start-from` entries for V2 extraction.
   - `proofs/Gf2Algebra/Proofs/RyserBounded.lean` (commit `762ce0ac`) — 198 lines. Defines `decodeFp3`, `gray`, `ryserRHS`. Proves L1 corner cases (`gray_zero/one/two/three`, `gray_def`) + L7-at-n=0 (`ryserRHS_eq_permanent_n_zero`). **Headline theorem `permanent_ryser_fp3_correct` is NOT stated. L2–L6, L8, L9 NOT proved.** Worker estimates 1500–2500 more lines of Lean for L9, multi-session. No sorrys in the new file; lake build succeeds (8.5s replay).

## Open escalations / decisions for next session

User decision 2026-05-15 (session 10 close): "Let's do a handoff at this point. We should see what the current state of Charon/Aeneas is. We might have to do some additional work to get on par with upstream."

The two interconnected blockers:

### Charon gf2-core extraction panic (NEW, blocks 0606186a criterion 5)

`scripts/verify-lean.sh` gf2-core leg crashes at `src/pretty/fmt_with_ctx.rs:416` (the DynPredicate formatting assertion). Root cause: recent gf2-core SSOT perf commits (`8d71d62c`, `fc3e2cf2`, `e927ea83`) added `Option<Box<dyn Trait>>` return types in `crates/gf2-core/src/gfp/mod.rs:734,746,798`. Charon's pretty-printer doesn't handle these in return position.

The cached `proofs/Gf2Core/Funs.lean` (from an older extraction) is what current `lake build` uses; V1 and V2 partial proofs both build against it cleanly. The end-to-end "verify-lean.sh regenerates" criterion on 0606186a (and the implicit dep on f05ffbe1) is **not satisfied on a fresh-extraction run**.

**Resolution paths (user to decide alongside Charon-upstream state):**
- (a) Gate the three `dyn Trait` return helpers under `#[cfg(not(verify_lean))]` (the project's existing extraction-only cfg). 20-30 min task; matches the project's pattern of using `verify_lean` cfg for extraction-time variants.
- (b) Update local Charon at `/data/aeneas-build/charon/` to a newer upstream that handles this pattern. Requires nightly toolchain re-build; potential breakage of the existing three project-local patches.
- (c) Add additional `--opaque` markers to verify-lean.sh to skip the offending modules; risk that the dyn-returning helpers transitively reach into proof-targeted modules.

### V2 Ryser proof — scope vs session budget (0606186a)

Per the user's session-10-close direction, this is paused pending Charon/Aeneas state review. When V2 resumes, the most pragmatic path is to dispatch focused Opus sessions in this order:

1. **L1–L3 (Gray-code lemmas)** — 1 session. Gray bijection at `n ≤ 63` is the deepest part of the Gray sub-namespace (sketch §6.2 estimate: 30–50 lines for the bijection alone). The `bv_decide`/`bv_omega` tactics carry most of L1, L2.
2. **L4–L5 (column-sum loop invariant + fold-mul invariant)** — 1 session. These cite V1's `bipedal3_{add,sub,mul}_correct` lane-wise. Induction on the Gray walk step.
3. **L6–L7 (outer accumulator + pure-Mathlib Ryser identity)** — 1 session. L7 is the heaviest (sketch §3.3 estimate: 30–80 lines, Mathlib `Finset.sum_pow` + `sum_powerset_neg_one_pow_card`).
4. **L8–L9 (Aeneas `progress` chain + headline)** — 1 session. Walks the four extracted inner loops of `permanent_ryser_fp3` (the iterator-free Rust wrapper makes this tractable per worker's session-1 report).

Total estimated effort: 4 Opus sessions × ~30 min each, ~2 hours of dispatch wallclock. The session-1 worker's "1500–2500 lines" estimate may be conservative; the sketch §3.5 acknowledges `bv_decide` and pure-Mathlib lemmas carry most of the load.

Alternative if user prefers to ship the epic sooner: amend epic success criterion #8 to `[aspirational]` and the 0606186a hard criteria to track only definitions + L1–L3 + L7-corner (current session-1 state), close 0606186a partial. The V1 proof (f05ffbe1, closed) remains as the [hard] formal-verification deliverable for the epic.

### W5 remaining (a9e461de) — not yet closed

Worker still in flight. CSV expected in `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-*.csv`; writeup at `dev/plans/s5_gpu_crossover.md`. Next session: review the sim output, attach the writeup via `jit doc add`, run code-review gate, close 0606186a's sibling.

### W6 remaining (30e98ef1) — F_5/F_7 aspirational

Not started this session. Per CLAUDE.md verification-work convention, requires a new D4 proof sketch (no sketch exists yet). The criterion is aspirational so the "amend with written justification + close without sorrys" path remains available.

### W7 (8 issues) — not started

All 8 W7 issues remain pending: 7cd9afdb (publication artefact), 16f03734 (README + doctests), 8808b051 (CLAUDE.md/ROADMAP), 424aa94f (plot scripts), c90db5a4 (repro script), 333028c1 (Sage cross-val — Sage is installed at `/usr/bin/sage`), 8e4e19a0 (perm-vs-det sim), 9480f8a6 (S1g GPU 50× — needs a9e461de close to settle the GPU contender story).

## What worked / what to repeat

- **Holistic pre-gate audit** for f05ffbe1 word-lemma fix: the reviewer's finding (dead `*_word` lemmas) was substantive, not nit-picking. Rerunning `*_correct` proofs as `obtain + rw` through the word lemmas restored sketch fidelity in one rework round.

- **`no_run` + `# #[cfg(feature = "hip")] {}` doctest pattern** for cfg-gated GPU APIs (gpu.rs): satisfies CLAUDE.md "examples must compile and pass" without requiring ROCm at `cargo test --doc` time. This is now the project-canonical pattern for HIP-feature-gated public APIs.

- **GPU evidence capture in versioned verification doc** continues to be the right gate-clearance pattern for [hard] device-required criteria. The 2fbbdfa5 evidence doc reuses the format established for ad55b777 / b43cdf33 / 5c0505b2 in session 9.

- **Stop-and-report on out-of-budget verification work** (V2 worker session 1): the worker chose to land partial work without `sorry` rather than ship `sorry`-laden lemmas. This is the correct behaviour per CLAUDE.md verification work; preferable to a multi-round review cycle on incomplete proofs.

## Traps — do not repeat these

Carrying forward all traps from handoffs 1–12. New traps from session 10:

- **Trap session-10-1 (iterative reviewer surfaces docs findings one-at-a-time, even on infrastructure issues)**: 2fbbdfa5 review went three rounds because docs findings (feature-gating leak → doctests `ignore` → README feature matrix table) were surfaced one per round. **Fix:** before running code-review the first time on any issue that adds a new public API (especially feature-gated), do a holistic audit pass: (a) feature-matrix combinations (`cargo build --no-default-features --features <new-feature>`); (b) all 6 CLAUDE.md doc-comment standards on every new `pub fn`/`pub mod` (especially `# Examples` that compile-pass under `cargo test --doc`); (c) `grep -rn "<new-API-name>" *.md crates/*/README.md` to find stale docs that need updates. This is the same trap session-8-late-1 named but it recurred this session for a different reason.

- **Trap session-10-2 (Aeneas iterator-adapter library gap on V2 extraction)**: The first attempt at `permanent_ryser_fp3` used `Iterator::map` / `take` from the original generic `permanent_ryser` body; Aeneas's Lean library is missing clean models for `core::iter::adapters::{map,take}::*`, producing match-pattern metavariable errors during extraction. **Fix:** when writing extraction targets for Charon/Aeneas, **avoid iterator-adapter chains** — rewrite as explicit `while k < upper { ... }` loops using only `core::iter::range::Range`-shaped iteration. The iterator-free Rust wrapper in `ryser_fp3.rs` is the project pattern for this; reference it from any future extraction-target task.

- **Trap session-10-3 (recent gf2-core perf commits introduced Charon extraction panic via `Option<Box<dyn Trait>>` returns)**: `8d71d62c`, `fc3e2cf2`, `e927ea83` are correctness-fine but extraction-fragile. **Fix:** any addition of `dyn Trait` or `Box<dyn Trait>` to a Charon-extracted module must be either (a) gated under `#[cfg(not(verify_lean))]` from the start, or (b) verified by running `./scripts/verify-lean.sh` before commit. The existing `verify_lean` cfg is the right tool.

- **Trap session-10-4 (`.claude/scheduled_tasks.lock` re-tracked across two commits)**: even after `git rm --cached`, a subsequent `git add -A` re-tracked the lock file because `.gitignore` didn't have `.claude/` yet (it only had `.claude/worktrees/`). Two commits to clean. **Fix:** before `git add -A` ever, verify `.gitignore` covers the lock files this session creates; the project pattern is wide directory-level ignores for tool-managed paths.

## Active worktrees

None.

## Active background processes

1. **a9e461de worker (Sonnet GPU crossover sim)** — `pgrep -af permanent_gpu_crossover` shows two `cargo run` processes plus polling loops. The worker is running multiple sim sweeps. Expected wallclock: <30 min remaining as of handoff. The worker will return a structured handoff to its agent context when complete; the next session must check `agent-a5d9be8862af1f9e1.jsonl` for the structured result and close the issue per the standard W5 close pattern.

## Session 10 metrics

- **Issues closed:** 2 (2fbbdfa5, f05ffbe1).
- **Issues in review loop:** 0.
- **Issues partial:** 1 (0606186a, ~10% complete by lemma count).
- **Issues in flight at handoff:** 1 (a9e461de).
- **User escalations resolved:** 2 (f05ffbe1 div→neg substitution; session-close handoff direction).
- **User escalations open:** 1 (Charon/Aeneas state review per user's "see what the current state... is. We might have to do some additional work to get on par with upstream").
- **Commits on main this session:** 14 (excluding handoff commits).
- **Tests passing on HEAD `762ce0ac`:** 3783 + 471 unit + 173 doctests + 5 new V2-wrapper tests fast-tier; 9 gfx1030-only tests verified across kernels + dispatcher.

## Next-session priorities

1. **Review Charon/Aeneas upstream state.** The user explicitly flagged this. Check `/data/aeneas-build/charon/` git log vs upstream (`https://github.com/AeneasVerif/charon`); check `/data/aeneas-build/aeneas/` similarly. The local Charon has three project-applied patches (per MEMORY.md "Charon Fixes Applied"). Upstream may have absorbed or moved past them. Decide: rebase local patches onto current upstream, OR apply a minimal fix for the `dyn Trait` return regression and defer upstream rebase.

2. **Fix Charon gf2-core regression.** Per option (a): gate the three `Option<Box<dyn Trait>>` returns in `gfp/mod.rs` under `#[cfg(not(verify_lean))]`. Verify `./scripts/verify-lean.sh` runs end-to-end.

3. **Decide V2 Ryser proof path** (4 more sessions vs amendment vs drop) — per the user's three-option AskUserQuestion still pending.

4. **Pick up a9e461de close** when sim worker returns: review CSV + writeup, attach via `jit doc add`, code-review gate, close.

5. **W7 reporting wave**: 7 dispatchable issues, mostly parallelisable. 333028c1 needs Sage (installed at `/usr/bin/sage`); 16f03734 + 8808b051 are doc-only; 424aa94f is plot scripts; 8e4e19a0 is a perm-vs-det MC sim (CPU-bound, no GPU contention with a9e461de or 9480f8a6).

6. **W6 30e98ef1 sketch** if V2 path includes it.

7. **Final**: epic ae82bd73 completion report + transition to done.

## Files of note

- `dev/active/ae82bd73-w5-gpu-dispatcher-verification.md` — 2fbbdfa5 gfx1030 evidence.
- `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean` — V1 (f05ffbe1) closed proof, 20 lemmas, load-bearing word lemmas after rework.
- `proofs/Gf2Algebra/Proofs/RyserBounded.lean` — V2 (0606186a) session-1 infrastructure; L9 not yet stated.
- `crates/gf2-algebra/src/gpu.rs` — host dispatcher; doctests now `no_run` with cfg-guarded bodies.
- `crates/gf2-algebra/src/permanent/ryser_fp3.rs` — iterator-free F_3 monomorphisation wrapper for V2 extraction.
- `scripts/verify-lean.sh` — gf2-algebra extraction leg works; gf2-core leg blocked on Charon `dyn Trait` regression.
- `scripts/fix-aeneas-gf2algebra.py` — V1's post-processing for gf2-algebra extraction.
