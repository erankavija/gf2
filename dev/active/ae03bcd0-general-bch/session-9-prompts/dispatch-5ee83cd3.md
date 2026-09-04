You are dispatched to JIT issue 5ee83cd3. Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-5ee83cd3
on branch worktree-agent-5ee83cd3, anchored to main's HEAD at dispatch time.

Hard rules for path discipline (worktree-dispatch-protocol):
- Run every shell command from your worktree root. Use only paths relative to the worktree root in tool calls; checkout-specific absolute paths leak files into main's checkout.
- Never run `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your worktree branch only. Do not push.
- Keep every build/dependency cache inside the worktree. Never place a cache on tmpfs.
- Never write `.jit/`, never run `jit doc add`, never change issue state. The lead owns all tracker state.
- The seeded `target` cache may carry a stale `gf2-kernels-simd` rlib; if `cargo` reports `unresolved import gf2_kernels_simd::bch_encode` (or a stale rlib as fresh), run `cargo clean -p gf2-kernels-simd` for the profiles you use (`--profile ci-test`, `--release`, dev) and rebuild. Do not treat it as a source defect.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says. This overrides the global delegation rule.
- Report to the lead with SendMessage (to: "team-lead") in messages under 2500 characters each, then finish with the same content as your final response.
- If `dev/tools/tuning-profile-compose/Cargo.lock` is missing, copy it from `/home/vkaskivuo/Projects/gf2/dev/tools/tuning-profile-compose/Cargo.lock` (that one copy is allowed).

You are implementing issue 5ee83cd3 in the current JIT-managed repository.

## Issue

**Title:** Migrate the component-code BCH consumers to the canonical model
**ID:** 5ee83cd3 (run `jit issue show 5ee83cd3` for the full record)

Migrate the remaining gf2-coding library components consuming legacy BCH constructors: the BCJR, GLDPC, and OSD touchpoints, the simulation module, and the crate root re-exports, per the investigation inventory.

### Success Criteria

- [hard] REQ-01: Each file in this task's inventory builds and passes on the canonical model with unchanged observable results.
- [hard] REQ-02: No file in this task's inventory constructs a BCH code through the legacy constructors any longer.

### Gates: `cargo-ci`, `code-review` (lead runs them on the merged tree).

### Inventory (the design's disjoint file group for `cutover-components`, `dev/active/ae03bcd0-general-bch/bch-api-design.md` "Consumer migration files"): `crates/gf2-coding/src/bcjr/mod.rs`, `crates/gf2-coding/src/gldpc/mod.rs`, `crates/gf2-coding/src/osd/generator.rs`, `crates/gf2-coding/src/simulation.rs`, `crates/gf2-coding/src/lib.rs`. Tests that exercise these modules may change only where a legacy BCH construction sits inside them; report any other file that needs a change as a footprint finding instead of editing it.

## Addressable context

Resolve before acting: `jit item show @/issue/5ee83cd3/requirement/REQ-01 @/issue/5ee83cd3/requirement/REQ-02 @/issue/ae03bcd0/requirement/REQ-10 @/issue/ae03bcd0/requirement/REQ-06`, invariants `@/invariant/canonical-cutover @/invariant/convention-convergence @/invariant/backend-behavioral-equivalence @/invariant/deterministic-seeded-execution @/invariant/present-tense-prose @/invariant/no-deferred-defects`. Read `AGENTS.md` in full, the design's "Consumer-family mapping" tables (`bch-api-design.md` from line 843: which families are canonical and which stay on the `binary-code-v1` boundary), and the investigation's consumer inventory (`dev/active/ae03bcd0-general-bch/investigation.md`, "consumer-inventory") for the exact call sites.

## The canonical surface you migrate to

- Construction: `gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance, RootExponent, …}` with the delegating conveniences `BinaryBchCode::primitive_narrow_sense(…)`, `primitive_narrow_sense_auto(…)`, `from_generator(…)` etc. (1c9a0f9f; see `crates/gf2-coding/src/bch/spec.rs` module docs and the README).
- Encoding: `traits::block::BlockEncoder` (`encode`, allocation-free `encode_into`, workspace batch from 8f68699b, `crates/gf2-coding/src/bch/encode.rs`).
- Matrices: `traits::block::{GeneratorMatrixAccess, ParityCheckMatrixAccess}`; canonical BCH matrices are `G = [I_k | P]` in the user layout with `is_systematic() == true` and `H = [-Pᵀ | I]` (444c06bc). GLDPC reads `G = [I | P]` directly (`gldpc/mod.rs` around lines 118–130) — that assumption now holds on the canonical matrices; verify it with a test rather than assuming.
- Extended BCH components: `Extended<BinaryBchCode>` / `ExtendedBchComponent` (6d67e57e) where a component is eBCH.
- The compatibility boundary `traits::compat::binary_v1` stays for non-BCH families (CRC, DRM, LDPC, product components other than eBCH, BCJR consumers of non-BCH matrices, mock encoders); a BCH value passed through a v1 interface uses the blanket adapter. Do not migrate non-BCH families.

## What to do

1. Inventory: `rg -n "BchCode::new|BchCode::from_generator|BchEncoder::new|BchDecoder::new|bch::BchCode\b|bch::core::" crates/gf2-coding/src/{bcjr/mod.rs,gldpc/mod.rs,osd/generator.rs,simulation.rs,lib.rs}` and read each site with its tests.
2. Test-first: for each site, identify the existing test that pins its observable result (component parity, decoder outcome, GLDPC generator layout, OSD generator adapter behavior, simulation outputs with a fixed seed). Where none exists, add a minimal one on the current behavior before migrating, so "unchanged observable results" is evidenced, not asserted.
3. Migrate each site to the canonical construction, encoder, and matrix traits; keep decoder use on the hardened binary decoder (`bch::core` decoding stays until `cutover-removal`; the issue is about construction and encoding/matrix consumption). `lib.rs` re-exports: export the canonical surface and keep legacy re-exports only where another not-yet-migrated inventory still uses them (the removal is 4a2baa12's job); document nothing as "legacy" in permanent prose beyond the named boundary.
4. `simulation.rs`: keep seeds and outputs identical (`@/invariant/deterministic-seeded-execution`); any fixture pinned to a legacy generator polynomial must reproduce it through `from_generator` or the primitive convenience with the same field modulus.
5. REQ-02 check: the grep from step 1 returns nothing in the inventory files at the end.

## Verification before you report (paste outputs)

- Focused: `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --cargo-profile ci-test --profile ci -E 'test(bcjr) | test(gldpc) | test(osd) | test(simulation)'`.
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table.
- The step-1 grep output at HEAD (empty).
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Commits

Conventional subjects under 72 chars with the scope: `refactor(jit:5ee83cd3): …`, `test(jit:5ee83cd3): …`. One commit per inventory file is a good shape. Stage explicitly; never `commit -am`.

## Report

Lead with essentials in under 2500 characters: branch tip SHA, per-file migration summary (one line each), the empty grep, the cargo-ci table, footprint findings (files outside the inventory that still hold legacy BCH construction), anything unverified. Details in a second message.
