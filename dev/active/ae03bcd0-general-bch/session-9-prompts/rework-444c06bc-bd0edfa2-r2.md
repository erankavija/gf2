# Rework Required — 444c06bc (attempt 2 of 2) and bd0edfa2 (attempt 2 of 2)

You are dispatched to JIT issues 444c06bc (Canonical BCH matrices use the systematic user layout) and bd0edfa2 (Optimize the reference generator-matrix materialization). Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-bd0edfa2
on branch worktree-agent-bd0edfa2. Rework 1 of both issues was merged to main at 8754afc8 and gated there: 444c06bc cargo-ci PASSED, doc-review R2 PASSED, code-review R1 **FAILED** (one finding, below); bd0edfa2 cargo-ci PASSED, code-review R2 result appended at the end of this file. **First command:** `git merge main` (main is at 686830ae or later; expect no conflicts).

Hard rules (worktree-dispatch-protocol):
- Run every shell command from your worktree root; use only paths relative to it in tool calls. Never `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your branch only; do not push.
- Never write `.jit/`, never `jit doc add`, never change issue state. The lead links documents after merge.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says.
- Keep build caches inside the worktree, never on tmpfs (/tmp is tmpfs). If `cargo` reports a stale rlib as fresh after the merge, `find crates/gf2-coding/src -name '*.rs' -exec touch {} +`.

Fix **only** the listed items plus what the mandatory sweeps surface. Do not change the recurrence, the benchmark, the receipt, or the survey amendment.

## Required pre-commit audit (paste raw output at the top of your report)

```bash
jit gate status 444c06bc code-review --all; jit gate status bd0edfa2 code-review --all
python3 -c 'import json,glob
for path in glob.glob(".jit/gate-runs/*/result.json"):
    r=json.load(open(path))
    if r.get("status")=="failed" and r.get("issue_id","")[:8] in ("bd0edfa2","444c06bc") and r.get("gate_key") in ("code-review","doc-review"):
        for f in (r.get("findings") or {}).get("findings",[]):
            print(r["issue_id"][:8], r["gate_key"], "[{}] {}: {} ({}:{})".format(f.get("severity"),f.get("id"),f.get("summary"),f.get("file"),f.get("line")))'
