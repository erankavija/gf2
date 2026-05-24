# Handoff — Continue gf2-core SOTA catch-up — close A8 + §6.3 follow-ups (026fc832) — session 3

**Date:** 2026-05-24
**Session number:** 3
**Prior handoffs:** `dev/active/026fc832-handoff.md` (session 1), `dev/active/026fc832-handoff-2.md` (session 2)

## Current state

- Epic: `026fc832` — state: `backlog` (still gated on remaining wave-3/4/5 deps; assignee `agent:project-lead`)
- Wave in progress: wave 3 of 5 (2/3 prototypes through review; 91429c1c R1 in background, fc182ed5 not yet dispatched)
- Children summary (direct epic deps + transitive): 8 done (615db3b9, 27bb2f75, 5ce13bae, aaa847cf, 52cce970, a70b1c70, bd9c6e13, 68cdf4c8), 1 in_progress with R1 worker mid-flight (91429c1c), 1 ready (fc182ed5), 3 backlog (873cbec1, e8a0c47a, b0fa00af — all chained via 41096af5 wave-4 fan-in), 1 backlog (41096af5)
- Active claims:
  - Epic itself claimed by `agent:project-lead`
  - **91429c1c** claimed by `agent:claude` (worker currently running R1 refactor; lead dispatched `aa105bd8a0368beb88` opus/background)
