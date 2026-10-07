# Protocol v3 r2 bounded-convergence validation

This record describes the active v3 r2 implementation and evidence. Historical
v1/v2 records retain their pinned semantics. V3 r1 remains immutable falsified
incomplete-provenance evidence because its producing closure does not identify
the launcher that recorded its sessions.

## Behavioral coverage

| Finding | Current validation |
|---|---|
| F13 | Exact equality and one-percent-above cases cover worthwhile, equivalence, and material-gap margins. |
| F14 | Direct-alpha bootstrap coverage uses 10,000 draws at family sizes 1, 2, 5, 10, and 25, with a decision-boundary case. A checkpoint-consistent altered v3 stored alpha rejects at P-20. |
| F15/F8 | Exact factor equality flags; stable 6x-separated and larger arms have no cross-arm flags. |
| F16 | The protocol guard compares value plus semantic justification and derives all P rules from evaluator source. |
| F17 | A v1 `../` prior receipt path rejects through the existing repository-relative helper. |
| F18 | Under-declared pilot resolution, cross-family pilot evidence, and a mutated pilot `claim.interval.alpha` reject. Pilot resolution recomputes with alpha derived from frozen pilot addendum and ledger snapshots. |
| F19 | V3 lock acceptance requires observed descriptor/conflict facts. |

`./scripts/cargo-budget.sh --test cargo nextest run --manifest-path dev/tools/tuning-campaign-support/Cargo.toml --release --test protocol_contracts` passes 41 of 41 tests.

## Receipt evidence

- `dev/bench_results/f547c394/v3-r2-pilot/receipt.json`: acceptance is `Accepted`, `qualifies=false`, findings `0`.
- `dev/bench_results/f547c394/v3-r2-confirmation/receipt.json`: acceptance is `Accepted`, `qualifies=false`, findings `0`.

The r2 confirmation addendum pins the r2 pilot receipt digest. Both receipts
freeze `run-smoke-v3.sh` in behavior, lifecycle, and build producing closures;
the confirmation additionally freezes the pilot addendum and trial-ledger
snapshots used to derive its corrected alpha. The preservation check in
`research-r5-v3-r1-preservation.json` reports 90 byte-identical r1 evidence
paths against `7756e1fd` and records the r1 provenance contradiction.

## Validation

The focused release suite passes 167 of 167 tests. Rust 1.95 checks the focused
crate with all targets and features. Formatting and strict focused clippy pass.
Independent acceptance evaluation reports `Accepted`, `qualifies=false`, and
zero findings for both r2 receipts.

## Scope and deferred-items audit

The named session lifecycle-store exception in `design.md` retains its stated
convergence condition. The separate a835 producing-input fragility and family
surveys remain outside this bounded protocol change. No new deferred defect or
framework layer is introduced.
