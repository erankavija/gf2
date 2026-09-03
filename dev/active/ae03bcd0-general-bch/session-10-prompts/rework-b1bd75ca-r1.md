You are dispatched to JIT issue b1bd75ca (rework attempt 1). Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b1bd75ca
on branch worktree-agent-b1bd75ca, fast-forwarded to main at 0854ccd2 (your previous commits for this issue are already merged into main; `proofs/.lake` is warm from your earlier build).

Hard rules for path discipline (worktree-dispatch-protocol):
- Run every shell command from your worktree root. Prefix tool calls with `...` if your shell state has drifted. Use only paths relative to the worktree root in tool calls; checkout-specific absolute paths leak files into main's checkout.
- Never run `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your worktree branch only. Do not push.
- Keep every build/dependency cache inside the worktree (`proofs/.lake`, `target`). Never place a cache on tmpfs (`/tmp` is tmpfs on this host) or outside the worktree.
- Never write `.jit/`, never run `jit doc add`, never change issue state. The lead owns all tracker state.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says. This overrides the global delegation rule.
- Post-completion: run `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` from the worktree root and paste its output.
- Report to the lead with SendMessage (to: "team-lead") in messages under 2500 characters each, leading with the actionable items, then finish with the same content as your final response.

## Rework Required (attempt 1 of 2)

Previous work on issue **Lean proofs: generator base-field membership and root correctness** (b1bd75ca) failed the lead's quality review. The specific failures are listed below. Fix **only** the listed issues (plus anything surfaced by the mandatory pre-commit audit below). Do not refactor unrelated areas, do not change the overall approach unless the feedback specifically requires it.

### Required pre-commit audit (attempt 1 must include this)

**Before editing any source**, run the following and include the raw output at the top of your final report. The lead cross-references this output against your resolution table; any omission is a rejection.

```bash
# 1. Enumerate every prior code-review failure for this issue (run from the MAIN checkout's .jit; gate-runs are gitignored and absent from your worktree — read-only)
jit gate status b1bd75ca code-review --all
python3 -c 'import json,sys,glob
for path in glob.glob("/home/vkaskivuo/Projects/gf2/.jit/gate-runs/*/result.json"):
    record=json.load(open(path, encoding="utf-8"))
    if record.get("gate_key")=="code-review" and record.get("status")=="failed" and record.get("issue_id","").startswith("b1bd75ca"):
        for finding in (record.get("findings") or {}).get("findings",[]):
            print("[{}] {}: {} ({}:{})".format(finding.get("severity","-"),finding.get("id","-"),finding.get("summary",""),finding.get("file","-"),finding.get("line","-")))'

# 2. List every design doc linked to this issue (none are linked; the sketch is 64fd3afd's doc)
jit doc list b1bd75ca
jit doc list 64fd3afd