- Open escalations: None unresolved. Three resolved this session (bd9c6e13 SC#1 amendment; 91429c1c verdict acceptance via no-criterion-requires-pass; the 68cdf4c8 stale-doc post-amendment sweep done lead-direct)
- Progress file: `dev/active/026fc832-progress.json` (still session-2's schema; needs update — wave 3 = 1/3 done after 68cdf4c8 closes; bd9c6e13 done out-of-band)

## What just happened

Wave 3 dispatch + close cycles this session. Plus bd9c6e13 dense-RREF bug (out-of-band epic dep) closed.

- **bd9c6e13** (dense `FieldMatrix::rref` non-canonical RREF bug): 4 review rounds.
  - R0 worker (worktree `agent-bd9c6e13`, opus, `95f28a57`): correct algorithmic fix — `ple_in_place_window` now sources `L1`/`L1_bot` from `pivot_cols[start..start+r1]` instead of contiguous-prefix; 47/960 grid cells previously divergent → 0 post-fix. Review FAIL: named-reproducer 15×17 GF(7)/seed=1 doesn't actually trigger bug under `dense_random_fp_seeded` (R0 worker's own evidence § 10 admitted this); proptest was `#[test]` not `proptest!`; "7 shapes" docstring drift; SSOT — local `direct_rref_oracle_fp` + `dense_random_fp_seeded` duplicating `sparse_matrix.rs`.
  - R1 worker (sonnet, on main, `dd0346d8`): replaced structural test with 5-cell `test_rref_canonical_known_buggy_cells_jit_bd9c6e13` regression guard; real `proptest!` block, 128 rank-deficient cases via outer-product `A = F·G`; shared helpers in `crates/gf2-core/src/field/test_random_matrix.rs`; docstring fixed. Review FAIL: SC#1 empirically falsified (criterion names seed=1 but R0 proved it agrees by chance); design note + evidence doc § 1 had stale references.
  - **Escalated** to user 2026-05-24: option 1 (amend SC#1 to drop seed=1 specificity, require ≥5 cells from 47-cell sweep) chosen. Issue description updated with amendment block.
  - R2 lead-direct (`5b73a5e0`): swept evidence doc + design note; proptest comment "duplicating a row" → outer-product description; § 1 verbatim SC updated. Closed PASS.
- **68cdf4c8** (route A in-Rust f32/FMA cascade): 3 review rounds.
  - R0 worker (worktree `agent-68cdf4c8`, opus, `31fa0345` later rebased to `a77d9e96`): correct route-A kernel + 5 levers (from_mont_f32 + to_mont byte tables, 32-bit-lane Barrett SIMD on 12 i32 accumulators). Results: n=1024 PASS at 0.679 (+24.5% vs Candidate C); n=256 SHORTFALL at 0.547 (pack-cost-dominated). All 6 non-regression cells within ±1%. Review FAIL: SC#3 violated — `unsafe { std::env::set_var }` in `gf2-core/tests/route_a_gf251_parity.rs` (Rust 1.78+ made set_var unsafe); SSOT — local `fp251_matrix_from_seed` duplicates `bench_seed::fp_matrix_from_seed`; local `barrett_reduce_lane32_local` duplicates the existing helper in `fp_small.rs`.
  - R1 worker (sonnet, on main, `4bad2e72`): replaced env-var toggle with `pub fn set_route_a_gf251_enabled(bool)` + `AtomicBool`; tests use safe setter; shared helpers reused; `barrett_reduce_lane32_local` is now a one-line wrapper. Review FAIL: post-amendment sweep not done — bench driver still set env var with no effect (`bench_gemm_fp_251` didn't call the new setter); `fp_small_f32.rs` module + struct doc still claimed env-var gating; design note § 3 + evidence doc § 2 methodology + bench-driver header all described retired env-var path.
  - R2 lead-direct (`aabe32e7`): bench file now reads `GF2_GF251_ROUTE_A` (safe) and calls the safe setter; reset to `false` after; module + struct docs updated; design note § 3 rewritten with shipped AtomicBool toggle + § 3.1 historical-R0 draft; evidence doc § 2 + driver header rewritten. Closed PASS.
- **91429c1c** (route B BLAS-backed cascade): R0 + R1 in flight.
  - R0 worker (opus, on main, `58176dea`): out-of-tree harness at `dev/research/blas_sgemm_gf251/` (standalone non-workspace crate); OpenBLAS 0.3.33 BSD-3 single-threaded; bit-exact at n ∈ {64, 256, 1024}; **conclusive SHORTFALL at both cells** — n=256: 0.273-0.386, n=1024: 0.481-0.566. Dominated by route A at both sizes. Recommendation: research-only, do not promote to default-off accelerator. No criterion requires route B to clear 1.5× — only that evidence + recommendation be recorded; criteria all met by R0. Review FAIL: SSOT — `blas_gf251_gemm` + `blas_gf251_gemm_canonical_bytes` duplicate the chunked-sgemm cascade core; `matrix_to_canonical_bytes` duplicated locally; 4 public APIs missing `# Arguments` / `# Examples` / `# Complexity` per CLAUDE.md.
  - **R1 in flight** (sonnet, `aa105bd8a0368beb88` agent ID, dispatched after R0 review): extract shared `cascade_chunked_sgemm_internal` helper; dedupe `matrix_to_canonical_bytes`; add required doc sections. No perf re-bench needed.

## What to do next

In order of priority:

- [ ] **Wait for 91429c1c R1 worker completion** (agent ID `aa105bd8a0368beb88` in background). Verify the worker stayed in `dev/research/blas_sgemm_gf251/`, didn't touch unrelated files, didn't change perf characteristics. Re-run code-review gate.
- [ ] **If 91429c1c R1 PASSes:** close 91429c1c. If FAILs again: rework counter would be at 2 (MAX = 2 = no more reworks); next FAIL → escalate.
- [ ] **Dispatch fc182ed5** (Phase 1 route C — pure integer panelized GF(251) micro-kernel; opus; on main; should NOT need worktree since it'll be the only worker active). Touches `crates/gf2-kernels-simd/`. Expect ~60 min impl + bench.
- [ ] **Dispatch wave 4: 41096af5** (route selection / Phase 1 fan-in). Single task. Compares route A (PASS n=1024, SHORTFALL n=256), route B (SHORTFALL both, research-only recommended), route C (TBD). May escalate per its SC#7 if no route clears the threshold at both n=256 and n=1024 (currently only route A has any PASS).
- [ ] **Dispatch wave 5 in parallel**: e8a0c47a (Phase 2 GF(p) generalization), 873cbec1 (Phase 4 ext-field GEMM design), b0fa00af (Phase 5 terminal scorecard, supersedes 2cfc4372). All three depend on 41096af5.
- [ ] **Epic close** (Section 10): final gates + completion report. Map success criteria to closures. Archive progress file.

## Traps — do not repeat these

**This section is mandatory and carries forward session-1 + session-2 traps + new session-3 traps.**

**Carry forward** (link, don't copy): `dev/active/026fc832-handoff.md` § Traps (AVX-512 routing to 7f809931; 615db3b9 design-only; code-review surfaces one finding subset per round; cargo-ci stub-detection makes 5-7s runs not full signal; worktree isolation REQUIRED for parallel kernels-simd work) and `dev/active/026fc832-handoff-2.md` § Traps (worker AVX-512 framing trap; "same code path" proxy arguments for `[hard]` non-regression criteria; public-trait extension violates "API stays unchanged"; `#[test]` vs `proptest!` format; false `SAFETY:` claims; `lateout` vs `out` for inline asm; doc-comment drift across multiple cycles; `--lib` excluding doctests; scope-creep on ci-slow.yml). **All still in force.**

**New session-3 traps:**

- **Do NOT use `unsafe { std::env::set_var }` in `gf2-core` test code.** Rust 1.78+ made `set_var`/`remove_var` unsafe. Even in `tests/`, this counts as unsafe-in-gf2-core per SC#3 (unsafe-isolation). 68cdf4c8 R0 used this pattern to drive an env-var-based dispatch toggle; R1 had to replace with an `AtomicBool` + safe `pub fn set_route_a_gf251_enabled(bool)` setter. **Pattern**: any runtime debug switch in `gf2-core` should be backed by `std::sync::atomic::*` (or `OnceLock` / `Cell`) + a safe setter, NOT an env var read at dispatch time.

- **Worker post-amendment sweep needs to cover the bench file + the bench driver script + ALL doc comments + the design note + the evidence doc methodology.** 68cdf4c8 R1 changed env-var → AtomicBool but missed the bench file (which still needed to call the new setter or the toggle would be a no-op), the driver script header description, the kernel wrapper doc, the design note § 3, and the evidence doc § 2 methodology. R2 (lead-direct) needed 7 files in one commit. Pattern: after an API change, grep for ALL references to the OLD api name, fix every one in the same commit.

- **Worktree-branch stale-base requires rebase before merging.** 68cdf4c8 worktree was anchored at `7ff02338`; meanwhile main moved (bd9c6e13 R0 + lead state commits). Naive `git merge --ff-only worktree-agent-68cdf4c8` either fails or produces a misleading diff that shows main's changes as "deletions" from the worker's perspective. Pattern: `cd <worktree> && git rebase main && cd /repo-root && git merge --ff-only worktree-agent-<id>`. The pattern works because workers touch different files than the lead's parallel work in this epic. After rebase, do `./scripts/cargo-ci.sh` from the worktree to verify the post-rebase state still builds, THEN merge.

- **`git merge --ff-only` from inside a worktree is a no-op.** When `cd .claude/worktrees/agent-<id>` then running `git merge --ff-only worktree-agent-<id>`, git sees you're already on that branch and reports "Already up to date". The merge actually has to be initiated from main's worktree (the repo root). Pattern: always `cd /home/vkaskivuo/Projects/gf2` before any `git merge --ff-only <worker-branch>`.

- **Leak-check false positives on legitimate JIT state.** `scripts/check-leak-into-main.sh` flags `.jit/events.jsonl` and `.jit/issues/<id>.json` as differing from the pre-dispatch snapshot. These are LEGITIMATE lead-side modifications from gate runs + state transitions made during the worker's run. They are NOT worker leaks. Pattern: leak check is informational for `.jit/*` files; the real signal is whether `crates/` / `dev/` source files are unexpectedly modified.

- **Empirically-falsified `[hard]` criteria require escalation, not silent rework.** bd9c6e13 SC#1 named a specific reproducer (15×17 GF(7)/seed=1/density=0.05) that R0 worker proved empirically does not trigger the bug under the actual `dense_random_fp_seeded` generator (the 5ce13bae evidence used a different generator name that doesn't exist in the codebase). Per `feedback_measurements_not_guesses` + `feedback_no_autonomous_amendments`, escalate; do NOT keep dispatching workers to rework against the wrong contract. The user approved an amendment dropping the seed=1 specificity. Pattern: when a worker's R0 evidence proves a criterion is empirically false, the lead's first response should be escalation to the user, not R1 dispatch.

- **`barrett_reduce_lane32_local` style traps (kernel-helper duplication).** Workers tend to copy-paste a kernel helper rather than reuse the existing one because the existing one is `pub(super)`-or-private and not directly accessible. Pattern: when dispatching a kernel-touching worker, instruct them up-front: "if you need helper X that's `pub(super)` in another module, EITHER `pub(crate)`-elevate it (preferred) OR document why a local variant is needed in a one-line comment". 68cdf4c8 R0 had `barrett_reduce_lane32_local`; R1 deduped via thin wrapper.

- **Worker dev/research/ stubs need `.gitignore` for `target/` + `Cargo.lock`.** Per project memory `feedback_dev_research_target_gitignore`. The 91429c1c R0 worker correctly added the `.gitignore`. Pattern: include this in the dispatch prompt for any new dev/research/ stub.

- **`# Examples` doctest tests count toward review even in non-workspace stubs.** CLAUDE.md doc standard applies to all public items. Doctests in `dev/research/<stub>/` won't be reached by `cargo test --doc -p gf2-core`, but they must compile when someone runs `cargo test --doc` from within the stub. 91429c1c R0 was flagged for missing `# Arguments` / `# Examples` / `# Complexity` sections on 4 public APIs. R1 dispatched to add them.

## Open questions needing user input

None unresolved. The 91429c1c R1 worker is mid-flight; verdict pending. fc182ed5 + 41096af5 + wave-5 issues all queued.

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (needs update — wave 3 status not yet reflected)
- Session-1 handoff: `dev/active/026fc832-handoff.md`
- Session-2 handoff: `dev/active/026fc832-handoff-2.md`
- Wave-3 artefacts so far:
  - 68cdf4c8 evidence: `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` (route A: n=1024 PASS 0.679, n=256 SHORTFALL 0.547)
  - 68cdf4c8 design: `dev/active/68cdf4c8-route-a-design.md`
  - 91429c1c evidence: `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` (route B: SHORTFALL both cells, research-only)
  - 91429c1c harness: `dev/research/blas_sgemm_gf251/` (standalone non-workspace crate)
  - bd9c6e13 evidence: `dev/bench_results/2026-05-24-bd9c6e13-canonical-rref-fix.md` (47/960 → 0 cells)
  - bd9c6e13 design: `dev/active/bd9c6e13-canonical-rref-fix.md`
- Authoritative GF(251) baseline: `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` (drift check confirmed 58.98 / 70.89 Gop/s within ±5%)
- Predecessor scorecard (still to be superseded by Wave-5 b0fa00af): `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md`
- AVX-512 routing memory: `~/.claude/projects/-home-vkaskivuo-Projects-gf2/memory/feedback_avx512_scope_to_7f809931.md`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md` (NEW trap: worktree-branch stale-base requires rebase; `git merge --ff-only` is a no-op from inside the worktree)
- Lead review protocol: `.claude/skills/project-lead/references/lead-review-protocol.md`
- Reference host: AMD Ryzen 9 5900X (Zen 3), 12c/24t, AVX2+BMI2+VAES+VPCLMULQDQ, **no AVX-512**.
- BLAS provider on host (verified this session): OpenBLAS 0.3.33 at `/usr/lib/libopenblas.so.0`, BSD-3 license. Used by 91429c1c route-B harness.
- M4RI 20260122, LinBox 1.7.1, fflas-ffpack 2.5.0 at `/home/vkaskivuo/Projects/fflas-ffpack/`.

## Background worker still active when session ended

- `aa105bd8a0368beb88` — 91429c1c R1 (sonnet, on main, refactor SSOT cascade core + add 4 public-API doc sections). No perf re-bench expected. ETA: ~10-20 min after dispatch.

The next-session lead should check for the task-notification first thing, run gate `code-review` on 91429c1c at the R1 commit, then proceed.
