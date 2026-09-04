You are dispatched to JIT issue b1bd75ca. Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b1bd75ca
on branch worktree-agent-b1bd75ca, anchored to main at 188c8717e54ddd1862bb834ef113163537b2fd70. `proofs/.lake` is pre-seeded warm from the previous Lean issue's worktree, and `target` from the cache pool.

Hard rules for path discipline (worktree-dispatch-protocol):
- Run every shell command from your worktree root. Prefix tool calls with `cd /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b1bd75ca && ...` if your shell state has drifted. Use only paths relative to the worktree root in tool calls; checkout-specific absolute paths leak files into main's checkout.
- Never run `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your worktree branch only. Do not push.
- Keep every build/dependency cache inside the worktree (`proofs/.lake`, `target`). Never place a cache on tmpfs (`/tmp` is tmpfs on this host) or outside the worktree.
- Never write `.jit/`, never run `jit doc add`, never change issue state. The lead owns all tracker state.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says. This overrides the global delegation rule.
- Post-completion: run `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` from the worktree root and paste its output.

You are implementing issue b1bd75ca in the current JIT-managed repository.

## Issue

**Title:** Lean proofs: generator base-field membership and root correctness
**ID:** b1bd75ca-5003-4597-87c0-0f9808a7b760

Implement the sketch's Lean obligation that the constructed generator polynomial lies in the base field and vanishes on every requested root.

### Success Criteria

- [hard] REQ-01: The generator-correctness lemmas are proven and `lake build` passes with no new untracked assumptions.
- [hard] REQ-02: The obligation binds to the construction production path exactly as the approved sketch assigns, with refinement evidence landed where assigned.

### Gates on this issue (your work must be sufficient to pass all three; the lead runs them after merge)

- `lake-build` — `./scripts/lake-build-strict.sh` (any `sorry` in `proofs/Gf2Core/Proofs/` fails it; other errors propagate).
- `code-review` — independent AI review of every commit tagged `jit:b1bd75ca` against AGENTS.md, the invariants, the sketch, and the criteria.
- `doc-review` — independent AI review of documentation impact: proof inventories, module header, rustdoc of the new test, **and the approved sketch's own O-4 section** (see ruling 6).

## Addressable context

Resolve before acting (chain in one shell call): `jit item show @/issue/b1bd75ca/requirement/REQ-01 @/issue/b1bd75ca/requirement/REQ-02 @/issue/ae03bcd0/requirement/REQ-12`, and the invariants `@/invariant/present-tense-prose`, `@/invariant/single-source-prose`, `@/invariant/no-deferred-defects`, `@/invariant/semantic-test-assertions`, `@/invariant/shared-test-contracts`. Read `AGENTS.md` in full.

## The spec: obligation O-4 of the approved proof sketch

Read `dev/active/64fd3afd/proof-sketch.md` sections "The extraction surface decides every binding", "Obligation map", "What model plus refinement means here", **all of "O-4 — Generator base-field membership and root correctness"** (production path, lemma statements L4.1–L4.8, binding table, proof strategy, assumptions A-01/A-07/A-09/A-10/A-04/A-05), and the assumptions register rows A-07, A-09, A-10 plus risk R-02. The sketch is approved; implement it as written. Deviations are allowed only where a stated line number has drifted or a Mathlib name differs; record every such deviation in the module header and in your report. Do not weaken a lemma statement to make it provable; if a statement is wrong as written, stop and report with the counterexample.

Lead rulings that bind this issue (R-27, R-43, progress.json):

1. **Footprint:** exactly one new module `proofs/Gf2Core/Proofs/BchGenerator.lean` (the name the sketch's obligation map fixes), one added `import Gf2Core.Proofs.BchGenerator` line in `proofs/Gf2Core.lean` after `CyclotomicClosure`, the required Rust test below in `crates/gf2-coding/src/bch/spec.rs`'s own `#[cfg(test)] mod tests`, the proof-inventory rows (ruling 5), and the sketch's O-4 section edits (ruling 6). Nothing else.
2. **Required new refinement anchor** (sketch O-4 §3, the only lemma without an existing anchor): `the_generator_vanishes_at_every_defining_set_root` — for each of the binary, GF(5), and GF(9)-base codes the suite already builds (helpers `binary_narrow_sense`, `gf25`, `gf81_over_gf9` in `spec.rs`'s test module; locate them with `grep -n "fn binary_narrow_sense\|fn gf25\|fn gf81_over_gf9" crates/gf2-coding/src/bch/spec.rs` — the sketch's `:999/:1017/:1023` lines have drifted), lift the generator's base coefficients into $E$ with `FieldExtension::embed`, build the `FieldPoly<X::Ext>`, and assert `eval(root.pow(j)).is_zero()` for every $j$ in `code.defining_set()`; assert the contrapositive on at least one exponent outside the defining set so the test distinguishes the generator from the zero polynomial. Make it a shared generic helper applied to all three codes (`@/invariant/shared-test-contracts`), and make it load-bearing: mutate one generator coefficient (or one defining-set exponent) locally, watch it fail, revert, and say so in your report.
3. **Style model:** `proofs/Gf2Core/Proofs/CyclotomicClosure.lean` (77 declarations, landed by d7749931 — read its header for the deviation-recording style), `QuotientReduction.lean`, `RelativeExtension.lean` — a header stating the obligation, the binding mode, the refinement anchors in **present tense** with `path:line` at HEAD, the axiom footprint, and any deviation; sections mirroring the sketch's lemma numbering L4.1–L4.8; every model lemma's docstring names its refinement anchor (function name + `path:line`); no `sorry`, no new `axiom`. Binding mode is **abstract model plus refinement with no extraction anchor** (`gf2-coding` appears in no Charon invocation), as the sketch states. Reuse the O-1 model's definitions (`ExtDefs.lean`, `RelativeExtension.lean`: the embedding $\iota$, relative Frobenius $\varphi_B$, conjugate orbits, L1.3/L1.5/L1.8) and O-3's closure (`CyclotomicClosure.lean`: L3.2, L3.5) rather than restating them; import those modules.
4. **Verification you must run and paste:** `(cd proofs && lake build)` clean; `./scripts/lake-build-strict.sh` exit 0; an axiom audit — `#print axioms` for every theorem in the module, confirming nothing beyond `propext`, `Classical.choice`, `Quot.sound`; the Rust anchor in the fast tier: `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --lib --cargo-profile ci-test --profile ci -E 'test(spec)'` (or the narrowest expression that runs the new test plus its module); `cargo fmt --all -- --check`; `./scripts/cargo-budget.sh cargo clippy -p gf2-coding --all-targets --all-features -- -D warnings`. If `dev/tools/tuning-profile-compose/Cargo.lock` is missing, copy it from `/home/vkaskivuo/Projects/gf2/dev/tools/tuning-profile-compose/Cargo.lock` (that one copy is allowed).
5. **Doc impact — inventories:** `proofs/README.md` (table near line 41) and `docs/lean4-verification-pipeline.md` (table near line 334) list every module; add the `BchGenerator.lean` row in the same shape as the `CyclotomicClosure.lean` row. Present tense; no "new"/"added"/"now" narration; no dates; no root-absolute repository paths in Markdown links.
6. **Doc impact — the sketch's own O-4 section (R-43, mandatory).** The previous Lean issue failed doc-review twice because the approved sketch kept saying its anchor test "must be added" and cited line numbers its own test insertion had shifted. In the same submission, edit `dev/active/64fd3afd/proof-sketch.md`'s O-4 section so that at HEAD: the L4.3 row of the binding table names `the_generator_vanishes_at_every_defining_set_root` with its `path:line`; the "Test the Lean issue must add" paragraph becomes an "Anchor test for L4.3" paragraph in present tense citing the landed test; the L4.4 row no longer calls it "the new L4.3 test"; assumption A-01's "the new root-vanishing test" loses "new"; the R-03 register row's "O-4 adds …" becomes present tense with the test's `path:line` (compare how the O-2/O-3 clauses of R-03 read); and every `crates/gf2-coding/src/bch/spec.rs:LINE` citation in the O-4 production-path table and anchor table is verified against HEAD and remapped where it drifted (the test module has moved by several hundred lines since the sketch was written; use `grep -n` for each named item). Leave the O-5 section untouched. The lead refreshes the sketch's tracker link after merge.
7. **Known, out-of-scope gaps for awareness only:** (a) `proofs/Gf2Core/Proofs/FpField.lean` proves no link between `FpVal.mul'/add'` and `instCommRing`'s operations (recorded pitfall from 32f53280); O-4 has no extraction anchor, so it does not depend on that link; do not touch `FpField.lean`. (b) The BCH minimum-distance bound (Vandermonde) is out of scope by sketch A-09/R-02 and tracked as issue 501cf341; L4.7/L4.8 characterise the witnessed run only.
8. **Expected hard steps** (sketch §4): L4.5's separability-and-descent argument and L4.6's coprimality of distinct coset minimal polynomials. Order the work L4.4, L4.1, L4.2, L4.3, L4.5, L4.6, L4.7, L4.8 as the sketch prescribes. Model `witness_longest_run` (`spec.rs`, locate at HEAD) faithfully enough for L4.7's docstring to cite its exact lines, including the $|T| = n$ and $T = \emptyset$ base cases.

## Process

- Work test-first on the Rust side: write `the_generator_vanishes_at_every_defining_set_root`, run it, then the Lean module.
- Commit early and often with conventional subjects under 72 chars: `feat(jit:b1bd75ca): …` (Lean model), `test(jit:b1bd75ca): …` (Rust anchor), `docs(jit:b1bd75ca): …` (inventories/header/sketch). Never `commit -am`; stage explicitly.
- Before reporting, run `git diff --stat main...HEAD` and confirm the footprint is exactly the items of ruling 1.
- Run `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` from the worktree root at the end and include its output.

## Return

Report to the lead with SendMessage (to: "team-lead"), messages under 2500 characters each. The first carries: branch tip SHA; declaration count and `#print axioms` summary; the verification command outputs (lake strict exit, nextest summary line, fmt/clippy); the mutation check on the anchor test; every deviation from the sketch with the reason; anything left unverified. Details (lemma-to-anchor table, the sketch citation remap list) in a second message. Finish with the same content as your final response.