# 3. Grep the sketch's O-4 section and your module for deferred-item markers
grep -inE '\b(deferred|todo|future work|open question|not (yet )?implemented|follow-?up|out of scope|won'\''t (do|fix)|stays (available|open).*generic|punt(ed)?)\b' dev/active/64fd3afd/proof-sketch.md proofs/Gf2Core/Proofs/BchGenerator.lean
```

**For every match** from steps 1 and 3, a row MUST appear in the resolution table below. Step 1 gives you every finding that has ever been raised; step 3 gives deferred-item notes, which are silent scope gaps the reviewer will catch. A sketch match that the sketch itself scopes out by A-09/R-02 (the Vandermonde bound, tracked as 501cf341) is OUT-OF-SCOPE and is recorded as such.

### Review verdict

## Review: Lean proofs: generator base-field membership and root correctness (b1bd75ca)

**Verdict:** FAIL

### Gate status
lake-build passed (2026-09-02T17:23Z); doc-review passed (17:30Z); **code-review FAILED** (17:27Z, run 952fa884, one blocking finding).

### Success criteria
- [x] REQ-01: 84 declarations, 0 sorry, axiom audit clean, lake-build strict exit 0.
- [ ] REQ-02: the obligation must bind to the construction production path exactly as the sketch assigns. The witnessed-run model does not refine `witness_longest_run` on the full-defining-set branch (finding F1 below), so L4.7 and L4.8 are not refinement evidence for that branch.

### Reviewer finding (verbatim)
**[F1, high, blocking, issue-impact]** The Lean “witnessed run” model does not refine the valid full-defining-set production branch. `run_of_univ` (proofs/Gf2Core/Proofs/BchGenerator.lean:909) proves `bestRun T = 0` because its scan has no run starts, and `reportedBound` (proofs/Gf2Core/Proofs/BchGenerator.lean:936) derives a bound of `1`. Production instead handles this case before scanning and returns a run of `length` with bound `length + 1` (crates/gf2-coding/src/bch/spec.rs:1541); its boundary test confirms `15` and `16` respectively (crates/gf2-coding/src/bch/spec.rs:2673). Model the explicit full-set branch and connect it to the reported bound/radius before claiming L4.7/L4.8 refinement. References: `@/issue/b1bd75ca/requirement/REQ-02`, `@/issue/ae03bcd0/requirement/REQ-12`.

### Lead verification of the finding
Confirmed by reading both sides. `witness_longest_run` (spec.rs:1533-1573) has three branches: empty set → (None, 0, bound 1); `defining_set.len() == length` → (Some(0), run = length, bound = length + 1); otherwise the scan. `correction_radius = consecutive_root_count / 2` (spec.rs:1514). The module's `reportedBound T := bestRun T + 1` and `correctionRadius T := bestRun T / 2` are defined from the scan's `bestRun` alone, so for `T = univ` they yield 1 and 0 while production yields n + 1 and n / 2 (the boundary test `a_full_defining_set_yields_the_zero_dimensional_code`, spec.rs:2673, asserts 15, 16, 7 for n = 15). The sketch's L4.7 text (proof-sketch.md:886-893 and 989-994) already names the $|T| = n$ and $T = \emptyset$ branches as separate base cases "because they have no run boundary", and states the reported bound is run + 1 and the radius ⌊run/2⌋ of the returned run — the returned run, which is n on the full set.

### Stale-narrative sweep (Tier 2.5)
No stale forward references to this module found.

### Deferred-items audit (Tier 2.75)
The sketch's O-4 section scopes out the Vandermonde minimum-distance step (A-09, R-02, tracked 501cf341): OUT-OF-SCOPE, unchanged. Nothing else deferred.

### Required changes
1. **Model production's returned witness, not only the scan.** In `proofs/Gf2Core/Proofs/BchGenerator.lean`, define the witness `witness_longest_run` returns as a function of `T` with production's three branches, e.g. `witnessedRun T : ℕ := if T = ∅ then 0 else if T.card = n then n else bestRun T` and the matching first root (`none` / `0` / `bestStart`), and define `reportedBound T := witnessedRun T + 1` and `correctionRadius T := witnessedRun T / 2` from it. Keep `bestRun`/`bestStart` as the scan's values. Location: the L4.7 section (`run_of_empty`, `run_of_univ`, `reportedBound`, `correctionRadius`, `two_mul_correctionRadius_lt_reportedBound`).
2. **`run_of_univ` states production's report.** For `∀ x, x ∈ T`: `witnessedRun T = n`, first root `0`, `reportedBound T = n + 1`, `correctionRadius T = n / 2`, every exponent `0 + i` (i < n) in `T`. Keep, as a separate fact, that the scan finds no run start on the full set (that is why production decides the case up front). Likewise `run_of_empty`: `witnessedRun T = 0`, `reportedBound T = 1`, first root `none`.
3. **L4.7 maximality and boundary lemmas hold for the witnessed run on every `T`.** The existing scan lemmas (run present, left and right boundaries when run < n, no longer cyclic run, least start among ties) must be restated or wrapped so that they are theorems about `witnessedRun`/the witnessed start for all `T`: for `T ≠ ∅` and `T ≠ univ` they coincide with the scan's (`witnessedRun T = bestRun T`), for `univ` the run is `n` with no exponent outside it, for `∅` the run is `0`. The sketch's L4.7 statement (proof-sketch.md:886-893) is the contract: every exponent start+i (i < run) in T; if run < n then start−1 ∉ T and start+run ∉ T; no cyclic run longer than run; ties choose the least start; bound = run + 1; radius = ⌊run/2⌋.
4. **L4.8 covers full-set codes.** `le_bestRun_of_consecutive` currently requires `hne : ∃ y ∉ T`. State the L4.8 theorems (consecutive seeds ⇒ `δ − 1 ≤ witnessedRun T` ⇒ `δ ≤ reportedBound T`) for all `T`, discharging the full-set case through change 2 (with `m ≤ n` the seeds cannot exceed the cycle), so the zero-dimensional boundary code (δ = n + 1, `T = univ`) is covered. Keep the non-full case's proof via the scan.
5. **Docstrings and anchors.** Every changed lemma's docstring names its refinement anchor with `path:line` at HEAD (re-grep; `spec.rs` lines have not moved since your first submission unless you touch that file). Add `a_full_defining_set_yields_the_zero_dimensional_code` (spec.rs:2673) as the refinement anchor for the `|T| = n` base case and `a_designed_distance_of_one_yields_the_full_space_code` (spec.rs:2659, asserting run 0, bound 1, radius 0 on the empty defining set) for the `T = ∅` base case, in the module header's anchor list and in the sketch's O-4 anchor table (proof-sketch.md:924, the L4.7 row) and binding table where the row cites lemma names that you rename. Present tense, no "new"/"now" narration.
6. **Verification to paste:** `(cd proofs && lake build)` clean; `./scripts/lake-build-strict.sh` exit 0; `#print axioms` for every theorem you changed or added (nothing beyond `propext`, `Classical.choice`, `Quot.sound`); declaration count; the Rust anchor tests `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --lib --cargo-profile ci-test --profile ci -E 'test(a_full_defining_set_yields_the_zero_dimensional_code) | test(the_generator_vanishes_at_every_defining_set_root) | test(witnessed_run)'` (adjust the expression to the actual names); `cargo fmt --all -- --check` if you touch Rust (you should not need to); the check-leak script output. Do not touch `FpField.lean`, `proofs/Gf2Core.lean` beyond what already exists, or the O-5 section of the sketch.

