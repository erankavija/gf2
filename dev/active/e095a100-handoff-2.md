# Epic e095a100 handoff — 2026-04-19 (session 2)

Lead: agent:project-lead (session 2). Next lead: resume from the
**SSOT override decision** below.

## Durable policy decision (session-spanning)

**SSOT is inviolable.** The `FieldPoly<F>` / `Gf2mPoly_<V>` parallel-
implementation loophole granted by `dev/plans/bdf95060_breakdown.md`
"Out of scope / deferred" (lines 19–20, 240) is **rescinded** as of
2026-04-19. The reviewer was correct to fail on the SSOT finding; the
breakdown's scope carveout does not override the project-wide SSOT
invariant.

Consequence: **70972f06 cannot close on its current implementation.**
The `FieldPoly<F>` work stays in the tree, but next session must
collapse the duplication by rewriting `Gf2mPoly_<V>` to delegate to
`FieldPoly<Gf2mElement_<V>>`, or by moving the missing bits from
`Gf2mPoly_` into `FieldPoly` and retiring `Gf2mPoly_`.

Scope of that delegation work (previously "out of scope" per
breakdown, now in scope):

- Audit every `gf2-coding::bch` call into `Gf2mPoly_` — `from_roots`,
  `generator`, `gcd`, `minimal_polynomial`, `eval`, `eval_batch`,
  `div_rem`, schoolbook+Karatsuba `mul`, iteration.
- Decide per call-site: (a) delegate to a `FieldPoly<Gf2mElement>`
  method, (b) add the missing method to `FieldPoly` and delegate, or
  (c) retain as a thin `Gf2mPoly_` adapter over `FieldPoly`.
- Re-run all DVB-T2 / BCH / Reed-Solomon test vectors to confirm no
  numerical regression.
- Originally scoped as Task 8 (`224a7d9e`) plus a later-epic. Now
  merged into 70972f06's closing rework.

Estimated size: 1–3 days of work (multi-agent dispatches most likely).

## Session 2 summary

### Closed this session (DONE)

| Issue | Type | Title | Final commit(s) |
|---|---|---|---|
| 72a2118a | task | Batch inversion (Montgomery trick) | Wave 2a |
| d11b769a | task | Wide accumulator for tower types | Wave 2a |
| 2ce2a757 | task | Karatsuba vs naive cross-verify | Wave 2a |
| 86b3dc7d | task | Nested tower verification GF(p^4,6,12) | Wave 2b (after 2 reworks: SSOT + axiom harness) |
| 3f4b946c | story | Tower extension field architecture | Closed after 86b3dc7d + ExtConfig bound relaxation (2ad2f99, 68e0c7a, 9ac7e54) |
| 3e947c3f | task | TwoAdicField trait + Proth roots | bdf95060 Task 6 (after 2 reworks: docs + Proth SSOT) |
| 9fa99685 | task | Gf2mWide<N, Cfg> type shell + config trait | 6fb4abad Task 1 (after 2 reworks: proptests + boundary + M=1 + irreducible doctest) |

### Not done, blocked on your decisions

| Issue | State | What's next |
|---|---|---|
| **70972f06** | bdf95060 Task 1; code passes all concrete criteria (API, doctests, tests, clippy) but code-review FAILed on SSOT | **Per SSOT override above**: session 3 must expand this to include the `Gf2mPoly_ → FieldPoly<Gf2mElement>` delegation (subsumes Task 8 `224a7d9e`). Reopen the issue scope; rewrite; re-review. |
| **5fad4d0f** | epic child; SSOT SoA layout landed, benchmark records 0.92× vs the 3× spec criterion | **Your scope call still pending** from session 2 escalation: (1) split — close 5fad4d0f on layout deliverable, create a new task for the required AVX2 Fp<65537> SIMD kernel, (2) drive the SIMD kernel now in-scope, or (3) reject with reason. Recommend (1). |

### Created but not started (wave 3b / wave 4)

**bdf95060 (batch polynomial operations) child tasks**:

- **70972f06** — Task 1: FieldPoly core type. **Open per SSOT override.**
- **d9c3d414** — Task 2: div_rem + gcd + Horner + Karatsuba. Blocked on Task 1.
- **2e7db385** — Task 3: batch_evaluate via subproduct tree. Blocked on Task 2.
- **3cff65f7** — Task 4: Lagrange interpolation. Blocked on Task 3.
- **a7c81834** — Task 5: batch_mul + batch_gcd. Blocked on Task 2.
- **3e947c3f** — Task 6: TwoAdicField trait. **DONE.**
- **e0b6f940** — Task 7: NTT-based multiplication. Blocked on Tasks 2 and 6.
- **224a7d9e** — Task 8: integration docs + bench consolidation. Previously the `Gf2mPoly_` conversion helper; per the SSOT override this may merge into 70972f06's rework.

