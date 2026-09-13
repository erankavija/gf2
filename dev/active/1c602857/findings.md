# Public wide carry-less product: dispatch routing and its receipt

> **Diátaxis Type:** Explanation

Issue `1c602857`. The public long-product API of `gf2-core` reaches the same
capability-dispatched kernels as the crate's own wide-field arithmetic, through
one dispatch shared by public and internal callers. No multiplication algorithm
changed: the kernels already existed, and the routing is what this issue adds.

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at
[version 4](../f547c394/amendment-v4.md) govern the receipt this issue needs.
Both the pilot and the confirmation receipt exist and are accepted. This
report states no measured value of its own; every number points at the
acceptance summary row that holds it.

## What the code does now

`gf2m::wide::clmul_wide_dispatch` is the single place a wide carry-less product
selects its kernel. `clmul_wide`, `clmul_wide_slice`, `Gf2mWide::mul_ref` and
the wide Barrett reducer all reach it. The kernel selection itself is
`#[cfg(feature = "simd")]`: a build with that feature enabled, on a host with
PCLMULQDQ, computes the 4-word and 9-word products in the `gf2-kernels-simd`
kernels whichever of those a caller used. `simd` is not a default feature of
`gf2-core` (`default = ["rand", "io"]`); every other width, every host
without the capability, and every build that does not enable `simd` —
including a plain `cargo build -p gf2-core` — runs
`clmul_wide_slice_portable`.

Two consequences change how callers and tests behave:

- The slice form XOR-accumulates and the kernels overwrite, so the dispatch
  carries a write mode. Accumulating at a dispatched width costs a scratch
  product and one XOR pass; the owned form pays neither. `clmul_wide_slice`'s
  rustdoc points a caller who wants a plain 4- or 9-word product at
  `clmul_wide`.
- `clmul_wide_slice` used to be the crate's independent scalar oracle. It
  dispatches now, so the in-crate reference multiplications, the conformance
  oracle and the benchmark's scalar-baseline arms call
  `clmul_wide_slice_portable` instead and stay independent of the path they
  check.

The lane witness and the forced-fallback switch that the conformance suite
reads follow `gf2-coding`'s `force_scalar_encode_kernels`. Both compile away
outside test and `test-support` builds, and the receipt's arm crate enables
`simd` alone, so nothing instrumented is ever measured.

## Evidence

| Question | Artifact |
|---|---|
| Does the public path reach the kernels, at every width, against an independent oracle, fallback included? | `crates/gf2-core/tests/clmul_wide_conformance.rs`, recorded in [validation.json](validation.json) |
| What did the contract look like before the routing? | [conformance-before-routing.txt](conformance-before-routing.txt) |
| What does the receipt measure, and under which frozen settings? | [addendum-v4-public-clmul-pilot.json](addendum-v4-public-clmul-pilot.json), [addendum-v4-public-clmul-confirmation.json](addendum-v4-public-clmul-confirmation.json) |
| What produces the measurement? | [producing-inputs.json](producing-inputs.json), [run-public-clmul.sh](run-public-clmul.sh), [make-plan.py](make-plan.py), `arms/` |
| How was the confirmation resolution and margins derived from the pilot? | [confirmation-derivation.txt](confirmation-derivation.txt) |
| What did the pilot measure? | [pilot acceptance-summary.md](../../bench_results/1c602857/2026-09-13-1c602857-public-clmul-pilot/acceptance-summary.md) |
| What did the confirmation measure, and what does it decide? | [confirmation acceptance-summary.md](../../bench_results/1c602857/2026-09-13-1c602857-public-clmul-confirmation/acceptance-summary.md) |

The failing behavioural evidence preceded the routing: at the pre-routing
commit the public 4-word product reported the portable lane while the host
offered its AVX2 kernel, and the `Gf2mWide` multiplication test at the same
commit passed — the asymmetry the issue describes.

