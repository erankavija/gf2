# Epic e095a100 handoff — 2026-04-18

Lead: agent:project-lead (session 1). Next lead: pick up from Wave 2.

## Epic state

- **Epic e095a100**: "Implement general Galois field GF(p^m) arithmetic" — **in_progress**.
- Gates on epic: `code-review`, `doc-review`, `cargo-ci`, `lake-build` (lake-build added this session).
- Deferred HW-gated item: `b59fa661` (AVX-512 IFMA — absent on this Zen 3 host); dependency edge from epic was removed this session.

## Wave 1 complete (all 3 issues done, gates passed)

| Issue | State | Deliverable |
|---|---|---|
| `0a7e2555` | done | RNS research doc + 483-line standalone prototype with u256 reconstruction and 3 × 256-case proptests. Recommendation: defer production integration. |
| `c488ed29` | done | `Gf2mElement_<u128>` support for m = 64..=127; Seroussi+FIPS 186-4 polynomial catalog (irreducible-verified); Barrett m≤63 contract-pinned (extension deferred to `6fb4abad`). |
| `1f2f8371` | done | Mersenne/Proth/Goldilocks compile-time specialised `Fp<P>`; AVX2 batch Mersenne31 kernel in `gf2-kernels-simd` (measured 4.0×–4.4× scalar Montgomery, 14.22× in some configs); Goldilocks scalar 1.99×. |

## Approved scope deviations (documented in issue descriptions)

- **1f2f8371**: Scalar Mersenne31 ≥2× target not reachable on modern x86 (REDC pipelines ~4 muls). Target moved to AVX2 batch path; Proth reduction uses compiler-strength-reduced `%` (benchmarked at parity with hand-rolled shift/subtract on x86-64).
- **c488ed29**: BarrettReducer and PCLMULQDQ extension to m≥64 deferred to `6fb4abad` (requires 256-bit polynomial arithmetic). Primitive-polynomial verification for m≥64 deferred; `standard_u128` returns irreducible-only polynomials with explicit contract wording in `standard_u128_irreducibility_note()`.

## Local commits ahead of origin/main (9 commits)

```
34fe1ed docs(jit:c488ed29): fix Gf2mField_ struct + new() + primitive_polynomial() docs
f1ca433 docs(jit:c488ed29): clarify primitive vs irreducible contract in gf2m/field.rs
7b35236 docs(jit:c488ed29): clarify remaining GF(2^64) polynomial comments as irreducible
89c737a docs(jit:c488ed29): clarify GF(2^64) polynomial as irreducible, not primitive
83a1c5b docs(jit:1f2f8371): add missing SAFETY comments on M31 batch wrappers
5a7f047 fix(jit:1f2f8371,c488ed29): gate SIMD bench + widen standard_u128 fallback
f9b9126 feat(jit:1f2f8371): Mersenne/Proth/Goldilocks Fp + AVX2 M31 batch kernel
53f9e20 feat(jit:c488ed29): support u128 Gf2mElement for GF(2^64..=127)
88f05ee feat(jit:0a7e2555): RNS research with honest u256 reconstruction
316cca7 chore(jit): extend code-review gate timeout to 1800s
```

**Baseline gates on the integrated state**: fmt clean, clippy clean, 2899 tests pass.

**Open question for next lead**: push to `origin/main`? The code-review gate diffs vs `origin/main`; pushing would ensure future Wave 2 reviews see only Wave 2 diffs instead of complaining about mixed-scope branches. User did not authorise a push in this session.

## Wave 2 plan (ready to dispatch)

### Wave 2a — 3 parallel agents, disjoint directories

All three are `ready` (assignee cleared at end of this session).

1. **`72a2118a`** — Batch inversion using Montgomery's trick. New file `crates/gf2-core/src/field/batch_ops.rs`. Small (~1–2 days). Generic over `FiniteField`. Success: 1 inv + 3(N-1) muls; ≥5× vs individual inversions for N=100, Fp<65537>.
2. **`d11b769a`** — Wide accumulator integration for tower types. Modifies `crates/gf2-core/src/gfpn/{quadratic.rs,cubic.rs,mod.rs}`. Design plan at `dev/plans/wide_accumulator_tower.md`. Replaces `type Wide = Self` placeholder with proper `QuadraticExtWide<W>` / `CubicExtWide<W>`. Approved approach: Option 1 (practical — mul_to_wide does full Karatsuba then to_wide; accumulation is the lazy win).
3. **`2ce2a757`** — Karatsuba vs naive cross-verification. New file `crates/gf2-core/tests/karatsuba_cross_verify.rs`. Design plan at `dev/plans/karatsuba_cross_verification.md`. 10000 proptest cases per configuration across 4 QuadraticExt + 3 CubicExt configs. No library code changes.