**6fb4abad (multi-word GF(2^m) for m > 128) child tasks**:

- **9fa99685** — Task 1: Gf2mWide type shell. **DONE.**
- **e1fbf2d4** — Task 2: clmul_wide carry-less multiplication. Blocked on Task 1 (done).
- **9dd11973** — Task 3: BarrettReducerWide scalar reducer. Blocked on Tasks 1, 2.
- **b77768f0** — Task 4: Mul/Inv/FiniteField+ConstField. Blocked on Tasks 2, 3.
- **a1229d72** — Task 5: axiom harness for Gf2mWide<4>. Blocked on Task 4.
- **afac2262** — Task 6: VPCLMULQDQ SIMD kernel (opt-in). Blocked on Tasks 4, 5.
- **d013cfdf** — Task 7: Karatsuba N=4 (conditional on bench). Blocked on Task 6.

### Rework cycles this session (telemetry)

| Issue | Initial | Rework #1 | Rework #2 | Rework #3 | Final |
|---|---|---|---|---|---|
| 86b3dc7d | d5d7d26 | c57eaac+0f64678+2442cd3 | d6be562+7ba297a | — | DONE |
| 3e947c3f | d1b9e58 | 9fd9802 | 3c15f8e+cdeff68 | — | DONE |
| 70972f06 | 6c4ac0d | 5df084a | eb3cedc+9d3531f | 4ebae91 | **FAILED, reopening** |
| 9fa99685 | b332ed5 | 35079d1 | 18e504e | — | DONE |
| 5fad4d0f | a6cca7b | d3938cb (inline) | — | — | **3× target not met** |

Rate-limit impact: 4 sub-agent dispatches hit Anthropic's usage limit
mid-session; all recovered via inline rework (commits eb3cedc,
cdeff68, 35079d1, d3938cb, 4ebae91, 18e504e). Limit resets 6am
Europe/Helsinki.

## Infrastructure observations

- `code-review` gate behaviour:
  - The reviewer sometimes runs cargo during an unrelated race
    condition (another concurrent gate holding the build lock); that
    produces false failures on cargo-ci. If cargo-ci fails with
    "mismatched types" errors referencing another issue's files,
    re-run after the concurrent work commits.
  - Secondary independent reviewers occasionally contradict the
    primary verdict (see 70972f06 rework #3: primary "PASS with
    observation", primary verdict stamp "FAIL"). Verdicts are
    authoritative; the `ERROR: Could not extract VERDICT` message in
    stderr indicates the extractor couldn't disambiguate and treated
    as failure.
- Keep commit authorship clean: every code commit includes
  `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`.
- The `scripts/code-review-prompt.md` calibration (commit-ordering is
  not a review-failure signal) from session 1 is still active and
  helpful.

## Hardware / environment

- Host: AMD Ryzen 9 5900X (Zen 3). Has AVX2, PCLMUL, VPCLMULQDQ,
  VAES, SHA-NI; **no AVX-512**.
- Rust MSRV 1.80. `#![deny(unsafe_code)]` in `gf2-core` and `gf2-coding`.

## Branch state at handoff

- Branch: `main`, **39 commits ahead of origin/main**.
- Working tree: only `.jit/` metadata modified (no source drift).
- Full workspace tests at head: **3189 passed, 0 failed, 71 ignored**.
- fmt + clippy: clean.

## Continuation pointers for session 3

1. **Respond to the 5fad4d0f scope question** (split / drive SIMD /
   reject).
2. **Reopen 70972f06** scope to include Gf2mPoly_ delegation per SSOT
   override. Task 8 (`224a7d9e`) likely folds in here.
3. Once 70972f06 closes, dispatch **bdf95060 Wave 3b**: d9c3d414 +
   a7c81834 (Task 2 + Task 5, parallel after Task 2 → d9c3d414 lands).
4. Dispatch **6fb4abad Wave 4b**: e1fbf2d4 (Task 2, depends on
   9fa99685 which is DONE).
5. Waterfall through both DAGs to completion.
6. Close epic per Section 10 of the project-lead skill.