The polynomial-multiplication baseline survey (`c7113c5a`) recorded the call
paths that surfaced this issue and measured the public path against the
dispatched kernel at four words. That measurement is exploratory and is cited
as motivation only; it is not the before measurement this issue requires.

## The receipt and its confirmed result

The family measures the routed public path against the path it replaced. The
baseline arm calls `clmul_wide_slice_portable`, which is the pre-change public
function word for word: a zeroed destination followed by the accumulating
schoolbook. One executable serves both arms and selects its entry point from
`GF2_CLMUL_PATH`, so a measured ratio attributes to the entry point rather than
to two builds. Six cells: the owned form at the two dispatched widths, the
accumulating slice form at the smaller of them, and the dispatch decision at
three widths without a kernel.

It runs in two windows. The pilot fixes the measurement resolution at `0.02`
([confirmation-derivation.txt](confirmation-derivation.txt), widest relative
half-width row). `dev/active/c7113c5a/survey/freeze-confirmation.py` derives
the confirmation addendum from the pilot addendum against that resolution,
pins the pilot receipt by path and SHA-256, and writes the derivation record.
Every one of the addendum's three `effect` margins —
`worthwhile_speedup` (1.1), `equivalence_margin` (1.05) and
`material_gap_threshold` (1.25) — strictly exceeds `1 + 0.02`, so the freeze
carries all three pilot-declared margins forward unchanged; no margin needs
replacing, and the derivation record shows no `replaced` line.

`run-public-clmul.sh confirmation` ran the six confirmatory cells at
`confirmatory_pairs = 24` each (P-20's frozen shared setting), spanning three
resumed sessions under the shared CCX1 lock; the confirmation
[execution.log](../../bench_results/1c602857/2026-09-13-1c602857-public-clmul-confirmation/execution.log)
records six `cell-start` and six `cell-complete` events, every one at `status:
measured` and `pairs: 24`, and a terminal `complete` event. The
[acceptance summary](../../bench_results/1c602857/2026-09-13-1c602857-public-clmul-confirmation/acceptance-summary.md)
evaluates that receipt: `verdict: accepted`, `qualifies: true`, `findings: []`.

Per cell, against the confirmation acceptance summary's rows:

- `clmul-wide-4w-owned` and `clmul-wide-9w-owned` (the owned form at the two
  dispatched widths) both decide `improved`: their interval's lower bound
  clears `worthwhile_speedup`. The dispatched kernel's owned product is
  markedly faster than the portable schoolbook it replaces at both widths.
- `clmul-wide-4w-accumulate` (the XOR-accumulating slice form at 4 words)
  also decides `improved`, despite paying the scratch-buffer-plus-XOR cost
  `clmul_wide_slice`'s rustdoc describes: the dispatched kernel's product is
  fast enough that the extra pass does not erase the gain.
- `clmul-wide-1w-dispatch-overhead`, `clmul-wide-2w-dispatch-overhead` and
  `clmul-wide-16w-dispatch-overhead` (dispatch decision at widths with no
  kernel) all decide `not-worse`: each interval sits within the
  `equivalence_margin`, so routing through `clmul_wide_dispatch` at a width
  the kernels do not cover costs nothing measurable relative to calling
  `clmul_wide_slice_portable` directly.

Every cell's outcome is `pass`; every stochastic value above (`estimate`,
interval) carries `pairs: 24` at `confidence: 0.9958` in the acceptance
summary's own rows, not restated here as a bare number.

**Adoption decision.** The measurement contract selects a candidate "only when
confidence-bound improvement, correctness and non-regression rules pass on
their declared dispatch domain"; the acceptance tool encodes that rule as the
receipt's `qualifies` flag. The confirmation acceptance summary reports
`qualifies: true` with every cell at outcome `pass` and `findings: []`, so the
routing this issue makes is adopted as measured. No cell records `regressed`
or `inconclusive`; there is no negative outcome to preserve.

## Status against the success criteria

