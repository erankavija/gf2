# Handoff — Fast matrix permanents over F_3 / F_5 / F_7 via packed bipedal arithmetic (ae82bd73) — session 2

**Date:** 2026-05-09 (session 2; supersedes session 1's handoff-1 and handoff-2 in terms of "current state", but does not replace them — read all three when resuming)
**Session number:** 2
**Prior handoffs:** ae82bd73-handoff-1.md (session 1 early), ae82bd73-handoff-2.md (session 1 close — W0 done, W1 ready)

## Current state

- Epic: `ae82bd73` — state: `backlog` (still has 37 unresolved deps)
- Wave in progress: **W1 sub-wave 1c (T3 ready) and 1d (T4, T5 sequential)**.
- Children summary: 15 done (was 11 at session-1 close), 0 in_progress, ~37 still backlog.
  - W1 done: T1 (e6b1216a), T2 (1aa0cb99), T6 (94870b84), T16 (b17bec62).
  - W1 still pending: T3 (46330802 — Bipedal3 element), T4 (053e4016 — Bipedal3Vec), T5 (ef7d0633 — Bipedal3Matrix).
- Active claims: none — all closed.
- Open escalations: none. One was resolved this session (T2's "65 elements" criterion → user chose Option C: implement PackedFieldVec).
- Progress file: `dev/active/ae82bd73-progress.json` (updated this session — wave 1 sub-state).
- Worktrees: cleaned up (none active; all branches deleted).

## What just happened (session 2)

### Issues closed (in close-order)

1. **T1 (e6b1216a — gf2-algebra crate skeleton)** — 0 reworks. Worker: parallel with T16 in worktree. Gates: cargo-ci PASS (10.8 s), code-review PASS (3 min, gpt-5.4 review). Closed at gate run 17af1889. Notable: 64-cell feature-matrix sweep ran natively (host has hipcc); two minor lead-noted deviations (placeholder smoke test + `#![warn(missing_docs)]`) accepted by reviewer without comment.

2. **T6 (94870b84 — gray_code_iter)** — 1 lead-direct fix (no rework cycle). Worker: parallel with T2 in worktree. Lead-direct fix corrected docs from `n <= 64` → `n <= 63` (the `1u64 << 64` shift is UB; reviewer flagged). Gates: cargo-ci PASS, code-review PASS after fix.

3. **T2 (1aa0cb99 — PackedField + ScalarPackedFp3)** — 1 rework cycle + 1 lead-direct doc fix + 1 lead-direct issue-description amendment. Reworks/fixes:
   - **Round 1 fail:** "65 elements" criterion infeasible for fixed-LANES=64 type. Lead escalated to user via AskUserQuestion. **User chose Option C: expand T2 scope to also implement PackedFieldVec.** Worker did rework cycle 1.
   - **Round 2 fail:** the user-approved amendment was visible only in source comments, not in the JIT issue description. Lead-direct edit added `## Amendment 2026-05-09 (user-approved, Option C)` block to the issue description.
   - **Round 3 fail:** stale narrative in `crates/gf2-algebra/src/lib.rs` Status section ("W1-T1 skeleton with no functional surface" — false after T2/T6 landed). Lead-direct doc fix.
   - **Round 4 PASS.**

4. **T16 (b17bec62 — BatchedBipedalLike framework)** — 2 rework cycles. Reworks:
   - **Round 1 fail:** (a) proptests used local oracle instead of `dev/research/f3_bipedal::Bipedal3`; (b) `x86/bipedal_avx2.rs` hard-coded `Config3` so "no extra kernel code per new prime" criterion not met. Worker dispatched rework cycle 1. **Lead pre-flight error:** my round-1 dispatch prompt told the worker the scalar Bipedal3 didn't exist yet — wrong, `dev/research/f3_bipedal/src/bipedal.rs::Bipedal3` exists. Add to traps below.
   - **Round 2 (rw1) fail:** prior 2 findings closed, but new finding: `BipedalLikeConfig` doesn't expose lane shape as associated types (the criterion 1 phrasing "associated types/`const`s for the per-prime arithmetic plug-in points" was unmet). Worker dispatched rework cycle 2 (structural refactor: lane shape moves from `BatchedBipedalLike<C, Mag, Sgn>` generics to `BipedalLikeConfig::{MagLane, SgnLane}` associated types).
   - **Round 3 PASS.**

### Lead-direct minor edits (no worker dispatch)

Three lead-direct edits this session, all narrowly scoped Tier 2.5 / 2.75 fixes:
- **T6:** `n <= 64` → `n <= 63` in 3 doc locations + comment in `crates/gf2-algebra/src/gray.rs`.
- **T2:** `## Amendment 2026-05-09 (user-approved, Option C)` block appended to JIT issue description.
- **T2:** `crates/gf2-algebra/src/lib.rs` `# Status` section refreshed from "W1-T1 skeleton" to "W1 sub-wave 1b in progress; T2/T6 landed".

Lead-direct edits are appropriate for narrowly mechanical fixes (doc bound, narrative refresh, criterion amendment recording) where dispatching a worker would burn 10+ min for a 30-second edit. They are NOT appropriate for any code change that the worker should iterate on — those still go through worker dispatch.

### Process improvements applied

- **Worktree dispatch protocol** used for all 5 dispatches this session (T1, T16, T2, T6, T16-rw1, T16-rw2, T2-rw1). Pre-flight + leak-check both ran each time; one false-positive leak detection (snapshot-vs-current diff caught the lead's own gate_pass edits, not worker leaks) — flagged but not blocking.
- **Worktree+excluded-hip cargo metadata error** is environmental (per T1 worker's note, confirmed). `cargo fmt --all` from a worktree fails because cargo walks the FS into the parent's `.claude/worktrees/.../crates/gf2-kernels-hip/` and double-counts the workspace. Fallback: `cargo fmt -p <crate> -- --check`. Doesn't affect the gates that run from main.
- **Pre-existing `gf2-core` 6 dead-code warnings** (in `gfp/simd_ops.rs`: `PackedFpMatrix`, `PackedFpBasis`, `PackedFpChainPolys`, `fp_reduce_packed`) only surface when running `cargo clippy -p gf2-kernels-simd --all-targets -- -D warnings` (without `--no-deps`). They do NOT surface in `cargo clippy --workspace --all-targets --all-features -- -D warnings` (cargo-ci's command). Workers must use `--no-deps` when scoped to gf2-kernels-simd. The warnings are unrelated to the bipedal work.

## What to do next

**Immediate (next session start):**

- [ ] **Dispatch T3 (46330802 — Bipedal3 element)** as W1 sub-wave 1c. Single worker — file-isolated in `crates/gf2-algebra/src/packed/bipedal3.rs`. T3 implements `PackedField<Fp<3>>` for Bipedal3 (paper Theorem 2.1 add/sub/mul/neg formulas) and proptest cross-checks against `ScalarPackedFp3` (T2's deliverable) on 1000 random pairs. Use a worktree (single agent, but worktree convention is universal). Note: T3's tests can NOW reference `gf2_algebra::packed::ScalarPackedFp3` directly (T2 landed it) — no need for an inline oracle this time.

- [ ] **After T3 closes, dispatch T4 (053e4016 — Bipedal3Vec)**. Sequential — T4 depends on T3 (`Bipedal3Vec` is a `Vec<u64>`-based packed type whose Element is `Bipedal3`).

- [ ] **After T4 closes, dispatch T5 (ef7d0633 — Bipedal3Matrix column-major)**. Sequential — T5 depends on T4. **CRITICAL: column-major** per the R3/D1a SSOT fix in session 1 (epic doc §7.2 was changed from row-major to column-major; T5's worker must follow this). The trap is documented in handoff-2 and remains in force.

- [ ] **W1 fully closed** when T3+T4+T5 done. W2 (T7+T9 in parallel) unblocks at that point — generic `permanent_ryser<F>` and `permanent_bipedal3` single-word fast path.

**Process improvements (lessons embedded into next-session dispatch):**

- [ ] **Verify factual claims in dispatch prompts.** Before stating "X does not exist" or "Y is not yet landed", grep the repo to confirm. The T16 round-1 dispatch told the worker `dev/research/f3_bipedal::Bipedal3` didn't exist — wrong, cost a rework cycle. Memory `feedback_dispatch_prompt_facts` already exists; this session reinforces it.

- [ ] **Pre-amend `[hard]` criteria with visible JIT-description blocks** when the criterion text is incoherent against design constraints. T2's "65 elements" criterion was infeasible against fixed-LANES=64; should have been amended IN the description before T2 dispatch. The `## Amendment YYYY-MM-DD (user-approved, Option N)` block format used this session is the precedent.

- [ ] **For workers that depend on a forthcoming type (e.g., T3 cross-checks against ScalarPackedFp3 from T2)**: the dispatch prompt should explicitly cite the dep's path and suggest `cargo doc -p <dep_crate>` if the worker needs to discover the surface. T3's prompt should reference `gf2_algebra::packed::ScalarPackedFp3` and `gf2_algebra::packed::ScalarPackedFp3Vec` (T2 landed both per the user-approved scope expansion).

- [ ] **Check `crates/gf2-algebra/src/lib.rs` `# Status` section** before each W1 close — it goes stale fast. Add a step in the rework-prompt template asking the worker to refresh it as part of any new W1 surface landing.

- [ ] **Use `--no-deps` for crate-scoped clippy** in worker dispatch prompts (esp. `cargo clippy -p gf2-kernels-simd --all-targets --no-deps -- -D warnings`). The pre-existing gf2-core dead-code warnings will fail `-D warnings` if `--no-deps` is omitted.

## Traps — do not repeat these

Carrying forward from handoff-1 and handoff-2 (all still in force unless explicitly resolved). Adding session-2's new traps:

### Carried forward from handoff-1/2 (re-stated for emphasis)

- **DO NOT use a JIT "comment" primitive — there isn't one.** Sketch / trait-surface / decision-doc approval criteria are satisfied via a `## Approval` or `## Amendment` section in the issue description.

- **DO NOT cite `gf2-kernels-simd → gf2-core` as a forward edge.** The actual workspace dependency is `gf2-core → gf2-kernels-simd`.

- **DO NOT use `(k >> flip) & 1` for Gray-code add/sub.** Use `g_k = k ^ (k >> 1); ((g_k >> flip) & 1) == 1`. Hand-verified table in `dev/plans/r3_multi_word_streaming.md`.

- **DO NOT cite epic doc subsections "§12.1" / "§12.2" / "§12.3"** — section 12 has named subsections V1, V2, V3.

- **DO NOT submit a deliverable until it passes a self-audit against ALL of CLAUDE.md's documentation standards** (description + Arguments + Examples + Panics + Complexity).

- **DO NOT dispatch parallel agents on issues that share working-tree files.**

- **DO NOT commit `dev/research/<crate>/target/` build artefacts.**

- **DO NOT forget to claim + transition to in_progress BEFORE Agent dispatch.**

- **DO NOT assume `dev/research/` stubs are exempt from CLAUDE.md unsafe-isolation rule WITHOUT the explicit carve-out** (already amended in CLAUDE.md as of 2026-05-09).

### NEW from session 2

- **DO NOT tell a worker "the scalar Bipedal3 reference doesn't exist yet" — it does, at `dev/research/f3_bipedal/src/bipedal.rs::Bipedal3`.** As of session 2 close, that crate has a `lib.rs` exposing `Bipedal3` and `F3Encoding`, and `gf2-kernels-simd` has it as a path dev-dependency. T3's dispatch prompt should also reference it as the SSOT scalar reference. Memory `feedback_dispatch_prompt_facts` is the relevant existing memory; reinforced this session.

- **DO NOT use `1u64 << n` to compute the iteration upper bound when the docs claim `n <= 64`.** `1u64 << 64` is UB per the Rust reference (shift by full type width). The correct documented bound is `n <= 63`. T6's docs were corrected this session; future Gray-walk consumers (T7, T9) inheriting the bound should also state `n <= 63`.

- **DO NOT design a `[hard]` criterion that requires "N elements" when N exceeds the type's fixed-LANES const.** T2's "65 elements" criterion was infeasible for fixed-LANES=64; user resolved via Option C (scope expansion to PackedFieldVec). Future criteria writers (lead, breakdown agents) should sanity-check that "N elements" claims fit the trait's surface BEFORE the criterion lands. If it doesn't fit, either use a vector type or amend pre-dispatch. Memory `feedback_pre_dispatch_criterion_audit` covers this; session 2's T2 incident is a fresh datapoint.

- **DO NOT hard-code a per-prime config (e.g., `Config3`) inside what the criterion calls a "generic" entry point.** T16 rw1 had `BatchedBipedalLike::<Config3, Avx2Lane, Avx2Lane>` inside `run_*_batch` — reviewer correctly flagged this as not-actually-generic. The fix in rw2 was to make `run_*_batch::<C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>>` (per-ISA, generic over the prime config). Future SIMD-kernel issues (T12 F_3 kernel, T21 F_5/F_7 kernels) will instantiate via the generic entry point, not write new wrappers.

- **DO NOT forget to record `[hard]` criterion amendments in the JIT issue description.** Source-code comments documenting "user-approved Option C" are not a substitute. The reviewer LLM reads the issue description's literal text; a comment in `scalar.rs` saying "see user resolution" doesn't satisfy the criterion-amendment-trail rule. The fix is a `## Amendment YYYY-MM-DD (user-approved, Option N)` block IN the issue description, with a satisfaction trace mapping each criterion cell to the implementing test/file.

- **DO NOT use `cargo fmt --all` from a worktree checkout.** Cargo walks the FS into `.claude/worktrees/.../crates/gf2-kernels-hip/` (excluded from the main workspace) and the conflicting workspace tables cause a metadata error. Use `cargo fmt -p <crate> -- --check` from the worktree, or run `cargo fmt --all` from main. cargo-ci's gate runner does the latter and is unaffected.

- **DO NOT skip the `--no-deps` flag when running clippy scoped to one kernel crate.** `cargo clippy -p gf2-kernels-simd --all-targets -- -D warnings` will fail because gf2-core has 6 pre-existing dead-code warnings (in `gfp/simd_ops.rs`). Use `--no-deps` to scope linting to the target crate only.

- **DO NOT trust the worker to remember to refresh `crates/gf2-algebra/src/lib.rs` `# Status`.** Each W1 close-out leaves the Status doc one wave behind. Lead-direct refresh at close time is the cheapest path; alternatively, embed a "refresh lib.rs Status" instruction in every W1 close prompt.

- **DO NOT confuse `git status` showing modified `.jit/` files with worker leaks.** The leak-check script's snapshot diff flags any post-snapshot modification, including the lead's own `jit_gate_pass` calls (which mutate `.jit/events.jsonl` and the issue JSONs). After every wave, refresh the snapshot or interpret the diff carefully — only files NOT under `.jit/` and NOT lead-touched are real leaks.

## Open questions needing user input

None as of session 2 close. The one in-flight escalation (T2 "65 elements") was resolved by the user mid-session; the resolution is recorded as the `## Amendment 2026-05-09` block in `1aa0cb99`'s description.

## Reference artefacts

- Epic: `jit issue show ae82bd73`
- Epic design doc: `dev/plans/gf2_algebra_permanent.md`
- W0 design/research artefacts (all attached to their issues): see handoff-2.
- W1 deliverables landed this session:
  - `crates/gf2-algebra/src/lib.rs` (T1, T2/T6 status refresh)
  - `crates/gf2-algebra/src/packed/{mod.rs, scalar.rs}` (T2 — PackedField, PackedFieldVec, ScalarPackedFp3, ScalarPackedFp3Vec)
  - `crates/gf2-algebra/src/gray.rs` (T6 — gray_code_iter)
  - `crates/gf2-algebra/scripts/check-feature-matrix.sh` (T1 — 64-cell sweep driver)
  - `crates/gf2-kernels-simd/src/bipedal/{framework.rs, lanes.rs, f3.rs, mod.rs}` (T16 — generic framework + Avx2Lane + Config3 with assoc-types)
  - `crates/gf2-kernels-simd/src/x86/{bipedal_avx2.rs, asm/bipedal_avx2.asm.txt}` (T16 — AVX2 entry points generic over `C: BipedalLikeConfig`)
  - `dev/research/f3_bipedal/src/lib.rs` (T16 rw1 — exposed `Bipedal3` + `F3Encoding` as library)
  - root `Cargo.toml` (T16 rw1 — added `dev/research/f3_bipedal` to `workspace.exclude`)
- Progress: `dev/active/ae82bd73-progress.json`
- Prior handoffs: `dev/active/ae82bd73-handoff-1.md`, `ae82bd73-handoff-2.md`
- External references unchanged.

## Session-2 metrics

- Issues closed: 4 (T1, T2, T6, T16)
- Total rework cycles dispatched: 3 (T16-rw1, T16-rw2, T2-rw1)
- Lead-direct mechanical fixes: 3 (T6 `n <= 63` doc fix, T2 issue-description amendment, T2 lib.rs Status refresh)
- User escalations: 1 (T2 "65 elements" → Option C)
- AI code-review gate runs (gpt-5.4): 6 (T1, T6×2, T2×3, T16×3) — average 3-5 min wall-clock each
- Sessions ahead in epic: epic at ~28% (15 of 53 closed); W1 ~67% closed; W2-W7 untouched.

