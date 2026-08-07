# Handoff — Fast matrix permanents over F_3 / F_5 / F_7 via packed bipedal arithmetic (ae82bd73) — session 3

**Date:** 2026-05-10 (session 3 close)
**Session number:** 3
**Prior handoffs:** ae82bd73-handoff-1.md, ae82bd73-handoff-2.md, ae82bd73-handoff-3.md (read all in order; traps from each carry forward unless explicitly resolved)

## Current state

- Epic: `ae82bd73` — state: `backlog` (still has unresolved deps).
- Wave in progress: **W2 sub-wave 2c — T9 in `in_progress` (3 unresolved review findings, see below); W2 then has T10/T11/Sa to dispatch.**
- Children summary (epic-level): 23+ done (was 15 at session-2 close); ~30 still backlog/ready; T9 in_progress.
  - W1 fully closed: T1, T2, T3, T4, T5, T6, T16. (Session 3 closed T3, T4, T5; sessions 1+2 closed the rest.)
  - W2 partially closed: T7, T8 done; T9 in_progress; T10/T11/Sa not yet dispatched.
- Active claims: T9 (`b0857ae9`) — `agent:claude`.
- Open escalations: NONE blocking. The epic + T9 criterion-3 amendment was completed mid-session; T9 has open findings noted below that need resolution next session (1 of them likely needs another small criterion amendment).
- Progress file: `dev/active/ae82bd73-progress.json` — refresh inline before commit.
- Worktrees: cleaned up (none active).

## What just happened (session 3)

### Issues closed in close-order

1. **T3 (46330802 — Bipedal3 element)** — 2 reworks. Closed PASS.
   - rw0 FAIL: sub implemented as `add(self, neg(rhs))` (7 ops, derived) — criterion required 6-op direct transliteration.
   - rw1 FAIL: rewrote sub as 6-op CSE on `am^bm` — bit-identical but reviewer flagged as not matching the canonical paper §2.2 form.
   - rw2 PASS: rewrote sub to canonical paper §2.2 verbatim (`t = s1^s2; u = m1&t; mag = u|(m1^m2); sgn = u^(m2^s2)`).