| Requirement | Status | Pointer |
|---|---|---|
| REQ-01 (contract: correctness, reproducible release benchmarks, comparison validity, adoption, pinned receipts, before/after evidence, negative outcomes preserved) | Met | [conformance-before-routing.txt](conformance-before-routing.txt) is the before evidence; both receipts pin the committed protocol, contract and addendum in their `inputs/` snapshots; the confirmation [acceptance-summary.json](../../bench_results/1c602857/2026-09-13-1c602857-public-clmul-confirmation/acceptance-summary.json) records `verdict: accepted` with no cell requiring a preserved negative outcome |
| REQ-02 (public path reaches the same dispatch as internal wide-field multiplication, portable fallback kept elsewhere) | Met | `crates/gf2-core/src/gf2m/wide.rs`: `clmul_wide`, `clmul_wide_slice` and `Gf2mWide::mul_ref` all call `clmul_wide_dispatch`; the kernel-selecting arm of that function is `#[cfg(feature = "simd")]` (not a default feature), so the same dispatch point compiles to always-portable without it. Asserted directly by `public_product_reaches_capability_dispatch` and `field_multiplication_shares_the_canonical_dispatch` in `crates/gf2-core/tests/clmul_wide_conformance.rs`, both of which pass with and without `--all-features` (`cargo nextest run -p gf2-core --test clmul_wide_conformance --cargo-profile ci-test --profile ci` via `./scripts/cargo-budget.sh --test`: 4/4 each) |
| REQ-03 (conformance suite covers public functions at dispatched and non-dispatched widths, random and adversarial operands, complete double-width output against the portable oracle, fallback included) | Met | `crates/gf2-core/tests/clmul_wide_conformance.rs`: `public_product_matches_the_oracle_at_every_width` (widths 1,2,3,4,5,8,9,16, random and adversarial pairs via `pairs`), `portable_fallback_matches_the_oracle_at_every_width` (forced-fallback lane), `public_product_reaches_capability_dispatch` (lane witness, `expected_lane` itself `#[cfg]`-split on `simd` so the dispatched lane is only asserted when the build can reach it); recorded passing in [validation.json](validation.json), and confirmed passing both with and without `--all-features` above |
| REQ-04 (frozen before/after receipt at dispatched and non-dispatched widths including dispatch overhead, adoption follows the frozen non-regression rule) | Met | Confirmation [acceptance-summary.md](../../bench_results/1c602857/2026-09-13-1c602857-public-clmul-confirmation/acceptance-summary.md): six cells, all `pass`, `qualifies: true`; dispatch overhead at 1/2/16 words is the three `*-dispatch-overhead` rows, all `not-worse` |
| REQ-05 (rustdoc of the public functions and `Gf2mWide::mul_ref` states the actual mechanism and complexity) | Met | `crates/gf2-core/src/gf2m/wide.rs`: `clmul_wide` and `clmul_wide_slice` rustdoc `# Mechanism`/`# Complexity` sections name the dispatched-kernel/portable-schoolbook split and state that the kernel path additionally needs the `simd` feature, which is off by default; `Gf2mWide::mul_ref`'s `# Mechanism` and `# Complexity` sections do the same and name `clmul_wide_dispatch` and the two extra Barrett-reduction products, matching the body read at HEAD |

## A recurring defect class: the arm's wire shape

The untimed smoke through the real `benchmark-ab-runner` caught this arm
serializing `cold_calls` and `decoder` as explicit nulls where the runner skips
them when absent. The canonical round trip rejects that, and the arm died in
13 ms before producing a result line.

That is the same signature that killed three queued `6c6b09b1` pilots and cost
a whole benchmark window slot: an arm whose request type does not round-trip to
the runner's own bytes. Two independent surveys have now hit it, so it is a
defect class, not one survey's mistake. The child-v2 request type is a wire
contract, not a struct that merely has to decode: `deny_unknown_fields` plus
canonical re-serialization means every optional field needs the runner's own
`skip_serializing_if`, and field order must match. Code reading does not
establish this. Run the campaign end to end through the real runner with a
throwaway plan, and reach a result line from every arm, before queuing
anything.
