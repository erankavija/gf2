You are dispatched to JIT issue 203ee826. Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-203ee826
on branch worktree-agent-203ee826, anchored to main's HEAD at dispatch time.

Hard rules for path discipline (worktree-dispatch-protocol):
- Run every shell command from your worktree root. Use only paths relative to the worktree root in tool calls; checkout-specific absolute paths leak files into main's checkout.
- Never run `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your worktree branch only. Do not push.
- Keep every build/dependency cache inside the worktree. Never place a cache on tmpfs.
- Never write `.jit/`, never run `jit doc add`, never change issue state. The lead owns all tracker state.
- The seeded `target` cache may carry a stale `gf2-kernels-simd` rlib; if `cargo` reports `unresolved import gf2_kernels_simd::bch_encode` (or a stale rlib as fresh), run `cargo clean -p gf2-kernels-simd` for the profiles you use (`--profile ci-test`, `--release`, dev) and rebuild. Do not treat it as a source defect.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says. This overrides the global delegation rule.
- Report to the lead with SendMessage (to: "team-lead") in messages under 2500 characters each, then finish with the same content as your final response.
- If `dev/tools/tuning-profile-compose/Cargo.lock` is missing, copy it from `/home/vkaskivuo/Projects/gf2/dev/tools/tuning-profile-compose/Cargo.lock` (that one copy is allowed). If cargo reports a stale rlib as fresh after a pool-seeded build, `find crates/gf2-coding/src -name '*.rs' -exec touch {} +`.

You are implementing issue 203ee826 in the current JIT-managed repository.

## Issue

Run `jit issue show 203ee826` and read the full description; it is the specification (summary, background, four hard criteria REQ-01..REQ-04, notes). Its gates are `cargo-ci`, `code-review`, `doc-review`; the lead runs them on the merged tree, and your work must be sufficient to pass all three.

## Addressable context

Resolve before acting (chain in one call): `jit item show @/issue/203ee826/requirement/REQ-01 @/issue/203ee826/requirement/REQ-02 @/issue/203ee826/requirement/REQ-03 @/issue/203ee826/requirement/REQ-04 @/issue/ae03bcd0/requirement/REQ-05 @/issue/ae03bcd0/requirement/REQ-13 @/issue/8aa98a25/requirement/REQ-01 @/issue/444c06bc/requirement/REQ-01`, and invariants `@/invariant/library-first-generality @/invariant/convention-convergence @/invariant/shared-test-contracts @/invariant/canonical-bit-indexing @/invariant/test-tier-budgets @/invariant/present-tense-prose @/invariant/single-source-prose @/invariant/no-deferred-defects @/invariant/caller-trusted-fast-paths`. Read `AGENTS.md` in full.

## Where the current code is

- `crates/gf2-coding/src/transform/mod.rs`: `Shortened<C>` (struct near line 392, owns `generator: FieldMatrix<C::Symbol>` and `information_set`), constructors `shorten`/`shorten_first`/`shorten_last` (~489–520), `BlockEncoder`/`encode` impl (dense product, ~640–672), `GeneratorMatrixAccess for Shortened<C>` (~674), `build_shortened` (~997: materializes the whole mother generator, then `derive_shortened_data` ~1142: constraints, nullspace, candidate product, `reduce_to_rref_basis` ~1220). `Punctured<C>` and `Extended<C>` share the rank machinery; leave their behavior unchanged.
- `crates/gf2-coding/src/traits.rs`: `traits::block` (`BlockCode`, `BlockEncoder`, `GeneratorMatrixAccess` with `is_systematic()`, `ParityCheckMatrixAccess`, `SymbolMatrix`).
- `crates/gf2-coding/src/bch/matrix.rs`: the canonical BCH matrices are `G = [I_k | P]` and `H = [-Pᵀ | I_{n-k}]` in the user layout with `is_systematic() == true` (landed by 444c06bc); the module doc states the layout and the parity recurrence.
- `crates/gf2-coding/src/bch/spec.rs`: `BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense { extension: BinaryPrimeExt::new(Gf2mField::new(16, 0b1_0000_0000_0010_1101))?, designed_distance: 25 })` builds the DVB-T2 normal-frame mother (65343 × 65535, generator degree 192); `crates/gf2-coding/benches/bch_genmatrix.rs` shows the construction idiom.
- Design: `dev/active/ae03bcd0-general-bch/bch-api-design.md` — "Static code types" (the `Shortened`/`Punctured`/`Extended` sketch near line 443) and "Derived-code transformations" (near line 946). The design's rule that wrappers contain no BCH-specific logic stays: the fast path is stated purely through the canonical traits.

## What to build

1. **Derivation data as an enum.** Replace the always-dense `generator` field with derivation data that is either `RankDerived { generator: FieldMatrix<_>, information_set }` (today's path) or `SystematicRestriction { kept_message_positions, … }` (the fast path). `Clone`/`Debug` follow. Expose the path through a documented accessor (for example `pub fn derivation(&self) -> ShortenedDerivation`, a small `Copy` enum), so tests assert which path a construction took (REQ-03).
2. **Fast-path condition** (evaluated in `build_shortened`, before any mother matrix is touched): `mother.is_systematic()? == true` and every removed coordinate `< mother.k()`. Then `k' = k − |removed|`, `information_set = 0..k'` (kept message positions in order), `n' = n − |removed|`; no `generator_matrix()`, no nullspace, no RREF. Otherwise the existing general path, byte-for-byte unchanged in behavior.
3. **Encoding on the fast path**: build the mother message with zero symbols at the removed positions and the shortened message symbols at the kept positions, encode through the mother's `BlockEncoder`, delete the removed coordinates. Reuse the mother's allocation-free/workspace encoder shape where the traits expose it; at minimum the allocating `encode` and any `encode_into` the wrapper already implements. Codewords must equal the general path's on every shared fixture.
4. **Matrices on the fast path**: `generator_matrix_into` writes, for each kept message position `i` (in order), the mother's generator row `i` with the removed columns deleted; `parity_check_matrix_into` writes the mother's parity-check matrix with the removed columns deleted (full row rank `n − k` survives because the identity block sits on parity coordinates). Obtain the mother's rows through the canonical traits only: materialize the mother's matrix into a buffer sized by the mother (`C::GeneratorMatrix::zeroed(k, n, …)` via `generator_matrix_into`) and copy the sub-block, or, where a cheaper row-wise access exists on the trait surface, use it — but do not add BCH-specific knowledge to the wrapper. Document the memory cost (one mother-sized buffer) in rustdoc. `is_systematic()` returns true; `information_set()` reports `0..k'`.
5. **Tests (test-first):**
   - Equality of the fast path against the general path on small fixtures where both run: binary (`BinaryBchCode` B1/B2/B3 mothers) and nonbinary (`DenseBchCode` over GF(3)/GF(5)/GF(9) rows — `gf2_coding::test_support::visit_bch_corpus` constructs the predeclared corpus) for encode, generator, parity-check, dimension, information set, coordinate map. Force the general path for the comparison by shortening on a parity coordinate or a non-systematic mother (state how in the test's rustdoc).
   - Packed word-boundary lengths 0, 1, 63, 64, 65 as shortened lengths where a mother admits them (B2 with n = 127 shortened to 65/64/63; full shortening of message positions to the zero-dimensional boundary; a mother shortened to length 1 where that is a valid code) — little-endian indexing and zero tail padding asserted on the packed outputs.
   - The DVB-T2 witness (REQ-01): construct the normal-frame mother, `Shortened::shorten_first(mother, 33135)`, assert `(n, k) == (32400, 32208)`, `derivation()` is the fast path, `is_systematic()`, and that encoding a seeded payload equals the mother's encoding of the zero-prefixed message with the prefix removed; keep it inside the fast tier (construction plus a few encodes). Materializing the 32208 × 32400 generator (130 MB packed) belongs in an `#[ignore = "slow: …"]` test that also checks `G Hᵀ = 0` on sampled rows.
   - Boundary codes: full-space mother, zero-dimensional result, empty removed set.
   - Wire the new cases into the existing shared transformation conformance tests (8aa98a25/`Punctured`/`Extended` suites in the module and under `tests/`) so both paths run through the same assertions (`@/invariant/shared-test-contracts`).
6. **Docs (REQ-04):** rustdoc on `Shortened<C>` and the constructors states the condition, the complexity of both paths, and the memory cost of matrix materialization; amend `dev/active/ae03bcd0-general-bch/bch-api-design.md` (the `Shortened` sketch and the derived-code transformations table) in present tense with no history narration; check `crates/gf2-coding/README.md` for any shortening description and keep it accurate. No dates in prose, no root-absolute Markdown links, no TODO/deferred markers.

## Out of scope

`Punctured<C>`, `Extended<C>`, the DVB-T2 modules (`crates/gf2-coding/src/bch/dvb_t2/*`, migrated by 97410c80 after you), benchmarks, and the survey contract. Report anything you find there as a footprint finding rather than editing it.

## Verification before you report (paste outputs)

- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --cargo-profile ci-test --profile ci -E 'test(shorten) | test(transform) | test(Shortened)'` with per-test wall times; the slow-tier witness under `--release --profile slow --run-ignored ignored-only` with its wall time.
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table.
- `cargo doc -p gf2-coding --all-features --no-deps` builds without warnings on the touched items.
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Commits

Conventional subjects under 72 chars with the scope: `feat(jit:203ee826): …`, `test(jit:203ee826): …`, `docs(jit:203ee826): …`. Stage explicitly; never `commit -am`. Commit test-first evidence before implementation where practical.

## Report

Lead with essentials in under 2500 characters: branch tip SHA, the condition as implemented (one line), the accessor name, the DVB-T2 witness numbers and wall time, the cargo-ci step table, and anything unverified or deviating from the issue with the reason. Details (test inventory, design-doc diff summary, footprint findings) in a second message.