jit doc list 444c06bc; jit doc list bd0edfa2
```

Resolve before acting: `jit item show @/issue/444c06bc/requirement/REQ-01 @/issue/444c06bc/requirement/REQ-02 @/issue/444c06bc/requirement/REQ-03 @/issue/bd0edfa2/requirement/REQ-01 @/issue/bd0edfa2/requirement/REQ-02 @/issue/ae03bcd0/requirement/REQ-06 @/issue/ae03bcd0/requirement/REQ-08`, invariants `@/invariant/convention-convergence @/invariant/shared-test-contracts @/invariant/library-first-generality @/invariant/present-tense-prose @/invariant/single-source-prose @/invariant/no-deferred-defects @/invariant/canonical-bit-indexing`.

## Review verdict

### 444c06bc — code-review R1 F1 (high, blocking, issue-impact, `@/issue/ae03bcd0/requirement/REQ-06`)

Reviewer, verbatim: "The new private `MatrixFill` bound removes `GeneratorMatrixAccess` and `ParityCheckMatrixAccess` from publicly constructible `BchCode<X, S, M>` values using any external `SymbolMatrix` implementation. `BchCode` remains publicly generic over `M`, but downstream representations cannot implement the private bound. This regresses the generic static-representation contract." (`crates/gf2-coding/src/bch/matrix.rs:459`.)

The trait was introduced by bd0edfa2's recurrence commit (68730015) and the reviewer read it through the shared merge, so both issues own the fix.

**Lead ruling R-45 (binding; no-argue):** the materialization contract becomes a **public, documented trait** with **generic default method bodies**, so every `SymbolMatrix` representation can opt in with an empty impl and the two canonical representations keep their overrides:

1. `pub trait MatrixFill<F>: SymbolMatrix<F>` (keep the name unless a clearer one such as `SystematicMatrixFill` reads better; one name, used consistently) with `fill_generator` and `fill_parity_check` **provided** (default) bodies written only through `SymbolMatrix::{rows, cols, get, set}` and `FieldIdentity` arithmetic — the same recurrence the field-generic impl runs, coordinate by coordinate. The trait doc states the contract exactly as the private doc did (caller has checked the shape; the impl writes only in-range coordinates, overwrites every coordinate, cannot fail; `dimension` is $k$), states that the provided bodies are correct for any representation and that overriding is a performance choice, and states the layout written ($G = [I_k \mid P]$, $H = [-P^{\mathsf T} \mid I_{n-k}]$).
2. `impl MatrixFill<Fp<2>> for BitMatrix` (packed word-level path) and `impl<F> MatrixFill<F> for FieldMatrix<F>` (row-slice path) override the defaults exactly as now; their behavior does not change.
3. Export the trait where the canonical matrix traits live for consumers: re-export it from `crates/gf2-coding/src/bch/mod.rs` next to `CachedMatrices`, and mention it in the crate README's matrix paragraph if that paragraph enumerates the matrix traits (check; do not add a paragraph otherwise).
4. Rewrite the module doc's "Representation" section (`crates/gf2-coding/src/bch/matrix.rs` around lines 42–49): it currently says materialization dispatches "through a private trait" and argues against "a second public matrix hierarchy". State the current design instead: `SymbolMatrix` is the storage contract, `MatrixFill` is the materialization contract with provided generic bodies, the two canonical representations override it with word-level and row-slice paths, and any other representation opts in with an empty impl. Present tense, no history.
5. Amend `dev/active/ae03bcd0-general-bch/bch-api-design.md` "Static code types" / the traits subsection (near lines 430–437 and 596–610, "These traits describe storage behavior only… The canonical implementations are: …") so it names the materialization contract: `SymbolMatrix` describes storage; `MatrixFill` describes canonical-matrix materialization over that storage with provided generic bodies and representation overrides; the packed override is the specialization the packed-binary decision names. Present tense; no dates; no root-absolute links. The lead refreshes the design's tracker links after merge.
6. **Tests (`@/invariant/shared-test-contracts`):** add a test-only representation that stands in for an out-of-tree one — a small `Vec<Vec<F>>`-backed (or similar) `SymbolMatrix<F>` implementor in `matrix.rs`'s test module — with the **empty** `impl<F> MatrixFill<F> for TestMatrix<F> {}`, and run it through `assert_matrix_contract` (adjust that helper's bounds so the trait's provided bodies satisfy them) on the same GF(2), GF(5), and GF(9)-base fixtures the module already uses, including the full-space and zero-dimensional boundary codes; assert its generator and parity-check equal the canonical `FieldMatrix` results cell by cell, and for GF(2) equal the packed `BitMatrix` results through `get`. Also assert the provided body and the `FieldMatrix` override agree on B1–B3 (call the provided bodies directly through a type whose impl is empty). Test names describe the property, not the mechanism.
7. Keep every existing test passing unchanged; the recurrence, the sampled T2S witness, the slow-tier witnesses, the bench, the receipt, and the survey amendment are out of scope for this round.

### bd0edfa2 — code-review R2

See the appended section.

### Stale-narrative sweep (Tier 2.5, lead)

`git grep -n "private trait\|MatrixFill\|second public matrix hierarchy" -- crates dev/active/ae03bcd0-general-bch` — every match describes the state after your work.

### Deferred-items audit (Tier 2.75, lead)

Linked docs: none on 444c06bc; bd0edfa2's receipt has no deferred markers. Keep it so.

## Mandatory sweeps (paste raw results)

1. Prior findings cumulative (audit block above): one resolution-table row per finding across both issues and all rounds (444c06bc: cargo-ci timeout, doc-review R1 F1, code-review R1 F1; bd0edfa2: code-review R1 F1–F3, R2 findings if any).
2. Deferred-item grep over every touched file: `grep -inE '\b(deferred|todo|future work|open question|not (yet )?implemented|follow-?up|out of scope|punt(ed)?)\b' <files>`.
3. Per-finding grep: `rg -n "private trait|cannot implement|MatrixFill" crates/gf2-coding dev/active/ae03bcd0-general-bch/bch-api-design.md crates/gf2-coding/README.md`.
4. Narration grep over touched prose: `rg -n "previously|no longer|now (that|uses|exposes)|legacy|before-and-after" <touched .rs/.md files>` — fix every match except quoted reviewer text in commit bodies.

## Verification before you report (paste outputs)

- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --lib --all-features --cargo-profile ci-test --profile ci -E 'test(matrix)'` with wall times.
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table (copy `dev/tools/tuning-profile-compose/Cargo.lock` from the main checkout if missing).
- `cargo doc -p gf2-coding --all-features --no-deps` builds without warnings on the touched items.
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Resolution table (required)

| # | Issue | Round | Source | Finding (verbatim) | Resolution (file:line or SHA at HEAD) |
|---|-------|-------|--------|--------------------|---------------------------------------|

One row per finding across all rounds of both issues plus every sweep match.

## Commits

Tag the trait change so both gates see it: `fix(jit:444c06bc, jit:bd0edfa2): make the matrix materialization contract public` (subject under 72 chars), tests as `test(jit:444c06bc, jit:bd0edfa2): …`, docs as `docs(jit:444c06bc, jit:bd0edfa2): …`. Stage explicitly; never `commit -am`.

## Report

Reply to the lead with SendMessage (to: "team-lead") in messages under 2500 characters each; the first carries: branch tip SHA, the trait's public signature (one line), the resolution table, the cargo-ci step table, and anything not closed with the reason. Details in a second message. Finish with the same content as your final response.

## Appended: bd0edfa2 — code-review R2 (2026-09-02 16:47Z)

**PASSED** with no findings on the merged tree at 8754afc8. Nothing further is required for bd0edfa2's own criteria; the trait change above is tagged with both issues because the reviewer reads it through the shared merge, and the lead re-evaluates bd0edfa2's code-review after your merge so the passed verdict covers the public contract too.
