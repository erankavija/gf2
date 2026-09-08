# Protocol v3 bounded-convergence validation

This record describes the active v3 implementation. Historical v1/v2 records
remain evidence of their own pinned semantics.

## Behavioral coverage

| Finding | Current validation |
|---|---|
| F13 | Exact equality and one-percent-above cases cover worthwhile, equivalence, and material-gap margins. |
| F14 | Direct-alpha bootstrap coverage uses 10,000 draws at family sizes 1, 2, 5, 10, and 25, with a decision-boundary case. |
| F15/F8 | Exact factor equality flags; stable 6x-separated and larger arms have no cross-arm flags. |
| F16 | The protocol guard compares value plus semantic justification and derives all P rules from evaluator source. |
| F17 | A v1 `../` prior receipt path rejects through the existing repository-relative helper. |
| F18 | Under-declared pilot resolution and a pilot from another family reject. |
| F19 | V3 lock acceptance requires observed descriptor/conflict facts. |

`./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release --test protocol_contracts` passes 40 of 40 tests.

## Receipt evidence

- `dev/bench_results/f547c394/v3-pilot/receipt.json`: acceptance is `Accepted`, `qualifies=false`, findings `0`.
- `dev/bench_results/f547c394/v3-confirmation/receipt.json`: acceptance is `Accepted`, `qualifies=false`, findings `0`.

Both receipts use fresh v3 samples and the confirmation addendum binds the
pilot digest. The preservation check in
`research-r4-v1-v2-preservation.json` reports 205 unchanged tracked v1/v2
evidence paths against `c01be44e`.

## Scope and deferred-items audit

The named session lifecycle-store exception in `design.md` retains its stated
convergence condition. The separate a835 producing-input fragility and family
surveys remain outside this bounded protocol change. No new deferred defect or
framework layer is introduced.