### Constraints
- Fix only the listed issues. Unrelated changes will be flagged in re-review.
- All gates on this issue (lake-build, code-review, doc-review) must still pass after your changes; doc-review passed on the previous submission, so keep the inventories (`proofs/README.md`, `docs/lean4-verification-pipeline.md`) accurate if the declaration count they state changes.
- Do not weaken a lemma statement to make it provable; if a statement is wrong as written, stop and report with the counterexample.
- If you believe a required change is incorrect or impossible, explain why clearly rather than silently ignoring it.
- When done, confirm which items you addressed and how.

### Resolution table (required — do not submit without completing)
Before submitting, produce one row per finding across all rounds plus every match from the pre-commit audit. Submissions with any empty row are rejected.

| # | Round | Source (reviewer / audit-step-1 / audit-step-3) | Finding (paste verbatim) | Resolution (file:line or commit SHA proving closure at HEAD) |
|---|-------|--------------------------------------------------|--------------------------|---------------------------------------------------------------|
| 1 | R1    | reviewer                                         | F1 (above)               | <file:line of witnessedRun, run_of_univ, reportedBound; sketch rows> |
| … |       |                                                  |                          |                                                               |

### Mandatory workspace-wide sweeps
**Sweep 1** — the pre-commit audit block above. **Sweep 2** — the audit step-3 grep. **Sweep 3** — per-finding greps, paste raw results and fix every match: `rg -n "bestRun|reportedBound|correctionRadius|run_of_univ|run_of_empty" proofs/ dev/active/64fd3afd/proof-sketch.md proofs/README.md docs/lean4-verification-pipeline.md` (every citation of a renamed or re-stated lemma must resolve at HEAD) and `rg -n "witness_longest_run|1534|1541|1547|2673" proofs/Gf2Core/Proofs/BchGenerator.lean dev/active/64fd3afd/proof-sketch.md` (every `spec.rs:LINE` must match `grep -n` at HEAD).

## Commits
Conventional subjects under 72 chars: `fix(jit:b1bd75ca): …` for the model, `docs(jit:b1bd75ca): …` for header/sketch/inventories. Stage explicitly; never `commit -am`. Run `git diff --stat main...HEAD` before reporting and confirm the footprint is the module, the sketch's O-4 section, and (only if counts changed) the two inventories.

## Original dispatch brief (for context; the sketch and rulings below still bind)


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