2. **T4 (053e4016 — Bipedal3Vec)** — 1 rework. Closed PASS.
   - rw0 FAIL: `neg_assign` missing (criterion 3 said "add, sub, mul, neg" but `PackedFieldVec` trait doesn't have `neg_assign`); proptest cross-check used only `ScalarPackedFp3Vec`, not "per-lane Bipedal3 ops".
   - rw1 PASS: added inherent `Bipedal3Vec::neg_assign` + per-chunk Bipedal3 cross-check proptests.

3. **T5 (ef7d0633 — Bipedal3Matrix)** — 0 reworks (1 lead-direct doc amendment). Closed PASS.
   - rw0 FAIL: `Vec<Bipedal3Vec>` storage matched T5 criterion 1 verbatim, but conflicted with R3 §2.1 + epic §7.2's flat `mag/sgn: Vec<u64>` SSOT.
   - **User-approved Option A (2026-05-10):** keep `Vec<Bipedal3Vec>`, amend R3 §2.1 + epic §7.2 to declare it the SSOT. Each column remains contiguous Vec<u64> per leg, satisfying SIMD wide-load rationale.
   - Lead-direct: edited `dev/plans/r3_multi_word_streaming.md` §2.1 (added `### 2.1.1 Layout amendment 2026-05-10`) and `dev/plans/gf2_algebra_permanent.md` §7.2. Re-ran code-review → PASS.

4. **T7 (93e5a5e8 — generic permanent_ryser<F: FiniteField>)** — 2 reworks + 1 lead-direct. Closed PASS.
   - rw0 FAIL: bound `<F: ConstField>` not `<F: FiniteField>` per criterion 1.
   - rw1 FAIL: rebound to FiniteField but n=0 panic undocumented + no non-ConstField test + stale lib.rs Status.
   - rw2 FAIL: missing `n <= 63` assertion (gray_code_iter precondition; gray.rs even claims permanent driver enforces it).
   - Lead-direct rw3: 12-line surgical fix (assert + doc bullet + #[should_panic] regression test). PASS.

5. **T8 (632d92ad — permanent_mod3_reference)** — 1 rework + 1 lead-direct. Closed PASS.
   - rw0 FAIL: 500 cross-checks vs criterion's 12000; missing paper-port comment markers; inline MMIX LCG vs `gf2_core::rng::Lcg` SSOT; stale lib.rs/mod.rs Status.
   - rw1 FAIL: T8's own findings fixed, but reviewer's cross-issue SSOT scan flagged T7's `random_matrix` test helper (`crates/gf2-algebra/src/permanent/ryser.rs:255-273`) as ALSO using inline MMIX LCG — counted as same-subsystem blocker.
   - Lead-direct rw2: refactored T7's `random_matrix` to use `gf2_core::rng::Lcg::new(seed) + next_u64() % P`. Re-ran T8 code-review → PASS.

### Lead-direct minor edits (no worker dispatch) — three this session:

- **T5:** R3 §2.1 + epic §7.2 amendments per user-approved Option A (Vec<Bipedal3Vec> SSOT layout).
- **T7:** 12-line `assert!(n <= 63, ...)` + `# Panics` doc bullet + `#[should_panic]` test for n=64.
- **T8 follow-on:** T7's `random_matrix` test helper refactored to use `gf2_core::rng::Lcg`.

### User escalations resolved this session

1. **T5 SSOT contract conflict (2026-05-10):** Option A — keep Vec<Bipedal3Vec>, amend R3 + epic §7.2. Recorded in `r3_multi_word_streaming.md` §2.1.1.
2. **T9/epic large-n cross-check infeasibility (2026-05-10):** Option A — trim n ∈ {28, 32} from the cross-check; keep {20, 24} only. Recorded as `## Amendment 2026-05-10` blocks in T9 (`b0857ae9`) and epic (`ae82bd73`) descriptions.

### Issues still in flight (T9)

**T9 (b0857ae9 — permanent_bipedal3 single-word fast path)** — 1 rework done; gates state:
- `cargo-ci`: PASS
- `tests`: PASS
- `code-review`: **FAIL** with three remaining findings:
  1. **Contract conflict on n ≤ 63 vs n ≤ 64.** Criterion 1 verbatim: "requires `matrix.cols() == matrix.rows()` and `matrix.cols() <= 64`." But `gray_code_iter` requires `n <= 63` (`1u64 << 64` is UB). T9-rw1 changed assertions/docs from `<= 64` to `<= 63`; reviewer reads this as a hard contract violation.
  2. **SSOT — inline raw-word arithmetic.** T9-rw1's halving-tree fold uses inline paper-§2.2 add/sub formulas on raw `u64` words at `crates/gf2-algebra/src/permanent/bipedal3.rs:173-187` instead of going through `Bipedal3::add` / `Bipedal3::mul` / `Bipedal3::from_raw`. Duplicates packed-field arithmetic that already exists in T3.
  3. **Cross-check oracle mismatch.** T9-rw1 cross-checks n=20/24 against `permanent_mod3_reference` (T8) instead of `permanent_ryser` (T7). The 2026-05-10 amendment to criterion 3 trimmed the n range but did NOT change the oracle from `permanent_ryser`. Empirical reality: `permanent_ryser` at n=24 takes ~80s/matrix, exceeding the 120s slow-tier per-test budget for any reasonable matrix count.

### Process improvements applied this session

- **Memory written:** `feedback_sonnet_for_planned_dispatches.md` (use Sonnet model for transliteration / wrapper / mechanical tasks).
- **Memory written:** `feedback_quote_ssot_formulas_verbatim.md` (when transliterating a paper formula, grep project design docs and quote them token-for-token; never re-derive).
- **Memory written:** `feedback_quote_jit_criteria_verbatim.md` (copy issue success criteria verbatim into dispatch prompts; never paraphrase counts/ranges).
- **Confirmed:** lead-direct mechanical fixes (CLAUDE.md doc bound, Status refresh, JIT description amendment, surgical assertion + doc bullet) are appropriate for narrowly scoped Tier 2.5/2.75/2.5 fixes. Three this session.

## What to do next

**Immediate (session 4 start):**

- [ ] **Resolve T9 finding 1 (n=63 vs n=64 contract).** Two options:
  - (a) Amend T9 criterion 1 to `mat.cols() <= 63` (user approval, escalation pattern matches T5/criterion-3 amendments). Lowest churn — current implementation already enforces `<= 63`.
  - (b) Extend `gray_code_iter` (or hand-roll a u128-based walk in permanent_bipedal3) to support n=64, keeping criterion 1 as written. Adds complexity; criterion's `<= 64` was likely written without UB awareness.
  - **Recommend (a)** — the n=64 case would require a special u128 walk path that doesn't compose with existing gray_code_iter and is unmotivated by any known consumer.
- [ ] **Resolve T9 finding 2 (SSOT — inline raw-word arithmetic).** Refactor T9's halving-tree fold to use `Bipedal3::from_raw(mag, sgn)` constructors + `Bipedal3::mul(other)` instead of inline add/sub/mul on raw u64. Mechanical worker dispatch (~30 min).
- [ ] **Resolve T9 finding 3 (cross-check oracle).** Amend T9 criterion 3 (or add a sub-clause to the existing 2026-05-10 amendment) to permit `permanent_mod3_reference` as the slow-tier oracle for n ∈ {20, 24}, with the transitivity argument noted (T8's own n ≤ 12 cross-check vs `permanent_ryser` proves equivalence; for n=20/24 we cross-check vs T8 instead). User approval needed.
- [ ] **Dispatch T9 rework cycle 2** addressing all three findings simultaneously (after the criterion amendments are in). This will be the LAST rework cycle allowed before MAX_REWORK_ATTEMPTS=2 escalation.
- [ ] **After T9 closes, dispatch W2 sub-wave 2c:** T10 (criterion bench `b315564a`), T11 (test-vectors `1cd3eb09`), Sa (paper Table 2 reproduction `96dcbec4`). T10 and T11 are file-disjoint; can parallelize. Sa depends on T9 + T10 (it consumes the criterion bench results).
- [ ] **After W2 fully closed, dispatch W3 sub-wave 3a:** T12 (SIMD bipedal3 kernel via BatchedBipedalLike `d181e95b`), T13 (SIMD dispatch `686ee1b5`), T14 (multi-word streaming `a7886bd8`), T15 (rayon parallel `05250df5`), then S1/S2/S3 (perf criteria).
- [ ] Continue down `dev/plans/gf2_algebra_permanent.md` §13.

**Process improvements (lessons embedded into next-session dispatch):**

- [ ] **Quote JIT criteria verbatim** in dispatch prompts (memory `feedback_quote_jit_criteria_verbatim.md`). Two sessions in a row I've mis-quoted criteria (T7 ConstField vs FiniteField; T8 100 vs 1000 matrices). Open `jit issue show <id> --json | jq -r '.description'` BEFORE composing every prompt and paste the criteria block verbatim.
- [ ] **Quote SSOT formulas verbatim** in dispatch prompts (memory `feedback_quote_ssot_formulas_verbatim.md`). Grep `dev/plans/`, `dev/research/` for the formula's variable names before dispatching transliteration tasks.
- [ ] **Pre-flight criterion vs reality check** for every issue with computational thresholds. T9's n ∈ {28, 32} criterion was infeasible against the generic Ryser oracle (~8 days for 100 matrices); should have been caught BEFORE dispatch via a back-of-envelope feasibility estimate. Memory `feedback_pre_dispatch_criterion_audit` already exists; reinforce.
- [ ] **Reviewer cross-issue SSOT scans:** when reviewing T8, the reviewer flagged T7's old test helper as in-scope SSOT debt. Lesson: when landing T<N>, audit T<N-1>'s recently-merged code in the same subsystem for SSOT/duplication issues that the prior reviewer let through. Cheap to audit, prevents reviewer-pollution-style FAILs in T<N+1>.
- [ ] **For cross-check tests against expensive oracles:** establish the oracle-cost budget BEFORE writing the criterion. Per-issue rule: if criterion involves 100+ cross-checks at large n vs a generic oracle, run a 5-matrix smoke test first to estimate per-matrix cost, then size the criterion to fit the slow-tier 120s/test budget.

## Traps — do not repeat these

Carrying forward from handoff-1, handoff-2, handoff-3 (every trap from those handoffs remains in force unless explicitly resolved). Adding session-3's new traps:

### Carried forward (re-stated for emphasis)

- **DO NOT use a JIT "comment" primitive.** Sketch / approval criteria → use `## Approval` or `## Amendment YYYY-MM-DD` block in the issue description.
- **DO NOT cite `gf2-kernels-simd → gf2-core` as a forward edge.** The actual edge is `gf2-core → gf2-kernels-simd`.
- **DO NOT use `(k >> flip) & 1`** for Gray-code add/sub — use `g_k = k ^ (k >> 1); ((g_k >> flip) & 1) == 1`. (The `gray_code_iter` API already resolves this internally; consumers don't need to recompute.)
- **DO NOT cite epic doc subsections "§12.1" / "§12.2" / "§12.3"** — section 12 has named subsections V1, V2, V3.
- **DO NOT submit a deliverable until it passes a self-audit against ALL of CLAUDE.md's documentation standards** (description + Arguments + Examples + Panics + Complexity).
- **DO NOT dispatch parallel agents on issues that share working-tree files** unless using worktrees (which we always do).
- **DO NOT commit `dev/research/<crate>/target/` build artefacts.** Every new `dev/research/<crate>/` MUST have `.gitignore` with `target/` + `Cargo.lock` in the first commit.
- **DO NOT forget to claim + transition to in_progress BEFORE Agent dispatch.**
- **DO NOT use `1u64 << n` to compute the iteration upper bound when the docs claim `n <= 64`.** `1u64 << 64` is UB; use `n <= 63`.
- **DO NOT design a `[hard]` criterion that requires "N elements" when N exceeds the type's fixed-LANES const.** Pre-amend before dispatch.
- **DO NOT hard-code a per-prime config inside what the criterion calls a "generic" entry point.**
- **DO NOT forget to record `[hard]` criterion amendments in the JIT issue description.** Source-code comments don't satisfy the reviewer.
- **DO NOT use `cargo fmt --all` from a worktree checkout.** Use `cargo fmt -p gf2-algebra -- --check`.
- **DO NOT skip the `--no-deps` flag when running clippy scoped to one kernel/algebra crate.**
- **DO NOT trust the worker to remember to refresh `crates/gf2-algebra/src/lib.rs` `# Status`.** Lead-direct refresh at close-out is the cheapest path; alternatively, embed it in every dispatch prompt.
- **DO NOT confuse `git status` showing modified `.jit/` files with worker leaks.** Lead's `jit gate pass` calls mutate `.jit/`; only files NOT under `.jit/` and NOT lead-touched are real leaks.
- **DO NOT tell a worker "the scalar Bipedal3 reference doesn't exist yet" — it does, at `dev/research/f3_bipedal/src/bipedal.rs::Bipedal3`.**

### NEW from session 3

- **DO NOT paraphrase or re-derive paper formulas in dispatch prompts.** T3-rw1 burned a full review cycle because the lead's prompt told the worker to derive the 6-op sub formula via "substitute neg(b).sgn = bsg^bm into add and apply CSE on (am^bm)". The worker produced a 6-op result that was bit-identical but did NOT match the canonical paper §2.2 form documented in 4 SSOT sources. Always grep `dev/plans/`, `dev/research/` for the canonical form first and quote it token-for-token. Memory: `feedback_quote_ssot_formulas_verbatim.md`.

- **DO NOT paraphrase JIT issue success criteria in dispatch prompts.** T7-rw0 shipped with `<F: ConstField>` because the lead's prompt emphasized "use `Fp<P>`-style construction" and the worker matched that. T8-rw0 shipped with 500 cross-checks because the lead's prompt said "100 matrices for n ∈ {1..5}" — actual criterion was "1000 matrices for n ∈ {1..12}" = 12000 total. Always paste the issue's `## Success Criteria` block verbatim into the prompt before composing the rest. Memory: `feedback_quote_jit_criteria_verbatim.md`.

- **DO NOT design `[hard]` criteria with computational thresholds without a feasibility estimate.** T9 criterion 3 + epic criterion 2 required cross-checks at n ∈ {28, 32} × 100 matrices vs `permanent_ryser`. Reality (measured this session): n=32 takes ~2 hours/matrix; 100 matrices = 8+ days. Both were `[hard]`-marked. The criteria had to be amended mid-flight (user-approved Option A: trim to {20, 24}). Pre-flight feasibility estimates are cheap — back-of-envelope: "n=32 means 2^32 = 4·10^9 Gray steps, at 10^7 trait-Fp<3> ops/sec that's ~7 minutes per matrix" would have flagged it before dispatch.

- **DO NOT amend a criterion's matrix count without also amending its oracle clause.** This session's T9/epic criterion-3 amendment trimmed n from {20,24,28,32} to {20,24} but kept the oracle as "permanent_ryser". Reviewer flagged the 100-matrix-at-n=24 cross-check as still oracle-constrained even though the worker had transparently switched to `permanent_mod3_reference` (T8) for the slow-tier path. The amendment should have been "trim n AND permit `permanent_mod3_reference` as the slow-tier oracle for n ∈ {20, 24} via transitivity through T8's own n ≤ 12 cross-check". Open at session-3 close.

- **DO NOT trust reviewer cross-issue scans to be consistent across prior reviewer rounds.** T7 was reviewer-approved with `random_matrix` using inline MMIX LCG. T8's reviewer flagged T7's helper as an in-scope SSOT violation blocking T8 closure. Same code, different verdict. The lesson: when landing T<N>, audit T<N-1>'s same-subsystem code for SSOT/duplication issues even if T<N-1>'s reviewer didn't flag them. Lead-direct fix at T<N> close is cheaper than another rework round.

- **DO NOT count `#[ignore = "sim:"]`'d tests as "the test exists".** Reviewer-readers count ignored tests as "not exercising the criterion in any tier that runs". For criteria that say "1000 cross-checks at n=16" — if the criterion is `[hard]`, the test must actually run somewhere (slow-tier OK, but the slow-tier must run in nightly CI, which it does). Unrunnable tests are paper compliance only.

- **DO NOT assume a previously-passed gate stays valid for cross-issue findings.** When a reviewer flags T<N-1>'s code while reviewing T<N>, both T<N-1>'s gate and T<N>'s gate are in question. T7's `random_matrix` LCG was flagged in T8's review; lead-direct fix landed in main, but T7's `code-review` gate was never re-run. Per project policy this is acceptable (gates record state at one point in time), but flag in the handoff so future reviewers don't get confused.

## Open questions needing user input (next session)

1. **Amend T9 criterion 1 to `mat.cols() <= 63`?** (Or implement the n=64 special case via u128 walk.) Lead recommends amendment; minimal churn.
2. **Amend T9 criterion 3 to permit `permanent_mod3_reference` as slow-tier oracle for n ∈ {20, 24}?** Transitivity argument: T8's n ≤ 12 cross-check vs `permanent_ryser` proves T8 ↔ T7 equivalence; T9 vs T8 at n=20/24 then implies T9 vs T7 at those sizes. Lead recommends amendment.

Both can be packaged as a single `AskUserQuestion` at the start of session 4.

## Reference artefacts

- Epic: `jit issue show ae82bd73` (description includes `## Amendment 2026-05-10` for criterion 2)
- Epic design doc: `dev/plans/gf2_algebra_permanent.md` (§7.2 amended 2026-05-10)
- R3 design doc: `dev/plans/r3_multi_word_streaming.md` (§2.1.1 added 2026-05-10)
- W1 deliverables landed in earlier sessions: see handoff-2, handoff-3.
- W2 deliverables landed this session:
  - `crates/gf2-algebra/src/permanent/ryser.rs` (T7 — generic permanent_ryser<F: FiniteField>; lead-direct n<=63 assert + LCG SSOT refactor)
  - `crates/gf2-algebra/src/permanent/reference.rs` (T8 — paper Listing 1 port, paper-line markers, 12k cross-checks)
  - `crates/gf2-algebra/src/permanent/bipedal3.rs` (T9 — single-word fast path; CURRENT STATE has open code-review findings)
  - `crates/gf2-algebra/src/packed/bipedal3.rs` (T3, T4, T5 — Bipedal3 element + Bipedal3Vec + Bipedal3Matrix)
  - `crates/gf2-algebra/src/lib.rs` (Status sections refreshed multiple times)
  - `crates/gf2-algebra/src/permanent/mod.rs` (Status section + re-exports)
- Progress: `dev/active/ae82bd73-progress.json` (refresh inline before commit)
- Prior handoffs: ae82bd73-handoff-1.md, ae82bd73-handoff-2.md, ae82bd73-handoff-3.md
- Memory additions (2026-05-10):
  - `feedback_sonnet_for_planned_dispatches.md`
  - `feedback_quote_ssot_formulas_verbatim.md`
  - `feedback_quote_jit_criteria_verbatim.md`
- External references unchanged.

## Session-3 metrics

- Issues closed: 5 (T3, T4, T5, T7, T8) — 7 distinct rework cycles + 4 lead-direct fixes
- T9 in flight: 1 rework cycle dispatched, 3 findings unresolved at session close
- Reviewer-flagged cross-issue SSOT debt: 1 (T7's MMIX LCG, surfaced by T8's reviewer; lead-direct fixed)
- User escalations resolved this session: 2 (T5 Vec<Bipedal3Vec> SSOT; T9/epic criterion-3 trim from {20,24,28,32} to {20,24})
- AI code-review gate runs (gpt-5.4): ~14 (T3 ×3, T4 ×2, T5 ×2, T7 ×4, T8 ×2, T9 ×2). Average ~3-5 min wall-clock each.
- Sessions ahead in epic: epic at ~43% (23 of 53 children closed; W1 fully closed; W2 mostly closed; W3-W7 untouched).
