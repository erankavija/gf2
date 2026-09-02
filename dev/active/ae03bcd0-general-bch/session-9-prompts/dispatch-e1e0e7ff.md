You are dispatched to JIT issue e1e0e7ff. Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-e1e0e7ff
on branch worktree-agent-e1e0e7ff, anchored to main's HEAD at dispatch time.

Hard rules for path discipline (worktree-dispatch-protocol):
- Run every shell command from your worktree root. Use only paths relative to the worktree root in tool calls; checkout-specific absolute paths leak files into main's checkout.
- Never run `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your worktree branch only. Do not push.
- Keep every build/dependency cache inside the worktree. Never place a cache on tmpfs.
- Never write `.jit/`, never run `jit doc add`, never change issue state. The lead owns all tracker state.
- The seeded `target` cache may carry a stale `gf2-kernels-simd` rlib; if `cargo` reports `unresolved import gf2_kernels_simd::bch_encode` (or a stale rlib as fresh), run `cargo clean -p gf2-kernels-simd` for the profiles you use (`--profile ci-test`, `--release`, dev) and rebuild. Do not treat it as a source defect.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says. This overrides the global delegation rule.
- Report to the lead with SendMessage (to: "team-lead") in messages under 2500 characters each, then finish with the same content as your final response.
- If `dev/tools/tuning-profile-compose/Cargo.lock` is missing, copy it from `/home/vkaskivuo/Projects/gf2/dev/tools/tuning-profile-compose/Cargo.lock` (that one copy is allowed).

You are implementing issue e1e0e7ff in the current JIT-managed repository.

## Issue

**Title:** Shared property and conformance suites across field classes
**ID:** e1e0e7ff (run `jit issue show e1e0e7ff` for the full record)

Extend the shared property/conformance suites to cover construction, encoding, matrices, and the derived-code transformations (including coordinate maps and rank-derived dimensions) over base fields of forms GF(2), GF(p), and GF(p^r), including valid boundary codes.

### Background

The pre-cutover binary property tests provide the baseline invariants; this suite generalizes them to the canonical model and all field classes under the repository's shared-test-contract and tier-budget rules, over the evidence protocol's predeclared corpus rows.

### Success Criteria

- [hard] REQ-01: One shared suite exercises construction, encoding, matrix, and transformation invariants over the three field classes on the predeclared corpus, with boundary codes included and deterministic seeds.
- [hard] REQ-02: Packed binary cases include the word-boundary lengths 0, 1, 63, 64, and 65 with little-endian indexing and zero tail padding.
- [hard] REQ-03: Every implementation of the shared code interfaces runs the same behavioral conformance cases; fast-tier budgets hold with slow cases tagged.

### Gates: `cargo-ci`, `code-review` (lead runs them on the merged tree). Manifest footprint: creates `crates/gf2-coding/tests/bch_conformance.rs`; shared helpers may extend `crates/gf2-coding/src/test_support.rs`.

## Addressable context

Resolve before acting: `jit item show @/issue/e1e0e7ff/requirement/REQ-01 @/issue/e1e0e7ff/requirement/REQ-02 @/issue/e1e0e7ff/requirement/REQ-03 @/issue/ae03bcd0/requirement/REQ-11 @/issue/ae03bcd0/requirement/REQ-04 @/issue/ae03bcd0/requirement/REQ-05 @/issue/444c06bc/requirement/REQ-01 @/issue/444c06bc/requirement/REQ-02`, invariants `@/invariant/shared-test-contracts @/invariant/semantic-test-assertions @/invariant/canonical-bit-indexing @/invariant/deterministic-seeded-execution @/invariant/test-tier-budgets @/invariant/convention-convergence @/invariant/finite-field-laws`. Read `AGENTS.md` in full and the plan's `evidence-protocol` section (`dev/active/ae03bcd0-general-bch/plan.md` from line 94: the corpus rows B1–B4, N1–N4, seed `0xAE03BCD0`).

## What exists — reuse, do not duplicate (`@/invariant/convention-convergence`)

- `gf2_coding::test_support::visit_bch_corpus` / `BchCorpusVisitor` / `bch_corpus_messages` (landed by 3f7edef1) construct every predeclared corpus row in its canonical representation with the seeded messages; the corpus is the row set REQ-01 names.
- `crates/gf2-coding/tests/bch_property_tests.rs` (or wherever 3243bc1f's binary property tests live — locate with `rg -l "proptest" crates/gf2-coding/tests`) are the baseline invariants; generalize them into the shared suite and remove or redirect duplicates rather than leaving two suites asserting the same law.
- `traits::block` (`crates/gf2-coding/src/traits.rs`): `BlockCode`, `BlockEncoder`, `GeneratorMatrixAccess` (`is_systematic`), `ParityCheckMatrixAccess`; implementors in gf2-coding: `BchCode<X,S,M>` (packed `BinaryBchCode` and `DenseBchCode`), `LinearBlockCode`, `Shortened<C>`, `Punctured<C>`, `Extended<C>`, `RepetitionCode`, plus boundary-family types on `compat::binary_v1`. REQ-03 means one generic conformance function set, applied to every canonical implementor (a `fn conformance_cases<C>(code: &C, …)` family), with implementation-specific tests left to their own modules.
- Matrix contract (444c06bc): `G = [I_k | P]`, `is_systematic()`, `H` full row rank with `G Hᵀ = 0`, boundary codes (full-space and zero-dimensional) supported.
- Transformations (8aa98a25, puncture, 981c5ae3): coordinate maps compose back to the mother; dimensions derive by rank; `information_set()` reports pivots; one-symbol extension adds the coordinate making the symbol sum zero. If 203ee826 (systematic fast path for `Shortened<C>`) has landed on main when you start, cover both of its derivation paths through the same assertions.
- Construction (REQ-04 of the epic): derived `k`, witnessed distance bound and correction radius, `g | xⁿ − 1`, validation of `gcd(n, q) = 1`, exact root order, coefficient restriction; typed errors, never panics, for invalid inputs.

## What to build

1. `crates/gf2-coding/tests/bch_conformance.rs`: one shared suite, deterministic (seed `0xAE03BCD0` through the repository's seeded RNG helper), organized as generic case functions applied to every corpus row in both representations where a row admits both (packed for GF(2) rows, dense for all), plus the boundary codes (full-space, zero-dimensional) and the packed word-boundary lengths 0, 1, 63, 64, 65 (through shortening/puncturing to those lengths or through fixtures the module already offers; assert little-endian indexing and zero tail padding on packed outputs).
2. Invariant groups: construction (derived parameters, validation errors are typed), encoding (systematic layout, linearity, every encoder family agrees with the scalar reference where the family seam exposes selection, allocation-free paths equal allocating ones), matrices (identity block, `G Hᵀ = 0`, rank, caller-buffer shape errors, cache wrapper equality), transformations (coordinate map composition, rank-derived dimension against a direct generator-rank computation, membership of transformed codewords, extension parity).
3. Slow cases: anything over the fast-tier per-test budget on this host carries `#[ignore = "slow: …"]`; measure and paste wall times. The T2N mother row (B4) is the one likely to need it for matrix work; its construction and encoding stay in the fast tier.
4. Deduplicate: where 3243bc1f's binary property tests assert a law the shared suite now asserts for all field classes, fold them in (keep any binary-only law where it lives). State in the suite's module doc what it covers and where implementation-specific tests remain.

## Verification before you report (paste outputs)

- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --test bch_conformance --cargo-profile ci-test --profile ci` with per-test wall times; the slow tier for any ignored case.
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table.
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Commits

Conventional subjects under 72 chars with the scope: `test(jit:e1e0e7ff): …`, `refactor(jit:e1e0e7ff): …` for deduplication. Stage explicitly; never `commit -am`.

## Report

Lead with essentials in under 2500 characters: branch tip SHA, the list of implementors the generic cases run over, the count of cases per group, wall times, the cargo-ci table, anything unverified or deviating with the reason. Details in a second message.