**Conflict note**: 72a2118a and `bdf95060` (Wave 2b below) both touch `field/mod.rs` (add module declarations). Serialise or wait for 72a2118a commit before dispatching bdf95060.

### Wave 2b — after 2a

4. **`bdf95060`** — Batch polynomial operations for extension fields. Story (~1–2 weeks), `crates/gf2-core/src/field/poly.rs` (new). FieldPoly<F>, Horner, batch evaluate via subproduct tree, interpolation, NTT for NTT-friendly primes.
5. **`86b3dc7d`** — Verify nested tower compositions (GF(p^4), GF(p^6), GF(p^12)). Depends on d11b769a (Wide) being stable. Axiom tests for nested extensions + cross-verify vs SageMath if feasible.

### Wave 3 — story close-outs & SoA

6. **`3f4b946c`** — Close the tower extension field story after d11b769a, 86b3dc7d, 2ce2a757 are done. Verify all success criteria met.
7. **`5fad4d0f`** — SoA batch layout for GF(p^n) SIMD. Blocked on 3f4b946c being done. New file `crates/gf2-core/src/gfpn/batch.rs`. Success: SoA batch mul ≥3× scalar for 1000 GF(p^2) elements.
8. **`6fb4abad`** — Multi-word GF(2^m) for m > 128 (story). Biggest remaining item. Uses c488ed29's u128 as stepping stone. New file `crates/gf2-core/src/gf2m/wide.rs`. Success: `Gf2mWide<4>` (GF(2^256)) passes axiom harness; within 2× of handwritten SIMD. Story needs breakdown when reached.

## Lessons learned (saved to memory)

1. **`Bash run_in_background, not & disown`** (`feedback_bash_background.md`): Use the Bash tool's `run_in_background: true` parameter so the harness tracks the process and fires a `task-notification` on completion. Inline `& disown` doesn't register with the tool.

## Infrastructure observations

- `code-review` gate timeout extended from 600s → 1800s in `.jit/gates.json` this session (copilot with full shell-tool access exceeds 10 min on non-trivial diffs).
- `code-review` gate uses `git diff origin/main..HEAD` scope. Until local commits are pushed to origin, each review sees all unpushed commits; this causes "branch scope mixing" FAIL verdicts with LLM variance (some runs PASS with judgment, others FAIL). Pushing to origin after each wave is the cleanest fix.
- LLM variance in the reviewer is significant — several issues required 2–4 review retries with no code changes before a PASS verdict was produced. When all substantive concerns are resolved and the only objection is cross-commit scope, retry once or twice before intervening.

## Hardware / environment

- Host: AMD Ryzen 9 5900X (Zen 3). Has AVX2, PCLMUL, VPCLMULQDQ, VAES, SHA-NI; **no AVX-512**.
- `gf2-kernels-hip` excluded from default workspace (no hipcc in CI path).
- Rust MSRV 1.80, cargo workspace with `gf2-core`, `gf2-coding`, `gf2-kernels-simd`, `gf2-kernels-hip`.
- `#![deny(unsafe_code)]` in gf2-core and gf2-coding; unsafe only in the two kernel crates.

## Continuation pointers

- To resume: claim Wave 2a issues, follow the project-lead skill. The design plans for d11b769a and 2ce2a757 already exist at `dev/plans/`.
- Strategic decisions made this session (recorded in issue descriptions): scope deviations on 1f2f8371 and c488ed29 approved by user 2026-04-18.
- Dispatches in the session used the Agent tool with `subagent_type: general-purpose`. 1f2f8371 required one rework cycle after initial (for SIMD path) and one polish cycle (safety comments, bench feature-gating). c488ed29 required one polish cycle (docstrings). 0a7e2555 was single-shot.
